//! Local AppStream catalog.
//!
//! Every configured remote ships AppStream metadata that flatpak keeps at
//! `<installation>/appstream/<remote>/<arch>/active/appstream.xml(.gz)`,
//! together with pre-rendered icons. Kiosque indexes those files in memory and
//! uses them for search, categories and app details, so browsing works offline
//! and every remote (not only Flathub) shows up in the store. The Flathub web
//! API is only used to enrich this with things it does better: popularity,
//! trending, recently updated, statistics and sandbox permissions.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::{Deserialize, Serialize};

use crate::flathub::types::FlathubApp;
use crate::flatpak::{FlatpakBackend, RemoteInfo, Scope};

/// One application from a remote's AppStream data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogApp {
    pub id: String,
    pub remote: String,
    pub scope: Scope,
    pub name: String,
    pub summary: String,
    /// Description converted to the HTML subset Qt's RichText understands.
    pub description: String,
    pub developer: String,
    pub license: String,
    /// `file://` URL of a cached icon, or a remote URL.
    pub icon: String,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    /// Best-resolution URL of every screenshot, default screenshot first.
    pub screenshots: Vec<String>,
    /// `homepage`, `bugtracker`, `donation`, `help`, `vcs-browser`, …
    pub urls: HashMap<String, String>,
    pub version: String,
    pub release_timestamp: u64,
    /// Notes of the newest release, in the same HTML subset as `description`.
    pub release_notes: String,
    /// `flathub::manifest` and similar `<metadata>` values.
    pub metadata: HashMap<String, String>,
    /// The flatpak ref from `<bundle type="flatpak">`.
    pub bundle: String,
}

impl CatalogApp {
    pub fn to_flathub_app(&self) -> FlathubApp {
        FlathubApp {
            name: self.name.clone(),
            app_id: self.id.clone(),
            summary: Some(self.summary.clone()),
            icon: Some(self.icon.clone()),
            developer_name: Some(self.developer.clone()).filter(|d| !d.is_empty()),
            description: Some(self.description.clone()),
            sub_categories: self.categories.clone(),
            added_at: None,
        }
    }

    pub fn has_category(&self, cat: &str) -> bool {
        self.categories.iter().any(|c| c.eq_ignore_ascii_case(cat))
    }
}

/// The indexed metadata of all remotes.
#[derive(Default)]
pub struct Catalog {
    apps: HashMap<String, CatalogApp>,
    remotes: Vec<RemoteInfo>,
    /// Remotes that contributed at least one app.
    indexed: HashSet<String>,
}

impl Catalog {
    pub fn from_apps(apps: Vec<CatalogApp>, remotes: Vec<RemoteInfo>) -> Self {
        let mut map: HashMap<String, CatalogApp> = HashMap::new();
        let mut indexed = HashSet::new();
        for app in apps {
            indexed.insert(app.remote.clone());
            // Keep the first remote that carries an app (remotes are ordered).
            map.entry(app.id.clone()).or_insert(app);
        }
        Self {
            apps: map,
            remotes,
            indexed,
        }
    }

    /// True when `remote`'s AppStream data is indexed (it has at least one app).
    pub fn indexes(&self, remote: &str) -> bool {
        self.indexed.contains(remote)
    }

