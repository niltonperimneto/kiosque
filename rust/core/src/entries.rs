//! Serializable view data; independent of Qt types.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub app_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeaturedEntry {
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub app_id: String,
    pub description: String,
    pub developer: String,
    #[serde(default)]
    pub screenshot_url: String,
    #[serde(default)]
    pub brand_color_light: String,
    #[serde(default)]
    pub brand_color_dark: String,
    #[serde(default)]
    pub is_app_of_the_day: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InstalledEntry {
    pub name: String,
    pub app_id: String,
    pub version: String,
    /// Human-readable installed size (see [`format_size`]).
    pub size: String,
    /// Installed size in bytes; used for sorting.
    pub size_bytes: u64,
    pub origin: String,
    pub has_update: bool,
    pub is_checked_for_update: bool,
    pub is_runtime: bool,
}

/// Disk-cache schema of `Vec<InstalledEntry>`. The cache is postcard, which is
/// not self-describing: bump this whenever the struct's fields change so older
/// files are ignored instead of misread. 2: added `size_bytes`.
pub const INSTALLED_CACHE_SCHEMA: u32 = 2;

/// Format a byte count with decimal (SI) units, matching the `flatpak` CLI:
/// `565400000` → `"565.4 MB"`. Zero (unknown) formats as an empty string.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["kB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return String::new();
    }
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    // Round first so 999.95 kB is promoted to "1.0 MB" rather than "1000.0 kB".
    while (value * 10.0).round() / 10.0 >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes_with_si_units() {
        assert_eq!(format_size(0), "");
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1000), "1.0 kB");
        assert_eq!(format_size(3_400_000), "3.4 MB");
        assert_eq!(format_size(565_400_000), "565.4 MB");
        assert_eq!(format_size(999_960), "1.0 MB");
        assert_eq!(format_size(1_260_000_000), "1.3 GB");
    }

    #[test]
    fn installed_cache_written_before_size_bytes_is_ignored() {
        // The layout written under schema 1, before `size_bytes` existed.
        #[derive(Serialize)]
        struct V1 {
            name: String,
            app_id: String,
            version: String,
            size: String,
            origin: String,
            has_update: bool,
            is_checked_for_update: bool,
            is_runtime: bool,
        }
        let old = vec![V1 {
            name: "A".into(),
            app_id: "org.a.A".into(),
            version: "1".into(),
            size: "1 B".into(),
            origin: "flathub".into(),
            has_update: false,
            is_checked_for_update: false,
            is_runtime: false,
        }];
        let bytes = crate::disk_cache::encode(1, 0, &old).unwrap();
        let read = crate::disk_cache::decode::<Vec<InstalledEntry>>(INSTALLED_CACHE_SCHEMA, &bytes);
        assert!(
            read.is_none(),
            "a schema-1 installed cache must not be decoded"
        );

        let current = vec![InstalledEntry {
            size_bytes: 1,
            ..Default::default()
        }];
        let bytes = crate::disk_cache::encode(INSTALLED_CACHE_SCHEMA, 0, &current).unwrap();
        let (_, read) =
            crate::disk_cache::decode::<Vec<InstalledEntry>>(INSTALLED_CACHE_SCHEMA, &bytes)
                .unwrap();
        assert_eq!(read, current);
    }
}
