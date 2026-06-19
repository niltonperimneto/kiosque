use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::flathub::types::{AppDetails, FlathubApp, AppStats, GlobalStats, LinterExceptions};
use crate::flathub::odrs::{OdrsRatings, OdrsReview};
use crate::flatpak::cli::FlatpakJsonApp;

// ── TTL Constants ───────────────────────────────────────────────────────────

/// How long collection responses (popular, trending, category, etc.) remain valid.
const COLLECTION_TTL: Duration = Duration::from_secs(5 * 60); // 5 minutes

/// How long individual app detail responses remain valid.
const DETAILS_TTL: Duration = Duration::from_secs(10 * 60); // 10 minutes

/// How long ODRS responses remain valid.
const ODRS_TTL: Duration = Duration::from_secs(10 * 60); // 10 minutes

/// How long statistics remain valid.
const STATS_TTL: Duration = Duration::from_secs(60 * 60); // 1 hour

/// How long linter exceptions remain valid.
const EXCEPTIONS_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

/// How long the locally-installed apps list remains valid.
/// Kept short because installs/uninstalls mutate this frequently.
const INSTALLED_TTL: Duration = Duration::from_secs(30); // 30 seconds

// ── Cache entry wrapper ─────────────────────────────────────────────────────

struct CacheEntry<T> {
    value: T,
    cached_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            cached_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }
}

// ── Main application cache ──────────────────────────────────────────────────

/// Thread-safe, TTL-based in-memory cache for Flathub API responses and
/// local Flatpak state. Accessed as a process-wide singleton via [`app_cache()`].
pub struct AppCache {
    /// Flathub collection responses, keyed by endpoint (e.g. "popular",
    /// "category/Game"). Each entry stores the full `Vec<FlathubApp>`.
    collections: RwLock<HashMap<String, CacheEntry<Vec<FlathubApp>>>>,

    /// Individual app detail responses, keyed by app ID.
    details: RwLock<HashMap<String, CacheEntry<AppDetails>>>,

    /// ODRS Ratings, keyed by app ID.
    odrs_ratings: RwLock<HashMap<String, CacheEntry<OdrsRatings>>>,

    /// ODRS Reviews, keyed by app ID.
    odrs_reviews: RwLock<HashMap<String, CacheEntry<Vec<OdrsReview>>>>,

    /// Parsed list of locally installed Flatpak apps.
    installed_list: RwLock<Option<CacheEntry<Vec<FlatpakJsonApp>>>>,

    /// Derived set of installed app IDs for fast `is_installed` lookups.
    installed_set: RwLock<Option<CacheEntry<HashSet<String>>>>,

    /// App statistics, keyed by app ID.
    app_stats: RwLock<HashMap<String, CacheEntry<AppStats>>>,

    /// Global statistics.
    global_stats: RwLock<Option<CacheEntry<GlobalStats>>>,

    /// Linter exceptions, keyed by app ID.
    exceptions: RwLock<HashMap<String, CacheEntry<LinterExceptions>>>,
}