    /// The configured remote `name` in `scope`, if known.
    pub fn remote(&self, name: &str, scope: Scope) -> Option<&RemoteInfo> {
        self.remotes
            .iter()
            .find(|r| r.name == name && r.scope == scope)
            .or_else(|| self.remotes.iter().find(|r| r.name == name))
    }

    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.apps.len()
    }

    pub fn get(&self, id: &str) -> Option<CatalogApp> {
        self.apps.get(id).cloned()
    }

    pub fn remotes(&self) -> &[RemoteInfo] {
        &self.remotes
    }

    pub fn remote_scope(&self, remote: &str) -> Option<Scope> {
        let candidates: Vec<&RemoteInfo> =
            self.remotes.iter().filter(|r| r.name == remote).collect();
        // Prefer the system remote when both exist, as flatpak does.
        candidates
            .iter()
            .find(|r| r.scope == Scope::System)
            .or(candidates.first())
            .map(|r| r.scope)
    }

    /// Ranked full-text search over id, name, keywords and summary. Every
    /// query word must match somewhere.
    pub fn search(&self, query: &str, limit: usize) -> Vec<CatalogApp> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return vec![];
        }
        let mut scored: Vec<(u32, &CatalogApp)> = self
            .apps
            .values()
            .filter_map(|app| score(app, &words).map(|s| (s, app)))
            .collect();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, a)| a.clone())
            .collect()
    }

    pub fn by_category(&self, category: &str) -> Vec<CatalogApp> {
        let mut v: Vec<CatalogApp> = self
            .apps
            .values()
            .filter(|a| a.has_category(category))
            .cloned()
            .collect();
        v.sort_by_key(|a| a.name.to_lowercase());
        v
    }

    pub fn by_developer(&self, developer: &str) -> Vec<CatalogApp> {
        let mut v: Vec<CatalogApp> = self
            .apps
            .values()
            .filter(|a| a.developer.eq_ignore_ascii_case(developer))
            .cloned()
            .collect();
        v.sort_by_key(|a| a.name.to_lowercase());
        v
    }

    /// Apps sharing the most categories with `app`, excluding itself.
    pub fn similar(&self, app: &CatalogApp, limit: usize) -> Vec<CatalogApp> {
        const GENERIC: [&str; 4] = ["GTK", "Qt", "KDE", "GNOME"];
        let cats: Vec<&String> = app
            .categories
            .iter()
            .filter(|c| !GENERIC.contains(&c.as_str()))
            .collect();
        let mut scored: Vec<(usize, &CatalogApp)> = self
            .apps
            .values()
            .filter(|a| a.id != app.id)
            .map(|a| (cats.iter().filter(|c| a.has_category(c)).count(), a))
            .filter(|(n, _)| *n > 0)
            .collect();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.release_timestamp.cmp(&a.1.release_timestamp))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, a)| a.clone())
            .collect()
    }

    /// Most recently released apps.
    pub fn recently_updated(&self, limit: usize) -> Vec<CatalogApp> {
        let mut v: Vec<&CatalogApp> = self.apps.values().collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.release_timestamp));
        v.into_iter().take(limit).cloned().collect()
    }
}

fn score(app: &CatalogApp, words: &[String]) -> Option<u32> {
    let id = app.id.to_lowercase();
    let name = app.name.to_lowercase();
    let summary = app.summary.to_lowercase();
    let mut total = 0;
    for w in words {
        let mut s = 0;
        if id == *w || name == *w {
            s = s.max(100);
        }
        if name.starts_with(w.as_str()) {
            s = s.max(60);
        }
        if name.contains(w.as_str()) {
            s = s.max(40);
        }
        if app
            .keywords
            .iter()
            .any(|k| k.to_lowercase().contains(w.as_str()))
        {
            s = s.max(25);
        }
        if id.contains(w.as_str()) {
            s = s.max(15);
        }
        if summary.contains(w.as_str()) {
            s = s.max(10);
        }
        if s == 0 {
            return None;
        }
        total += s;
    }
    Some(total)
}

// ── Global instance ─────────────────────────────────────────────────────────

fn slot() -> &'static RwLock<Arc<Catalog>> {
    static SLOT: OnceLock<RwLock<Arc<Catalog>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(Arc::new(Catalog::default())))
}

/// The current catalog (possibly still empty while the first load runs).
pub fn catalog() -> Arc<Catalog> {
    slot().read().unwrap().clone()
}

static FIRST_LOAD: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// The catalog after the first load from disk has completed.
pub async fn loaded() -> Arc<Catalog> {
    FIRST_LOAD
        .get_or_init(|| async {
            reload().await;
        })
        .await;
    catalog()
}

