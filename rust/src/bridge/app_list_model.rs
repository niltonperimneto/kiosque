use cxx_qt::{CxxQtType, Threading};

use crate::flathub::types::FlathubApp;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
    }

    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(bool, loading)]
        #[qproperty(QString, error_message)]
        type AppListModel = super::AppListModelRust;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &AppListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "rowCount"]
        #[cxx_override]
        fn row_count(self: &AppListModel, parent: &QModelIndex) -> i32;

        #[cxx_name = "roleNames"]
        #[cxx_override]
        fn role_names(self: &AppListModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn refresh(self: Pin<&mut AppListModel>);

        #[qinvokable]
        #[cxx_name = "loadPopular"]
        fn load_popular(self: Pin<&mut AppListModel>);

        #[qinvokable]
        #[cxx_name = "loadNew"]
        fn load_new(self: Pin<&mut AppListModel>);

        #[qinvokable]
        #[cxx_name = "loadUpdated"]
        fn load_updated(self: Pin<&mut AppListModel>);

        #[qinvokable]
        #[cxx_name = "loadMobile"]
        fn load_mobile(self: Pin<&mut AppListModel>);

        #[qinvokable]
        #[cxx_name = "loadCategorySorted"]
        fn load_category_sorted(self: Pin<&mut AppListModel>, category: QString, sort_by: QString);

        #[qinvokable]
        fn search(self: Pin<&mut AppListModel>, query: QString);

        #[qinvokable]
        #[cxx_name = "loadCategory"]
        fn load_category(self: Pin<&mut AppListModel>, category: QString);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut AppListModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut AppListModel>);
    }

    impl cxx_qt::Threading for AppListModel {}
    impl cxx_qt::Initialize for AppListModel {}
}

pub use kiosque_core::entries::AppEntry;

/// A request answered from the local catalog (and Flathub).
#[derive(Clone, Debug)]
enum Query {
    Search(String),
    Category(String),
}

#[derive(Default)]
pub struct AppListModelRust {
    pub items: Vec<AppEntry>,
    pub loading: bool,
    /// Why the last load failed; empty after a successful load.
    pub error_message: cxx_qt_lib::QString,
    generation: u64,
    /// The last catalog-backed request, repeated when the catalog changes.
    last_query: Option<Query>,
    /// Stops the catalog subscription when the QObject is destroyed.
    watch: Option<tokio_util::sync::CancellationToken>,
}

impl Drop for AppListModelRust {
    fn drop(&mut self) {
        if let Some(watch) = &self.watch {
            watch.cancel();
        }
    }
}

