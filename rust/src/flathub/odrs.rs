use serde::{Deserialize, Serialize};
use super::error::FlathubError;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct OdrsRatings {
    #[serde(default)]
    pub star0: i32,
    #[serde(default)]
    pub star1: i32,
    #[serde(default)]
    pub star2: i32,
    #[serde(default)]
    pub star3: i32,
    #[serde(default)]
    pub star4: i32,
    #[serde(default)]
    pub star5: i32,
    #[serde(default)]
    pub total: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OdrsReview {
    pub app_id: String,
    pub date_created: f64,
    pub description: String,
    pub distro: String,
    pub karma_down: i32,
    pub karma_up: i32,
    pub locale: String,
    pub rating: i32,
    pub reported: i32,
    pub review_id: i64,
    pub summary: String,
    pub user_display: String,
    pub user_hash: String,
    pub version: String,
}

pub struct OdrsClient;

impl Default for OdrsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OdrsClient {
    pub fn new() -> Self {
        Self
    }

    /// Fetch aggregate ratings for an app from ODRS.
    /// Example URL: https://odrs.gnome.org/1.0/reviews/api/ratings/{app_id}
    pub async fn fetch_ratings(&self, app_id: &str) -> Result<OdrsRatings, FlathubError> {
        let cache = crate::cache::app_cache();

        if let Some(cached) = cache.get_odrs_ratings(app_id).await {
            return Ok(cached);
        }

        let url = format!("https://odrs.gnome.org/1.0/reviews/api/ratings/{}", app_id);
        
        let body = match crate::flathub::client::fetch_text(&url).await {
            Ok(b) => b,
            Err(e) => {
                // Not all apps have ratings. Return default if 404.
                if matches!(e, FlathubError::NotFound) {
                    let default_ratings = OdrsRatings::default();
                    cache.put_odrs_ratings(app_id.to_string(), default_ratings.clone()).await;
                    return Ok(default_ratings);
                }
                return Err(e);
            }
        };

        match serde_json::from_str::<OdrsRatings>(&body) {
            Ok(ratings) => {
                cache.put_odrs_ratings(app_id.to_string(), ratings.clone()).await;
                Ok(ratings)
            }
            Err(e) => {
                eprintln!("[kiosque] ERROR fetch_ratings(\"{}\"): serde parse failed: {}", app_id, e);
                Err(FlathubError::Parse(e))
            }
        }
    }

    /// Fetch list of individual reviews for an app from ODRS.
    /// Example URL: https://odrs.gnome.org/1.0/reviews/api/app/{app_id}
    pub async fn fetch_reviews(&self, app_id: &str) -> Result<Vec<OdrsReview>, FlathubError> {
        let cache = crate::cache::app_cache();

        if let Some(cached) = cache.get_odrs_reviews(app_id).await {
            return Ok(cached);
        }

        let url = format!("https://odrs.gnome.org/1.0/reviews/api/app/{}", app_id);
        
        let body = match crate::flathub::client::fetch_text(&url).await {
            Ok(b) => b,
            Err(e) => {
                if matches!(e, FlathubError::NotFound) {
                    cache.put_odrs_reviews(app_id.to_string(), vec![]).await;
                    return Ok(vec![]);
                }
                return Err(e);
            }
        };

        match serde_json::from_str::<Vec<OdrsReview>>(&body) {
            Ok(reviews) => {
                cache.put_odrs_reviews(app_id.to_string(), reviews.clone()).await;
                Ok(reviews)
            }
            Err(e) => {
                eprintln!("[kiosque] ERROR fetch_reviews(\"{}\"): serde parse failed: {}", app_id, e);
                Err(FlathubError::Parse(e))
            }
        }
    }
}
