use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Flathub sends `null` for absent lists and strings (e.g. `"branding": null`);
/// `#[serde(default)]` only covers a missing key, so treat `null` the same way.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

// ── Flathub API v2 collection response ──────────────────────────────────────
// The /api/v2/collection/* endpoints return this wrapper.
// Fields we don't use are captured by `flatten` + ignored via `deny_unknown_fields`
// being absent, so extra keys from the API won't break us.

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlathubResponse {
    #[serde(default, deserialize_with = "null_as_default")]
    pub hits: Vec<FlathubApp>,
}

/// Represents a single app entry from Flathub collection/search endpoints.
/// Every field except `app_id` and `name` is optional because different
/// endpoints populate different subsets, and even within the same endpoint
/// individual apps may omit fields.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlathubApp {
    /// Human-readable display name.
    #[serde(default, deserialize_with = "null_as_default")]
    pub name: String,

    /// Reverse-DNS application identifier (e.g. "org.mozilla.firefox").
    #[serde(default, deserialize_with = "null_as_default")]
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
    #[serde(default, deserialize_with = "null_as_default")]
    pub sub_categories: Vec<String>,

    /// Unix timestamp when the app was added to Flathub (collection endpoints).
    #[serde(default)]
    pub added_at: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScreenshotSize {
    #[serde(default, deserialize_with = "null_as_default")]
    pub src: String,
    #[serde(default)]
    pub width: Option<serde_json::Value>,
    #[serde(default)]
    pub height: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Screenshot {
    #[serde(default, deserialize_with = "null_as_default")]
    pub sizes: Vec<ScreenshotSize>,
}

impl Screenshot {
    /// Pick the highest-resolution size URL, falling back to the first
    /// available size. `width` arrives from the API as either a JSON string
    /// or a number, so both forms are parsed.
    pub fn best_url(&self) -> Option<String> {
        self.sizes
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
            .or_else(|| self.sizes.first().map(|size| size.src.clone()))
    }
}

/// One entry of the appstream `branding` array, e.g.
/// `{"scheme_preference": "light", "type": "primary", "value": "#d2b0ff"}`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BrandingColor {
    #[serde(default, rename = "type")]
    pub color_type: Option<String>,
    #[serde(default)]
    pub scheme_preference: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
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
    #[serde(default, deserialize_with = "null_as_default")]
    pub id: String,

    #[serde(default, deserialize_with = "null_as_default")]
    pub name: String,

    #[serde(default)]
    pub developer_name: Option<String>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub summary: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub icon: Option<String>,

    #[serde(default)]
    pub project_license: Option<String>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub screenshots: Vec<Screenshot>,

    #[serde(default)]
    pub urls: Option<AppUrls>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub categories: Vec<String>,

    #[serde(default)]
    pub metadata: Option<AppMetadata>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub branding: Vec<BrandingColor>,

    /// AppStream releases; entries that cannot be parsed are skipped.
    #[serde(default, deserialize_with = "lenient_releases")]
    pub releases: Vec<Release>,
}

/// One AppStream `<release>` entry.
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
pub struct Release {
    #[serde(default, deserialize_with = "null_as_default")]
    pub version: String,
    /// Unix time in seconds; Flathub sends it as a string or a number.
    #[serde(default, deserialize_with = "lenient_timestamp")]
    pub timestamp: Option<i64>,
    /// Release notes as AppStream markup.
    #[serde(default)]
    pub description: Option<String>,
}

fn lenient_timestamp<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            Some(serde_json::Value::Number(n)) => n.as_i64(),
            Some(serde_json::Value::String(s)) => s.trim().parse().ok(),
            _ => None,
        },
    )
}

fn lenient_releases<'de, D>(deserializer: D) -> Result<Vec<Release>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

impl AppDetails {
    /// The newest release with a version: the latest timestamp, or the first
    /// listed entry (AppStream lists newest first) when none has a timestamp.
    pub fn latest_release(&self) -> Option<&Release> {
        let versioned = || self.releases.iter().filter(|r| !r.version.is_empty());
        versioned()
            .filter(|r| r.timestamp.is_some())
            .max_by_key(|r| r.timestamp)
            .or_else(|| versioned().next())
    }

    /// Primary branding colors as (light, dark) hex values.
    pub fn brand_colors(&self) -> (Option<String>, Option<String>) {
        let pick = |scheme: &str| {
            self.branding
                .iter()
                .find(|b| {
                    b.color_type.as_deref().unwrap_or("primary") == "primary"
                        && b.scheme_preference.as_deref() == Some(scheme)
                })
                .and_then(|b| b.value.clone())
        };
        (pick("light"), pick("dark"))
    }

    /// Highest-resolution URL of the first screenshot, if any.
    pub fn best_screenshot_url(&self) -> Option<String> {
        self.screenshots.first().and_then(|s| s.best_url())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn details(json: &str) -> AppDetails {
        serde_json::from_str(json).expect("details parse")
    }

    #[test]
    fn releases_are_optional() {
        assert!(details(r#"{"id":"a"}"#).releases.is_empty());
        assert!(details(r#"{"id":"a","releases":null}"#).releases.is_empty());
        assert!(details(r#"{"id":"a"}"#).latest_release().is_none());
    }

    #[test]
    fn latest_release_uses_timestamps_in_either_encoding() {
        let d = details(
            r#"{"id":"a","releases":[
                {"version":"1.0","timestamp":"1600000000","description":"<p>Old</p>"},
                {"version":"2.0","timestamp":1700000000,"description":"<p>New</p>"},
                {"version":"0.9"}
            ]}"#,
        );
        assert_eq!(d.releases.len(), 3);
        let latest = d.latest_release().unwrap();
        assert_eq!(latest.version, "2.0");
        assert_eq!(latest.timestamp, Some(1_700_000_000));
    }

    #[test]
    fn malformed_releases_do_not_break_details() {
        let d = details(
            r#"{"id":"a","name":"App","releases":[
                {"version":3,"timestamp":"soon"},
                {"version":"1.2","timestamp":"not-a-number","description":null},
                "garbage"
            ]}"#,
        );
        assert_eq!(d.name, "App");
        assert_eq!(d.releases.len(), 1);
        let latest = d.latest_release().unwrap();
        assert_eq!(latest.version, "1.2");
        assert_eq!(latest.timestamp, None);
    }
}
