//! Detail data and review actions for AppDetails.

use crate::flathub::odrs::{OdrsRatings, OdrsReview};

pub use kiosque_core::apps::DetailData;

/// Load the detail-page data of `app_id` from the catalog and Flathub.
pub async fn load(app_id: &str) -> Result<DetailData, String> {
    let services = crate::services::services();
    let catalog = kiosque_core::catalog::catalog();
    let config = kiosque_core::config::config();
    kiosque_core::apps::details(services.sources(&catalog, &config), app_id).await
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
        && crate::view::ops::reload_installed().await.is_ok()
        && let Some(v) = crate::view::ops::installed_version(&app_id)
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
