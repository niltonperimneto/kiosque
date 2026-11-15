use super::error::FlathubError;
use super::types::*;
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

/// Instance-owned transport with the same timeouts as the production client.
#[derive(Clone)]
pub struct HttpClient {
    pub client: Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            client: shared_client().clone(),
        }
    }
}

impl HttpClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn fetch_text(&self, url: &str) -> Result<String, FlathubError> {
        checked_text(self.client.get(url).send().await?).await
    }

    pub async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, FlathubError> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(FlathubError::Http {
                status: status.as_u16(),
                message: String::new(),
            });
        }
        Ok(response.bytes().await?.to_vec())
    }
}

pub(crate) fn preview(body: &str, max: usize) -> String {
    body.chars().take(max).collect()
}

pub(crate) async fn checked_text(response: reqwest::Response) -> Result<String, FlathubError> {
    let status = response.status();
    let body = response.text().await?;
    if status.as_u16() == 404 {
        return Err(FlathubError::NotFound);
    }
    if !status.is_success() {
        return Err(FlathubError::Http {
            status: status.as_u16(),
            message: preview(&body, 200),
        });
    }
    Ok(body)
}

#[cfg(feature = "libflatpak")]
pub(crate) async fn fetch_bytes(url: &str) -> Result<Vec<u8>, FlathubError> {
    HttpClient::default().fetch_bytes(url).await
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
            eprintln!("[kiosque]   body preview: {}", preview(body, 300));
            Err(FlathubError::Parse(e))
        }
    }
}

#[derive(Clone)]
pub struct FlathubClient {
    http: HttpClient,
    base_url: String,
    cache: crate::cache::AppCache,
}

impl Default for FlathubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl FlathubClient {
    pub fn new() -> Self {
        Self::with_dependencies(
            HttpClient::default(),
            "https://flathub.org/api/v2",
            crate::cache::app_cache().clone(),
        )
    }

    pub fn with_dependencies(
        http: HttpClient,
        base_url: &str,
        cache: crate::cache::AppCache,
    ) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').into(),
            cache,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub async fn fetch_text(&self, path: &str) -> Result<String, FlathubError> {
        self.http.fetch_text(&self.url(path)).await
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
        let cache = &self.cache;

        // Try cache first
        if let Some(cached) = cache.get_collection(cache_key).await {
            return Ok(cached);
        }

        // Cache miss — fetch from network
        eprintln!("[kiosque] CACHE MISS collection \"{}\"", cache_key);
        let body = self.http.fetch_text(url).await?;
        let apps = parse_collection(&body, label)?;

        // Store in cache
        cache
            .put_collection(cache_key.to_string(), apps.clone())
            .await;
        Ok(apps)
    }

    // ── Collection endpoints ────────────────────────────────────────────

    pub async fn fetch_popular(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached("popular", &self.url("collection/popular"), "fetch_popular")
            .await
    }

