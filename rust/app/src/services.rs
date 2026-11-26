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
    /// User and system installation directories (AppStream data lives there).
    pub installations: Vec<(kiosque_core::flatpak::Scope, std::path::PathBuf)>,
    local_transactions: Option<Arc<Transactions>>,
    #[cfg(feature = "test-util")]
    fixture_backend: Option<Arc<kiosque_core::flatpak::mock::MockBackend>>,
}

impl Services {
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
        images: Arc::new(kiosque_core::sanitized_image::ImageLoader::new(
            kiosque_core::sanitized_image::default_dir(),
        )),
        settings: SettingsStore::new(kiosque_core::settings::settings_path()),
        installations: kiosque_core::flatpak::installation_paths(),
        local_transactions: None,
        #[cfg(feature = "test-util")]
        fixture_backend: None,
    })
}

/// Called only by dedicated harnesses, before any QObject is constructed.
#[cfg(feature = "test-util")]
pub fn install_test_services(endpoint: &str, root: &std::path::Path) -> Result<(), String> {
    use kiosque_core::flatpak::{InstalledApp, RemoteInfo, Scope, mock::MockBackend};
    use kiosque_core::{
        cache::AppCache, flathub::client::HttpClient, transactions::TransactionManager,
    };
    if !endpoint.starts_with("http://127.0.0.1:") {
        return Err("fixture endpoint must be loopback".into());
    }
    if SERVICES.get().is_some() {
        return Err("services already initialized".into());
    }
    let backend = Arc::new(MockBackend::with_installed(vec![InstalledApp {
        id: "org.kiosque.Fixture".into(),
        name: "Fixture".into(),
        installed_size: 565_400_000,
        ..Default::default()
    }]));
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
    std::fs::create_dir_all(&appstream).map_err(|e| e.to_string())?;
    std::fs::write(appstream.join("appstream.xml"), FIXTURE_APPSTREAM)
        .map_err(|e| e.to_string())?;
    backend.remotes.lock().unwrap().push(RemoteInfo {
        name: "fixture-repo".into(),
        title: "Fixture Repository".into(),
        url: "https://repo.example.org/".into(),
        scope: Scope::User,
        ..Default::default()
    });
    backend.metadata.lock().unwrap().insert(
        "org.example.Atlas".into(),
        "[Application]\nname=org.example.Atlas\nruntime=org.gnome.Platform/x86_64/48\n\n[Context]\nshared=network;\nsockets=wayland;\n"
            .into(),
    );
    kiosque_core::runtime::runtime().block_on(kiosque_core::catalog::reload_with(
        &*backend,
        installations.clone(),
    ));
    if kiosque_core::catalog::catalog().is_empty() {
        return Err("fixture catalog is empty".into());
    }
    let registry = Arc::new(Registry::new());
    let manager = TransactionManager::new(backend.clone(), registry.clone());
    SERVICES
        .set(Services {
            http: HttpClient::for_fixture(),
            backend: backend.clone(),
            registry,
            flathub: FlathubClient::with_dependencies(
                HttpClient::for_fixture(),
                endpoint,
                AppCache::new(),
            ),
            odrs: OdrsClient::with_dependencies(
                HttpClient::for_fixture(),
                endpoint,
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
            installations,
            local_transactions: Some(Arc::new(Transactions::Local(manager))),
            fixture_backend: Some(backend),
        })
        .map_err(|_| "services already initialized")?;
    Ok(())
}

#[cfg(feature = "test-util")]
const FIXTURE_APPSTREAM: &str = include_str!("../../../tests/fixtures/appstream/fixture-repo.xml");

#[cfg(feature = "test-util")]
fn fixture_backend() -> Option<&'static kiosque_core::flatpak::mock::MockBackend> {
    SERVICES.get()?.fixture_backend.as_deref()
}

/// Test harness only: the mock backend behind the isolated services.
#[cfg(feature = "test-util")]
pub(crate) fn test_backend() -> Option<&'static Arc<kiosque_core::flatpak::mock::MockBackend>> {
    SERVICES.get()?.fixture_backend.as_ref()
}

#[cfg(feature = "test-util")]
pub(crate) fn set_test_updates(ids: &[String]) -> bool {
    let Some(backend) = fixture_backend() else {
        return false;
    };
    *backend.updates.lock().unwrap() = ids
        .iter()
        .filter(|id| !id.is_empty())
        .map(|id| kiosque_core::flatpak::UpdateInfo {
            id: id.clone(),
            name: id.clone(),
            ref_string: format!("app/{id}/x86_64/stable"),
            ..Default::default()
        })
        .collect();
    true
}

/// Test harness only: while held, mock transactions start (and are counted)
/// but do not finish until released or cancelled.
#[cfg(feature = "test-util")]
pub(crate) fn hold_test_operations(hold: bool) {
    let Some(backend) = fixture_backend() else {
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
pub(crate) fn test_operation_count() -> u64 {
    fixture_backend().map_or(0, |backend| backend.ran.lock().unwrap().len() as u64)
}

/// Test harness only: the remote of the most recent mock install, or an
/// empty string.
#[cfg(feature = "test-util")]
pub(crate) fn last_test_install_remote() -> String {
    use kiosque_core::flatpak::Operation;
    fixture_backend()
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
        .unwrap_or_default()
}

#[cfg(feature = "test-util")]
pub fn shutdown_test_services() {
    if let Some(services) = SERVICES.get()
        && let Some(client) = &services.local_transactions
        && let Transactions::Local(manager) = client.as_ref()
    {
        kiosque_core::runtime::runtime().block_on(manager.shutdown());
    }
}

/// Start production composition; fixtures never initialize host services.
pub fn init() {
    #[cfg(feature = "test-util")]
    if fixture_backend().is_some() {
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
    // The Flatpak build keeps itself current through the Flatpak portal,
    // with or without the host companion.
    crate::runtime::runtime().spawn(kiosque_core::update_status::keep_kiosque_current());
}
