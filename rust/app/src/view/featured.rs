//! Featured apps: Flathub's app of the day plus a per-distribution
//! selection, falling back to trending apps. Qt-free.

use kiosque_core::entries::FeaturedEntry;
use kiosque_core::flathub::client::FlathubClient;

/// At most this many featured apps are shown.
pub const MAX_FEATURED: usize = 5;

/// Fetch the featured list for `distro_id` on `day` (`YYYY-MM-DD`).
pub async fn fetch(client: &FlathubClient, distro_id: &str, day: &str) -> Vec<FeaturedEntry> {
    let mut ids: Vec<String> = vec![];
    if let Ok(body) = client
        .fetch_text(&format!("app-picks/app-of-the-day/{day}"))
        .await
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
        && let Some(id) = json.get("app_id").and_then(|v| v.as_str())
    {
        ids.push(id.to_string());
    }
    for id in curated_apps(distro_id) {
        if !ids.iter().any(|i| i == id) {
            ids.push(id.to_string());
        }
    }
    let fetches = ids.iter().map(|id| client.fetch_details(id));
    let mut items: Vec<FeaturedEntry> = futures_util::future::join_all(fetches)
        .await
        .into_iter()
        .filter_map(Result::ok)
        .map(|d| FeaturedEntry {
            description: d.description.clone().unwrap_or_else(|| d.summary.clone()),
            name: d.name,
            summary: d.summary,
            icon_url: d.icon.unwrap_or_default(),
            app_id: d.id,
            developer: d.developer_name.unwrap_or_default(),
        })
        .collect();
    if items.is_empty() {
        kerr!("featured: curated list unavailable, falling back to trending apps");
        if let Ok(apps) = client.fetch_trending().await {
            items = apps
                .into_iter()
                .map(|app| FeaturedEntry {
                    description: app
                        .description
                        .clone()
                        .unwrap_or_else(|| app.summary.clone().unwrap_or_default()),
                    name: app.name,
                    summary: app.summary.unwrap_or_default(),
                    icon_url: app.icon.unwrap_or_default(),
                    app_id: app.app_id,
                    developer: app.developer_name.unwrap_or_default(),
                })
                .collect();
        }
    }
    items.truncate(MAX_FEATURED);
    items
}

pub fn curated_apps(distro_id: &str) -> Vec<&'static str> {
    match distro_id {
        "fedora" => vec![
            "org.mozilla.firefox",
            "org.gimp.GIMP",
            "org.videolan.VLC",
            "org.blender.Blender",
            "org.inkscape.Inkscape",
        ],
        "ubuntu" | "pop" | "mint" => vec![
            "org.mozilla.firefox",
            "org.telegram.desktop",
            "org.videolan.VLC",
            "org.gimp.GIMP",
            "com.spotify.Client",
        ],
        "neon" | "arch" | "opensuse" | "manjaro" => vec![
            "org.kde.krita",
            "org.kde.kdenlive",
            "org.kde.kate",
            "org.kde.kstars",
            "org.mozilla.firefox",
        ],
        _ => vec![
            "org.mozilla.firefox",
            "org.kde.krita",
            "org.videolan.VLC",
            "org.gimp.GIMP",
            "org.blender.Blender",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_lists_are_short_and_unique() {
        for distro in ["fedora", "ubuntu", "arch", "generic", "unknown"] {
            let apps = curated_apps(distro);
            assert!(!apps.is_empty() && apps.len() <= MAX_FEATURED, "{distro}");
            let mut sorted = apps.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), apps.len(), "{distro}");
        }
    }
}
