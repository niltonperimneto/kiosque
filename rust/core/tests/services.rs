use kiosque_core::{
    cache::AppCache,
    disk_cache::DiskCache,
    flathub::{
        client::{FlathubClient, HttpClient},
        error::FlathubError,
        odrs::OdrsClient,
    },
    settings::{AppSettings, SettingsStore},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const COLLECTION: &str = include_str!("../../../tests/fixtures/http/collection.json");
struct Server {
    url: String,
    hits: Arc<AtomicUsize>,
    requests: Arc<std::sync::Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn new(status: u16, body: String, delay: Duration) -> Self {
        Self::new_bytes(status, body.into_bytes(), delay).await
    }
    async fn new_bytes(status: u16, body: Vec<u8>, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 8192];
                let len = stream.read(&mut request).await.unwrap();
                recorded.lock().unwrap().push(
                    String::from_utf8_lossy(&request[..len])
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string(),
                );
                count.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(delay).await;
                let response = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            }
        });
        Self {
            url,
            hits,
            requests,
            task,
        }
    }
    fn client(&self, cache: AppCache) -> FlathubClient {
        FlathubClient::with_dependencies(
            HttpClient::new(reqwest::Client::builder().no_proxy().build().unwrap()),
            &self.url,
            cache,
        )
    }
}

