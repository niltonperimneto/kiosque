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
    pub images: kiosque_core::image_cache::ImageLoader,
    pub settings: SettingsStore,
    /// User and system installation directories (AppStream data lives there).
    pub installations: Vec<(kiosque_core::flatpak::Scope, std::path::PathBuf)>,
    local_transactions: Option<Arc<Transactions>>,
}

impl Services {
    pub fn install_target(&self, id: &str) -> (String, kiosque_core::flatpak::Scope) {
        self.install_target_from(id, "")
    }

    /// The remote and scope to install `id` from, preferring `remote_hint`
    /// (the origin the detail page showed).
    pub fn install_target_from(
        &self,
        id: &str,
        remote_hint: &str,
    ) -> (String, kiosque_core::flatpak::Scope) {
        if self.local_transactions.is_some() {
            // Isolated harnesses never consult the process-wide registry.
            if remote_hint.is_empty() {
                return ("fixture".into(), kiosque_core::flatpak::Scope::User);
            }
            let scope = kiosque_core::catalog::catalog()
                .remote_scope(remote_hint)
                .unwrap_or_default();
            (remote_hint.into(), scope)
        } else {
            kiosque_core::transactions::install_target(id, remote_hint)
        }
    }

    /// The catalog, Flathub client, backend and blocklist that app listings
    /// and details are built from.
    pub fn sources<'a>(
        &'a self,
        catalog: &'a kiosque_core::catalog::Catalog,
        config: &'a kiosque_core::config::DistroConfig,
    ) -> kiosque_core::apps::Sources<'a> {
        kiosque_core::apps::Sources {
            catalog,
            flathub: &self.flathub,
            backend: &*self.backend,
            blocklist: &config.blocklist,
        }
    }

    /// Re-read every remote's AppStream data through this backend.
    pub async fn reload_catalog(&self) {
        kiosque_core::catalog::reload_with(&*self.backend, self.installations.clone()).await;
    }

    /// Download `remote`'s AppStream data, then reload the catalog.
    pub async fn refresh_catalog_remote(&self, remote: &str) {
        kiosque_core::catalog::refresh_remote_with(
            &*self.backend,
            self.installations.clone(),
            remote,
        )
        .await;
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
        images: kiosque_core::image_cache::ImageLoader::new(
            kiosque_core::flathub::client::HttpClient::default(),
            kiosque_core::disk_cache::cache_dir().join("images"),
        ),
        settings: SettingsStore::new(kiosque_core::settings::settings_path()),
        installations: kiosque_core::flatpak::installation_paths(),
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
    use kiosque_core::flatpak::{InstalledApp, RemoteInfo, Scope, mock::MockBackend};
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
    // A non-Flathub repository whose AppStream data is on disk, so catalog
    // apps are searched locally and Flathub fixtures supplement them.
    let installations = vec![
        (Scope::User, root.join("flatpak-user")),
        (Scope::System, root.join("flatpak-system")),
    ];
    let appstream = installations[0]
        .1
        .join("appstream/fixture-repo")
        .join(kiosque_core::flatpak::default_arch())
        .join("active");
    if std::fs::create_dir_all(&appstream).is_err()
        || std::fs::write(appstream.join("appstream.xml"), FIXTURE_APPSTREAM).is_err()
    {
        return false;
    }
    backend.remotes.lock().unwrap().push(RemoteInfo {
        name: "fixture-repo".into(),
        title: "Fixture Repository".into(),
        url: "https://repo.example.org/".into(),
        scope: Scope::User,
        ..Default::default()
    });
    backend.metadata.lock().unwrap().insert(
        "org.example.Atlas".into(),
        "[Application]\nname=org.example.Atlas\n\n[Context]\nshared=network;\nsockets=wayland;\n"
            .into(),
    );
    kiosque_core::runtime::runtime().block_on(kiosque_core::catalog::reload_with(
        &*backend,
        installations.clone(),
    ));
    if kiosque_core::catalog::catalog().is_empty() {
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
            images: kiosque_core::image_cache::ImageLoader::new(
                HttpClient::for_fixture(),
                root.join("images"),
            ),
            settings: SettingsStore::new(root.join("settings.json")),
            installations,
            local_transactions: Some(Arc::new(Transactions::Local(manager))),
        })
        .is_ok()
}

#[cfg(feature = "test-util")]
const FIXTURE_APPSTREAM: &str = include_str!("../../tests/fixtures/appstream/fixture-repo.xml");

#[cfg(feature = "test-util")]
static TEST_BACKEND: OnceLock<Arc<kiosque_core::flatpak::mock::MockBackend>> = OnceLock::new();

/// Test harness only: replace the mock backend's pending updates with the
/// newline-separated application ids in `ids`.
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

/// Test harness only: the remote of the most recent mock install, or an
/// empty string. The returned pointer stays valid until the next call.
#[cfg(feature = "test-util")]
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_test_last_install_remote() -> *const std::ffi::c_char {
    use kiosque_core::flatpak::Operation;
    static LAST: std::sync::Mutex<Option<std::ffi::CString>> = std::sync::Mutex::new(None);
    let remote = TEST_BACKEND
        .get()
        .and_then(|backend| {
            backend
                .ran
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(|op| match op {
                    Operation::Install { remote, .. } => Some(remote.clone()),
                    _ => None,
                })
        })
        .unwrap_or_default();
    let mut last = LAST.lock().unwrap();
    let value = last.insert(std::ffi::CString::new(remote).unwrap_or_default());
    value.as_ptr()
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
