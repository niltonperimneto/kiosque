//! Application composition root. Test binaries install isolated instances before QML creation.
use kiosque_core::{
    disk_cache::DiskCache,
    flathub::{client::FlathubClient, odrs::OdrsClient},
    flatpak::FlatpakBackend,
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
        local_transactions: None,
    })
}

/// Called only by dedicated harnesses, before any QObject is constructed.
#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kiosque_test_setup(
    endpoint: *const std::ffi::c_char,
    root: *const std::ffi::c_char,
) -> bool {
    use kiosque_core::flatpak::{InstalledApp, mock::MockBackend};
    use kiosque_core::{
        cache::AppCache, flathub::client::HttpClient, transactions::TransactionManager,
    };
    if endpoint.is_null() || root.is_null() {
        return false;
    }
    let endpoint = unsafe { std::ffi::CStr::from_ptr(endpoint) }.to_string_lossy();
    let root = std::path::PathBuf::from(
        unsafe { std::ffi::CStr::from_ptr(root) }
            .to_string_lossy()
            .as_ref(),
    );
    let backend = Arc::new(MockBackend::with_installed(vec![InstalledApp {
        id: "org.kiosque.Fixture".into(),
        name: "Fixture".into(),
        ..Default::default()
    }]));
    let registry = Arc::new(Registry::new());
    let manager = TransactionManager::new(backend.clone(), registry.clone());
    SERVICES
        .set(Services {
            backend,
            registry,
            flathub: FlathubClient::with_dependencies(
                HttpClient::default(),
                &endpoint,
                AppCache::new(),
            ),
            odrs: OdrsClient::with_dependencies(HttpClient::default(), &endpoint, AppCache::new()),
            disk: DiskCache::new(root.join("cache")),
            settings: SettingsStore::new(root.join("settings.json")),
            local_transactions: Some(Arc::new(Transactions::Local(manager))),
        })
        .is_ok()
}

#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_test_shutdown() {
    if let Some(services) = SERVICES.get() {
        if let Some(client) = &services.local_transactions {
            if let Transactions::Local(manager) = client.as_ref() {
                kiosque_core::runtime::runtime().block_on(manager.shutdown());
            }
        }
    }
}