impl cxx_qt::Initialize for qobject::AppListModel {
    fn initialize(mut self: std::pin::Pin<&mut Self>) {
        let cancel = tokio_util::sync::CancellationToken::new();
        self.as_mut().rust_mut().watch = Some(cancel.clone());
        let mut changes = kiosque_core::catalog::changes();
        let thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    change = changes.recv() => {
                        if matches!(change, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            break;
                        }
                        let queued = thread.queue(|mut object| {
                            // Only while no newer request is in flight.
                            if let Some(query) = object.last_query.clone()
                                && !object.loading
                            {
                                object.as_mut().start_query(query);
                            }
                        });
                        if queued.is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }
}

/// List entries for `apps`.
fn entries(apps: Vec<FlathubApp>) -> Vec<AppEntry> {
    apps.into_iter()
        .map(|app| AppEntry {
            name: app.name,
            summary: app.summary.unwrap_or_default(),
            icon_url: app.icon.unwrap_or_default(),
            app_id: app.app_id,
        })
        .collect()
}

/// Drop apps the distributor blocklist hides.
fn allowed(mut apps: Vec<FlathubApp>) -> Vec<FlathubApp> {
    let config = kiosque_core::config::config();
    apps.retain(|app| !config.blocklist.is_blocked(&app.app_id));
    apps
}

impl qobject::AppListModel {
    pub const NAME_ROLE: i32 = 256;
    pub const SUMMARY_ROLE: i32 = 257;
    pub const ICON_URL_ROLE: i32 = 258;
    pub const APP_ID_ROLE: i32 = 259;

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        if !index.is_valid() {
            return cxx_qt_lib::QVariant::default();
        }
        let row = index.row() as usize;
        if let Some(item) = self.items.get(row) {
            match role {
                Self::NAME_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.name))
                }
                Self::SUMMARY_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.summary))
                }
                Self::ICON_URL_ROLE => {
                    let qml_url = crate::image_cache::image_url(&item.icon_url);
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&qml_url))
                }
                Self::APP_ID_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.app_id))
                }
                _ => cxx_qt_lib::QVariant::default(),
            }
        } else {
            cxx_qt_lib::QVariant::default()
        }
    }

    pub fn row_count(&self, parent: &cxx_qt_lib::QModelIndex) -> i32 {
        if parent.is_valid() {
            return 0;
        }
        self.items.len() as i32
    }

    pub fn role_names(&self) -> cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray> {
        let mut roles = cxx_qt_lib::QHash::<cxx_qt_lib::QHashPair_i32_QByteArray>::default();
        roles.insert(Self::NAME_ROLE, cxx_qt_lib::QByteArray::from("name"));
        roles.insert(Self::SUMMARY_ROLE, cxx_qt_lib::QByteArray::from("summary"));
        roles.insert(Self::ICON_URL_ROLE, cxx_qt_lib::QByteArray::from("iconUrl"));
        roles.insert(Self::APP_ID_ROLE, cxx_qt_lib::QByteArray::from("appId"));
        roles
    }

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().last_query = None;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            // SWR Step 1: Try reading from disk cache first
            let mut cached_items = None;
            let mut is_expired = true;

            if let Some((timestamp, items)) = crate::disk_cache::load_popular_cache().await {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                is_expired = now.saturating_sub(timestamp) > 300; // 5 minutes TTL
                cached_items = Some(items.clone());

                // Yield cached items to Qt immediately (Instant load)
                let _ = qt_thread.queue(move |mut qobject| {
                    if qobject.generation != generation {
                        return;
                    }
                    klog!(
                        "AppListModel::refresh: Loading cached popular list ({} items)",
                        items.len()
                    );
                    qobject.as_mut().apply_items(items);
                });
            }

            // SWR Step 2: Fetch from network if cache is expired or missing
            if is_expired || cached_items.is_none() {
                let client = crate::services::services().flathub.clone();
                let mut error = String::new();
                let items = match client.fetch_popular().await.map(allowed) {
                    Ok(apps) => {
                        klog!(
                            "AppListModel::refresh: mapping {} apps to entries",
                            apps.len()
                        );
                        apps.into_iter()
                            .map(|app| AppEntry {
                                name: app.name,
                                summary: app.summary.unwrap_or_default(),
                                icon_url: app.icon.unwrap_or_default(),
                                app_id: app.app_id,
                            })
                            .collect()
                    }
                    Err(e) => {
                        kerr!("AppListModel::refresh: {}", e);
                        error = e.to_string();
                        vec![]
                    }
                };

                // Write back to cache
                if !items.is_empty() {
                    let _ = crate::disk_cache::save_popular_cache(&items).await;
                }

                let _ = qt_thread.queue(move |mut qobject| {
                    if qobject.generation != generation {
                        return;
                    }
                    klog!(
                        "AppListModel::refresh: updating UI with fresh {} items",
                        items.len()
                    );
                    qobject
                        .as_mut()
                        .set_error_message(cxx_qt_lib::QString::from(&error));
                    qobject.as_mut().apply_items(items);
                    qobject.as_mut().set_loading(false);
                });
            } else {
                let _ = qt_thread.queue(move |mut qobject| {
                    if qobject.generation != generation {
                        return;
                    }
                    qobject.as_mut().set_loading(false);
                });
            }
        });
    }

    pub fn load_popular(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().last_query = None;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let mut error = String::new();
            let items = match client.fetch_popular().await.map(allowed) {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(e) => {
                    error = e.to_string();
                    vec![]
                }
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_popular: updating UI with {} items",
                    items.len()
                );
                qobject
                    .as_mut()
                    .set_error_message(cxx_qt_lib::QString::from(&error));
                qobject.as_mut().apply_items(items);
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_new(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().last_query = None;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let mut error = String::new();
            let items = match client.fetch_recently_added().await.map(allowed) {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(e) => {
                    error = e.to_string();
                    vec![]
                }
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_new: updating UI with {} items",
                    items.len()
                );
                qobject
                    .as_mut()
                    .set_error_message(cxx_qt_lib::QString::from(&error));
                qobject.as_mut().apply_items(items);
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_updated(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().last_query = None;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let mut error = String::new();
            // Offline, the catalog's newest releases stand in for Flathub's list.
            let result = match client.fetch_recently_updated().await {
                Err(e) => {
                    let local = kiosque_core::catalog::catalog().recently_updated(30);
                    if local.is_empty() {
                        Err(e)
                    } else {
                        kerr!("AppListModel::load_updated: {}; using the catalog", e);
                        Ok(local.iter().map(|a| a.to_flathub_app()).collect())
                    }
                }
                ok => ok,
            };
            let items = match result.map(allowed) {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(e) => {
                    error = e.to_string();
                    vec![]
                }
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_updated: updating UI with {} items",
                    items.len()
                );
                qobject
                    .as_mut()
                    .set_error_message(cxx_qt_lib::QString::from(&error));
                qobject.as_mut().apply_items(items);
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn search(mut self: std::pin::Pin<&mut Self>, query: cxx_qt_lib::QString) {
        self.as_mut().start_query(Query::Search(query.to_string()));
    }

    pub fn load_category(mut self: std::pin::Pin<&mut Self>, category: cxx_qt_lib::QString) {
        self.as_mut()
            .start_query(Query::Category(category.to_string()));
    }

    /// Run a catalog-backed request; it is repeated when the catalog changes.
    fn start_query(mut self: std::pin::Pin<&mut Self>, query: Query) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().last_query = None;
        self.as_mut().rust_mut().last_query = Some(query.clone());
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let services = crate::services::services();
            let catalog = kiosque_core::catalog::catalog();
            let config = kiosque_core::config::config();
            let sources = services.sources(&catalog, &config);
            let result = match &query {
                Query::Search(text) => kiosque_core::apps::search(sources, text).await,
                Query::Category(id) => kiosque_core::apps::category(sources, id).await,
            };
            let (items, error) = match result {
                Ok(apps) => {
                    klog!("AppListModel: {:?}: {} apps", query, apps.len());
                    (entries(apps), String::new())
                }
                Err(e) => {
                    kerr!("AppListModel: {:?}: {}", query, e);
                    (vec![], e)
                }
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                qobject
                    .as_mut()
                    .set_error_message(cxx_qt_lib::QString::from(&error));
                qobject.as_mut().apply_items(items);
                qobject.as_mut().set_loading(false);
            });
        });
    }

    fn apply_items(mut self: std::pin::Pin<&mut Self>, items: Vec<AppEntry>) {
        if self.items != items {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().items = items;
            self.as_mut().end_reset_model();
        }
    }

    pub fn load_mobile(self: std::pin::Pin<&mut Self>) {
        self.load_storefront_collection(None);
    }

    pub fn load_category_sorted(
        self: std::pin::Pin<&mut Self>,
        category: cxx_qt_lib::QString,
        sort_by: cxx_qt_lib::QString,
    ) {
        self.load_storefront_collection(Some((category.to_string(), sort_by.to_string())));
    }

    fn load_storefront_collection(
        mut self: std::pin::Pin<&mut Self>,
        category: Option<(String, String)>,
    ) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        self.as_mut().rust_mut().last_query = None;
        let generation = self.generation;
        let thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = &crate::services::services().flathub;
            let result = match category {
                None => client.fetch_mobile().await.map(allowed),
                Some((category, sort)) if sort == "newest" => client
                    .fetch_category(&category)
                    .await
                    .map(allowed)
                    .map(|mut apps| {
                        apps.sort_by_key(|app| std::cmp::Reverse(app.added_at.unwrap_or(0)));
                        apps.truncate(30);
                        apps
                    }),
                Some((category, sort)) => client
                    .fetch_category_sorted(&category, &sort)
                    .await
                    .map(allowed),
            };
            let mut error = String::new();
            let items: Vec<AppEntry> = match result {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(e) => {
                    kerr!("storefront collection: {}", e);
                    error = e.to_string();
                    vec![]
                }
            };
            let _ = thread.queue(move |mut model| {
                if model.generation != generation {
                    return;
                }
                model
                    .as_mut()
                    .set_error_message(cxx_qt_lib::QString::from(&error));
                model.as_mut().apply_items(items);
                model.as_mut().set_loading(false);
            });
        });
    }
}
