use super::types::*;
use super::error::FlathubError;
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

/// Returns a shared, long-lived HTTP client.
/// Reusing one `Client` lets reqwest maintain a connection pool and avoid
/// the overhead of TLS negotiation on every request.
pub(crate) fn shared_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(4)
            .user_agent("Kiosque/0.1 (Linux; Flatpak storefront)")
            .build()
            .expect("Failed to build reqwest client")
    })
}

/// Fetch a URL, check status, return the body text.
pub(crate) async fn fetch_text(url: &str) -> Result<String, FlathubError> {
    let resp = shared_client().get(url).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        if status.as_u16() == 404 {
            return Err(FlathubError::NotFound);
        }
        return Err(FlathubError::Http {
            status: status.as_u16(),
            message: body[..body.len().min(200)].to_string(),
        });
    }
    Ok(body)
}

/// Parse a JSON string into `FlathubResponse`, logging exact error on failure.
fn parse_collection(body: &str, label: &str) -> Result<Vec<FlathubApp>, FlathubError> {
    match serde_json::from_str::<FlathubResponse>(body) {
        Ok(data) => {
            eprintln!("[kiosque] {}: received {} apps", label, data.hits.len());
            Ok(data.hits)
        }
        Err(e) => {
            eprintln!("[kiosque] ERROR {}: serde parse failed: {}", label, e);
            eprintln!("[kiosque]   body preview: {}", &body[..body.len().min(300)]);
            Err(FlathubError::Parse(e))
        }
    }
}

pub struct FlathubClient;

impl Default for FlathubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl FlathubClient {
    pub fn new() -> Self {
        Self
    }

    // ── Cached collection helper ────────────────────────────────────────

    /// Fetch a collection endpoint, using the in-memory cache when possible.
    /// `cache_key` identifies the endpoint (e.g. "popular", "category/Game").
    async fn fetch_collection_cached(
        &self,
        cache_key: &str,
        url: &str,
        label: &str,
    ) -> Result<Vec<FlathubApp>, FlathubError> {
        let cache = crate::cache::app_cache();

        // Try cache first
        if let Some(cached) = cache.get_collection(cache_key).await {
            return Ok(cached);
        }

        // Cache miss — fetch from network
        eprintln!("[kiosque] CACHE MISS collection \"{}\"", cache_key);
        let body = fetch_text(url).await?;
        let apps = parse_collection(&body, label)?;

        // Store in cache
        cache.put_collection(cache_key.to_string(), apps.clone()).await;
        Ok(apps)
    }

    // ── Collection endpoints ────────────────────────────────────────────

