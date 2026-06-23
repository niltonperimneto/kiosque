use serde::{Deserialize, Serialize};
use super::error::FlathubError;
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

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

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct OdrsReview {
    pub app_id: String,
    
    #[serde(default)]
    pub date_created: f64,
    
    #[serde(default)]
    pub description: String,
    
    #[serde(default)]
    pub distro: Option<String>,
    
    #[serde(default)]
    pub karma_down: i32,
    
    #[serde(default)]
    pub karma_up: i32,
    
    pub locale: String,
    pub rating: i32,
    
    #[serde(default)]
    pub reported: i32,
    
    pub review_id: i64,
    
    #[serde(default)]
    pub summary: String,
    
    #[serde(default)]
    pub user_display: Option<String>,
    
    #[serde(default)]
    pub user_hash: Option<String>,
    
    #[serde(default)]
    pub version: Option<String>,
}

/// Generates a cryptographically secure, pseudonymous user hash.
/// Uses HMAC-SHA256 keyed by the local random salt. If logged in via social auth,
/// it hashes the provider and user identity. Otherwise, it hashes a local machine ID fallback.
/// The resulting hash is formatted to a 40-character hex string to fit ODRS user_hash expectations.
pub fn get_user_hash() -> String {
    let settings = crate::settings::load_settings();
    let salt = settings.odrs_salt.as_bytes();
    
    let identity = if let (Some(provider), Some(user_id)) = (&settings.oauth_provider, &settings.oauth_user_id) {
        format!("{}:{}", provider, user_id)
    } else {
        let machine_id = std::fs::read_to_string("/etc/machine-id")
            .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
            .unwrap_or_else(|_| "kiosque-default-fallback-id".to_string());
        format!("machine:{}", machine_id.trim())
    };

    let mut mac = HmacSha256::new_from_slice(salt)
        .expect("HMAC can take key of any size");
    mac.update(identity.as_bytes());
    let result = mac.finalize();
    let code_bytes = result.into_bytes();
    
    let hex_str: String = code_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    hex_str[..40].to_string()
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
    pub async fn fetch_ratings(&self, app_id: &str) -> Result<OdrsRatings, FlathubError> {
        let cache = crate::cache::app_cache();

        if let Some(cached) = cache.get_odrs_ratings(app_id).await {
            return Ok(cached);
        }

        let url = format!("https://odrs.gnome.org/1.0/reviews/api/ratings/{}", app_id);
        
        let body = match crate::flathub::client::fetch_text(&url).await {
            Ok(b) => b,
            Err(e) => {
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

    /// Submit a rating and review to ODRS.
    pub async fn submit_review(
        &self,
        app_id: &str,
        rating: i32, // Star rating, e.g. 1 to 5
        summary: &str,
        description: &str,
        version: &str,
        distro: &str,
        locale: &str,
        is_anonymous: bool,
    ) -> Result<(), FlathubError> {
        let settings = crate::settings::load_settings();
        let user_hash = get_user_hash();
        let user_display = if is_anonymous {
            "Anonymous".to_string()
        } else {
            settings.oauth_username.clone().unwrap_or_else(|| "Kiosque User".to_string())
        };
        
        let payload = serde_json::json!({
            "app_id": app_id,
            "locale": locale,
            "summary": summary,
            "description": description,
            "user_hash": user_hash,
            "version": version,
            "distro": distro,
            "rating": rating * 20, // Convert star rating in [1, 5] to percent [20, 100]
            "user_display": user_display
        });

        let url = "https://odrs.gnome.org/1.0/reviews/api/submit";
        let client = super::client::shared_client();
        let resp = client.post(url).json(&payload).send().await?;
        
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: body[..body.len().min(200)].to_string(),
            });
        }
        Ok(())
    }

    /// Upvote a specific review by ID.
    pub async fn upvote_review(&self, review_id: i64) -> Result<(), FlathubError> {
        let user_hash = get_user_hash();
        let payload = serde_json::json!({
            "review_id": review_id,
            "user_hash": user_hash
        });

        let url = "https://odrs.gnome.org/1.0/reviews/api/upvote";
        let client = super::client::shared_client();
        let resp = client.post(url).json(&payload).send().await?;
        
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: body[..body.len().min(200)].to_string(),
            });
        }
        Ok(())
    }

    /// Downvote a specific review by ID.
    pub async fn downvote_review(&self, review_id: i64) -> Result<(), FlathubError> {
        let user_hash = get_user_hash();
        let payload = serde_json::json!({
            "review_id": review_id,
            "user_hash": user_hash
        });

        let url = "https://odrs.gnome.org/1.0/reviews/api/downvote";
        let client = super::client::shared_client();
        let resp = client.post(url).json(&payload).send().await?;
        
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: body[..body.len().min(200)].to_string(),
            });
        }
        Ok(())
    }

    /// Report/dismiss a specific review.
    pub async fn dismiss_review(&self, review_id: i64) -> Result<(), FlathubError> {
        let user_hash = get_user_hash();
        let payload = serde_json::json!({
            "review_id": review_id,
            "user_hash": user_hash
        });

        let url = "https://odrs.gnome.org/1.0/reviews/api/dismiss";
        let client = super::client::shared_client();
        let resp = client.post(url).json(&payload).send().await?;
        
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: body[..body.len().min(200)].to_string(),
            });
        }
        Ok(())
    }

    /// Remove a review previously submitted by the user.
    pub async fn remove_review(&self, review_id: i64) -> Result<(), FlathubError> {
        let user_hash = get_user_hash();
        let payload = serde_json::json!({
            "review_id": review_id,
            "user_hash": user_hash
        });

        let url = "https://odrs.gnome.org/1.0/reviews/api/remove";
        let client = super::client::shared_client();
        let resp = client.post(url).json(&payload).send().await?;
        
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: body[..body.len().min(200)].to_string(),
            });
        }
        Ok(())
    }
}
