//! Where application listings and details come from.
//!
//! The local AppStream [`Catalog`] of every configured remote is the base:
//! it works offline and covers every repository, not only Flathub. The
//! Flathub web API enriches it (popularity order) and stands in for Flathub
//! itself only while its AppStream data is not indexed locally (first run,
//! or Flathub not configured). Permissions come from the origin repository's
//! own metadata, which describes exactly what an installation grants.

use std::time::Duration;

use crate::catalog::{Catalog, CatalogApp};
use crate::config::Blocklist;
use crate::flathub::client::FlathubClient;
use crate::flathub::types::{AppDetails, FlathubApp};
use crate::flatpak::FlatpakBackend;

/// The remote whose data the Flathub web API describes.
pub const FLATHUB: &str = "flathub";

const SEARCH_LIMIT: usize = 200;
const RELATED_LIMIT: usize = 8;
/// How long an optional enrichment request may delay a local answer.
const ENRICH_TIMEOUT: Duration = Duration::from_secs(3);

/// The data sources a request consults.
#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub catalog: &'a Catalog,
    pub flathub: &'a FlathubClient,
    pub backend: &'a dyn FlatpakBackend,
    pub blocklist: &'a Blocklist,
}

impl Sources<'_> {
    /// True when Flathub's own AppStream data is indexed, so the web API is
    /// only needed for enrichment.
    fn flathub_is_local(&self) -> bool {
        self.catalog.indexes(FLATHUB)
    }

    fn allowed(&self, apps: &mut Vec<FlathubApp>) {
        apps.retain(|app| !self.blocklist.is_blocked(&app.app_id));
    }
}

/// Append the apps of `extra` whose ids are not in `apps` yet.
fn merge(apps: &mut Vec<FlathubApp>, extra: Vec<FlathubApp>) {
    for app in extra {
        if !apps.iter().any(|a| a.app_id == app.app_id) {
            apps.push(app);
        }
    }
}

fn listing(apps: Vec<CatalogApp>) -> Vec<FlathubApp> {
    apps.iter().map(CatalogApp::to_flathub_app).collect()
}

/// Search every indexed remote. Flathub's web search adds results only while
/// Flathub's AppStream data is not available locally. Fails only when no
/// source produced anything and the web search failed.
pub async fn search(src: Sources<'_>, query: &str) -> Result<Vec<FlathubApp>, String> {
    let mut apps = listing(src.catalog.search(query, SEARCH_LIMIT));
    if !src.flathub_is_local() {
        match src.flathub.search(query).await {
            Ok(web) => merge(&mut apps, web),
            Err(e) if apps.is_empty() => return Err(e.to_string()),
            Err(e) => kerr!("apps::search: Flathub search failed: {}", e),
        }
    }
    src.allowed(&mut apps);
    Ok(apps)
}

/// The apps of `category`, which is either a main category (`Graphics`) or
/// `<Main>-<Sub>` (see [`matches_subcategory`]).
pub async fn category(src: Sources<'_>, category: &str) -> Result<Vec<FlathubApp>, String> {
    let (main, sub) = match category.split_once('-') {
        Some((main, sub)) => (main, Some(sub)),
        None => (category, None),
    };
    let mut apps = listing(src.catalog.by_category(main));
    if src.flathub_is_local() {
        // Flathub's order reflects popularity; apps it does not list follow
        // in name order. Without an answer in time, name order stands.
        if let Ok(Ok(web)) =
            tokio::time::timeout(ENRICH_TIMEOUT, src.flathub.fetch_category(main)).await
        {
            let rank = |id: &str| {
                web.iter()
                    .position(|w| w.app_id == id)
                    .unwrap_or(usize::MAX)
            };
            apps.sort_by_key(|app| rank(&app.app_id));
        }
    } else {
        match src.flathub.fetch_category(main).await {
            Ok(mut web) => {
                merge(&mut web, apps);
                apps = web;
            }
            Err(e) if apps.is_empty() => return Err(e.to_string()),
            Err(e) => kerr!("apps::category: Flathub category failed: {}", e),
        }
    }
    if let Some(sub) = sub {
        apps.retain(|app| matches_subcategory(app, main, sub));
    }
    src.allowed(&mut apps);
    Ok(apps)
}

