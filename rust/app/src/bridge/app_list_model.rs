use cxx_qt::{CxxQtType, Threading};

use crate::view::app_list::matches_subcategory;

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
}

pub use kiosque_core::entries::AppEntry;

#[derive(Default)]
pub struct AppListModelRust {
    pub items: Vec<AppEntry>,
    pub loading: bool,
    generation: u64,
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
                    qobject.as_mut().begin_reset_model();
                    qobject.as_mut().rust_mut().items = items;
                    qobject.as_mut().end_reset_model();
                });
            }

            // SWR Step 2: Fetch from network if cache is expired or missing
            if is_expired || cached_items.is_none() {
                let client = crate::services::services().flathub.clone();
                let items = match client.fetch_popular().await {
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
                    qobject.as_mut().begin_reset_model();
                    qobject.as_mut().rust_mut().items = items;
                    qobject.as_mut().end_reset_model();
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
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let items = match client.fetch_popular().await {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(_) => vec![],
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_popular: updating UI with {} items",
                    items.len()
                );
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_new(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let items = match client.fetch_recently_added().await {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(_) => vec![],
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_new: updating UI with {} items",
                    items.len()
                );
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_updated(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let items = match client.fetch_recently_updated().await {
                Ok(apps) => apps
                    .into_iter()
                    .map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    })
                    .collect(),
                Err(_) => vec![],
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_updated: updating UI with {} items",
                    items.len()
                );
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn search(mut self: std::pin::Pin<&mut Self>, query: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        let query_str = query.to_string();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();
            let items = match client.search(&query_str).await {
                Ok(apps) => {
                    klog!(
                        "AppListModel::search(\"{}\"): {} results",
                        query_str,
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
                    kerr!("AppListModel::search: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::search: updating UI with {} items",
                    items.len()
                );
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_category(mut self: std::pin::Pin<&mut Self>, category: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        let category_str = category.to_string();
        crate::runtime::runtime().spawn(async move {
            let client = crate::services::services().flathub.clone();

            let api_category = if let Some(idx) = category_str.find('-') {
                &category_str[..idx]
            } else {
                &category_str
            };

            let items = match client.fetch_category(api_category).await {
                Ok(apps) => {
                    klog!(
                        "AppListModel::load_category(\"{}\"): {} apps from API",
                        category_str,
                        apps.len()
                    );

                    // Categories without a "Prefix-SubId" form keep every app; otherwise
                    // narrow to the apps matching the requested subcategory.
                    let filtered: Vec<_> = apps
                        .into_iter()
                        .filter(|app| match category_str.split_once('-') {
                            Some((prefix, sub_id)) => matches_subcategory(app, prefix, sub_id),
                            None => true,
                        })
                        .collect();

                    klog!(
                        "AppListModel::load_category(\"{}\"): {} apps after filter",
                        category_str,
                        filtered.len()
                    );

                    filtered
                        .into_iter()
                        .map(|app| AppEntry {
                            name: app.name,
                            summary: app.summary.unwrap_or_default(),
                            icon_url: app.icon.unwrap_or_default(),
                            app_id: app.app_id,
                        })
                        .collect()
                }
                Err(e) => {
                    kerr!("AppListModel::load_category: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                klog!(
                    "AppListModel::load_category: updating UI with {} items",
                    items.len()
                );
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }
}
