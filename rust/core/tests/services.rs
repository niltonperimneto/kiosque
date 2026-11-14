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
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn new(status: u16, body: String, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 8192];
                let _ = stream.read(&mut request).await.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(delay).await;
                let response = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        Self { url, hits, task }
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
async fn capped_downloads_refuse_oversized_bodies() {
    let server = Server::new(200, "x".repeat(4096), Duration::ZERO).await;
    let http = HttpClient::new(reqwest::Client::builder().no_proxy().build().unwrap());
    let url = format!("{}/image.png", server.url);
    match http.fetch_bytes_capped(&url, 1024).await {
        Err(FlathubError::TooLarge { max: 1024 }) => {}
        other => panic!("expected TooLarge, got {:?}", other.map(|b| b.len())),
    }
    assert_eq!(
        http.fetch_bytes_capped(&url, 4096).await.unwrap().len(),
        4096
    );
}