/// Drop from `similar` the apps already listed in `shown`, so the detail
/// page never lists an app under both "More by" and "Similar".
fn distinct_from(similar: &mut Vec<FlathubApp>, shown: &[FlathubApp]) {
    similar.retain(|app| !shown.iter().any(|s| s.app_id == app.app_id));
}

/// Everything the detail page shows apart from ratings and reviews.
#[derive(Debug, Clone, Default)]
pub struct DetailData {
    pub name: String,
    pub summary: String,
    /// Plain text (see [`crate::security::description_text`]).
    pub description: String,
    /// Raw icon URL (`file://` for cached catalog icons).
    pub icon: String,
    pub developer: String,
    pub license: String,
    /// Raw screenshot URLs, default screenshot first.
    pub screenshots: Vec<String>,
    /// Link type → HTTPS URL (`homepage`, `bugtracker`, `donation`, `help`,
    /// `vcs_browser`, `manifest`).
    pub urls: serde_json::Value,
    /// Flathub's `metadata.permissions` shape, or `null` when unknown.
    pub permissions: serde_json::Value,
    pub developer_apps: Vec<FlathubApp>,
    pub similar_apps: Vec<FlathubApp>,
    /// `{version, timestamp, description}` of the newest release (plain-text
    /// notes, timestamp 0 when unknown), or `null` without release data.
    pub latest_release: serde_json::Value,
    /// `{name, title, scope, url}` of the repository the app comes from, or
    /// `null` when it is only known from the Flathub web API.
    pub origin: serde_json::Value,
}

/// Details of `app_id`: from its origin repository's AppStream data when the
/// catalog has it, otherwise from the Flathub web API.
pub async fn details(src: Sources<'_>, app_id: &str) -> Result<DetailData, String> {
    match src.catalog.get(app_id) {
        Some(entry) => Ok(catalog_details(src, entry).await),
        None => flathub_details(src.flathub, app_id).await,
    }
}

const LINK_KEYS: [&str; 5] = ["homepage", "bugtracker", "donation", "help", "vcs_browser"];

fn secure_urls<'a>(links: impl Iterator<Item = (&'a str, &'a str)>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in links {
        if let Ok(url) = crate::security::https_url(value) {
            map.insert(key.to_string(), serde_json::Value::String(url.into()));
        }
    }
    serde_json::Value::Object(map)
}

async fn catalog_details(src: Sources<'_>, entry: CatalogApp) -> DetailData {
    let reference = if entry.bundle.is_empty() {
        entry.id.clone()
    } else {
        entry.bundle.clone()
    };
    let permissions = match src
        .backend
        .remote_metadata(&entry.remote, entry.scope, &reference)
        .await
    {
        Ok(keyfile) => crate::flatpak::permissions_from_metadata(&keyfile),
        Err(e) => {
            kerr!(
                "apps::details: metadata of {} from {}: {}",
                entry.id,
                entry.remote,
                e
            );
            // Flathub's summary describes only what Flathub publishes.
            if entry.remote == FLATHUB {
                summary_permissions(src.flathub, &entry.id).await
            } else {
                serde_json::Value::Null
            }
        }
    };

    let mut developer_apps = if entry.developer.is_empty() {
        vec![]
    } else {
        listing(src.catalog.by_developer(&entry.developer))
    };
    developer_apps.retain(|app| app.app_id != entry.id);
    developer_apps.truncate(RELATED_LIMIT);
    let mut similar_apps = listing(src.catalog.similar(&entry, RELATED_LIMIT * 2));
    src.allowed(&mut developer_apps);
    src.allowed(&mut similar_apps);
    distinct_from(&mut similar_apps, &developer_apps);
    similar_apps.truncate(RELATED_LIMIT);

    let latest_release = if entry.version.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::json!({
            "version": entry.version,
            "timestamp": entry.release_timestamp,
            "description": crate::security::description_text(&entry.release_notes),
        })
    };
    let remote = src.catalog.remote(&entry.remote, entry.scope);
    let origin = serde_json::json!({
        "name": entry.remote,
        "title": remote
            .map(|r| r.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| entry.remote.clone()),
        "scope": entry.scope.as_str(),
        "url": remote
            .and_then(|r| crate::security::https_url(&r.url).ok())
            .map(String::from)
            .unwrap_or_default(),
    });
    let manifest = entry.metadata.get("flathub::manifest");
    let urls = secure_urls(
        LINK_KEYS
            .iter()
            .filter_map(|k| entry.urls.get(*k).map(|v| (*k, v.as_str())))
            .chain(manifest.map(|m| ("manifest", m.as_str()))),
    );

    DetailData {
        description: crate::security::description_text(&entry.description),
        screenshots: entry.screenshots.clone(),
        icon: entry.icon.clone(),
        developer: entry.developer.clone(),
        license: entry.license.clone(),
        summary: entry.summary.clone(),
        name: entry.name.clone(),
        urls,
        permissions,
        developer_apps,
        similar_apps,
        latest_release,
        origin,
    }
}

