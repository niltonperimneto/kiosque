use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;
use moka::future::Cache;

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
const INSTALLED_TTL: Duration = Duration::from_secs(5 * 60); // 5 minutes

// ── Accessor macro ──────────────────────────────────────────────────────────

/// Generates the standard `get_*` / `put_*` pair for a string-keyed cache field
/// whose log lines are simply `CACHE HIT <label> "<key>"` / `CACHE STORE <label>
/// "<key>"`. Fields with count-bearing or unit-keyed log lines are written by
/// hand below.
macro_rules! string_keyed_accessors {
    ($field:ident, $ty:ty, $label:literal, $get:ident, $put:ident) => {
        pub async fn $get(&self, key: &str) -> Option<$ty> {
            if let Some(value) = self.$field.get(key).await {
                klog!(concat!("CACHE HIT ", $label, " \"{}\""), key);
                Some(value)
            } else {
                None
            }
        }

        pub async fn $put(&self, key: String, value: $ty) {
            klog!(concat!("CACHE STORE ", $label, " \"{}\""), key);
            self.$field.insert(key, value).await;
        }
    };
}

// ── Main application cache ──────────────────────────────────────────────────

/// Thread-safe, TTL-based in-memory cache for Flathub API responses and
/// local Flatpak state. Leverages Moka for concurrent LFU eviction and lock-free reads.
pub struct AppCache {
    collections: Cache<String, Vec<FlathubApp>>,
    details: Cache<String, AppDetails>,
    odrs_ratings: Cache<String, OdrsRatings>,
    odrs_reviews: Cache<String, Vec<OdrsReview>>,
    installed_list: Cache<(), Vec<FlatpakJsonApp>>,
    installed_set: Cache<(), HashSet<String>>,
    app_stats: Cache<String, AppStats>,
    global_stats: Cache<(), GlobalStats>,
    exceptions: Cache<String, LinterExceptions>,
    summaries: Cache<String, serde_json::Value>,
}

impl AppCache {
    fn new() -> Self {
        Self {
            collections: Cache::builder().time_to_live(COLLECTION_TTL).max_capacity(100).build(),
            details: Cache::builder().time_to_live(DETAILS_TTL).max_capacity(100).build(),
            odrs_ratings: Cache::builder().time_to_live(ODRS_TTL).max_capacity(100).build(),
            odrs_reviews: Cache::builder().time_to_live(ODRS_TTL).max_capacity(100).build(),
            installed_list: Cache::builder().time_to_live(INSTALLED_TTL).max_capacity(1).build(),
            installed_set: Cache::builder().time_to_live(INSTALLED_TTL).max_capacity(1).build(),
            app_stats: Cache::builder().time_to_live(STATS_TTL).max_capacity(200).build(),
            global_stats: Cache::builder().time_to_live(STATS_TTL).max_capacity(1).build(),
            exceptions: Cache::builder().time_to_live(EXCEPTIONS_TTL).max_capacity(200).build(),
            summaries: Cache::builder().time_to_live(DETAILS_TTL).max_capacity(100).build(),
        }
    }

    // ── Collection cache ────────────────────────────────────────────────

    pub async fn get_collection(&self, key: &str) -> Option<Vec<FlathubApp>> {
        if let Some(value) = self.collections.get(key).await {
            klog!("CACHE HIT collection \"{}\"", key);
            Some(value)
        } else {
            None
        }
    }

    pub async fn put_collection(&self, key: String, value: Vec<FlathubApp>) {
        klog!("CACHE STORE collection \"{}\" ({} items)", key, value.len());
        self.collections.insert(key, value).await;
    }

    // ── Details cache ───────────────────────────────────────────────────

    string_keyed_accessors!(details, AppDetails, "details", get_details, put_details);

    // ── ODRS cache ──────────────────────────────────────────────────────

    string_keyed_accessors!(odrs_ratings, OdrsRatings, "odrs_ratings", get_odrs_ratings, put_odrs_ratings);

    pub async fn get_odrs_reviews(&self, app_id: &str) -> Option<Vec<OdrsReview>> {
        if let Some(value) = self.odrs_reviews.get(app_id).await {
            klog!("CACHE HIT odrs_reviews \"{}\" ({} reviews)", app_id, value.len());
            Some(value)
        } else {
            None
        }
    }

    pub async fn put_odrs_reviews(&self, app_id: String, value: Vec<OdrsReview>) {
        klog!("CACHE STORE odrs_reviews \"{}\" ({} reviews)", app_id, value.len());
        self.odrs_reviews.insert(app_id, value).await;
    }

    // ── Stats cache ─────────────────────────────────────────────────────

    string_keyed_accessors!(app_stats, AppStats, "app_stats", get_app_stats, put_app_stats);

    pub async fn get_global_stats(&self) -> Option<GlobalStats> {
        if let Some(value) = self.global_stats.get(&()).await {
            klog!("CACHE HIT global_stats");
            Some(value)
        } else {
            None
        }
    }

    pub async fn put_global_stats(&self, value: GlobalStats) {
        klog!("CACHE STORE global_stats");
        self.global_stats.insert((), value).await;
    }

    // ── Exceptions cache ────────────────────────────────────────────────

    string_keyed_accessors!(exceptions, LinterExceptions, "exceptions", get_exceptions, put_exceptions);

    // ── Summary cache ───────────────────────────────────────────────────

    string_keyed_accessors!(summaries, serde_json::Value, "summaries", get_summary, put_summary);

    // ── Installed list cache ────────────────────────────────────────────

    pub async fn get_installed_list(&self) -> Option<Vec<FlatpakJsonApp>> {
        if let Some(value) = self.installed_list.get(&()).await {
            klog!("CACHE HIT installed_list ({} apps)", value.len());
            Some(value)
        } else {
            None
        }
    }

    pub async fn put_installed_list(&self, apps: Vec<FlatpakJsonApp>) {
        klog!("CACHE STORE installed_list ({} apps)", apps.len());
        let id_set: HashSet<String> = apps.iter()
            .map(|app| app.application_id.clone())
            .collect();

        self.installed_list.insert((), apps).await;
        self.installed_set.insert((), id_set).await;
    }

    pub async fn is_installed(&self, app_id: &str) -> Option<bool> {
        if let Some(set) = self.installed_set.get(&()).await {
            let result = set.contains(app_id);
            klog!("CACHE HIT is_installed(\"{}\") = {}", app_id, result);
            Some(result)
        } else {
            None
        }
    }

    pub async fn invalidate_installed(&self) {
        klog!("CACHE INVALIDATE installed_list + installed_set");
        self.installed_list.invalidate(&()).await;
        self.installed_set.invalidate(&()).await;
        crate::disk_cache::invalidate_installed_cache().await;
    }

    pub async fn clear(&self) {
        klog!("CACHE CLEAR: Invalidate all moka cache entries");
        self.collections.invalidate_all();
        self.details.invalidate_all();
        self.odrs_ratings.invalidate_all();
        self.odrs_reviews.invalidate_all();
        self.installed_list.invalidate_all();
        self.installed_set.invalidate_all();
        self.app_stats.invalidate_all();
        self.global_stats.invalidate_all();
        self.exceptions.invalidate_all();
        self.summaries.invalidate_all();
    }
}

// ── Singleton accessor ──────────────────────────────────────────────────────

pub fn app_cache() -> &'static AppCache {
    static CACHE: OnceLock<AppCache> = OnceLock::new();
    CACHE.get_or_init(AppCache::new)
}
