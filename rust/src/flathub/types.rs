use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Flathub API v2 collection response ──────────────────────────────────────
// The /api/v2/collection/* endpoints return this wrapper.
// Fields we don't use are captured by `flatten` + ignored via `deny_unknown_fields`
// being absent, so extra keys from the API won't break us.

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlathubResponse {
    #[serde(default)]
    pub hits: Vec<FlathubApp>,
}

/// Represents a single app entry from Flathub collection/search endpoints.
/// Every field except `app_id` and `name` is optional because different
/// endpoints populate different subsets, and even within the same endpoint
/// individual apps may omit fields.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlathubApp {
    /// Human-readable display name.
    #[serde(default)]
    pub name: String,

    /// Reverse-DNS application identifier (e.g. "org.mozilla.firefox").
    #[serde(default)]
    pub app_id: String,

    /// One-line description shown below the name.
    #[serde(default)]
    pub summary: Option<String>,

    /// URL to the app icon (typically a PNG from dl.flathub.org).
    #[serde(default)]
    pub icon: Option<String>,

    /// Developer or publisher name.
    #[serde(default)]
    pub developer_name: Option<String>,

    /// Long description, may contain HTML markup.
    #[serde(default)]
    pub description: Option<String>,

    /// Subcategories of the app (e.g. Emulator, Utility).
    #[serde(default)]
    pub sub_categories: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScreenshotSize {
    #[serde(default)]
    pub src: String,
    #[serde(default)]
    pub width: Option<serde_json::Value>,
    #[serde(default)]
    pub height: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Screenshot {
    #[serde(default)]
    pub sizes: Vec<ScreenshotSize>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppUrls {
    pub homepage: Option<String>,
    pub bugtracker: Option<String>,
    pub donation: Option<String>,
    pub help: Option<String>,
    pub vcs_browser: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppMetadata {
    #[serde(rename = "flathub::manifest")]
    pub manifest: Option<String>,
}

// ── Flathub API v2 appstream detail response ────────────────────────────────

/// Full detail payload returned by /api/v2/appstream/{app_id}.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppDetails {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub developer_name: Option<String>,

    #[serde(default)]
    pub summary: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub icon: Option<String>,

    #[serde(default)]
    pub project_license: Option<String>,

    #[serde(default)]
    pub screenshots: Vec<Screenshot>,

    #[serde(default)]
    pub urls: Option<AppUrls>,

    #[serde(default)]
    pub categories: Vec<String>,

    #[serde(default)]
    pub metadata: Option<AppMetadata>,
}

// ── Flathub API v2 Stats and Exceptions responses ───────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AppStats {
    pub id: String,
    pub installs_total: u64,
    pub installs_last_month: u64,
    pub installs_last_7_days: u64,
    #[serde(default)]
    pub installs_per_day: HashMap<String, u64>,
    #[serde(default)]
    pub installs_per_country: HashMap<String, u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct GlobalStats {
    #[serde(default)]
    pub category_totals: HashMap<String, u64>,
    #[serde(default)]
    pub countries: HashMap<String, u64>,
    #[serde(default)]
    pub delta_downloads_per_day: HashMap<String, u64>,
    #[serde(default)]
    pub downloads_per_day: HashMap<String, u64>,
    #[serde(default)]
    pub flatpak_versions: HashMap<String, u64>,
    #[serde(default)]
    pub os_flatpak_versions: HashMap<String, HashMap<String, u64>>,
    #[serde(default)]
    pub os_versions: HashMap<String, u64>,
    #[serde(default)]
    pub totals: HashMap<String, u64>,
    #[serde(default)]
    pub updates_per_day: HashMap<String, u64>,
}

pub type LinterExceptions = HashMap<String, HashMap<String, String>>;