async fn summary_permissions(client: &FlathubClient, app_id: &str) -> serde_json::Value {
    match client.fetch_summary(app_id).await {
        Ok(summary) => summary
            .get("metadata")
            .and_then(|m| m.get("permissions"))
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        Err(e) => {
            kerr!("apps::details: fetch_summary failed: {}", e);
            serde_json::Value::Null
        }
    }
}

/// Details from the Flathub web API, for apps not in the local catalog.
pub async fn flathub_details(client: &FlathubClient, app_id: &str) -> Result<DetailData, String> {
    let details = client
        .fetch_details(app_id)
        .await
        .map_err(|e| e.to_string())?;
    let (permissions, developer_apps, similar_apps) =
        fetch_supplementary(client, &details, app_id).await;
    let latest_release = details
        .latest_release()
        .map(|release| {
            serde_json::json!({
                "version": release.version,
                "timestamp": release.timestamp.unwrap_or(0),
                "description": crate::security::description_text(
                    release.description.as_deref().unwrap_or_default(),
                ),
            })
        })
        .unwrap_or(serde_json::Value::Null);
    Ok(DetailData {
        latest_release,
        screenshots: screenshot_urls(&details),
        urls: urls_object(&details),
        name: details.name,
        summary: details.summary,
        description: crate::security::description_text(
            details.description.as_deref().unwrap_or_default(),
        ),
        icon: details.icon.unwrap_or_default(),
        developer: details.developer_name.unwrap_or_default(),
        license: details.project_license.unwrap_or_default(),
        permissions,
        developer_apps,
        similar_apps,
        origin: serde_json::Value::Null,
    })
}

/// Concurrently fetch an app's sandbox permissions (summary endpoint), other
/// apps by the same developer and similar apps from its primary category.
/// Failures degrade to empty values. Lists exclude the app itself and are
/// capped.
async fn fetch_supplementary(
    client: &FlathubClient,
    details: &AppDetails,
    app_id: &str,
) -> (serde_json::Value, Vec<FlathubApp>, Vec<FlathubApp>) {
    let dev_name = details.developer_name.clone();
    let category = details.categories.first().cloned();

    let dev_fut = async {
        match dev_name {
            Some(ref dev) => client.fetch_developer_apps(dev).await.unwrap_or_else(|e| {
                kerr!("apps::details: fetch_developer_apps failed: {}", e);
                vec![]
            }),
            None => vec![],
        }
    };
    let cat_fut = async {
        match category {
            Some(ref cat) => client.fetch_category(cat).await.unwrap_or_else(|e| {
                kerr!("apps::details: fetch_category failed: {}", e);
                vec![]
            }),
            None => vec![],
        }
    };

    let (permissions, mut developer_apps, mut similar_apps) =
        tokio::join!(summary_permissions(client, app_id), dev_fut, cat_fut);

    similar_apps.retain(|app| app.app_id != app_id);
    developer_apps.retain(|app| app.app_id != app_id);
    developer_apps.truncate(RELATED_LIMIT);
    distinct_from(&mut similar_apps, &developer_apps);
    similar_apps.truncate(RELATED_LIMIT);

    (permissions, developer_apps, similar_apps)
}

