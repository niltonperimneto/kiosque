//! Qt-free loading of an app's detail-page data, shared by `AppDetails` and
//! the `StoreController` compatibility facade.

use crate::flathub::client::FlathubClient;
use crate::flathub::odrs::{OdrsRatings, OdrsReview};
use crate::flathub::types::{AppDetails, FlathubApp};

/// Everything the detail page shows apart from ratings and reviews.
pub struct DetailData {
    pub name: String,
    pub summary: String,
    pub description: String,
    /// Raw icon URL (not yet mapped through the image provider).
    pub icon: String,
    pub developer: String,
    pub license: String,
    /// Raw screenshot URLs, highest resolution first per screenshot.
    pub screenshots: Vec<String>,
    /// Link type → URL (`homepage`, `bugtracker`, `donation`, `help`,
    /// `vcs_browser`, `manifest`).
    pub urls: serde_json::Value,
    /// The `metadata.permissions` object of the summary endpoint.
    pub permissions: serde_json::Value,
    pub developer_apps: Vec<FlathubApp>,
    pub similar_apps: Vec<FlathubApp>,
}

/// Fetch details plus the supplementary lists for `app_id`.
pub async fn load(app_id: &str) -> Result<DetailData, String> {
    let client = crate::services::services().flathub.clone();
    let details = client
        .fetch_details(app_id)
        .await
        .map_err(|e| e.to_string())?;
    let (permissions_val, developer_apps, similar_apps) =
        fetch_supplementary(&client, &details, app_id).await;
    let permissions = permissions_val
        .get("metadata")
        .and_then(|m| m.get("permissions"))
        .filter(|value| value.is_object())
        .unwrap_or(&serde_json::Value::Null)
        .clone();
    Ok(DetailData {
        screenshots: screenshot_urls(&details),
        urls: urls_object(&details),
        name: details.name,
        summary: details.summary,
        description: kiosque_core::security::description_text(
            details.description.as_deref().unwrap_or_default(),
        ),
        icon: details.icon.unwrap_or_default(),
        developer: details.developer_name.unwrap_or_default(),
        license: details.project_license.unwrap_or_default(),
        permissions,
        developer_apps,
        similar_apps,
    })
}

/// Fetch ODRS ratings and reviews concurrently; failures degrade to empty.
pub async fn load_reviews(app_id: &str) -> (OdrsRatings, Vec<OdrsReview>) {
    let odrs = crate::services::services().odrs.clone();
    let (ratings, reviews) = tokio::join!(odrs.fetch_ratings(app_id), odrs.fetch_reviews(app_id));
    (
        ratings.unwrap_or_else(|e| {
            kerr!("load_reviews: ratings for {}: {}", app_id, e);
            OdrsRatings::default()
        }),
        reviews.unwrap_or_else(|e| {
            kerr!("load_reviews: reviews for {}: {}", app_id, e);
            vec![]
        }),
    )
}

/// Submit an ODRS review, auto-detecting empty version/distro/locale.
#[allow(clippy::too_many_arguments)]
pub async fn submit_review(
    app_id: String,
    rating: i32,
    summary: String,
    description: String,
    mut version: String,
    mut distro: String,
    mut locale: String,
    is_anonymous: bool,
) -> Result<(), String> {
    // Inside the sandbox /etc/os-release describes the KDE runtime;
    // `distro_id` reads the host's /run/host/os-release first.
    if distro.is_empty() {
        distro = match kiosque_core::config::distro_id() {
            id if id == "generic" => "Linux".to_string(),
            id => id,
        };
    }
    if locale.is_empty() {
        locale = std::env::var("LANG")
            .unwrap_or_else(|_| "en_US".to_string())
            .split('.')
            .next()
            .unwrap_or("en_US")
            .to_string();
    }
    if version.is_empty()
        && let Ok(installed_apps) = crate::flatpak::ui::list_installed_cached().await
        && let Some(v) = installed_apps
            .iter()
            .find(|a| a.application_id == app_id)
            .and_then(|a| a.version.clone())
    {
        version = v;
    }
    if version.is_empty() {
        version = "1.0".to_string();
    }
    crate::services::services()
        .odrs
        .submit_review(
            &app_id,
            rating,
            &summary,
            &description,
            &version,
            &distro,
            &locale,
            is_anonymous,
        )
        .await
        .map_err(|e| e.to_string())
}

/// An ODRS review moderation/voting action.
#[derive(Clone, Copy, Debug)]
pub enum ReviewAction {
    Upvote,
    Downvote,
    Dismiss,
    Remove,
}

pub async fn review_action(action: ReviewAction, review_id: i64) -> Result<(), String> {
    let odrs = crate::services::services().odrs.clone();
    let result = match action {
        ReviewAction::Upvote => odrs.upvote_review(review_id).await,
        ReviewAction::Downvote => odrs.downvote_review(review_id).await,
        ReviewAction::Dismiss => odrs.dismiss_review(review_id).await,
        ReviewAction::Remove => odrs.remove_review(review_id).await,
    };
    result.map_err(|e| e.to_string())
}

/// Concurrently fetch the supplementary detail lists for an app: its sandbox
/// permissions (from the summary endpoint), other apps by the same developer,
/// and similar apps from its primary category. Failures degrade to empty values
/// rather than aborting the whole detail load. The current app is removed from
/// the similar list and both app lists are capped at 8 entries.
async fn fetch_supplementary(
    client: &FlathubClient,
    details: &AppDetails,
    app_id: &str,
) -> (serde_json::Value, Vec<FlathubApp>, Vec<FlathubApp>) {
    let dev_name = details.developer_name.clone();
    let category = details.categories.first().cloned();

    let perm_fut = async {
        client.fetch_summary(app_id).await.unwrap_or_else(|e| {
            kerr!("load_app_details: fetch_summary failed: {}", e);
            serde_json::Value::Null
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

/// Collect the app's external URLs (homepage, bugtracker, donation, …) plus the
/// build manifest into a JSON object keyed by link type.
fn urls_object(details: &AppDetails) -> serde_json::Value {
    let mut urls_map = serde_json::Map::new();
    let mut put = |key: &str, value: &Option<String>| {
        if let Some(v) = value
            && let Ok(url) = kiosque_core::security::https_url(v)
        {
            urls_map.insert(key.to_string(), serde_json::Value::String(url.into()));
        }
    };
    if let Some(ref u) = details.urls {
        put("homepage", &u.homepage);
        put("bugtracker", &u.bugtracker);
        put("donation", &u.donation);
        put("help", &u.help);
        put("vcs_browser", &u.vcs_browser);
    }
    if let Some(ref m) = details.metadata {
        put("manifest", &m.manifest);
    }
    serde_json::Value::Object(urls_map)
}

/// Convert a list of apps into the compact `{app_id, name, summary, icon}` JSON
/// shape consumed by the legacy developer-apps and similar-apps JSON properties.
pub fn apps_to_json(apps: &[FlathubApp]) -> Vec<serde_json::Value> {
    apps.iter()
        .map(|app| {
            serde_json::json!({
                "app_id": app.app_id,
                "name": app.name,
                "summary": app.summary.clone().unwrap_or_default(),
                "icon": app.icon.clone().unwrap_or_default()
            })
        })
        .collect()
}
