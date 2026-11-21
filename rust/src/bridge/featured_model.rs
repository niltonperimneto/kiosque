use cxx_qt::{CxxQtType, Threading};

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
        #[qproperty(QString, aotd_json)]
        type FeaturedModel = super::FeaturedModelRust;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &FeaturedModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "rowCount"]
        #[cxx_override]
        fn row_count(self: &FeaturedModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_name = "roleNames"]
        #[cxx_override]
        fn role_names(self: &FeaturedModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn refresh(self: Pin<&mut FeaturedModel>);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut FeaturedModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut FeaturedModel>);
    }

    impl cxx_qt::Threading for FeaturedModel {}
}

pub use kiosque_core::entries::FeaturedEntry;

#[derive(Default)]
pub struct FeaturedModelRust {
    pub items: Vec<FeaturedEntry>,
    pub loading: bool,
    pub aotd_json: cxx_qt_lib::QString,
    aotd: Option<FeaturedEntry>,
    generation: u64,
}

impl qobject::FeaturedModel {
    pub const NAME_ROLE: i32 = 256;
    pub const SUMMARY_ROLE: i32 = 257;
    pub const ICON_URL_ROLE: i32 = 258;
    pub const APP_ID_ROLE: i32 = 259;
    pub const DESCRIPTION_ROLE: i32 = 260;
    pub const DEVELOPER_ROLE: i32 = 261;
    pub const SCREENSHOT_URL_ROLE: i32 = 262;
    pub const BRAND_COLOR_LIGHT_ROLE: i32 = 263;
    pub const BRAND_COLOR_DARK_ROLE: i32 = 264;

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
                Self::DESCRIPTION_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.description))
                }
                Self::DEVELOPER_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.developer))
                }
                Self::SCREENSHOT_URL_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(
                        &crate::image_cache::image_url(&item.screenshot_url),
                    ))
                }
                Self::BRAND_COLOR_LIGHT_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.brand_color_light))
                }
                Self::BRAND_COLOR_DARK_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.brand_color_dark))
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
        roles.insert(
            Self::DESCRIPTION_ROLE,
            cxx_qt_lib::QByteArray::from("description"),
        );
        roles.insert(
            Self::DEVELOPER_ROLE,
            cxx_qt_lib::QByteArray::from("developer"),
        );
        roles.insert(
            Self::SCREENSHOT_URL_ROLE,
            cxx_qt_lib::QByteArray::from("screenshotUrl"),
        );
        roles.insert(
            Self::BRAND_COLOR_LIGHT_ROLE,
            cxx_qt_lib::QByteArray::from("brandColorLight"),
        );
        roles.insert(
            Self::BRAND_COLOR_DARK_ROLE,
            cxx_qt_lib::QByteArray::from("brandColorDark"),
        );
        roles
    }

    fn apply_items(mut self: std::pin::Pin<&mut Self>, mut items: Vec<FeaturedEntry>) {
        let aotd = items.iter().find(|item| item.is_app_of_the_day).cloned();
        items.retain(|item| !item.is_app_of_the_day);
        if self.items != items {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().items = items;
            self.as_mut().end_reset_model();
        }
        if self.aotd != aotd {
            let json = aotd
                .as_ref()
                .map(|entry| {
                    serde_json::json!({
                        "appId": entry.app_id, "name": entry.name, "summary": entry.summary,
                        "developer": entry.developer,
                        "iconUrl": crate::image_cache::image_url(&entry.icon_url),
                        "screenshotUrl": crate::image_cache::image_url(&entry.screenshot_url),
                    })
                })
                .unwrap_or_else(|| serde_json::json!({}));
            self.as_mut()
                .set_aotd_json(cxx_qt_lib::QString::from(&json.to_string()));
            self.as_mut().rust_mut().aotd = aotd;
        }
    }

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let cached = crate::disk_cache::load_featured_cache().await;
            let fresh = cached.as_ref().is_some_and(|(timestamp, _)| {
                crate::disk_cache::now_secs().saturating_sub(*timestamp) <= 300
            });
            if let Some((_, items)) = cached {
                let _ = thread.queue(move |mut model| {
                    if model.generation != generation {
                        return;
                    }
                    model.as_mut().apply_items(items);
                    if fresh {
                        model.as_mut().set_loading(false);
                    }
                });
            }
            if fresh {
                return;
            }
            let client = &crate::services::services().flathub;
            let today = jiff::Zoned::now().strftime("%Y-%m-%d").to_string();
            let aotd_id = client
                .fetch_text(&format!("app-picks/app-of-the-day/{today}"))
                .await
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|json| {
                    json.get("app_id")
                        .and_then(|id| id.as_str())
                        .map(str::to_owned)
                });
            let mut ids = Vec::new();
            if let Some(id) = &aotd_id {
                ids.push(id.clone());
            }
            for id in get_curated_apps_for_distro(&detect_distro_id()) {
                if !ids.iter().any(|existing| existing == id) {
                    ids.push(id.into());
                }
            }
            let details = futures_for_details(client, ids).await;
            let mut items: Vec<FeaturedEntry> = details
                .into_iter()
                .map(|details| {
                    let (light, dark) = details.brand_colors();
                    let screenshot_url = details.best_screenshot_url().unwrap_or_default();
                    FeaturedEntry {
                        is_app_of_the_day: aotd_id.as_deref() == Some(details.id.as_str()),
                        screenshot_url,
                        brand_color_light: light.unwrap_or_default(),
                        brand_color_dark: dark.unwrap_or_default(),
                        name: details.name,
                        summary: details.summary.clone(),
                        icon_url: details.icon.unwrap_or_default(),
                        app_id: details.id,
                        description: details.description.unwrap_or(details.summary),
                        developer: details.developer_name.unwrap_or_default(),
                    }
                })
                .collect();
            if items.is_empty() {
                if let Ok(apps) = client.fetch_trending().await {
                    items = apps
                        .into_iter()
                        .map(|app| FeaturedEntry {
                            name: app.name,
                            summary: app.summary.clone().unwrap_or_default(),
                            icon_url: app.icon.unwrap_or_default(),
                            app_id: app.app_id,
                            description: app
                                .description
                                .unwrap_or_else(|| app.summary.unwrap_or_default()),
                            developer: app.developer_name.unwrap_or_default(),
                            ..Default::default()
                        })
                        .collect();
                }
            }
            items.truncate(if items.iter().any(|item| item.is_app_of_the_day) {
                6
            } else {
                5
            });
            let _ = thread.queue(move |mut model| {
                if model.generation != generation {
                    return;
                }
                // Persist only an accepted generation, so an older refresh cannot
                // overwrite the current cached state after its GUI response loses.
                if !items.is_empty() {
                    let cached = items.clone();
                    crate::runtime::runtime().spawn(async move {
                        let _ = crate::disk_cache::save_featured_cache(&cached).await;
                    });
                }
                model.as_mut().apply_items(items);
                model.as_mut().set_loading(false);
            });
        });
    }
}