/// The highest-resolution URL of each screenshot.
fn screenshot_urls(details: &AppDetails) -> Vec<String> {
    details
        .screenshots
        .iter()
        .filter_map(|s| {
            s.sizes
                .iter()
                .max_by_key(|size| {
                    size.width
                        .as_ref()
                        .and_then(|w| {
                            w.as_str()
                                .and_then(|s| s.parse::<i32>().ok())
                                .or_else(|| w.as_i64().map(|i| i as i32))
                        })
                        .unwrap_or(0)
                })
                .map(|size| size.src.clone())
                .or_else(|| s.sizes.first().map(|size| size.src.clone()))
        })
        .collect()
}

/// The app's secure external links plus its build manifest.
fn urls_object(details: &AppDetails) -> serde_json::Value {
    let mut links: Vec<(&str, &str)> = vec![];
    if let Some(ref u) = details.urls {
        for (key, value) in [
            ("homepage", &u.homepage),
            ("bugtracker", &u.bugtracker),
            ("donation", &u.donation),
            ("help", &u.help),
            ("vcs_browser", &u.vcs_browser),
        ] {
            if let Some(v) = value {
                links.push((key, v));
            }
        }
    }
    if let Some(ref m) = details.metadata
        && let Some(manifest) = &m.manifest
    {
        links.push(("manifest", manifest));
    }
    secure_urls(links.into_iter())
}

