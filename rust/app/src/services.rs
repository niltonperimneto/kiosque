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
    pub http: kiosque_core::flathub::client::HttpClient,
    pub backend: Arc<dyn FlatpakBackend>,
    pub registry: Arc<Registry>,
    pub flathub: FlathubClient,
    pub odrs: OdrsClient,
    pub disk: DiskCache,
    pub images: Arc<kiosque_core::sanitized_image::ImageLoader>,
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
        http: kiosque_core::flathub::client::HttpClient::default(),
        backend: kiosque_core::flatpak::backend(),
        registry: kiosque_core::registry::registry(),
        flathub: FlathubClient::new(),
        odrs: OdrsClient::new(),
        disk: DiskCache::new(kiosque_core::disk_cache::cache_dir()),
        images: Arc::new(kiosque_core::sanitized_image::ImageLoader::new(
            kiosque_core::sanitized_image::default_dir(),
        )),
        settings: SettingsStore::new(kiosque_core::settings::settings_path()),
        local_transactions: None,
    })
}

/// Called only by dedicated harnesses, before any QObject is constructed.
///
/// # Safety
/// Both pointers must reference NUL-terminated strings valid for this call.
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
        installed_size: 565_400_000,
        ..Default::default()
    }]));
    if TEST_BACKEND.set(backend.clone()).is_err() {
        return false;
    }
    let registry = Arc::new(Registry::new());
    let manager = TransactionManager::new(backend.clone(), registry.clone());
    SERVICES
        .set(Services {
            http: HttpClient::for_fixture(),
            backend,
            registry,
            flathub: FlathubClient::with_dependencies(
                HttpClient::for_fixture(),
                &endpoint,
                AppCache::new(),
            ),
            odrs: OdrsClient::with_dependencies(
                HttpClient::for_fixture(),
                &endpoint,
                AppCache::new(),
            ),
            disk: DiskCache::new(root.join("cache")),
            images: Arc::new(
                kiosque_core::sanitized_image::ImageLoader::with_fetcher(
                    root.join("images"),
                    Arc::new(|url| {
                        Box::pin(async move {
                            HttpClient::for_fixture()
                                .fetch_bytes_limited(&url, 16 * 1024 * 1024)
                                .await
                                .map_err(|e| e.to_string())
                        })
                    }),
                )
                .allow_http_loopback(),
            ),
            settings: SettingsStore::new(root.join("settings.json")),
            local_transactions: Some(Arc::new(Transactions::Local(manager))),
        })
        .is_ok()
}

#[cfg(feature = "test-util")]
static TEST_BACKEND: OnceLock<Arc<kiosque_core::flatpak::mock::MockBackend>> = OnceLock::new();

/// Test harness only: replace the mock backend's pending updates with the
/// newline-separated application ids in `ids`.
///
/// # Safety
/// `ids` must reference a NUL-terminated string valid for this call.
#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kiosque_test_set_updates(ids: *const std::ffi::c_char) -> bool {
    let Some(backend) = TEST_BACKEND.get() else {
        return false;
    };
    if ids.is_null() {
        return false;
    }
    let ids = unsafe { std::ffi::CStr::from_ptr(ids) }.to_string_lossy();
    *backend.updates.lock().unwrap() = ids
        .lines()
        .filter(|id| !id.is_empty())
        .map(|id| kiosque_core::flatpak::UpdateInfo {
            id: id.into(),
            name: id.into(),
            ref_string: format!("app/{id}/x86_64/stable"),
            ..Default::default()
        })
        .collect();
    true
}

/// Test harness only: while held, mock transactions start (and are counted)
/// but do not finish until released or cancelled.
#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_test_hold_operations(hold: bool) {
    let Some(backend) = TEST_BACKEND.get() else {
        return;
    };
    let mut gate = backend.gate.lock().unwrap();
    if hold {
        *gate = Some(Arc::new(tokio::sync::Semaphore::new(0)));
    } else if let Some(gate) = gate.take() {
        gate.add_permits(tokio::sync::Semaphore::MAX_PERMITS / 2);
    }
}

/// Test harness only: number of operations the mock backend has started.
#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_test_operation_count() -> u64 {
    TEST_BACKEND
        .get()
        .map_or(0, |backend| backend.ran.lock().unwrap().len() as u64)
}

#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_test_shutdown() {
    if let Some(services) = SERVICES.get()
        && let Some(client) = &services.local_transactions
        && let Transactions::Local(manager) = client.as_ref()
    {
        kiosque_core::runtime::runtime().block_on(manager.shutdown());
    }
}

/// Start production composition; fixtures never initialize host services.
pub fn init() {
    if TEST_MODE.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let _ = services();
    crate::runtime::runtime().spawn(async {
        let _ = services().transactions().await;
    });
    crate::runtime::runtime().spawn(async {
        crate::catalog::loaded().await;
        crate::catalog::refresh_if_stale().await;
    });
}
static TEST_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(feature = "test-util")]
pub fn install_test_services(endpoint: &str, root: &std::path::Path) -> Result<(), String> {
    if !endpoint.starts_with("http://127.0.0.1:") {
        return Err("fixture endpoint must be loopback".into());
    }
    let endpoint = std::ffi::CString::new(endpoint).map_err(|e| e.to_string())?;
    let root =
        std::ffi::CString::new(root.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    if unsafe { kiosque_test_setup(endpoint.as_ptr(), root.as_ptr()) } {
        TEST_MODE.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    } else {
        Err("services already initialized".into())
    }
}
#[cfg(feature = "test-util")]
pub fn shutdown_test_services() {
    kiosque_test_shutdown();
}