impl AppCache {
    fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
            details: RwLock::new(HashMap::new()),
            odrs_ratings: RwLock::new(HashMap::new()),
            odrs_reviews: RwLock::new(HashMap::new()),
            installed_list: RwLock::new(None),
            installed_set: RwLock::new(None),
            app_stats: RwLock::new(HashMap::new()),
            global_stats: RwLock::new(None),
            exceptions: RwLock::new(HashMap::new()),
        }
    }

    // ── Collection cache ────────────────────────────────────────────────

    /// Try to get a cached collection by key. Returns `None` on miss or expiry.
    pub async fn get_collection(&self, key: &str) -> Option<Vec<FlathubApp>> {
        let map = self.collections.read().await;
        match map.get(key) {
            Some(entry) if !entry.is_expired(COLLECTION_TTL) => {
                eprintln!("[kiosque] CACHE HIT collection \"{}\"", key);
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    /// Store a collection response in the cache.
    pub async fn put_collection(&self, key: String, value: Vec<FlathubApp>) {
        eprintln!("[kiosque] CACHE STORE collection \"{}\" ({} items)", key, value.len());
        let mut map = self.collections.write().await;
        map.insert(key, CacheEntry::new(value));
    }

    // ── Details cache ───────────────────────────────────────────────────

    /// Try to get cached app details by app ID.
    pub async fn get_details(&self, app_id: &str) -> Option<AppDetails> {
        let map = self.details.read().await;
        match map.get(app_id) {
            Some(entry) if !entry.is_expired(DETAILS_TTL) => {
                eprintln!("[kiosque] CACHE HIT details \"{}\"", app_id);
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    /// Store app details in the cache.
    pub async fn put_details(&self, app_id: String, value: AppDetails) {
        eprintln!("[kiosque] CACHE STORE details \"{}\"", app_id);
        let mut map = self.details.write().await;
        map.insert(app_id, CacheEntry::new(value));
    }
    // ── ODRS cache ──────────────────────────────────────────────────────

    pub async fn get_odrs_ratings(&self, app_id: &str) -> Option<OdrsRatings> {
        let map = self.odrs_ratings.read().await;
        match map.get(app_id) {
            Some(entry) if !entry.is_expired(ODRS_TTL) => {
                eprintln!("[kiosque] CACHE HIT odrs_ratings \"{}\"", app_id);
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    pub async fn put_odrs_ratings(&self, app_id: String, value: OdrsRatings) {
        eprintln!("[kiosque] CACHE STORE odrs_ratings \"{}\"", app_id);
        let mut map = self.odrs_ratings.write().await;
        map.insert(app_id, CacheEntry::new(value));
    }

    pub async fn get_odrs_reviews(&self, app_id: &str) -> Option<Vec<OdrsReview>> {
        let map = self.odrs_reviews.read().await;
        match map.get(app_id) {
            Some(entry) if !entry.is_expired(ODRS_TTL) => {
                eprintln!("[kiosque] CACHE HIT odrs_reviews \"{}\" ({} reviews)", app_id, entry.value.len());
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    pub async fn put_odrs_reviews(&self, app_id: String, value: Vec<OdrsReview>) {
        eprintln!("[kiosque] CACHE STORE odrs_reviews \"{}\" ({} reviews)", app_id, value.len());
        let mut map = self.odrs_reviews.write().await;
        map.insert(app_id, CacheEntry::new(value));
    }

    // ── Stats cache ─────────────────────────────────────────────────────

    pub async fn get_app_stats(&self, app_id: &str) -> Option<AppStats> {
        let map = self.app_stats.read().await;
        match map.get(app_id) {
            Some(entry) if !entry.is_expired(STATS_TTL) => {
                eprintln!("[kiosque] CACHE HIT app_stats \"{}\"", app_id);
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    pub async fn put_app_stats(&self, app_id: String, value: AppStats) {
        eprintln!("[kiosque] CACHE STORE app_stats \"{}\"", app_id);
        let mut map = self.app_stats.write().await;
        map.insert(app_id, CacheEntry::new(value));
    }

    pub async fn get_global_stats(&self) -> Option<GlobalStats> {
        let guard = self.global_stats.read().await;
        match guard.as_ref() {
            Some(entry) if !entry.is_expired(STATS_TTL) => {
                eprintln!("[kiosque] CACHE HIT global_stats");
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    pub async fn put_global_stats(&self, value: GlobalStats) {
        eprintln!("[kiosque] CACHE STORE global_stats");
        let mut guard = self.global_stats.write().await;
        *guard = Some(CacheEntry::new(value));
    }

    // ── Exceptions cache ────────────────────────────────────────────────

    pub async fn get_exceptions(&self, app_id: &str) -> Option<LinterExceptions> {
        let map = self.exceptions.read().await;
        match map.get(app_id) {
            Some(entry) if !entry.is_expired(EXCEPTIONS_TTL) => {
                eprintln!("[kiosque] CACHE HIT exceptions \"{}\"", app_id);
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    pub async fn put_exceptions(&self, app_id: String, value: LinterExceptions) {
        eprintln!("[kiosque] CACHE STORE exceptions \"{}\"", app_id);
        let mut map = self.exceptions.write().await;
        map.insert(app_id, CacheEntry::new(value));
    }


    // ── Installed list cache ────────────────────────────────────────────

    /// Try to get the cached installed apps list.
    pub async fn get_installed_list(&self) -> Option<Vec<FlatpakJsonApp>> {
        let guard = self.installed_list.read().await;
        match guard.as_ref() {
            Some(entry) if !entry.is_expired(INSTALLED_TTL) => {
                eprintln!("[kiosque] CACHE HIT installed_list ({} apps)", entry.value.len());
                Some(entry.value.clone())
            }
            _ => None,
        }
    }

    /// Store the installed apps list and rebuild the installed set.
    pub async fn put_installed_list(&self, apps: Vec<FlatpakJsonApp>) {
        eprintln!("[kiosque] CACHE STORE installed_list ({} apps)", apps.len());

        // Build the ID set from the list
        let id_set: HashSet<String> = apps.iter()
            .map(|app| app.application_id.clone())
            .collect();

        {
            let mut guard = self.installed_list.write().await;
            *guard = Some(CacheEntry::new(apps));
        }
        {
            let mut guard = self.installed_set.write().await;
            *guard = Some(CacheEntry::new(id_set));
        }
    }

    /// Check whether a specific app ID is in the cached installed set.
    /// Returns `None` if the cache is empty or expired (caller should
    /// fall back to the CLI check).
    pub async fn is_installed(&self, app_id: &str) -> Option<bool> {
        let guard = self.installed_set.read().await;
        match guard.as_ref() {
            Some(entry) if !entry.is_expired(INSTALLED_TTL) => {
                let result = entry.value.contains(app_id);
                eprintln!("[kiosque] CACHE HIT is_installed(\"{}\") = {}", app_id, result);
                Some(result)
            }
            _ => None,
        }
    }

    /// Invalidate the installed list and set caches. Called after
    /// install/uninstall operations to force a fresh fetch on next access.
    pub async fn invalidate_installed(&self) {
        eprintln!("[kiosque] CACHE INVALIDATE installed_list + installed_set");
        {
            let mut guard = self.installed_list.write().await;
            *guard = None;
        }
        {
            let mut guard = self.installed_set.write().await;
            *guard = None;
        }
    }
}

// ── Singleton accessor ──────────────────────────────────────────────────────

/// Returns a reference to the global application cache.
pub fn app_cache() -> &'static AppCache {
    static CACHE: OnceLock<AppCache> = OnceLock::new();
    CACHE.get_or_init(AppCache::new)
}
