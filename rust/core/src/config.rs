//! Distributor configuration.
//!
//! Distributions customise Kiosque with YAML files that follow **Bazaar's
//! schema**, so a spin that already ships Bazaar configs can point Kiosque at
//! the same files:
//!
//! * a *main* config (`main.yaml`) listing `curated-config-paths`,
//!   `yaml-blocklist-paths`, `txt-blocklist-paths`, plus `start-on-curated`
//!   and `hide-auto-update-options`;
//! * *curated* files with `rows:` of `section` (title, subtitle, `appids`) and
//!   `featured-carousel` entries (banners and articles are ignored);
//! * YAML blocklists (`blocklists:` with `priority`, `conditions`, `block`,
//!   `block-regex`, `allow`, `allow-regex`) and plain-text blocklists.
//!
//! Search order (all found files are merged, earlier directories win for
//! booleans): `$KIOSQUE_CONFIG_DIR`, `/etc/kiosque`, the build-time data dir
//! (`<prefix>/share/kiosque`), `/usr/share/kiosque` and, inside the Flatpak
//! sandbox, the host's `/etc/kiosque` and `/usr/share/kiosque` via
//! `/run/host`. When a directory has no `main.yaml`, `curated.yaml`,
//! `blocklist.yaml` and `blocklist.txt` in it are used directly.
//!
//! Files are re-read whenever their modification time changes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime};

use regex::Regex;
use serde::Deserialize;
use tokio::sync::broadcast;

// ── Public model ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CuratedRow {
    Section {
        title: String,
        subtitle: String,
        app_ids: Vec<String>,
    },
    Carousel {
        app_ids: Vec<String>,
    },
}

#[derive(Debug, Default)]
pub struct DistroConfig {
    pub rows: Vec<CuratedRow>,
    pub blocklist: Blocklist,
    pub start_on_curated: bool,
    pub hide_auto_update_options: bool,
    /// Every file that contributed, for change detection.
    pub files: Vec<PathBuf>,
}

impl DistroConfig {
    pub fn has_curated(&self) -> bool {
        !self.rows.is_empty()
    }