    pub async fn fetch_recently_added(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "recently-added",
            &self.url("collection/recently-added"),
            "fetch_recently_added",
        )
        .await
    }

    pub async fn fetch_recently_updated(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "recently-updated",
            &self.url("collection/recently-updated"),
            "fetch_recently_updated",
        )
        .await
    }

    pub async fn fetch_trending(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached(
            "trending",
            &self.url("collection/trending"),
            "fetch_trending",
        )
        .await
    }

    /// Apps adapted for phones and tablets ("On the Go" on flathub.org).
    pub async fn fetch_mobile(&self) -> Result<Vec<FlathubApp>, FlathubError> {
        self.fetch_collection_cached("mobile", &self.url("collection/mobile"), "fetch_mobile")
            .await
    }

    pub async fn fetch_category(&self, category_id: &str) -> Result<Vec<FlathubApp>, FlathubError> {
        let cache_key = format!("category/{}", category_id);
        let url = self.url(&format!("collection/category/{}", category_id));
        self.fetch_collection_cached(
            &cache_key,
            &url,
            &format!("fetch_category({})", category_id),
        )
        .await
    }

    /// Category collection sorted server-side. Flathub only accepts
    /// "trending" or "installs_last_month" here; `page` is mandatory once
    /// query parameters are present.
    pub async fn fetch_category_sorted(
        &self,
        category_id: &str,
        sort_by: &str,
    ) -> Result<Vec<FlathubApp>, FlathubError> {
        if !matches!(sort_by, "trending" | "installs_last_month") {
            return Err(FlathubError::InvalidRequest(
                "unsupported category sort".into(),
            ));
        }
        let cache_key = format!("category/{}/sorted/{}", category_id, sort_by);
        let mut url = reqwest::Url::parse(&self.url("collection/category/"))
            .map_err(|e| FlathubError::InvalidRequest(e.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| FlathubError::InvalidRequest("invalid API base".into()))?
            .pop_if_empty()
            .push(category_id);
        url.query_pairs_mut()
            .append_pair("sort_by", sort_by)
            .append_pair("page", "1")
            .append_pair("per_page", "30");
        self.fetch_collection_cached(
            &cache_key,
            url.as_str(),
            &format!("fetch_category_sorted({}, {})", category_id, sort_by),
        )
        .await
    }

    // ── Search (NOT cached — queries are too varied) ────────────────────

    pub async fn search(&self, query: &str) -> Result<Vec<FlathubApp>, FlathubError> {
        let resp = self
            .http
            .client
            .post(self.url("search"))
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
                message: preview(&body, 200),
            });
        }

        parse_collection(&body, &format!("search(\"{}\")", query))
    }

    // ── App detail (cached) ─────────────────────────────────────────────

    pub async fn fetch_details(&self, app_id: &str) -> Result<AppDetails, FlathubError> {
        let cache = &self.cache;

        // Try cache first
        if let Some(cached) = cache.get_details(app_id).await {
            return Ok(cached);
        }

        // Cache miss — fetch from network
        eprintln!("[kiosque] CACHE MISS details \"{}\"", app_id);
        let url = self.url(&format!("appstream/{}", app_id));
        let body = self.http.fetch_text(&url).await?;

        match serde_json::from_str::<AppDetails>(&body) {
            Ok(details) => {
                eprintln!(
                    "[kiosque] fetch_details(\"{}\"): OK — \"{}\"",
                    app_id, details.name
                );
                cache.put_details(app_id.to_string(), details.clone()).await;
                Ok(details)
            }
            Err(e) => {
                eprintln!(
                    "[kiosque] ERROR fetch_details(\"{}\"): serde parse failed: {}",
                    app_id, e
                );
                eprintln!("[kiosque]   body preview: {}", preview(&body, 300));
                Err(FlathubError::Parse(e))
            }
        }
    }

    pub async fn fetch_summary(&self, app_id: &str) -> Result<serde_json::Value, FlathubError> {
        let cache = &self.cache;
        if let Some(cached) = cache.get_summary(app_id).await {
            return Ok(cached);
        }

        let url = self.url(&format!("summary/{}", app_id));
        let body = self.http.fetch_text(&url).await?;
        let val = serde_json::from_str::<serde_json::Value>(&body)?;
        cache.put_summary(app_id.to_string(), val.clone()).await;
        Ok(val)
    }

    pub async fn fetch_developer_apps(
        &self,
        developer: &str,
    ) -> Result<Vec<FlathubApp>, FlathubError> {
        let cache_key = format!("developer/{}", developer);
        let encoded_dev = url_encode(developer);
        let url = self.url(&format!("collection/developer/{}", encoded_dev));
        self.fetch_collection_cached(
            &cache_key,
            &url,
            &format!("fetch_developer_apps({})", developer),
        )
        .await
    }

    // ── App statistics (cached) ─────────────────────────────────────────

    pub async fn fetch_app_stats(&self, app_id: &str) -> Result<AppStats, FlathubError> {
        let cache = &self.cache;
        if let Some(cached) = cache.get_app_stats(app_id).await {
            return Ok(cached);
        }

        let url = self.url(&format!("stats/{}", app_id));
        let body = self.http.fetch_text(&url).await?;
        let stats = serde_json::from_str::<AppStats>(&body)?;
        cache.put_app_stats(app_id.to_string(), stats.clone()).await;
        Ok(stats)
    }

    pub async fn fetch_global_stats(&self) -> Result<GlobalStats, FlathubError> {
        let cache = &self.cache;
        if let Some(cached) = cache.get_global_stats().await {
            return Ok(cached);
        }

        let url = self.url("stats/");
        let body = self.http.fetch_text(&url).await?;
        let stats = serde_json::from_str::<GlobalStats>(&body)?;
        cache.put_global_stats(stats.clone()).await;
        Ok(stats)
    }

    // ── Linter exceptions (cached) ──────────────────────────────────────

    pub async fn fetch_exceptions(&self, app_id: &str) -> Result<LinterExceptions, FlathubError> {
        let cache = &self.cache;
        if let Some(cached) = cache.get_exceptions(app_id).await {
            return Ok(cached);
        }

        let url = self.url(&format!("exceptions/{}", app_id));
        let body = self.http.fetch_text(&url).await?;
        let exceptions = serde_json::from_str::<LinterExceptions>(&body)?;
        cache
            .put_exceptions(app_id.to_string(), exceptions.clone())
            .await;
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
