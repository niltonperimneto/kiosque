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

pub struct AppEntry {
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub app_id: String,
}

#[derive(Default)]
pub struct AppListModelRust {
    pub items: Vec<AppEntry>,
    pub loading: bool,
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
                Self::NAME_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.name)),
                Self::SUMMARY_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.summary)),
                Self::ICON_URL_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.icon_url)),
                Self::APP_ID_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.app_id)),
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
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();
            let items = match client.fetch_popular().await {
                Ok(apps) => {
                    klog!("AppListModel::refresh: mapping {} apps to entries", apps.len());
                    apps.into_iter().map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    }).collect()
                }
                Err(e) => {
                    kerr!("AppListModel::refresh: {}", e);
                    vec![]
                }
            };
            
            let _ = qt_thread.queue(move |mut qobject| {
                klog!("AppListModel::refresh: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_popular(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();
            let items = match client.fetch_popular().await {
                Ok(apps) => {
                    apps.into_iter().map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    }).collect()
                }
                Err(_) => vec![],
            };
            let _ = qt_thread.queue(move |mut qobject| {
                klog!("AppListModel::load_popular: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_new(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();
            let items = match client.fetch_recently_added().await {
                Ok(apps) => {
                    apps.into_iter().map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    }).collect()
                }
                Err(_) => vec![],
            };
            let _ = qt_thread.queue(move |mut qobject| {
                klog!("AppListModel::load_new: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_updated(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();
            let items = match client.fetch_recently_updated().await {
                Ok(apps) => {
                    apps.into_iter().map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    }).collect()
                }
                Err(_) => vec![],
            };
            let _ = qt_thread.queue(move |mut qobject| {
                klog!("AppListModel::load_updated: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn search(mut self: std::pin::Pin<&mut Self>, query: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        let query_str = query.to_string();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();
            let items = match client.search(&query_str).await {
                Ok(apps) => {
                    klog!("AppListModel::search(\"{}\"): {} results", query_str, apps.len());
                    apps.into_iter().map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    }).collect()
                }
                Err(e) => {
                    kerr!("AppListModel::search: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                klog!("AppListModel::search: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn load_category(mut self: std::pin::Pin<&mut Self>, category: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        let category_str = category.to_string();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();
            
            let api_category = if let Some(idx) = category_str.find('-') {
                &category_str[..idx]
            } else {
                &category_str
            };

            let items = match client.fetch_category(api_category).await {
                Ok(apps) => {
                    klog!("AppListModel::load_category(\"{}\"): {} apps from API", category_str, apps.len());

                    // Categories without a "Prefix-SubId" form keep every app; otherwise
                    // narrow to the apps matching the requested subcategory.
                    let filtered: Vec<_> = apps.into_iter().filter(|app| {
                        match category_str.split_once('-') {
                            Some((prefix, sub_id)) => matches_subcategory(app, prefix, sub_id),
                            None => true,
                        }
                    }).collect();

                    klog!("AppListModel::load_category(\"{}\"): {} apps after filter", category_str, filtered.len());

                    filtered.into_iter().map(|app| AppEntry {
                        name: app.name,
                        summary: app.summary.unwrap_or_default(),
                        icon_url: app.icon.unwrap_or_default(),
                        app_id: app.app_id,
                    }).collect()
                }
                Err(e) => {
                    kerr!("AppListModel::load_category: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                klog!("AppListModel::load_category: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }
}

/// Decide whether `app` belongs in the `<prefix>-<sub_id>` subcategory, e.g.
/// `Game-Emulator` or `Graphics-Vector`. Flathub only tags apps with broad
/// top-level categories, so the finer subcategory split is derived here from the
/// app's `sub_categories` plus a few name/summary keyword heuristics for games.
/// Unknown prefixes/sub-ids fall through to `true` (keep the app).
fn matches_subcategory(app: &FlathubApp, prefix: &str, sub_id: &str) -> bool {
    match prefix {
        "Game" => {
            let name_lower = app.name.to_lowercase();
            let summary_lower = app.summary.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
            let is_emulator = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("emulator"));

            let is_launcher = (
                app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("packagemanager")) ||
                name_lower.contains("launcher") ||
                name_lower.contains("client") ||
                summary_lower.contains("launcher")
            ) && !is_emulator;

            let tool_keywords = ["tool", "compat", "patcher", "config", "manager", "overlay", "hud", "backup", "setup"];
            let is_tool = (
                app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("utility")) ||
                tool_keywords.iter().any(|kw| name_lower.contains(kw)) ||
                tool_keywords.iter().any(|kw| summary_lower.contains(kw))
            ) && !is_emulator && !is_launcher;

            match sub_id {
                "Emulator" => is_emulator,
                "Launcher" => is_launcher,
                "Tool" => is_tool,
                "Game" => !is_emulator && !is_launcher && !is_tool,
                _ => true,
            }
        }
        "AudioVideo" => {
            let is_player = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("player"));
            let is_recorder = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("recorder"));
            let is_editing = app.sub_categories.iter().any(|s| {
                s.eq_ignore_ascii_case("audiovideoediting") ||
                s.eq_ignore_ascii_case("midi") ||
                s.eq_ignore_ascii_case("sequencer") ||
                s.eq_ignore_ascii_case("mixer")
            });

            match sub_id {
                "Player" => is_player,
                "Recorder" => is_recorder,
                "Editing" => is_editing,
                "All" => !is_player && !is_recorder && !is_editing,
                _ => true,
            }
        }
        "Development" => {
            let is_ide = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("ide") || s.eq_ignore_ascii_case("guidedesigner"));
            let is_debugger = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("debugger") || s.eq_ignore_ascii_case("profiler"));
            let is_web = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("webdevelopment"));

            match sub_id {
                "IDE" => is_ide,
                "Debugger" => is_debugger,
                "Web" => is_web,
                "All" => !is_ide && !is_debugger && !is_web,
                _ => true,
            }
        }
        "Graphics" => {
            let is_3d = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("3dgraphics"));
            let is_vector = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("vectorgraphics"));
            let is_raster = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("rastergraphics"));
            let is_photography = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("photography"));
            let is_viewer = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("viewer"));

            match sub_id {
                "3D" => is_3d,
                "Vector" => is_vector,
                "Raster" => is_raster,
                "Photography" => is_photography,
                "Viewer" => is_viewer,
                "All" => !is_3d && !is_vector && !is_raster && !is_photography && !is_viewer,
                _ => true,
            }
        }
        "Office" => {
            let is_word = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("wordprocessor"));
            let is_spreadsheet = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("spreadsheet"));
            let is_presentation = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("presentation"));
            let is_finance = app.sub_categories.iter().any(|s| s.eq_ignore_ascii_case("finance"));

            match sub_id {
                "WordProcessor" => is_word,
                "Spreadsheet" => is_spreadsheet,
                "Presentation" => is_presentation,
                "Finance" => is_finance,
                "All" => !is_word && !is_spreadsheet && !is_presentation && !is_finance,
                _ => true,
            }
        }
        _ => true,
    }
}