fn changes_tx() -> &'static tokio::sync::broadcast::Sender<()> {
    static TX: OnceLock<tokio::sync::broadcast::Sender<()>> = OnceLock::new();
    TX.get_or_init(|| tokio::sync::broadcast::channel(4).0)
}

/// Fires after every successful reload of the catalog.
pub fn changes() -> tokio::sync::broadcast::Receiver<()> {
    changes_tx().subscribe()
}

/// Re-read every remote's AppStream data from disk.
pub async fn reload() {
    reload_with(
        &*crate::flatpak::backend(),
        crate::flatpak::installation_paths(),
    )
    .await;
}

/// Re-read the AppStream data of `backend`'s remotes found under the given
/// installation directories, then publish the new catalog.
pub async fn reload_with(backend: &dyn FlatpakBackend, installations: Vec<(Scope, PathBuf)>) {
    let remotes = match backend.list_remotes().await {
        Ok(r) => r,
        Err(e) => {
            kerr!("catalog: cannot list remotes: {}", e);
            return;
        }
    };
    let cat = tokio::task::spawn_blocking(move || load_all(remotes, &installations)).await;
    match cat {
        Ok(cat) => {
            klog!(
                "catalog: indexed {} apps from {} remotes",
                cat.len(),
                cat.remotes.len()
            );
            *slot().write().unwrap() = Arc::new(cat);
            let _ = changes_tx().send(());
        }
        Err(e) => kerr!("catalog: load failed: {}", e),
    }
}

/// How old AppStream data may get before it is refreshed from the network.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Refresh AppStream data of remotes whose copy is missing or older than a
/// day, then reload the catalog if anything changed.
pub async fn refresh_if_stale() {
    refresh_if_stale_with(
        &*crate::flatpak::backend(),
        crate::flatpak::installation_paths(),
    )
    .await;
}

