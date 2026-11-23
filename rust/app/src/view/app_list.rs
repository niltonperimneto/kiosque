//! App-list queries: which collection to load, request ordering and the
//! derived subcategory filter. Qt-free.

use std::time::Duration;

use kiosque_core::entries::AppEntry;
use kiosque_core::flathub::types::FlathubApp;

/// Delay before a search request is sent; newer keystrokes restart it.
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(120);

/// A collection an app list can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSource {
    Popular,
    Mobile,
    CategorySorted(String, String),
    New,
    Updated,
    Search(String),
    /// A Flathub category, optionally narrowed as `Prefix-SubId`.
    Category(String),
}

impl AppSource {
    /// The QML form: `load("popular" | "new" | "updated" | "search" | "category", arg)`.
    pub fn parse(source: &str, arg: &str) -> Option<Self> {
        match source {
            "mobile" => Some(Self::Mobile),
            "category-sorted" => arg
                .split_once(':')
                .map(|(c, s)| Self::CategorySorted(c.into(), s.into())),
            "popular" => Some(Self::Popular),
            "new" => Some(Self::New),
            "updated" => Some(Self::Updated),
            "search" => Some(Self::Search(arg.trim().to_string())),
            "category" if !arg.is_empty() => Some(Self::Category(arg.to_string())),
            _ => None,
        }
    }

    /// Searches are debounced; collections load immediately.
    pub fn debounce(&self) -> Option<Duration> {
        matches!(self, Self::Search(_)).then_some(SEARCH_DEBOUNCE)
    }
}

/// Request bookkeeping for one list. Every [`begin`](Self::begin) supersedes
/// earlier requests; a response is applied only if its generation is still
/// current, so out-of-order responses are dropped.
#[derive(Debug, Default)]
pub struct AppListQuery {
    generation: u64,
    current: Option<AppSource>,
}

impl AppListQuery {
    /// Start a request for `source` and return its generation.
    pub fn begin(&mut self, source: AppSource) -> u64 {
        self.generation += 1;
        self.current = Some(source);
        self.generation
    }

    /// Invalidate any in-flight request without starting a new one.
    pub fn cancel(&mut self) {
        self.generation += 1;
        self.current = None;
    }

    pub fn is_current(&self, generation: u64) -> bool {
        generation == self.generation && self.current.is_some()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn source(&self) -> Option<&AppSource> {
        self.current.as_ref()
    }
}

pub fn entry(app: FlathubApp) -> AppEntry {
    AppEntry {
        name: app.name,
        summary: app.summary.unwrap_or_default(),
        icon_url: app.icon.unwrap_or_default(),
        app_id: app.app_id,
    }
}

/// Fetch the entries for `source`. Search and categories come from the local
/// AppStream catalog of every repository, supplemented by Flathub (see
/// [`kiosque_core::apps`]); the other collections are Flathub's. Blocked apps
/// are dropped everywhere. An empty search yields no results without a
/// request.
pub async fn fetch(
    sources: kiosque_core::apps::Sources<'_>,
    source: &AppSource,
) -> Result<Vec<AppEntry>, String> {
    let client = sources.flathub;
    let apps = match source {
        AppSource::Search(q) if q.is_empty() => return Ok(vec![]),
        AppSource::Search(q) => return entries(kiosque_core::apps::search(sources, q).await),
        AppSource::Category(category) => {
            return entries(kiosque_core::apps::category(sources, category).await);
        }
        AppSource::Popular => client.fetch_popular().await,
        AppSource::Mobile => client.fetch_mobile().await,
        AppSource::CategorySorted(category, sort) if sort == "newest" => {
            client.fetch_category(category).await.map(|mut apps| {
                apps.sort_by_key(|app| std::cmp::Reverse(app.added_at.unwrap_or(0)));
                apps.truncate(30);
                apps
            })
        }
        AppSource::CategorySorted(category, sort) => {
            client.fetch_category_sorted(category, sort).await
        }
        AppSource::New => client.fetch_recently_added().await,
        // Offline, the catalog's newest releases stand in for Flathub's list.
        AppSource::Updated => match client.fetch_recently_updated().await {
            Err(e) if !sources.catalog.is_empty() => {
                kiosque_core::kerr!("recently updated: {}; using the catalog", e);
                Ok(sources
                    .catalog
                    .recently_updated(30)
                    .iter()
                    .map(|app| app.to_flathub_app())
                    .collect())
            }
            result => result,
        },
    }
    .map_err(|e| e.to_string())?;
    Ok(apps
        .into_iter()
        .filter(|app| !sources.blocklist.is_blocked(&app.app_id))
        .map(entry)
        .collect())
}

fn entries(result: Result<Vec<FlathubApp>, String>) -> Result<Vec<AppEntry>, String> {
    result.map(|apps| apps.into_iter().map(entry).collect())
}

/// The `<Prefix>-<SubId>` subcategory filter shared with the catalog policy.
pub use kiosque_core::apps::matches_subcategory;

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, subs: &[&str]) -> FlathubApp {
        FlathubApp {
            name: name.into(),
            sub_categories: subs.iter().map(|s| s.to_string()).collect(),
            ..serde_json::from_value::<FlathubApp>(serde_json::json!({})).unwrap()
        }
    }

    #[test]
    fn sources_parse_from_qml_arguments() {
        assert_eq!(AppSource::parse("popular", ""), Some(AppSource::Popular));
        assert_eq!(
            AppSource::parse("search", "  gimp "),
            Some(AppSource::Search("gimp".into()))
        );
        assert_eq!(
            AppSource::parse("category", "Game-Emulator"),
            Some(AppSource::Category("Game-Emulator".into()))
        );
        assert_eq!(AppSource::parse("category", ""), None);
        assert_eq!(AppSource::parse("bogus", "x"), None);
        assert_eq!(
            AppSource::Search("a".into()).debounce(),
            Some(SEARCH_DEBOUNCE)
        );
        assert_eq!(AppSource::Popular.debounce(), None);
    }

    #[test]
    fn stale_generations_are_rejected() {
        let mut q = AppListQuery::default();
        let first = q.begin(AppSource::Search("gi".into()));
        let second = q.begin(AppSource::Search("gimp".into()));
        assert!(!q.is_current(first), "older response must be dropped");
        assert!(q.is_current(second));
        q.cancel();
        assert!(!q.is_current(second));
        assert_eq!(q.source(), None);
    }

    #[test]
    fn subcategories_partition_games() {
        let emulator = app("RetroArch", &["Emulator"]);
        let launcher = app("Heroic Games Launcher", &[]);
        let tool = app("MangoHud config", &[]);
        let game = app("SuperTuxKart", &["ActionGame"]);
        for (a, expected) in [
            (&emulator, "Emulator"),
            (&launcher, "Launcher"),
            (&tool, "Tool"),
            (&game, "Game"),
        ] {
            for sub in ["Emulator", "Launcher", "Tool", "Game"] {
                assert_eq!(
                    matches_subcategory(a, "Game", sub),
                    sub == expected,
                    "{} in {}",
                    a.name,
                    sub
                );
            }
        }
        assert!(matches_subcategory(&game, "Unknown", "Whatever"));
        assert!(matches_subcategory(
            &app("Blender", &["3DGraphics"]),
            "Graphics",
            "3D"
        ));
        assert!(!matches_subcategory(
            &app("Blender", &["3DGraphics"]),
            "Graphics",
            "All"
        ));
    }
}
