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

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeaturedEntry {
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub app_id: String,
    pub description: String,
    pub developer: String,
    /// Raw remote URL of the app's primary screenshot ("" when absent).
    #[serde(default)]
    pub screenshot_url: String,
    /// Appstream branding primary colors ("" when the app has none).
    #[serde(default)]
    pub brand_color_light: String,
    #[serde(default)]
    pub brand_color_dark: String,
    /// Marks Flathub's App of the Day, which is shown in a dedicated banner
    /// instead of the carousel. Matched by id, not index: failed detail
    /// fetches can shift positions in the list.
    #[serde(default)]
    pub is_app_of_the_day: bool,
}

#[derive(Default)]
pub struct FeaturedModelRust {
    pub items: Vec<FeaturedEntry>,
    pub loading: bool,
    pub aotd: Option<FeaturedEntry>,
    pub aotd_json: cxx_qt_lib::QString,
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
                Self::NAME_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.name)),
                Self::SUMMARY_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.summary)),
                Self::ICON_URL_ROLE => {
                    let qml_url = format!("image://kiosque/{}/icon", item.app_id);
                    crate::image_registry::register_image_url(format!("{}:icon", item.app_id), item.icon_url.clone());
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&qml_url))
                }
                Self::APP_ID_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.app_id)),
                Self::DESCRIPTION_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.description)),
                Self::DEVELOPER_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.developer)),
                Self::SCREENSHOT_URL_ROLE => {
                    if item.screenshot_url.is_empty() {
                        return cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::default());
                    }
                    let qml_url = format!("image://kiosque/{}/screenshot/0", item.app_id);
                    crate::image_registry::register_image_url(format!("{}:screenshot:0", item.app_id), item.screenshot_url.clone());
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&qml_url))
                }
                Self::BRAND_COLOR_LIGHT_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.brand_color_light)),
                Self::BRAND_COLOR_DARK_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.brand_color_dark)),
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
        roles.insert(Self::DESCRIPTION_ROLE, cxx_qt_lib::QByteArray::from("description"));
        roles.insert(Self::DEVELOPER_ROLE, cxx_qt_lib::QByteArray::from("developer"));
        roles.insert(Self::SCREENSHOT_URL_ROLE, cxx_qt_lib::QByteArray::from("screenshotUrl"));
        roles.insert(Self::BRAND_COLOR_LIGHT_ROLE, cxx_qt_lib::QByteArray::from("brandColorLight"));
        roles.insert(Self::BRAND_COLOR_DARK_ROLE, cxx_qt_lib::QByteArray::from("brandColorDark"));
        roles
    }

    /// Split the App of the Day out of `items`, reset the carousel model with
    /// the remainder, and publish the AotD through the `aotd_json` property.
    /// The AotD never flows through `data()`, so its image URLs must be
    /// registered here or the banner would have nothing to load.
    fn apply_items(mut self: std::pin::Pin<&mut Self>, mut items: Vec<FeaturedEntry>) {
        let aotd = items.iter().find(|i| i.is_app_of_the_day).cloned();
        items.retain(|i| !i.is_app_of_the_day);

        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().items = items;
        self.as_mut().end_reset_model();

        let json = match &aotd {
            Some(entry) => {
                crate::image_registry::register_image_url(format!("{}:icon", entry.app_id), entry.icon_url.clone());
                let screenshot_qml_url = if entry.screenshot_url.is_empty() {
                    String::new()
                } else {
                    crate::image_registry::register_image_url(format!("{}:screenshot:0", entry.app_id), entry.screenshot_url.clone());
                    format!("image://kiosque/{}/screenshot/0", entry.app_id)
                };
                serde_json::json!({
                    "appId": entry.app_id,
                    "name": entry.name,
                    "summary": entry.summary,
                    "developer": entry.developer,
                    "iconUrl": format!("image://kiosque/{}/icon", entry.app_id),
                    "screenshotUrl": screenshot_qml_url,
                    "brandColorLight": entry.brand_color_light,
                    "brandColorDark": entry.brand_color_dark,
                })
                .to_string()
            }
            None => "{}".to_string(),
        };
        self.as_mut().rust_mut().aotd = aotd;
        self.as_mut().set_aotd_json(cxx_qt_lib::QString::from(&json));
    }

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            // SWR Step 1: Try reading from disk cache first
            let mut cached_items = None;
            let mut is_expired = true;
            
            if let Some((items, expired)) = crate::disk_cache::load_featured_cache().await {
                is_expired = expired;
                cached_items = Some(items.clone());
                
                // Yield cached items to Qt immediately (Instant load)
                let _ = qt_thread.queue(move |mut qobject| {
                    eprintln!("[kiosque] FeaturedModel::refresh: Loading cached featured list ({} items)", items.len());
                    qobject.as_mut().apply_items(items);
                });
            }
            
            // SWR Step 2: Fetch from network if cache is expired or missing
            if is_expired || cached_items.is_none() {
                let distro_id = detect_distro_id();
                eprintln!("[kiosque] FeaturedModel::refresh: detected distro ID: {}, fetching fresh content", distro_id);
                let curated_app_ids = get_curated_apps_for_distro(&distro_id);
                
                let client = std::sync::Arc::new(crate::flathub::client::FlathubClient::new());
                
                let mut all_ids = Vec::new();
                
                // 1. Try to fetch the Flathub App of the Day
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let aotd_url = format!("https://flathub.org/api/v2/app-picks/app-of-the-day/{}", today);
                let mut aotd_id = None;
                if let Ok(body) = crate::flathub::client::fetch_text(&aotd_url).await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(id) = json.get("app_id").and_then(|v| v.as_str()) {
                            aotd_id = Some(id.to_string());
                            all_ids.push(id.to_string());
                            eprintln!("[kiosque] FeaturedModel::refresh: Flathub App of the Day is {}", id);
                        }
                    }
                }

                // 2. Add curated apps, ensuring we don't duplicate the App of the Day
                for id in curated_app_ids {
                    if Some(id) != aotd_id.as_deref() {
                        all_ids.push(id.to_string());
                    }
                }

                // 3. Spawn details fetch in parallel for each app
                let mut handles = Vec::new();
                for app_id_str in all_ids {
                    let client = client.clone();
                    handles.push(tokio::spawn(async move {
                        client.fetch_details(&app_id_str).await
                    }));
                }
                
                let mut items = Vec::new();
                for handle in handles {
                    if let Ok(Ok(details)) = handle.await {
                        let screenshot_url = details.best_screenshot_url().unwrap_or_default();
                        let (brand_light, brand_dark) = details.brand_colors();
                        items.push(FeaturedEntry {
                            name: details.name,
                            summary: details.summary.clone(),
                            icon_url: details.icon.unwrap_or_default(),
                            description: details.description.unwrap_or_else(|| details.summary.clone()),
                            developer: details.developer_name.unwrap_or_default(),
                            screenshot_url,
                            brand_color_light: brand_light.unwrap_or_default(),
                            brand_color_dark: brand_dark.unwrap_or_default(),
                            is_app_of_the_day: Some(details.id.as_str()) == aotd_id.as_deref(),
                            app_id: details.id,
                        });
                    }
                }

                // Fallback to trending if list is empty
                if items.is_empty() {
                    eprintln!("[kiosque] WARNING: Curated list empty or failed, falling back to trending apps");
                    if let Ok(apps) = client.fetch_trending().await {
                        items = apps.into_iter().map(|app| FeaturedEntry {
                            name: app.name,
                            summary: app.summary.clone().unwrap_or_default(),
                            icon_url: app.icon.unwrap_or_default(),
                            app_id: app.app_id,
                            description: app.description.unwrap_or_else(|| app.summary.unwrap_or_default()),
                            developer: app.developer_name.unwrap_or_default(),
                            // Collection entries carry no screenshots or
                            // branding; the banner degrades to icon + tint.
                            screenshot_url: String::new(),
                            brand_color_light: String::new(),
                            brand_color_dark: String::new(),
                            is_app_of_the_day: false,
                        }).collect::<Vec<_>>();
                    }
                }
                items.truncate(5);
                
                // Write back to cache
                if !items.is_empty() {
                    let _ = crate::disk_cache::save_featured_cache(&items).await;
                }
                
                let _ = qt_thread.queue(move |mut qobject| {
                    // Skip the second model reset when the fresh list is identical to
                    // what we already showed from cache: a redundant reset only causes
                    // flicker and risks the SwipeView relayout glitch.
                    if Some(&items) == cached_items.as_ref() {
                        eprintln!("[kiosque] FeaturedModel::refresh: fresh list identical to cache, skipping reset");
                        qobject.as_mut().set_loading(false);
                        return;
                    }
                    eprintln!("[kiosque] FeaturedModel::refresh: updating UI with fresh {} items", items.len());
                    qobject.as_mut().apply_items(items);
                    qobject.as_mut().set_loading(false);
                });
            } else {
                let _ = qt_thread.queue(move |mut qobject| {
                    qobject.as_mut().set_loading(false);
                });
            }
        });
    }
}

fn detect_distro_id() -> String {
    let paths = ["/run/host/os-release", "/etc/os-release", "/usr/lib/os-release"];
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