    pub async fn fetch_popular(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "popular",
            "https://flathub.org/api/v2/collection/popular",
            "fetch_popular",
        ).await
    }

    pub async fn fetch_recently_added(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "recently-added",
            "https://flathub.org/api/v2/collection/recently-added",
            "fetch_recently_added",
        ).await
    }

    pub async fn fetch_recently_updated(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "recently-updated",
            "https://flathub.org/api/v2/collection/recently-updated",
            "fetch_recently_updated",
        ).await
    }

    pub async fn fetch_trending(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "trending",
            "https://flathub.org/api/v2/collection/trending",
            "fetch_trending",
        ).await
    }

    pub async fn fetch_category(&self, category_id: &str) -> Result<Vec<FlathubApp>, FlathubError> {
        let cache_key = format!("category/{}", category_id);
        let url = format!("https://flathub.org/api/v2/collection/category/{}", category_id);
        self.fetch_collection_cached(
            &cache_key,
            &url,
            &format!("fetch_category({})", category_id),
        ).await
    }

    // ── Search (NOT cached — queries are too varied) ────────────────────

    pub async fn search(&self, query: &str) -> Result<Vec<FlathubApp>, FlathubError> {
        let resp = shared_client()
            .post("https://flathub.org/api/v2/search")
            .json(&serde_json::json!({
                "query": query,
            }))
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            if status.as_u16() == 404 {
                return Err(FlathubError::NotFound);
            }
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: body[..body.len().min(200)].to_string(),
            });
        }

        parse_collection(&body, &format!("search(\"{}\")", query))
    }

    // ── App detail (cached) ─────────────────────────────────────────────

    pub async fn fetch_details(&self, app_id: &str) -> Result<AppDetails, FlathubError> {
        let cache = crate::cache::app_cache();

        // Try cache first
        if let Some(cached) = cache.get_details(app_id).await {
            return Ok(cached);
        }

        // Cache miss — fetch from network
        eprintln!("[kiosque] CACHE MISS details \"{}\"", app_id);
        let url = format!("https://flathub.org/api/v2/appstream/{}", app_id);
        let body = fetch_text(&url).await?;

        match serde_json::from_str::<AppDetails>(&body) {
            Ok(details) => {
                eprintln!("[kiosque] fetch_details(\"{}\"): OK — \"{}\"", app_id, details.name);
                cache.put_details(app_id.to_string(), details.clone()).await;
                Ok(details)
            }
            Err(e) => {
                eprintln!("[kiosque] ERROR fetch_details(\"{}\"): serde parse failed: {}", app_id, e);
                eprintln!("[kiosque]   body preview: {}", &body[..body.len().min(300)]);
                Err(FlathubError::Parse(e))
            }
        }
    }

    pub async fn fetch_summary(&self, app_id: &str) -> Result<serde_json::Value, FlathubError> {
        let cache = crate::cache::app_cache();
        if let Some(cached) = cache.get_summary(app_id).await {
            return Ok(cached);
        }

        let url = format!("https://flathub.org/api/v2/summary/{}", app_id);
        let body = fetch_text(&url).await?;
        let val = serde_json::from_str::<serde_json::Value>(&body)?;
        cache.put_summary(app_id.to_string(), val.clone()).await;
        Ok(val)
    }

    pub async fn fetch_developer_apps(&self, developer: &str) -> Result<Vec<FlathubApp>, FlathubError> {
        let cache_key = format!("developer/{}", developer);
        let encoded_dev = url_encode(developer);
        let url = format!("https://flathub.org/api/v2/collection/developer/{}", encoded_dev);
        self.fetch_collection_cached(
            &cache_key,
            &url,
            &format!("fetch_developer_apps({})", developer),
        ).await
    }

    // ── App statistics (cached) ─────────────────────────────────────────

    pub async fn fetch_app_stats(&self, app_id: &str) -> Result<AppStats, FlathubError> {
        let cache = crate::cache::app_cache();
        if let Some(cached) = cache.get_app_stats(app_id).await {
            return Ok(cached);
        }

        let url = format!("https://flathub.org/api/v2/stats/{}", app_id);
        let body = fetch_text(&url).await?;
        let stats = serde_json::from_str::<AppStats>(&body)?;
        cache.put_app_stats(app_id.to_string(), stats.clone()).await;
        Ok(stats)
    }

    pub async fn fetch_global_stats(&self) -> Result<GlobalStats, FlathubError> {
        let cache = crate::cache::app_cache();
        if let Some(cached) = cache.get_global_stats().await {
            return Ok(cached);
        }

        let url = "https://flathub.org/api/v2/stats/";
        let body = fetch_text(url).await?;
        let stats = serde_json::from_str::<GlobalStats>(&body)?;
        cache.put_global_stats(stats.clone()).await;
        Ok(stats)
    }

    // ── Linter exceptions (cached) ──────────────────────────────────────

    pub async fn fetch_exceptions(&self, app_id: &str) -> Result<LinterExceptions, FlathubError> {
        let cache = crate::cache::app_cache();
        if let Some(cached) = cache.get_exceptions(app_id).await {
            return Ok(cached);
        }

        let url = format!("https://flathub.org/api/v2/exceptions/{}", app_id);
        let body = fetch_text(&url).await?;
        let exceptions = serde_json::from_str::<LinterExceptions>(&body)?;
        cache.put_exceptions(app_id.to_string(), exceptions.clone()).await;
        Ok(exceptions)
    }
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}
