//! Typed data for one app's detail page. Qt-free.

use kiosque_core::entries::AppEntry;
use kiosque_core::flathub::client::FlathubClient;
use kiosque_core::flathub::odrs::{OdrsClient, OdrsRatings, OdrsReview};
use kiosque_core::flathub::types::{AppDetails, FlathubApp};

use super::app_list::entry;

/// Everything the detail page shows except ratings and reviews, which load
/// separately so a slow ODRS never delays the page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppDetailsData {
    pub app_id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon_url: String,
    pub developer: String,
    pub license: String,
    /// Largest available size of each screenshot, in order.
    pub screenshots: Vec<String>,
    /// Flatpak `[Context]`-style permissions (`shared`, `sockets`, …).
    pub permissions: serde_json::Value,
    /// Link type (`homepage`, `bugtracker`, …, `manifest`) to URL.
    pub links: serde_json::Value,
    pub developer_apps: Vec<AppEntry>,
    pub similar_apps: Vec<AppEntry>,
}

impl AppDetailsData {
    pub fn from_parts(
        details: AppDetails,
        summary_json: serde_json::Value,
        developer_apps: Vec<FlathubApp>,
        similar_apps: Vec<FlathubApp>,
    ) -> Self {
        let permissions = summary_json
            .get("metadata")
            .and_then(|m| m.get("permissions"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        Self {
            screenshots: screenshot_urls(&details),
            links: urls_object(&details),
            app_id: details.id,
            name: details.name,
            summary: details.summary,
            description: details.description.unwrap_or_default(),
            icon_url: details.icon.unwrap_or_default(),
            developer: details.developer_name.unwrap_or_default(),
            license: details.project_license.unwrap_or_default(),
            permissions,
            developer_apps: developer_apps.into_iter().map(entry).collect(),
            similar_apps: similar_apps.into_iter().map(entry).collect(),
        }
    }
}

/// Load the details of `app_id` with its supplementary lists.
pub async fn load(client: &FlathubClient, app_id: &str) -> Result<AppDetailsData, String> {
    let details = client
        .fetch_details(app_id)
        .await
        .map_err(|e| e.to_string())?;
    let (summary, developer_apps, similar_apps) =
        fetch_supplementary(client, &details, app_id).await;
    let mut data = AppDetailsData::from_parts(details, summary, developer_apps, similar_apps);
    if data.app_id.is_empty() {
        data.app_id = app_id.to_string();
    }
    Ok(data)
}

/// Star histogram and the derived average.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct RatingSummary {
    /// `stars[n]` = number of n-star ratings (ODRS reports 0…5).
    pub stars: [i32; 6],
    pub total: i32,
    pub average: f64,
}

impl From<OdrsRatings> for RatingSummary {
    fn from(r: OdrsRatings) -> Self {
        let stars = [r.star0, r.star1, r.star2, r.star3, r.star4, r.star5];
        let rated: i32 = stars[1..].iter().sum();
        let weighted: i32 = stars.iter().enumerate().map(|(n, c)| n as i32 * c).sum();
        Self {
            stars,
            total: r.total.max(rated),
            average: if rated > 0 {
                weighted as f64 / rated as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct ReviewEntry {
    pub review_id: i64,
    /// Empty for anonymous reviews; the view supplies a translated label.
    pub user_name: String,
    pub user_hash: String,
    /// Unix seconds.
    pub date: i64,
    /// ODRS scale, 0…100 (20 per star).
    pub rating: i32,
    pub summary: String,
    pub description: String,
    pub version: String,
    pub karma_up: i32,
    pub karma_down: i32,
}

impl From<OdrsReview> for ReviewEntry {
    fn from(r: OdrsReview) -> Self {
        Self {
            review_id: r.review_id,
            user_name: r.user_display.unwrap_or_default(),
            user_hash: r.user_hash.unwrap_or_default(),
            date: r.date_created as i64,
            rating: r.rating.clamp(0, 100),
            summary: r.summary,
            description: r.description,
            version: r.version.unwrap_or_default(),
            karma_up: r.karma_up,
            karma_down: r.karma_down,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReviewsData {
    pub ratings: RatingSummary,
    pub reviews: Vec<ReviewEntry>,
}

impl ReviewsData {
    /// `{stars: [n0…n5], total, average}` for QML.
    pub fn ratings_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.ratings).unwrap_or_default()
    }

    /// Review objects with [`ReviewEntry`]'s field names, newest first.
    pub fn reviews_value(&self) -> serde_json::Value {
        let mut reviews = self.reviews.clone();
        reviews.sort_by(|a, b| b.date.cmp(&a.date));
        serde_json::to_value(reviews).unwrap_or_default()
    }
}

/// Ratings and reviews; failures degrade to empty data.
pub async fn load_reviews(odrs: &OdrsClient, app_id: &str) -> ReviewsData {
    let (ratings, reviews) = tokio::join!(odrs.fetch_ratings(app_id), odrs.fetch_reviews(app_id));
    ReviewsData {
        ratings: ratings.map(RatingSummary::from).unwrap_or_default(),
        reviews: reviews
            .map(|r| r.into_iter().map(ReviewEntry::from).collect())
            .unwrap_or_default(),
    }
}

/// Concurrently fetch the supplementary detail lists for an app: its sandbox
/// permissions (from the summary endpoint), other apps by the same developer,
/// and similar apps from its primary category. Failures degrade to empty values
/// rather than aborting the whole detail load. The current app is removed from
/// the similar list and both app lists are capped at 8 entries.
pub async fn fetch_supplementary(
    client: &FlathubClient,
    details: &AppDetails,
    app_id: &str,
) -> (serde_json::Value, Vec<FlathubApp>, Vec<FlathubApp>) {
    let dev_name = details.developer_name.clone();
    let category = details.categories.first().cloned();

    let perm_fut = async {
        client.fetch_summary(app_id).await.unwrap_or_else(|e| {
            kerr!("load_app_details: fetch_summary failed: {}", e);
            serde_json::json!({})
        })
    };
    let dev_fut = async {
        match dev_name {
            Some(ref dev) => client.fetch_developer_apps(dev).await.unwrap_or_else(|e| {
                kerr!("load_app_details: fetch_developer_apps failed: {}", e);
                vec![]
            }),
            None => vec![],
        }
    };
    let cat_fut = async {
        match category {
            Some(ref cat) => client.fetch_category(cat).await.unwrap_or_else(|e| {
                kerr!("load_app_details: fetch_category failed: {}", e);
                vec![]
            }),
            None => vec![],
        }
    };

    let (permissions_val, mut developer_apps, mut similar_apps) =
        tokio::join!(perm_fut, dev_fut, cat_fut);

    similar_apps.retain(|app| app.app_id != app_id);
    developer_apps.truncate(8);
    similar_apps.truncate(8);

    (permissions_val, developer_apps, similar_apps)
}

/// Pick the highest-resolution screenshot URL for each screenshot entry,
/// falling back to the first available size.
pub fn screenshot_urls(details: &AppDetails) -> Vec<String> {
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

/// Collect the app's external URLs (homepage, bugtracker, donation, …) plus the
/// build manifest into a JSON object keyed by link type.
pub fn urls_object(details: &AppDetails) -> serde_json::Value {
    let mut urls_map = serde_json::Map::new();
    if let Some(ref u) = details.urls {
        if let Some(ref h) = u.homepage {
            urls_map.insert("homepage".to_string(), serde_json::Value::String(h.clone()));
        }
        if let Some(ref b) = u.bugtracker {
            urls_map.insert(
                "bugtracker".to_string(),
                serde_json::Value::String(b.clone()),
            );
        }
        if let Some(ref d) = u.donation {
            urls_map.insert("donation".to_string(), serde_json::Value::String(d.clone()));
        }
        if let Some(ref hp) = u.help {
            urls_map.insert("help".to_string(), serde_json::Value::String(hp.clone()));
        }
        if let Some(ref v) = u.vcs_browser {
            urls_map.insert(
                "vcs_browser".to_string(),
                serde_json::Value::String(v.clone()),
            );
        }
    }
    if let Some(ref m) = details.metadata
        && let Some(ref manifest) = m.manifest
    {
        urls_map.insert(
            "manifest".to_string(),
            serde_json::Value::String(manifest.clone()),
        );
    }
    serde_json::Value::Object(urls_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details() -> AppDetails {
        serde_json::from_value(serde_json::json!({
            "id": "org.gimp.GIMP",
            "name": "GIMP",
            "summary": "Image editor",
            "icon": "https://dl.flathub.org/icons/gimp.png",
            "developer_name": "The GIMP team",
            "project_license": "GPL-3.0-or-later",
            "screenshots": [
                {"sizes": [{"src": "https://x/s-small.png", "width": "624"},
                           {"src": "https://x/s-big.png", "width": 1248}]},
                {"sizes": [{"src": "https://x/t.png"}]}
            ],
            "urls": {"homepage": "https://gimp.org", "bugtracker": "https://gitlab.gnome.org/GNOME/gimp/issues"},
            "metadata": {"flathub::manifest": "https://github.com/flathub/org.gimp.GIMP"}
        }))
        .unwrap()
    }

    #[test]
    fn details_are_typed() {
        let summary = serde_json::json!({"metadata": {"permissions": {"shared": ["network"]}}});
        let similar = FlathubApp {
            app_id: "org.kde.krita".into(),
            name: "Krita".into(),
            ..Default::default()
        };
        let d = AppDetailsData::from_parts(details(), summary, vec![], vec![similar]);
        assert_eq!(d.name, "GIMP");
        assert_eq!(d.description, "");
        assert_eq!(d.screenshots, ["https://x/s-big.png", "https://x/t.png"]);
        assert_eq!(d.permissions["shared"][0], "network");
        assert_eq!(d.links["homepage"], "https://gimp.org");
        assert_eq!(
            d.links["manifest"],
            "https://github.com/flathub/org.gimp.GIMP"
        );
        assert_eq!(d.similar_apps[0].app_id, "org.kde.krita");
        let no_perms = AppDetailsData::from_parts(details(), serde_json::json!({}), vec![], vec![]);
        assert_eq!(no_perms.permissions, serde_json::json!({}));
    }

    #[test]
    fn ratings_and_reviews() {
        let r = RatingSummary::from(OdrsRatings {
            star1: 1,
            star5: 3,
            total: 4,
            ..Default::default()
        });
        assert_eq!(r.total, 4);
        assert!((r.average - 4.0).abs() < 1e-9);
        assert_eq!(RatingSummary::from(OdrsRatings::default()).average, 0.0);
        let review = ReviewEntry::from(OdrsReview {
            rating: 80,
            review_id: 9,
            date_created: 1_700_000_000.0,
            ..Default::default()
        });
        assert_eq!(
            (review.rating, review.review_id, review.date),
            (80, 9, 1_700_000_000)
        );
        assert_eq!(review.user_name, "");
        let data = ReviewsData {
            ratings: RatingSummary::default(),
            reviews: vec![
                ReviewEntry { date: 1, ..Default::default() },
                ReviewEntry { date: 2, review_id: 7, ..Default::default() },
            ],
        };
        assert_eq!(data.reviews_value()[0]["review_id"], 7);
        assert_eq!(data.ratings_value()["stars"].as_array().unwrap().len(), 6);
    }
}