async fn futures_for_details(
    client: &crate::flathub::client::FlathubClient,
    ids: Vec<String>,
) -> Vec<crate::flathub::types::AppDetails> {
    let mut tasks = tokio::task::JoinSet::new();
    for (index, id) in ids.into_iter().enumerate() {
        let client = client.clone();
        tasks.spawn(async move { (index, client.fetch_details(&id).await) });
    }
    let mut details = Vec::new();
    while let Some(result) = tasks.join_next().await {
        if let Ok((index, Ok(item))) = result {
            details.push((index, item));
        }
    }
    details.sort_by_key(|(index, _)| *index);
    details.into_iter().map(|(_, item)| item).collect()
}

fn detect_distro_id() -> String {
    let paths = [
        "/run/host/os-release",
        "/etc/os-release",
        "/usr/lib/os-release",
    ];
    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if line.starts_with("ID=") {
                    let id = line.trim_start_matches("ID=").trim_matches('"').to_string();
                    return id;
                }
            }
        }
    }
    "generic".to_string()
}

fn get_curated_apps_for_distro(distro_id: &str) -> Vec<&'static str> {
    match distro_id {
        "fedora" => vec![
            "org.mozilla.firefox",
            "org.gimp.GIMP",
            "org.videolan.VLC",
            "org.blender.Blender",
            "org.inkscape.Inkscape",
        ],
        "ubuntu" | "pop" | "mint" => vec![
            "org.mozilla.firefox",
            "org.telegram.desktop",
            "org.videolan.VLC",
            "org.gimp.GIMP",
            "com.spotify.Client",
        ],
        "neon" | "arch" | "opensuse" | "manjaro" => vec![
            "org.kde.krita",
            "org.kde.kdenlive",
            "org.kde.kate",
            "org.kde.kstars",
            "org.mozilla.firefox",
        ],
        _ => vec![
            "org.mozilla.firefox",
            "org.kde.krita",
            "org.videolan.VLC",
            "org.gimp.GIMP",
            "org.blender.Blender",
        ],
    }
}
