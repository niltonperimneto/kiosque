//! Application composition root. Test binaries install isolated instances
//! with [`install_test_services`] before any QObject exists.

use kiosque_core::{
    disk_cache::DiskCache,
    flathub::{client::FlathubClient, odrs::OdrsClient},
    flatpak::FlatpakBackend,
    image_cache::ImageLoader,
    registry::Registry,
    settings::SettingsStore,
    transactions::Transactions,
};
use std::sync::{Arc, OnceLock};

pub struct Services {
    pub backend: Arc<dyn FlatpakBackend>,
    pub registry: Arc<Registry>,
    pub flathub: FlathubClient,
    pub odrs: OdrsClient,
    pub disk: DiskCache,
    pub settings: SettingsStore,
    pub images: Arc<ImageLoader>,
    local_transactions: Option<Arc<Transactions>>,
}

impl Services {
    pub fn install_target(&self, id: &str) -> (String, kiosque_core::flatpak::Scope) {
        if self.local_transactions.is_some() {
            ("fixture".into(), kiosque_core::flatpak::Scope::User)
        } else {
            kiosque_core::transactions::install_target(id, "")
        }
    }

    pub async fn transactions(&self) -> Arc<Transactions> {
        match &self.local_transactions {
            Some(local) => local.clone(),
            None => kiosque_core::transactions::client().await,
        }
    }

    pub fn is_test(&self) -> bool {
        self.local_transactions.is_some()
    }
}

static SERVICES: OnceLock<Services> = OnceLock::new();

pub fn services() -> &'static Services {
    SERVICES.get_or_init(|| Services {
        backend: kiosque_core::flatpak::backend(),
        registry: kiosque_core::registry::registry(),
        flathub: FlathubClient::new(),
        odrs: OdrsClient::new(),
        disk: DiskCache::new(kiosque_core::disk_cache::cache_dir()),
        settings: SettingsStore::new(kiosque_core::settings::settings_path()),
        images: Arc::new(ImageLoader::new(kiosque_core::image_cache::default_dir())),
        local_transactions: None,
    })
}

/// Start the GUI's background services: the installed-state registry (and
/// its installation monitor), the transaction client and the AppStream
/// catalog. Called once before QML is loaded.
pub fn init() {
    let s = services();
    if s.is_test() {
        return;
    }
    let _ = kiosque_core::config::config();
    // Creating the production loader also drops the pre-sanitizing cache.
    let _ = kiosque_core::image_cache::loader();
    kiosque_core::runtime::runtime().spawn(async {
        let _ = services().transactions().await;
    });
    kiosque_core::runtime::runtime().spawn(async {
        kiosque_core::catalog::loaded().await;
        kiosque_core::catalog::refresh_if_stale().await;
    });
}

/// Install isolated services for test harnesses: a mock backend with one
/// installed fixture app, HTTP clients pointed at a loopback fixture server
/// and storage below `root`. Must run before any QObject is created.
#[cfg(feature = "test-util")]
pub fn install_test_services(endpoint: &str, root: &std::path::Path) -> Result<(), String> {
    use kiosque_core::flatpak::{InstalledApp, UpdateInfo, mock::MockBackend};
    use kiosque_core::{
        cache::AppCache, flathub::client::HttpClient, transactions::TransactionManager,
    };
    if !endpoint.starts_with("http://127.0.0.1:") {
        return Err(format!("fixture endpoint must be loopback, got {endpoint:?}"));
    }
    let backend = Arc::new(MockBackend::with_installed(vec![InstalledApp {
        id: "org.kiosque.Fixture".into(),
        name: "Fixture".into(),
        version: "1.0".into(),
        ..Default::default()
    }]));
    // One pending update, so selection and progress paths are exercised.
    backend.updates.lock().unwrap().push(UpdateInfo {
        id: "org.kiosque.Fixture".into(),
        name: "Fixture".into(),
        version: "2.0".into(),
        ..Default::default()
    });
    let registry = Arc::new(Registry::new());
    let manager = TransactionManager::new(backend.clone(), registry.clone());
    SERVICES
        .set(Services {
            backend,
            registry,
            flathub: FlathubClient::with_dependencies(
                HttpClient::default(),
                endpoint,
                AppCache::new(),
            ),
            odrs: OdrsClient::with_dependencies(HttpClient::default(), endpoint, AppCache::new()),
            disk: DiskCache::new(root.join("cache")),
            settings: SettingsStore::new(root.join("settings.json")),
            images: Arc::new(
                ImageLoader::new(root.join("cache/img/v2")).allow_http_loopback(),
            ),
            local_transactions: Some(Arc::new(Transactions::Local(manager))),
        })
        .map_err(|_| "services were already initialised".to_string())
}

/// Stop the isolated transaction worker installed by [`install_test_services`].
#[cfg(feature = "test-util")]
pub fn shutdown_test_services() {
    if let Some(Transactions::Local(manager)) = SERVICES
        .get()
        .and_then(|s| s.local_transactions.as_deref())
    {
        kiosque_core::runtime::runtime().block_on(manager.shutdown());
    }
}