    /// App ids for the hero carousel: the first `featured-carousel` row, else
    /// the first section.
    pub fn featured_ids(&self) -> Vec<String> {
        self.rows
            .iter()
            .find_map(|r| match r {
                CuratedRow::Carousel { app_ids } => Some(app_ids.clone()),
                _ => None,
            })
            .or_else(|| {
                self.rows.iter().find_map(|r| match r {
                    CuratedRow::Section { app_ids, .. } => Some(app_ids.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default()
    }

    pub fn sections(&self) -> Vec<(String, String, Vec<String>)> {
        self.rows
            .iter()
            .filter_map(|r| match r {
                CuratedRow::Section {
                    title,
                    subtitle,
                    app_ids,
                } => Some((title.clone(), subtitle.clone(), app_ids.clone())),
                _ => None,
            })
            .collect()
    }
}

// ── Blocklists ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Condition {
    Locale {
        regex: Regex,
        invert: bool,
    },
    EnvVar {
        var: String,
        regex: Regex,
        invert: bool,
    },
}

impl Condition {
    fn holds(&self, env: &dyn Fn(&str) -> Option<String>) -> bool {
        let (res, invert) = match self {
            Condition::Locale { regex, invert } => {
                let locale = env("LANG").or_else(|| env("LC_ALL")).unwrap_or_default();
                let locale = locale.split(['.', '@']).next().unwrap_or("").to_string();
                (regex.is_match(&locale), *invert)
            }
            Condition::EnvVar { var, regex, invert } => {
                (env(var).is_some_and(|v| regex.is_match(&v)), *invert)
            }
        };
        res != invert
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockRule {
    pub priority: i64,
    pub conditions: Vec<Condition>,
    pub block: HashSet<String>,
    pub block_regex: Vec<Regex>,
    pub allow: HashSet<String>,
    pub allow_regex: Vec<Regex>,
}

impl BlockRule {
    fn verdict(&self, id: &str) -> Option<bool> {
        if self.allow.contains(id) || self.allow_regex.iter().any(|r| r.is_match(id)) {
            return Some(false);
        }
        if self.block.contains(id) || self.block_regex.iter().any(|r| r.is_match(id)) {
            return Some(true);
        }
        None
    }
}

/// Layered allow/block rules. Rules whose conditions hold are consulted in
/// priority order (lower number first); the first rule that mentions an id
/// decides. Within one rule, `allow` beats `block`.
#[derive(Debug, Clone, Default)]
pub struct Blocklist {
    rules: Vec<BlockRule>,
}

impl Blocklist {
    pub fn new(mut rules: Vec<BlockRule>) -> Self {
        rules.sort_by_key(|r| r.priority);
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn is_blocked(&self, id: &str) -> bool {
        self.is_blocked_with(id, &|k| std::env::var(k).ok())
    }

    pub fn is_blocked_with(&self, id: &str, env: &dyn Fn(&str) -> Option<String>) -> bool {
        self.rules
            .iter()
            .filter(|r| r.conditions.iter().all(|c| c.holds(env)))
            .find_map(|r| r.verdict(id))
            .unwrap_or(false)
    }
}

// ── File formats ────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
struct MainFile {
    yaml_blocklist_paths: Vec<PathBuf>,
    txt_blocklist_paths: Vec<PathBuf>,
    curated_config_paths: Vec<PathBuf>,
    start_on_curated: Option<bool>,
    hide_auto_update_options: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BlocklistFile {
    blocklists: Vec<RawRule>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
struct RawRule {
    priority: i64,
    conditions: Vec<serde_json::Value>,
    block: Vec<String>,
    block_regex: Vec<String>,
    allow: Vec<String>,
    allow_regex: Vec<String>,
}

/// Compile a regex that must match the whole input, as Bazaar does.
fn full_regex(re: &str) -> Option<Regex> {
    match Regex::new(&format!("^(?:{})$", re)) {
        Ok(r) => Some(r),
        Err(e) => {
            kerr!("config: invalid regex {:?}: {}", re, e);
            None
        }
    }
}

fn parse_yaml<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    if text.len() > 2 * 1024 * 1024 {
        return Err("Configuration exceeds its size limit".into());
    }
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        strict_booleans: true,
        budget: serde_saphyr::budget! {
            max_depth: 32,
            max_nodes: 100_000,
            max_events: 200_000,
            max_aliases: 64,
            max_anchors: 64,
            max_total_scalar_bytes: 2 * 1024 * 1024,
            max_recorded_anchor_bytes: 2 * 1024 * 1024,
        },
    };
    serde_saphyr::from_str_with_options(text, options).map_err(|error| error.to_string())
}

fn parse_condition(v: &serde_json::Value) -> Option<Condition> {
    let map = v.as_object()?;
    let (kind, body) = map.iter().next()?;
    let kind = kind.as_str();
    let s = |k: &str| body.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let invert = s("post-process").as_deref() == Some("invert")
        || map.get("post-process").and_then(|x| x.as_str()) == Some("invert");
    match kind {
        "match-locale" => Some(Condition::Locale {
            regex: full_regex(&s("regex")?)?,
            invert,
        }),
        "match-envvar" => Some(Condition::EnvVar {
            var: s("var")?,
            regex: full_regex(&s("regex")?)?,
            invert,
        }),
        other => {
            kerr!("config: unknown blocklist condition {:?}", other);
            None
        }
    }
}

pub fn parse_yaml_blocklist(text: &str) -> Result<Vec<BlockRule>, String> {
    let file: BlocklistFile = parse_yaml(text)?;
    Ok(file
        .blocklists
        .into_iter()
        .map(|r| BlockRule {
            priority: r.priority,
            conditions: r.conditions.iter().filter_map(parse_condition).collect(),
            block: r.block.into_iter().collect(),
            block_regex: r.block_regex.iter().filter_map(|x| full_regex(x)).collect(),
            allow: r.allow.into_iter().collect(),
            allow_regex: r.allow_regex.iter().filter_map(|x| full_regex(x)).collect(),
        })
        .collect())
}

pub fn parse_txt_blocklist(text: &str) -> BlockRule {
    BlockRule {
        block: text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect(),
        ..Default::default()
    }
}

fn app_ids_of(v: &serde_json::Value) -> Vec<String> {
    v.get("appids")
        .and_then(|a| a.get("list").or(Some(a)))
        .and_then(|l| l.as_array())
        .map(|seq| {
            seq.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_curated(text: &str) -> Result<Vec<CuratedRow>, String> {
    let doc: serde_json::Value = parse_yaml(text)?;
    let rows = doc
        .get("rows")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(rows
        .iter()
        .filter_map(|row| {
            let map = row.as_object()?;
            let (kind, body) = map.iter().next()?;
            match kind.as_str() {
                "section" => {
                    let title = body
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let subtitle = body
                        .get("subtitle")
                        .and_then(|s| s.get("string").or(Some(s)))
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    let app_ids = app_ids_of(body);
                    (!app_ids.is_empty()).then_some(CuratedRow::Section {
                        title,
                        subtitle,
                        app_ids,
                    })
                }
                "featured-carousel" => {
                    let app_ids = app_ids_of(body);
                    (!app_ids.is_empty()).then_some(CuratedRow::Carousel { app_ids })
                }
                // banner, articles, … are not supported (yet).
                _ => None,
            }
        })
        .collect())
}

// ── Loading ─────────────────────────────────────────────────────────────────

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(d) = std::env::var_os("KIOSQUE_CONFIG_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if crate::flatpak::in_sandbox() {
        dirs.push("/run/host/etc/kiosque".into());
    }
    dirs.push("/etc/kiosque".into());
    if let Some(d) = option_env!("KIOSQUE_DATADIR") {
        dirs.push(Path::new(d).join("kiosque"));
    }
    if crate::flatpak::in_sandbox() {
        dirs.push("/run/host/usr/share/kiosque".into());
        dirs.push("/app/share/kiosque".into());
    }
    dirs.push("/usr/share/kiosque".into());
    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// Resolve a path from a main config: absolute paths are used as-is (and
/// mapped through `/run/host` inside the sandbox when needed), relative ones
/// are taken relative to the main config's directory.
fn resolve(base: &Path, p: &Path) -> PathBuf {
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    if crate::flatpak::in_sandbox() && !p.exists() && !p.starts_with("/run/host") {
        let host = Path::new("/run/host").join(p.strip_prefix("/").unwrap_or(&p));
        if host.exists() {
            return host;
        }
    }
    p
}

pub fn load_from_dirs(dirs: &[PathBuf]) -> DistroConfig {
    let mut cfg = DistroConfig::default();
    let mut rules = vec![];
    let mut start_on_curated = None;
    let mut hide_auto = None;

    let read = |path: &Path, files: &mut Vec<PathBuf>| -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        files.push(path.to_path_buf());
        Some(text)
    };

    for dir in dirs {
        let main_path = dir.join("main.yaml");
        let (curated, yaml_bl, txt_bl) = if let Some(text) = read(&main_path, &mut cfg.files) {
            let main: MainFile = match parse_yaml(&text) {
                Ok(m) => m,
                Err(e) => {
                    kerr!("config: {}: {}", main_path.display(), e);
                    continue;
                }
            };
            start_on_curated = start_on_curated.or(main.start_on_curated);
            hide_auto = hide_auto.or(main.hide_auto_update_options);
            let r = |v: Vec<PathBuf>| v.iter().map(|p| resolve(dir, p)).collect::<Vec<_>>();
            (
                r(main.curated_config_paths),
                r(main.yaml_blocklist_paths),
                r(main.txt_blocklist_paths),
            )
        } else {
            (
                vec![dir.join("curated.yaml")],
                vec![dir.join("blocklist.yaml")],
                vec![dir.join("blocklist.txt")],
            )
        };

        for p in curated {
            if let Some(text) = read(&p, &mut cfg.files) {
                match parse_curated(&text) {
                    Ok(rows) => cfg.rows.extend(rows),
                    Err(e) => kerr!("config: {}: {}", p.display(), e),
                }
            }
        }
        for p in yaml_bl {
            if let Some(text) = read(&p, &mut cfg.files) {
                match parse_yaml_blocklist(&text) {
                    Ok(r) => rules.extend(r),
                    Err(e) => kerr!("config: {}: {}", p.display(), e),
                }
            }
        }
        for p in txt_bl {
            if let Some(text) = read(&p, &mut cfg.files) {
                rules.push(parse_txt_blocklist(&text));
            }
        }
    }
    cfg.blocklist = Blocklist::new(rules);
    cfg.start_on_curated = start_on_curated.unwrap_or(false);
    cfg.hide_auto_update_options = hide_auto.unwrap_or(false);
    cfg
}

fn slot() -> &'static RwLock<Arc<DistroConfig>> {
    static SLOT: OnceLock<RwLock<Arc<DistroConfig>>> = OnceLock::new();
    SLOT.get_or_init(|| {
        let cfg = load_from_dirs(&search_dirs());
        if !cfg.files.is_empty() {
            klog!("config: loaded {:?}", cfg.files);
        }
        start_watcher();
        RwLock::new(Arc::new(cfg))
    })
}

/// The current distributor configuration.
pub fn config() -> Arc<DistroConfig> {
    slot().read().unwrap().clone()
}

fn changes_tx() -> &'static broadcast::Sender<()> {
    static TX: OnceLock<broadcast::Sender<()>> = OnceLock::new();
    TX.get_or_init(|| broadcast::channel(4).0)
}

/// Fires after the configuration was reloaded because a file changed.
pub fn config_changes() -> broadcast::Receiver<()> {
    changes_tx().subscribe()
}

fn start_watcher() {
    crate::runtime::runtime().spawn(async {
        // Candidate files: everything that was read plus the files whose
        // appearance would change the result.
        let stamps = || -> Vec<Option<SystemTime>> {
            let mut paths: Vec<PathBuf> = config().files.clone();
            for d in search_dirs() {
                for f in [
                    "main.yaml",
                    "curated.yaml",
                    "blocklist.yaml",
                    "blocklist.txt",
                ] {
                    paths.push(d.join(f));
                }
            }
            paths
                .iter()
                .map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
                .collect()
        };
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        tick.tick().await;
        let mut last = stamps();
        loop {
            tick.tick().await;
            let now = stamps();
            if now != last {
                let cfg = load_from_dirs(&search_dirs());
                klog!("config: reloaded ({} curated rows)", cfg.rows.len());
                *slot().write().unwrap() = Arc::new(cfg);
                last = stamps();
                let _ = changes_tx().send(());
            }
        }
    });
}

/// Built-in featured apps used when no curated config is installed.
pub fn default_featured(distro_id: &str) -> Vec<&'static str> {
    match distro_id {
        "fedora" => vec![
            "org.mozilla.firefox",
            "org.gimp.GIMP",
            "org.videolan.VLC",
            "org.blender.Blender",
            "org.inkscape.Inkscape",
        ],
        "ubuntu" | "pop" | "mint" => {
            vec![
                "org.mozilla.firefox",
                "org.telegram.desktop",
                "org.videolan.VLC",
                "org.gimp.GIMP",
                "com.spotify.Client",
            ]
        }
        "neon" | "arch" | "opensuse" | "opensuse-tumbleweed" | "manjaro" => {
            vec![
                "org.kde.krita",
                "org.kde.kdenlive",
                "org.kde.kate",
                "org.kde.kstars",
                "org.mozilla.firefox",
            ]
        }
        _ => vec![
            "org.mozilla.firefox",
            "org.kde.krita",
            "org.videolan.VLC",
            "org.gimp.GIMP",
            "org.blender.Blender",
        ],
    }
}

/// `ID=` from os-release (host's when sandboxed).
pub fn distro_id() -> String {
    for path in [
        "/run/host/os-release",
        "/etc/os-release",
        "/usr/lib/os-release",
    ] {
        if let Ok(content) = std::fs::read_to_string(path)
            && let Some(id) = content.lines().find_map(|l| l.strip_prefix("ID="))
        {
            return id.trim_matches('"').to_string();
        }
    }
    "generic".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    const BL: &str = r#"
blocklists:
  - priority: 1
    block:
      - io.github.kolunmi.Bazaar
      - com.valvesoftware.Steam
  - block:
      - org.kde.kate
    allow:
      - com.valvesoftware.Steam
    conditions:
      - match-envvar:
          var: XDG_CURRENT_DESKTOP
          regex: KDE
  - priority: 0
    block-regex:
      - com\.place\..*
  - priority: -1
    conditions:
      - match-locale:
          regex: ar
    allow:
      - com.place.App3
"#;

    #[test]
    fn bazaar_blocklist_semantics() {
        let bl = Blocklist::new(parse_yaml_blocklist(BL).unwrap());
        let kde = env(&[("XDG_CURRENT_DESKTOP", "KDE"), ("LANG", "en_US.UTF-8")]);
        let gnome = env(&[("XDG_CURRENT_DESKTOP", "GNOME"), ("LANG", "ar.UTF-8")]);
        // priority 0 KDE rule allows Steam before the priority 1 block
        assert!(!bl.is_blocked_with("com.valvesoftware.Steam", &kde));
        assert!(bl.is_blocked_with("com.valvesoftware.Steam", &gnome));
        assert!(bl.is_blocked_with("org.kde.kate", &kde));
        assert!(!bl.is_blocked_with("org.kde.kate", &gnome));
        assert!(bl.is_blocked_with("com.place.App3", &kde));
        // the Arabic-locale rule has more priority and allows it
        assert!(!bl.is_blocked_with("com.place.App3", &gnome));
        // regexes match whole ids only
        assert!(!bl.is_blocked_with("org.com.place.X", &kde));
        assert!(!bl.is_blocked_with("org.gnome.Calculator", &kde));
    }

    #[test]
    fn inverted_condition() {
        let yaml = "blocklists:\n  - block: [a]\n    conditions:\n      - match-locale:\n          regex: en.*\n          post-process: invert\n";
        let bl = Blocklist::new(parse_yaml_blocklist(yaml).unwrap());
        assert!(!bl.is_blocked_with("a", &env(&[("LANG", "en_GB.UTF-8")])));
        assert!(bl.is_blocked_with("a", &env(&[("LANG", "de_DE.UTF-8")])));
    }

    #[test]
    fn txt_blocklist() {
        let rule = parse_txt_blocklist("# comment\ncom.jetbrains.CLion\n\n  net.lutris.Lutris \n");
        let bl = Blocklist::new(vec![rule]);
        assert!(bl.is_blocked("net.lutris.Lutris"));
        assert!(!bl.is_blocked("# comment"));
    }

    #[test]
    fn curated_rows() {
        let yaml = r#"
rows:
  - banner:
      height: 250
  - section:
      title: "Welcome"
      subtitle:
        string: |
          Hello *there*
      appids:
        list: [org.gnome.Calendar, org.gnome.Maps]
  - featured-carousel:
      appids:
        list: [io.github.kolunmi.Bazaar]
"#;
        let rows = parse_curated(yaml).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            CuratedRow::Section {
                title: "Welcome".into(),
                subtitle: "Hello *there*".into(),
                app_ids: vec!["org.gnome.Calendar".into(), "org.gnome.Maps".into()]
            }
        );
        let cfg = DistroConfig {
            rows,
            ..Default::default()
        };
        assert_eq!(
            cfg.featured_ids(),
            vec!["io.github.kolunmi.Bazaar".to_string()]
        );
    }

    #[test]
    fn main_config_and_dropins() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(
            a.path().join("main.yaml"),
            "curated-config-paths: [extra/c.yaml]\ntxt-blocklist-paths: [bl.txt]\nstart-on-curated: true\n",
        )
        .unwrap();
        std::fs::create_dir(a.path().join("extra")).unwrap();
        std::fs::write(
            a.path().join("extra/c.yaml"),
            "rows:\n  - section:\n      title: A\n      appids: [x]\n",
        )
        .unwrap();
        std::fs::write(a.path().join("bl.txt"), "blocked.App\n").unwrap();
        // b has no main.yaml: drop-in names are used
        std::fs::write(
            b.path().join("curated.yaml"),
            "rows:\n  - section:\n      title: B\n      appids: [y]\n",
        )
        .unwrap();
        std::fs::write(
            b.path().join("main.yaml.disabled"),
            "start-on-curated: false\n",
        )
        .unwrap();

        let cfg = load_from_dirs(&[a.path().to_path_buf(), b.path().to_path_buf()]);
        assert!(cfg.start_on_curated);
        assert_eq!(cfg.sections().len(), 2);
        assert_eq!(cfg.sections()[0].0, "A");
        assert!(cfg.blocklist.is_blocked("blocked.App"));
        assert_eq!(cfg.files.len(), 4);
    }
}
