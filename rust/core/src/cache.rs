use moka::future::Cache;
use std::sync::OnceLock;
use std::time::Duration;

use crate::flathub::odrs::{OdrsRatings, OdrsReview};
use crate::flathub::types::{AppDetails, AppStats, FlathubApp, GlobalStats, LinterExceptions};

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

/// Thread-safe, TTL-based in-memory cache for Flathub API responses. (Local
/// Flatpak state lives in [`crate::registry`], which is event driven.) Leverages Moka for concurrent LFU eviction and lock-free reads.
#[derive(Clone)]
pub struct AppCache {
    collections: Cache<String, Vec<FlathubApp>>,
    details: Cache<String, AppDetails>,
    odrs_ratings: Cache<String, OdrsRatings>,
    odrs_reviews: Cache<String, Vec<OdrsReview>>,
    app_stats: Cache<String, AppStats>,
    global_stats: Cache<(), GlobalStats>,
    exceptions: Cache<String, LinterExceptions>,
    summaries: Cache<String, serde_json::Value>,
}

impl AppCache {
    pub fn new() -> Self {
        Self::with_ttl(None)
    }

    /// Override TTLs for independently configured service instances.
    pub fn with_ttl(ttl: Option<Duration>) -> Self {
        Self {
            collections: Cache::builder()
                .time_to_live(ttl.unwrap_or(COLLECTION_TTL))
                .max_capacity(100)
                .build(),
            details: Cache::builder()
                .time_to_live(ttl.unwrap_or(DETAILS_TTL))
                .max_capacity(100)
                .build(),
            odrs_ratings: Cache::builder()
                .time_to_live(ttl.unwrap_or(ODRS_TTL))
                .max_capacity(100)
                .build(),
            odrs_reviews: Cache::builder()
                .time_to_live(ttl.unwrap_or(ODRS_TTL))
                .max_capacity(100)
                .build(),
            app_stats: Cache::builder()
                .time_to_live(ttl.unwrap_or(STATS_TTL))
                .max_capacity(200)
                .build(),
            global_stats: Cache::builder()
                .time_to_live(ttl.unwrap_or(STATS_TTL))
                .max_capacity(1)
                .build(),
            exceptions: Cache::builder()
                .time_to_live(ttl.unwrap_or(EXCEPTIONS_TTL))
                .max_capacity(200)
                .build(),
            summaries: Cache::builder()
                .time_to_live(ttl.unwrap_or(DETAILS_TTL))
                .max_capacity(100)
                .build(),
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

    string_keyed_accessors!(
        odrs_ratings,
        OdrsRatings,
        "odrs_ratings",
        get_odrs_ratings,
        put_odrs_ratings
    );

    pub async fn get_odrs_reviews(&self, app_id: &str) -> Option<Vec<OdrsReview>> {
        if let Some(value) = self.odrs_reviews.get(app_id).await {
            klog!(
                "CACHE HIT odrs_reviews \"{}\" ({} reviews)",
                app_id,
                value.len()
            );
            Some(value)
        } else {
            None
        }
    }

    pub async fn put_odrs_reviews(&self, app_id: String, value: Vec<OdrsReview>) {
        klog!(
            "CACHE STORE odrs_reviews \"{}\" ({} reviews)",
            app_id,
            value.len()
        );
        self.odrs_reviews.insert(app_id, value).await;
    }

    // ── Stats cache ─────────────────────────────────────────────────────

    string_keyed_accessors!(
        app_stats,
        AppStats,
        "app_stats",
        get_app_stats,
        put_app_stats
    );

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

    string_keyed_accessors!(
        exceptions,
        LinterExceptions,
        "exceptions",
        get_exceptions,
        put_exceptions
    );

    // ── Summary cache ───────────────────────────────────────────────────

    string_keyed_accessors!(
        summaries,
        serde_json::Value,
        "summaries",
        get_summary,
        put_summary
    );

    pub async fn clear(&self) {
        klog!("CACHE CLEAR: Invalidate all moka cache entries");
        self.collections.invalidate_all();
        self.details.invalidate_all();
        self.odrs_ratings.invalidate_all();
        self.odrs_reviews.invalidate_all();
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

impl Default for AppCache {
    fn default() -> Self {
        Self::new()
    }
}