/// [`refresh_if_stale`] for an explicit backend and installation directories.
pub async fn refresh_if_stale_with(
    backend: &dyn FlatpakBackend,
    installations: Vec<(Scope, PathBuf)>,
) {
    let remotes = match backend.list_remotes().await {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut refreshed = false;
    for r in remotes.iter().filter(|r| !r.disabled && !r.noenumerate) {
        let age = appstream_file(r, &installations)
            .and_then(|p| p.metadata().ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| SystemTime::now().duration_since(t).ok());
        if age.is_some_and(|a| a < MAX_AGE) {
            continue;
        }
        klog!(
            "catalog: refreshing AppStream for {} ({})",
            r.name,
            r.scope.as_str()
        );
        match backend.refresh_appstream(r).await {
            Ok(()) => refreshed = true,
            Err(e) => kerr!("catalog: refreshing {} failed: {}", r.name, e),
        }
    }
    if refreshed || catalog().is_empty() {
        reload_with(backend, installations).await;
    }
}

/// Refresh one remote's AppStream data from the network, then reload the
/// catalog (used after a repository was added).
pub async fn refresh_remote_with(
    backend: &dyn FlatpakBackend,
    installations: Vec<(Scope, PathBuf)>,
    name: &str,
) {
    if let Ok(remotes) = backend.list_remotes().await {
        for r in remotes.iter().filter(|r| r.name == name && !r.disabled) {
            if let Err(e) = backend.refresh_appstream(r).await {
                kerr!("catalog: refreshing {} failed: {}", r.name, e);
            }
        }
    }
    reload_with(backend, installations).await;
}

fn appstream_dir(remote: &RemoteInfo, installations: &[(Scope, PathBuf)]) -> Option<PathBuf> {
    let base = installations
        .iter()
        .find(|(s, _)| *s == remote.scope)
        .map(|(_, p)| p.clone())?;
    Some(
        base.join("appstream")
            .join(&remote.name)
            .join(crate::flatpak::default_arch())
            .join("active"),
    )
}

fn appstream_file(remote: &RemoteInfo, installations: &[(Scope, PathBuf)]) -> Option<PathBuf> {
    let dir = appstream_dir(remote, installations)?;
    ["appstream.xml.gz", "appstream.xml"]
        .iter()
        .map(|f| dir.join(f))
        .find(|p| p.exists())
}

fn load_all(mut remotes: Vec<RemoteInfo>, installations: &[(Scope, PathBuf)]) -> Catalog {
    // Flathub first so it wins ties, then system before user.
    remotes.sort_by_key(|r| (r.name != "flathub", r.scope == Scope::User));
    let lang = user_language();
    let mut apps = vec![];
    for r in remotes.iter().filter(|r| !r.disabled && !r.noenumerate) {
        let Some(file) = appstream_file(r, installations) else {
            continue;
        };
        match read_xml(&file) {
            Ok(xml) => {
                let icons = file.parent().map(|d| d.join("icons")).unwrap_or_default();
                let parsed = parse_appstream(&xml, &r.name, r.scope, &icons, &lang);
                klog!(
                    "catalog: {} ({}): {} apps",
                    r.name,
                    r.scope.as_str(),
                    parsed.len()
                );
                apps.extend(parsed);
            }
            Err(e) => kerr!("catalog: cannot read {}: {}", file.display(), e),
        }
    }
    Catalog::from_apps(apps, remotes)
}

fn read_xml(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    if path.extension().is_some_and(|e| e == "gz") {
        let mut s = String::new();
        flate2::read::GzDecoder::new(&bytes[..]).read_to_string(&mut s)?;
        Ok(s)
    } else {
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// The user's language as `(full, short)`, e.g. `("pt_BR", "pt")`.
pub fn user_language() -> (String, String) {
    let raw = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| "C".into());
    let first = raw.split(':').next().unwrap_or("C");
    let full = first.split(['.', '@']).next().unwrap_or("C").to_string();
    let short = full.split('_').next().unwrap_or("C").to_string();
    (full, short)
}

// ── AppStream XML parsing ───────────────────────────────────────────────────

/// Rank of an `xml:lang` value: higher is better, `None` = not acceptable.
fn lang_rank(attr: Option<&str>, lang: &(String, String)) -> Option<u8> {
    match attr {
        None | Some("") | Some("C") => Some(1),
        Some(l) if l == lang.0 => Some(3),
        Some(l) if l == lang.1 => Some(2),
        _ => None,
    }
}

#[derive(Default)]
struct Localized {
    value: String,
    rank: u8,
}

impl Localized {
    fn offer(&mut self, value: String, rank: u8) {
        if rank > self.rank || (rank == self.rank && self.value.is_empty()) {
            self.value = value;
            self.rank = rank;
        }
    }
}

#[derive(Default)]
struct Builder {
    kind: String,
    app: CatalogApp,
    name: Localized,
    summary: Localized,
    developer: Localized,
    description: Localized,
    release_notes: Localized,
    /// (size, url) of the best cached and remote icon.
    cached_icon: Option<(u32, String)>,
    remote_icon: Option<(u32, String)>,
    /// (is_default, width, url) per screenshot.
    shots: Vec<(bool, u32, String)>,
    /// `<release>` elements seen so far (newest first).
    releases: u32,
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.try_get_attribute(key).ok().flatten().and_then(|a| {
        a.normalized_value(XmlVersion::Implicit1_0)
            .ok()
            .map(|v| v.into_owned())
    })
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Parse an AppStream collection, keeping desktop and console applications.
pub fn parse_appstream(
    xml: &str,
    remote: &str,
    scope: Scope,
    icons_dir: &Path,
    lang: &(String, String),
) -> Vec<CatalogApp> {
    let mut reader = Reader::from_str(xml);
    // Whitespace is significant inside <description> (between inline
    // elements), so text is trimmed per field instead.
    reader.config_mut().trim_text(false);

    let mut out = vec![];
    let mut stack: Vec<String> = vec![];
    let mut cur: Option<Builder> = None;
    let mut text = String::new();
    // Attributes of the element whose text is being collected.
    let mut el_attrs: HashMap<String, String> = HashMap::new();
    // While inside <description>: its language rank and the HTML so far.
    let mut desc: Option<(u8, String)> = None;
    let mut desc_skip = false;
    // Whether that description holds release notes (not the app's).
    let mut desc_release = false;

    loop {
        let ev = match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(ev) => ev,
            Err(e) => {
                kerr!(
                    "catalog: XML error in {} at {}: {}",
                    remote,
                    reader.buffer_position(),
                    e
                );
                break;
            }
        };
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(ev, Event::Empty(_));
                let name = e.local_name().as_ref().to_owned();
                let depth = stack.len();

                if name == "component" && depth <= 1 {
                    cur = Some(Builder {
                        kind: attr(e, "type").unwrap_or_default(),
                        ..Default::default()
                    });
                } else if let Some(b) = cur.as_mut() {
                    if let Some((_, html)) = desc.as_mut() {
                        if !desc_skip
                            && matches!(name.as_str(), "p" | "ul" | "ol" | "li" | "em" | "code")
                        {
                            html.push_str(&format!("<{}>", name));
                            if is_empty {
                                html.push_str(&format!("</{}>", name));
                            }
                        }
                    } else if name == "description"
                        && (stack.last().is_some_and(|p| p == "component")
                            || (stack.last().is_some_and(|p| p == "release") && b.releases == 1))
                    {
                        desc_release = stack.last().is_some_and(|p| p == "release");
                        match lang_rank(attr(e, "xml:lang").as_deref(), lang) {
                            Some(rank) => {
                                desc = Some((rank, String::new()));
                                desc_skip = false;
                            }
                            None => {
                                desc = Some((0, String::new()));
                                desc_skip = true;
                            }
                        }
                    } else if name == "screenshot" {
                        let screenshot_default = attr(e, "type").as_deref() == Some("default");
                        b.shots.push((screenshot_default, 0, String::new()));
                    } else if name == "release" && {
                        b.releases += 1;
                        b.releases == 1
                    } {
                        // Releases are listed newest first.
                        b.app.version = attr(e, "version").unwrap_or_default();
                        b.app.release_timestamp = attr(e, "timestamp")
                            .and_then(|t| t.parse().ok())
                            .or_else(|| attr(e, "date").and_then(|d| parse_date(&d)))
                            .unwrap_or(0);
                    }
                    el_attrs.clear();
                    for a in e.attributes().flatten() {
                        let k = a.key.as_ref().to_owned();
                        if let Ok(v) = a.normalized_value(XmlVersion::Implicit1_0) {
                            el_attrs.insert(k, v.into_owned());
                        }
                    }
                }
                text.clear();
                if !is_empty {
                    stack.push(name);
                } else if name == "component" {
                    cur = None;
                }
            }
            Event::Text(t) => {
                if let Some((_, html)) = desc.as_mut() {
                    if !desc_skip {
                        html.push_str(&html_escape(&collapse_ws(&t)));
                    }
                } else {
                    text.push_str(&t);
                }
            }
            Event::CData(t) => {
                text.push_str(&t);
            }
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => match r.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        _ => String::new(),
                    },
                };
                if let Some((_, html)) = desc.as_mut() {
                    if !desc_skip {
                        html.push_str(&html_escape(&resolved));
                    }
                } else {
                    text.push_str(&resolved);
                }
            }
            Event::End(_) => {
                let name = stack.pop().unwrap_or_default();
                let parent = stack.last().cloned().unwrap_or_default();
                if name == "component" {
                    if let Some(b) = cur.take()
                        && let Some(app) = finish(b, remote, scope, icons_dir)
                    {
                        out.push(app);
                    }
                    continue;
                }
                let Some(b) = cur.as_mut() else { continue };

                if desc.is_some() {
                    if name == "description" {
                        let (rank, html) = desc.take().unwrap();
                        if !desc_skip {
                            let target = if desc_release {
                                &mut b.release_notes
                            } else {
                                &mut b.description
                            };
                            target.offer(tidy_html(&html), rank);
                        }
                    } else if !desc_skip
                        && matches!(name.as_str(), "p" | "ul" | "ol" | "li" | "em" | "code")
                    {
                        desc.as_mut().unwrap().1.push_str(&format!("</{}>", name));
                    }
                    continue;
                }

                let value = std::mem::take(&mut text).trim().to_string();
                let rank = lang_rank(el_attrs.get("xml:lang").map(String::as_str), lang);
                match (name.as_str(), parent.as_str()) {
                    ("id", "component") => {
                        b.app.id = value.trim_end_matches(".desktop").to_string()
                    }
                    ("name", "component") => {
                        if let Some(r) = rank {
                            b.name.offer(value, r)
                        }
                    }
                    ("summary", "component") => {
                        if let Some(r) = rank {
                            b.summary.offer(value, r)
                        }
                    }
                    ("developer_name", "component") | ("name", "developer") => {
                        if let Some(r) = rank {
                            b.developer.offer(value, r)
                        }
                    }
                    ("project_license", "component") => b.app.license = value,
                    ("category", "categories") => b.app.categories.push(value),
                    ("keyword", "keywords") => {
                        if rank.is_some() && !value.is_empty() {
                            b.app.keywords.push(value)
                        }
                    }
                    ("url", "component") => {
                        if let Some(t) = el_attrs.get("type") {
                            b.app.urls.insert(t.replace('-', "_"), value);
                        }
                    }
                    ("bundle", "component") => {
                        if el_attrs.get("type").map(String::as_str) == Some("flatpak") {
                            b.app.bundle = value;
                        }
                    }
                    ("value", "metadata") | ("value", "custom") => {
                        if let Some(k) = el_attrs.get("key") {
                            b.app.metadata.insert(k.clone(), value);
                        }
                    }
                    ("icon", "component") => {
                        let size: u32 = el_attrs
                            .get("width")
                            .and_then(|w| w.parse().ok())
                            .unwrap_or(64);
                        match el_attrs.get("type").map(String::as_str) {
                            Some("cached") => {
                                if b.cached_icon.as_ref().is_none_or(|(s, _)| size > *s) {
                                    b.cached_icon = Some((size, value));
                                }
                            }
                            Some("remote")
                                if b.remote_icon.as_ref().is_none_or(|(s, _)| size > *s) =>
                            {
                                b.remote_icon = Some((size, value));
                            }
                            _ => {}
                        }
                    }
                    ("image", "screenshot") => {
                        let width: u32 = el_attrs
                            .get("width")
                            .and_then(|w| w.parse().ok())
                            .unwrap_or(0);
                        // Source images have no width; treat them as largest.
                        let width = if el_attrs.get("type").map(String::as_str) == Some("source") {
                            u32::MAX
                        } else {
                            width
                        };
                        if let Some(shot) = b.shots.last_mut()
                            && (shot.2.is_empty() || width > shot.1)
                        {
                            *shot = (shot.0, width, value);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_ws {
                out.push(' ');
            }
            last_ws = true;
        } else {
            out.push(c);
            last_ws = false;
        }
    }
    out
}

/// Drop whitespace around block-level tags left over from XML indentation.
fn tidy_html(html: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\s*(</?(?:p|ul|ol|li)>)\s*").unwrap());
    re.replace_all(html, "$1").trim().to_string()
}

fn parse_date(d: &str) -> Option<u64> {
    let date = jiff::civil::Date::strptime("%Y-%m-%d", d.get(..10)?).ok()?;
    Some(
        date.at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .ok()?
            .timestamp()
            .as_second() as u64,
    )
}

fn finish(b: Builder, remote: &str, scope: Scope, icons_dir: &Path) -> Option<CatalogApp> {
    if !matches!(
        b.kind.as_str(),
        "desktop-application" | "desktop" | "console-application"
    ) {
        return None;
    }
    let mut app = b.app;
    if app.id.is_empty() || app.bundle.starts_with("runtime/") {
        return None;
    }
    app.remote = remote.to_string();
    app.scope = scope;
    app.name = if b.name.value.is_empty() {
        app.id.clone()
    } else {
        b.name.value
    };
    app.summary = b.summary.value;
    app.developer = b.developer.value;
    app.description = b.description.value;
    app.release_notes = b.release_notes.value;
    app.icon = b
        .cached_icon
        .and_then(|(size, file)| {
            let dirs = [format!("{0}x{0}", size), "128x128".into(), "64x64".into()];
            dirs.iter()
                .map(|d| icons_dir.join(d).join(&file))
                .find(|p| p.exists())
        })
        .map(|p| format!("file://{}", p.display()))
        .or(b.remote_icon.map(|(_, url)| url))
        .unwrap_or_default();
    let mut shots = b.shots;
    shots.sort_by_key(|s| !s.0);
    app.screenshots = shots
        .into_iter()
        .map(|s| s.2)
        .filter(|u| !u.is_empty())
        .collect();
    Some(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<components version="0.8" origin="flathub">
  <component type="desktop-application">
    <id>org.example.Paint</id>
    <name>Paint</name>
    <name xml:lang="fr">Peinture</name>
    <name xml:lang="de">Malen</name>
    <summary>Draw &amp; paint</summary>
    <description>
      <p>A <em>simple</em> painter &lt;3</p>
      <ul><li>Layers</li><li>Brushes</li></ul>
    </description>
    <description xml:lang="de"><p>Ein Malprogramm</p></description>
    <developer id="org.example"><name>Example Devs</name></developer>
    <project_license>GPL-3.0+</project_license>
    <icon type="cached" width="64" height="64">org.example.Paint.png</icon>
    <icon type="remote" width="128" height="128">https://example.org/paint.png</icon>
    <categories><category>Graphics</category><category>RasterGraphics</category></categories>
    <keywords><keyword>draw</keyword></keywords>
    <url type="homepage">https://example.org</url>
    <url type="vcs-browser">https://git.example.org</url>
    <screenshots>
      <screenshot><image type="thumbnail" width="224">https://e.org/2-small.png</image></screenshot>
      <screenshot type="default">
        <image type="thumbnail" width="224">https://e.org/1-small.png</image>
        <image type="thumbnail" width="1248">https://e.org/1-big.png</image>
      </screenshot>
    </screenshots>
    <releases>
      <release version="2.0" timestamp="1700000000"><description><p>Faster <em>brushes</em></p></description></release>
      <release version="1.0" timestamp="1600000000"><description><p>First</p></description></release>
    </releases>
    <bundle type="flatpak" runtime="org.kde.Platform/x86_64/6.8">app/org.example.Paint/x86_64/stable</bundle>
    <metadata><value key="flathub::manifest">https://example.org/manifest.json</value></metadata>
  </component>
  <component type="runtime"><id>org.example.Platform</id><name>Platform</name></component>
  <component type="desktop">
    <id>org.example.Notes.desktop</id>
    <name>Notes</name>
    <summary>Take notes</summary>
    <categories><category>Office</category><category>Graphics</category></categories>
    <releases><release version="1.1" date="2024-01-02"/></releases>
  </component>
</components>"#;

    fn parse(lang: (&str, &str)) -> Vec<CatalogApp> {
        parse_appstream(
            XML,
            "flathub",
            Scope::System,
            Path::new("/nonexistent"),
            &(lang.0.into(), lang.1.into()),
        )
    }

    #[test]
    fn parses_fields() {
        let apps = parse(("en_US", "en"));
        assert_eq!(apps.len(), 2, "runtime must be skipped");
        let a = &apps[0];
        assert_eq!(a.id, "org.example.Paint");
        assert_eq!(a.name, "Paint");
        assert_eq!(a.summary, "Draw & paint");
        assert_eq!(a.developer, "Example Devs");
        assert_eq!(a.license, "GPL-3.0+");
        assert_eq!(
            a.description,
            "<p>A <em>simple</em> painter &lt;3</p><ul><li>Layers</li><li>Brushes</li></ul>"
        );
        assert_eq!(
            a.icon, "https://example.org/paint.png",
            "falls back to remote icon when no cached file"
        );
        assert_eq!(a.categories, vec!["Graphics", "RasterGraphics"]);
        assert_eq!(a.urls.get("homepage").unwrap(), "https://example.org");
        assert_eq!(
            a.urls.get("vcs_browser").unwrap(),
            "https://git.example.org"
        );
        assert_eq!(
            a.screenshots,
            vec!["https://e.org/1-big.png", "https://e.org/2-small.png"]
        );
        assert_eq!(a.version, "2.0");
        assert_eq!(a.release_timestamp, 1_700_000_000);
        assert_eq!(
            a.release_notes, "<p>Faster <em>brushes</em></p>",
            "only the newest release's notes are kept"
        );
        assert_eq!(
            a.metadata.get("flathub::manifest").unwrap(),
            "https://example.org/manifest.json"
        );
        assert_eq!(a.bundle, "app/org.example.Paint/x86_64/stable");
        assert_eq!(a.remote, "flathub");

        let n = &apps[1];
        assert_eq!(n.id, "org.example.Notes");
        assert_eq!(n.release_timestamp, 1_704_153_600);
        assert_eq!(n.release_notes, "");
    }

    #[test]
    fn picks_translation() {
        let apps = parse(("de_DE", "de"));
        assert_eq!(apps[0].name, "Malen");
        assert_eq!(apps[0].description, "<p>Ein Malprogramm</p>");
        let apps = parse(("fr", "fr"));
        assert_eq!(apps[0].name, "Peinture");
    }

    #[test]
    fn preserves_unicode_cdata_and_entity_references() {
        let xml = r#"<components><component type="desktop-application">
            <id>org.example.Editor</id>
            <name><![CDATA[Éditeur <画>]]></name>
            <summary>Éditer &amp; créer &#x1F3A8; &quot;art&quot;</summary>
            <description><p>Créer &lt;画&gt; &amp; &#233;.</p></description>
            <url type="homepage">https://example.org/?a=1&amp;b=2</url>
        </component></components>"#;
        let apps = parse_appstream(
            xml,
            "local",
            Scope::User,
            Path::new("/nonexistent"),
            &("C".into(), "C".into()),
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Éditeur <画>");
        assert_eq!(apps[0].summary, "Éditer & créer 🎨 \"art\"");
        assert_eq!(apps[0].description, "<p>Créer &lt;画&gt; &amp; é.</p>");
        assert_eq!(apps[0].urls["homepage"], "https://example.org/?a=1&b=2");
    }

    #[test]
    fn search_and_categories() {
        let cat = Catalog::from_apps(parse(("C", "C")), vec![]);
        let hits = cat.search("paint", 10);
        assert_eq!(hits[0].id, "org.example.Paint");
        assert_eq!(
            cat.search("draw notes", 10).len(),
            0,
            "all words must match one app"
        );
        assert_eq!(cat.search("draw", 10).len(), 1, "keywords are searched");
        assert_eq!(cat.by_category("office").len(), 1);
        let paint = cat.get("org.example.Paint").unwrap();
        assert_eq!(cat.similar(&paint, 5)[0].id, "org.example.Notes");
        // Notes (2024-01-02) was released after Paint (2023-11-14).
        assert_eq!(cat.recently_updated(1)[0].id, "org.example.Notes");
    }
}