#[tokio::test]
async fn collections_cache_and_invalidate_without_live_network() {
    let server = Server::new(200, COLLECTION.into(), Duration::ZERO).await;
    let cache = AppCache::new();
    let client = server.client(cache.clone());
    assert_eq!(
        client.fetch_popular().await.unwrap()[0].app_id,
        "org.kiosque.Fixture"
    );
    client.fetch_popular().await.unwrap();
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    cache.clear().await;
    client.fetch_popular().await.unwrap();
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn zero_ttl_expires_and_instances_do_not_share_state() {
    let server = Server::new(200, COLLECTION.into(), Duration::ZERO).await;
    let client = server.client(AppCache::with_ttl(Some(Duration::ZERO)));
    client.fetch_popular().await.unwrap();
    client.fetch_popular().await.unwrap();
    server
        .client(AppCache::new())
        .fetch_popular()
        .await
        .unwrap();
    assert_eq!(server.hits.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn malformed_and_multibyte_error_bodies_do_not_panic() {
    let malformed = Server::new(200, "é".repeat(350), Duration::ZERO).await;
    assert!(matches!(
        malformed.client(AppCache::new()).fetch_popular().await,
        Err(FlathubError::Parse(_))
    ));
    let unavailable = Server::new(503, "é".repeat(350), Duration::ZERO).await;
    assert!(matches!(
        unavailable.client(AppCache::new()).search("fixture").await,
        Err(FlathubError::Http { status: 503, .. })
    ));
}

#[tokio::test]
async fn odrs_missing_reviews_are_empty_and_cached() {
    let server = Server::new(404, "{}".into(), Duration::ZERO).await;
    let client = OdrsClient::with_dependencies(
        HttpClient::new(reqwest::Client::builder().no_proxy().build().unwrap()),
        &server.url,
        AppCache::new(),
    );
    assert!(
        client
            .fetch_reviews("org.kiosque.Fixture")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        client
            .fetch_reviews("org.kiosque.Fixture")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        client
            .fetch_ratings("org.kiosque.Fixture")
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn timeout_returns_transport_error() {
    let server = Server::new(200, COLLECTION.into(), Duration::from_secs(30)).await;
    let http = HttpClient::new(
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(30))
            .build()
            .unwrap(),
    );
    let client = FlathubClient::with_dependencies(http, &server.url, AppCache::new());
    assert!(client.fetch_popular().await.is_err());
}

#[tokio::test]
async fn disk_storage_is_independent_and_rejects_corruption() {
    let root = tempfile::tempdir().unwrap();
    let cache = DiskCache::new(root.path());
    cache.save_at("fixture", 1, 42, &vec!["é"]).await.unwrap();
    let (timestamp, value): (u64, Vec<String>) = cache.load("fixture", 1).await.unwrap();
    assert_eq!(timestamp, 42);
    assert_eq!(value, vec!["é"]);
    assert!(cache.load::<Vec<String>>("fixture", 2).await.is_none());
    std::fs::write(root.path().join("fixture.bin"), b"broken").unwrap();
    assert!(cache.load::<Vec<String>>("fixture", 1).await.is_none());
    assert!(cache.save_at("../escape", 1, 42, &value).await.is_err());
}

#[test]
fn settings_roundtrip_and_salt_migration_do_not_invoke_desktop_services() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config/settings.json");
    let store = SettingsStore::new(&path);
    let settings = AppSettings {
        auto_update: true,
        update_frequency: "Weekly".into(),
        ..Default::default()
    };
    store.save(&settings).unwrap();
    assert!(store.load().auto_update);
    assert_eq!(store.load().update_frequency, "Weekly");
    assert_eq!(store.load().odrs_salt, settings.odrs_salt);
    let mut old = settings;
    old.odrs_salt.clear();
    store.save(&old).unwrap();
    let migrated = store.load().odrs_salt;
    assert_eq!(migrated.len(), 64);
    assert_eq!(store.load().odrs_salt, migrated);
    assert!(!root.path().join("systemd").exists());
}

#[tokio::test]
async fn concurrent_disk_writers_leave_a_complete_record() {
    let root = tempfile::tempdir().unwrap();
    let cache = DiskCache::new(root.path());
    let first = vec![1u32; 4096];
    let second = vec![2u32; 4096];
    let (a, b) = tokio::join!(
        cache.save_at("shared", 1, 1, &first),
        cache.save_at("shared", 1, 2, &second)
    );
    a.unwrap();
    b.unwrap();
    let (timestamp, record): (u64, Vec<u32>) = cache.load("shared", 1).await.unwrap();
    assert!(timestamp == 1 || timestamp == 2);
    assert_eq!(record, if timestamp == 1 { first } else { second });
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn storefront_collections_use_injected_endpoints_and_separate_caches() {
    let server = Server::new(200, COLLECTION.into(), Duration::ZERO).await;
    let client = server.client(AppCache::new());
    client.fetch_mobile().await.unwrap();
    client
        .fetch_category_sorted("Game", "trending")
        .await
        .unwrap();
    client
        .fetch_category_sorted("Game", "installs_last_month")
        .await
        .unwrap();
    client.fetch_mobile().await.unwrap();
    client
        .fetch_category_sorted("Game", "trending")
        .await
        .unwrap();
    assert!(
        client
            .fetch_category_sorted("Game", "invalid")
            .await
            .is_err()
    );
    assert_eq!(server.hits.load(Ordering::SeqCst), 3);
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].contains("/collection/mobile "));
    assert!(requests[1].contains("/collection/category/Game?sort_by=trending&page=1&per_page=30 "));
    assert!(requests[2].contains("sort_by=installs_last_month"));
}

fn image_fixture() -> Vec<u8> {
    let frame = image::RgbaImage::from_pixel(8, 4, image::Rgba([200, 10, 20, 128]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(frame)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}
fn image_transport() -> HttpClient {
    HttpClient::new(reqwest::Client::builder().no_proxy().build().unwrap())
}
fn image_id(url: &str) -> String {
    kiosque_core::image_cache::image_url(url)
        .strip_prefix("image://kiosque/")
        .unwrap()
        .into()
}

#[tokio::test]
async fn image_requests_deduplicate_downloads_and_reuse_decoded_frames() {
    use kiosque_core::image_cache::ImageLoader;
    let server = Server::new_bytes(200, image_fixture(), Duration::from_millis(10)).await;
    let dir = tempfile::tempdir().unwrap();
    let loader = ImageLoader::new(image_transport(), dir.path());
    let id = image_id(&server.url);
    let (a, b, c) = tokio::join!(
        loader.load(&id, 4, 4),
        loader.load(&id, 4, 4),
        loader.load(&id, 2, -1)
    );
    let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.dimensions(), (4, 2));
    assert_eq!(c.dimensions(), (2, 1));
    assert_eq!(a.get_pixel(0, 0)[3], 128);
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    assert!(Arc::ptr_eq(&a, &loader.load(&id, 4, 4).await.unwrap()));
    assert_eq!(loader.load(&id, -1, 1).await.unwrap().dimensions(), (2, 1));
    assert_eq!(
        loader.load(&id, 100, 100).await.unwrap().dimensions(),
        (8, 4)
    );
}

#[tokio::test]
async fn image_loaders_are_isolated_recover_corruption_and_do_not_cache_errors() {
    use kiosque_core::image_cache::{ImageLoader, cache_path_for};
    let server = Server::new_bytes(200, image_fixture(), Duration::ZERO).await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(cache_path_for(dir.path(), &server.url), b"corrupt").unwrap();
    let loader = ImageLoader::new(image_transport(), dir.path());
    let id = image_id(&server.url);
    loader.load(&id, 2, 2).await.unwrap();
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    let other = tempfile::tempdir().unwrap();
    ImageLoader::new(image_transport(), other.path())
        .load(&id, 2, 2)
        .await
        .unwrap();
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);
    let broken = Server::new_bytes(200, b"broken".to_vec(), Duration::ZERO).await;
    let id = image_id(&broken.url);
    assert!(loader.load(&id, 2, 2).await.is_err());
    assert!(loader.load(&id, 2, 2).await.is_err());
    assert_eq!(broken.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn local_images_invalidate_on_changes_and_decoded_cache_can_expire() {
    use kiosque_core::image_cache::ImageLoader;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local image.png");
    std::fs::write(&path, image_fixture()).unwrap();
    let id = image_id(reqwest::Url::from_file_path(&path).unwrap().as_str());
    let loader = ImageLoader::new(image_transport(), dir.path().join("cache"));
    let old = loader.load(&id, 2, 2).await.unwrap();
    let other = image::RgbaImage::from_pixel(4, 8, image::Rgba([0, 255, 0, 255]));
    other.save(&path).unwrap();
    let changed = loader.load(&id, 2, 2).await.unwrap();
    assert_eq!(changed.dimensions(), (1, 2));
    assert!(!Arc::ptr_eq(&old, &changed));
    let expired = ImageLoader::with_cache_policy(
        image_transport(),
        dir.path().join("expired"),
        96,
        Duration::ZERO,
    );
    let a = expired.load(&id, 2, 2).await.unwrap();
    let b = expired.load(&id, 2, 2).await.unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
}

#[tokio::test]
async fn featured_schema_invalidates_old_entries_without_affecting_other_caches() {
    let root = tempfile::tempdir().unwrap();
    let disk = DiskCache::new(root.path());
    let entries = vec![kiosque_core::entries::FeaturedEntry {
        name: "Daily".into(),
        is_app_of_the_day: true,
        ..Default::default()
    }];
    disk.save_at("featured", 1, 10, &entries).await.unwrap();
    assert!(
        disk.load::<Vec<kiosque_core::entries::FeaturedEntry>>("featured", 2)
            .await
            .is_none()
    );
    disk.save_at("featured", 2, 20, &entries).await.unwrap();
    assert_eq!(
        disk.load::<Vec<kiosque_core::entries::FeaturedEntry>>("featured", 2)
            .await
            .unwrap(),
        (20, entries)
    );
}

#[test]
fn branding_and_screenshot_metadata_have_safe_defaults() {
    use kiosque_core::flathub::types::AppDetails;
    let missing: AppDetails =
        serde_json::from_value(serde_json::json!({"id":"a","name":"A"})).unwrap();
    assert_eq!(missing.brand_colors(), (None, None));
    assert_eq!(missing.best_screenshot_url(), None);
    let rich: AppDetails = serde_json::from_value(serde_json::json!({"id":"a","name":"A",
        "branding":[{"type":"primary","scheme_preference":"light","value":"#123456"}],
        "screenshots":[{"sizes":[{"src":"small", "width":"100"},{"src":"large", "width":900}]}]
    }))
    .unwrap();
    assert_eq!(rich.brand_colors(), (Some("#123456".into()), None));
    assert_eq!(rich.best_screenshot_url(), Some("large".into()));
}

#[tokio::test]
async fn decoded_frame_budget_evicts_frames_and_transport_failures_remain_retryable() {
    use kiosque_core::image_cache::ImageLoader;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.png");
    std::fs::write(&path, image_fixture()).unwrap();
    let id = image_id(reqwest::Url::from_file_path(path).unwrap().as_str());
    let loader = ImageLoader::with_cache_policy(
        image_transport(),
        dir.path().join("cache"),
        0,
        Duration::from_secs(600),
    );
    let first = loader.load(&id, 4, 4).await.unwrap();
    let second = loader.load(&id, 4, 4).await.unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    let server = Server::new(503, "unavailable".into(), Duration::ZERO).await;
    let id = image_id(&server.url);
    assert!(loader.load(&id, 4, 4).await.is_err());
    assert!(loader.load(&id, 4, 4).await.is_err());
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);
}