/// Decide whether `app` belongs in the `<prefix>-<sub_id>` subcategory, e.g.
/// `Game-Emulator` or `Graphics-Vector`, from its AppStream categories plus a
/// few name/summary keyword heuristics for games. Unknown prefixes/sub-ids
/// keep the app.
pub fn matches_subcategory(app: &FlathubApp, prefix: &str, sub_id: &str) -> bool {
    let has = |name: &str| {
        app.sub_categories
            .iter()
            .any(|s| s.eq_ignore_ascii_case(name))
    };
    match prefix {
        "Game" => {
            let name_lower = app.name.to_lowercase();
            let summary_lower = app
                .summary
                .as_ref()
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            let is_emulator = has("emulator");
            let is_launcher = (has("packagemanager")
                || name_lower.contains("launcher")
                || name_lower.contains("client")
                || summary_lower.contains("launcher"))
                && !is_emulator;
            let tool_keywords = [
                "tool", "compat", "patcher", "config", "manager", "overlay", "hud", "backup",
                "setup",
            ];
            let is_tool = (has("utility")
                || tool_keywords.iter().any(|kw| name_lower.contains(kw))
                || tool_keywords.iter().any(|kw| summary_lower.contains(kw)))
                && !is_emulator
                && !is_launcher;
            match sub_id {
                "Emulator" => is_emulator,
                "Launcher" => is_launcher,
                "Tool" => is_tool,
                "Game" => !is_emulator && !is_launcher && !is_tool,
                _ => true,
            }
        }
        "AudioVideo" => {
            let is_player = has("player");
            let is_recorder = has("recorder");
            let is_editing =
                has("audiovideoediting") || has("midi") || has("sequencer") || has("mixer");
            match sub_id {
                "Player" => is_player,
                "Recorder" => is_recorder,
                "Editing" => is_editing,
                "All" => !is_player && !is_recorder && !is_editing,
                _ => true,
            }
        }
        "Development" => {
            let is_ide = has("ide") || has("guidedesigner");
            let is_debugger = has("debugger") || has("profiler");
            let is_web = has("webdevelopment");
            match sub_id {
                "IDE" => is_ide,
                "Debugger" => is_debugger,
                "Web" => is_web,
                "All" => !is_ide && !is_debugger && !is_web,
                _ => true,
            }
        }
        "Graphics" => {
            let is_3d = has("3dgraphics");
            let is_vector = has("vectorgraphics");
            let is_raster = has("rastergraphics");
            let is_photography = has("photography");
            let is_viewer = has("viewer");
            match sub_id {
                "3D" => is_3d,
                "Vector" => is_vector,
                "Raster" => is_raster,
                "Photography" => is_photography,
                "Viewer" => is_viewer,
                "All" => !is_3d && !is_vector && !is_raster && !is_photography && !is_viewer,
                _ => true,
            }
        }
        "Office" => {
            let is_word = has("wordprocessor");
            let is_spreadsheet = has("spreadsheet");
            let is_presentation = has("presentation");
            let is_finance = has("finance");
            match sub_id {
                "WordProcessor" => is_word,
                "Spreadsheet" => is_spreadsheet,
                "Presentation" => is_presentation,
                "Finance" => is_finance,
                "All" => !is_word && !is_spreadsheet && !is_presentation && !is_finance,
                _ => true,
            }
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::AppCache;
    use crate::config::{Blocklist, parse_txt_blocklist};
    use crate::flathub::client::HttpClient;
    use crate::flatpak::mock::MockBackend;
    use crate::flatpak::{RemoteInfo, Scope};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn app(id: &str, remote: &str, name: &str, categories: &[&str]) -> CatalogApp {
        CatalogApp {
            id: id.into(),
            remote: remote.into(),
            scope: Scope::System,
            name: name.into(),
            summary: format!("{name} summary"),
            developer: "Example Devs".into(),
            categories: categories.iter().map(|c| c.to_string()).collect(),
            bundle: format!("app/{id}/x86_64/stable"),
            version: "2.0".into(),
            release_timestamp: 1_700_000_000,
            release_notes: "<p>Faster</p>".into(),
            urls: [
                ("homepage".to_string(), "https://example.org".to_string()),
                (
                    "donation".to_string(),
                    "http://insecure.example.org".to_string(),
                ),
            ]
            .into(),
            ..Default::default()
        }
    }

    fn remote(name: &str, title: &str) -> RemoteInfo {
        RemoteInfo {
            name: name.into(),
            title: title.into(),
            url: format!("https://{name}.example.org/repo/"),
            scope: Scope::System,
            ..Default::default()
        }
    }

    /// Answers every request with `status` and `body`, counting requests.
    async fn server(status: u16, body: &'static str) -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = vec![0; 8192];
                let _ = stream.read(&mut buffer).await;
                count.fetch_add(1, Ordering::SeqCst);
                let response = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        (url, hits)
    }

    fn client(url: &str) -> FlathubClient {
        FlathubClient::with_dependencies(HttpClient::for_fixture(), url, AppCache::new())
    }

    const WEB_HITS: &str = r#"{"hits":[{"app_id":"org.web.Paint","name":"Web Paint"},{"app_id":"org.local.Paint","name":"Paint"}]}"#;

    #[tokio::test]
    async fn local_flathub_data_answers_without_the_web_api() {
        let catalog = Catalog::from_apps(
            vec![
                app("org.local.Paint", "flathub", "Paint", &["Graphics"]),
                app("org.other.Paint", "fedora", "Paint Too", &["Graphics"]),
            ],
            vec![remote("flathub", "Flathub"), remote("fedora", "Fedora")],
        );
        let (url, hits) = server(503, "{}").await;
        let flathub = client(&url);
        let backend = MockBackend::default();
        let blocklist = Blocklist::default();
        let src = Sources {
            catalog: &catalog,
            flathub: &flathub,
            backend: &backend,
            blocklist: &blocklist,
        };
        let found = search(src, "paint").await.unwrap();
        let ids: Vec<_> = found.iter().map(|a| a.app_id.as_str()).collect();
        assert_eq!(ids, ["org.local.Paint", "org.other.Paint"]);
        assert_eq!(hits.load(Ordering::SeqCst), 0, "search stays offline");

        // Categories still work while the web API (used only for order) fails.
        let listed = category(src, "Graphics").await.unwrap();
        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn web_results_supplement_a_catalog_without_flathub() {
        let catalog = Catalog::from_apps(
            vec![app("org.local.Paint", "fedora", "Paint", &["Graphics"])],
            vec![remote("fedora", "Fedora")],
        );
        let (url, _) = server(200, WEB_HITS).await;
        let flathub = client(&url);
        let backend = MockBackend::default();
        let blocklist = Blocklist::new(vec![parse_txt_blocklist("org.web.Paint\n")]);
        let src = Sources {
            catalog: &catalog,
            flathub: &flathub,
            backend: &backend,
            blocklist: &blocklist,
        };
        let ids: Vec<_> = search(src, "paint")
            .await
            .unwrap()
            .into_iter()
            .map(|a| a.app_id)
            .collect();
        assert_eq!(ids, ["org.local.Paint"], "deduplicated, blocklist applied");

        // With the web API down, local results are still returned...
        let down = client("http://127.0.0.1:9");
        let src = Sources {
            flathub: &down,
            ..src
        };
        assert_eq!(search(src, "paint").await.unwrap().len(), 1);
        // ...and only a search nothing answers is an error.
        assert!(search(src, "nothing-matches").await.is_err());
        assert!(category(src, "Office").await.is_err());
        assert_eq!(category(src, "Graphics").await.unwrap().len(), 1);
        assert!(category(src, "Graphics-Vector").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn catalog_details_use_the_origin_repository() {
        let mut fedora_app = app("org.fedora.Atlas", "fedora", "Atlas", &["Education"]);
        fedora_app.description = "<p>Maps &amp; <em>more</em></p>".into();
        let catalog = Catalog::from_apps(
            vec![
                fedora_app,
                app("org.fedora.Globe", "fedora", "Globe", &["Education"]),
            ],
            vec![remote("fedora", "Fedora Flatpaks")],
        );
        let (url, hits) = server(503, "{}").await;
        let flathub = client(&url);
        let backend = MockBackend::default();
        backend.metadata.lock().unwrap().insert(
            "org.fedora.Atlas".into(),
            "[Context]\nshared=network;\nsockets=wayland;\n".into(),
        );
        let blocklist = Blocklist::default();
        let src = Sources {
            catalog: &catalog,
            flathub: &flathub,
            backend: &backend,
            blocklist: &blocklist,
        };
        let data = details(src, "org.fedora.Atlas").await.unwrap();
        assert_eq!(data.name, "Atlas");
        assert_eq!(data.description, "Maps & more");
        assert_eq!(
            data.permissions,
            serde_json::json!({"shared": ["network"], "sockets": ["wayland"]})
        );
        assert_eq!(
            backend.metadata_requests.lock().unwrap()[0],
            (
                "fedora".to_string(),
                "app/org.fedora.Atlas/x86_64/stable".to_string()
            )
        );
        assert_eq!(data.origin["name"], "fedora");
        assert_eq!(data.origin["title"], "Fedora Flatpaks");
        assert_eq!(data.origin["scope"], "system");
        assert_eq!(
            data.urls,
            serde_json::json!({"homepage": "https://example.org/"}),
            "insecure links are dropped"
        );
        assert_eq!(data.latest_release["version"], "2.0");
        assert_eq!(data.latest_release["description"], "Faster");
        let ids = |apps: &[FlathubApp]| apps.iter().map(|a| a.app_id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&data.developer_apps), ["org.fedora.Globe"]);
        assert!(
            data.similar_apps.is_empty(),
            "apps under \"More by\" are not repeated as similar"
        );

        // Without repository metadata a non-Flathub app's permissions stay
        // unknown rather than borrowing Flathub's description.
        let data = details(src, "org.fedora.Globe").await.unwrap();
        assert!(data.permissions.is_null());
        assert_eq!(hits.load(Ordering::SeqCst), 0, "no Flathub request");
    }

    #[tokio::test]
    async fn flathub_apps_fall_back_to_the_summary_permissions() {
        let catalog = Catalog::from_apps(
            vec![app("org.local.Paint", "flathub", "Paint", &["Graphics"])],
            vec![remote("flathub", "Flathub")],
        );
        let (url, hits) = server(200, r#"{"metadata":{"permissions":{"shared":["ipc"]}}}"#).await;
        let flathub = client(&url);
        let backend = MockBackend::default();
        let blocklist = Blocklist::default();
        let src = Sources {
            catalog: &catalog,
            flathub: &flathub,
            backend: &backend,
            blocklist: &blocklist,
        };
        let data = details(src, "org.local.Paint").await.unwrap();
        assert_eq!(data.permissions, serde_json::json!({"shared": ["ipc"]}));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
