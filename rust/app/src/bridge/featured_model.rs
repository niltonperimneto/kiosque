//! `FeaturedModel`: the shopfront hero carousel.

use kiosque_core::entries::FeaturedEntry;
use qtbridge::qobject;

pub use object::FeaturedModel;

/// Seconds a cached featured list is shown without refetching.
const FEATURED_TTL: u64 = 300;

#[derive(Debug, Default, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct FeaturedItem {
    pub name: String,
    pub summary: String,
    pub iconUrl: String,
    pub appId: String,
    pub description: String,
    pub developer: String,
    pub screenshotUrl: String,
    pub brandColorLight: String,
    pub brandColorDark: String,
}

impl From<FeaturedEntry> for FeaturedItem {
    fn from(e: FeaturedEntry) -> Self {
        Self {
            name: e.name,
            summary: e.summary,
            iconUrl: e.icon_url,
            appId: e.app_id,
            description: e.description,
            developer: e.developer,
            screenshotUrl: e.screenshot_url,
            brandColorLight: e.brand_color_light,
            brandColorDark: e.brand_color_dark,
        }
    }
}

#[qobject(Base = QListModel, NoQmlElement)]
mod object {
    use qtbridge::{QListModel, QListModelBase, QmlObject};

    use super::*;

    #[derive(Default)]
    pub struct FeaturedModel {
        pub(super) items: Vec<FeaturedItem>,
        pub(super) pending: Option<Vec<FeaturedItem>>,
        pub(super) loading: bool,
        pub(super) count: i32,
        pub(super) generation: u64,
        pub(super) aotd_json: String,
    }

    impl QListModel for FeaturedModel {
        type Item = FeaturedItem;
        fn len(&self) -> usize {
            self.items.len()
        }
        fn get(&self, index: usize) -> Option<&FeaturedItem> {
            self.items.get(index)
        }
        fn reset_unnotified(&mut self) {
            if let Some(items) = self.pending.take() {
                self.items = items;
            }
        }
    }

    impl FeaturedModel {
        qproperty!("aotd_json", Read = get_aotd, Notify = aotd_changed);
        fn get_aotd(&self) -> String {
            self.aotd_json.clone()
        }
        #[qsignal]
        pub fn aotd_changed(&mut self);
        qproperty!("loading", Read = loading_value, Notify = loading_changed);
        qproperty!("count", Read = count_value, Notify = count_changed);

        fn loading_value(&self) -> bool {
            self.loading
        }
        fn count_value(&self) -> i32 {
            self.count
        }

        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);
        #[qsignal(qml_name = "countChanged")]
        pub fn count_changed(&mut self);

        /// Show the cached list at once, then refresh it if it is stale.
        #[qslot]
        fn refresh(&mut self) {
            self.generation += 1;
            let generation = self.generation;
            self.set_loading(true);
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                if let Some((stamp, cached)) = crate::disk_cache::load_featured_cache().await {
                    let fresh =
                        kiosque_core::disk_cache::now_secs().saturating_sub(stamp) <= FEATURED_TTL;
                    handle.invoke_mut(move |m| m.apply(generation, cached, !fresh));
                    if fresh {
                        return;
                    }
                }
                let distro = kiosque_core::config::distro_id();
                let day = jiff::ZonedDateTime::now().strftime("%Y-%m-%d").to_string();
                let items = crate::view::featured::fetch(
                    &crate::services::services().flathub,
                    &distro,
                    &day,
                )
                .await;
                if !items.is_empty() {
                    let _ = crate::disk_cache::save_featured_cache(&items).await;
                }
                handle.invoke_mut(move |m| m.apply(generation, items, false));
            });
        }
    }

    impl FeaturedModel {
        pub(super) fn apply(&mut self, generation: u64, entries: Vec<FeaturedEntry>, more: bool) {
            if generation != self.generation {
                return;
            }
            let daily = entries.iter().find(|e| e.is_app_of_the_day);
            let aotd=daily.map(|e|serde_json::json!({"appId":e.app_id,"name":e.name,"summary":e.summary,"developer":e.developer,"iconUrl":e.icon_url,"screenshotUrl":e.screenshot_url,"screenshots":e.screenshot_urls})).unwrap_or_else(||serde_json::json!({})).to_string();
            if self.aotd_json != aotd {
                self.aotd_json = aotd;
                self.aotd_changed();
            }
            let items: Vec<FeaturedItem> = entries
                .into_iter()
                .filter(|e| !e.is_app_of_the_day)
                .map(FeaturedItem::from)
                .collect();
            if items != self.items {
                let count = items.len() as i32;
                self.pending = Some(items);
                self.reset();
                self.set_count(count);
            }
            if !more {
                self.set_loading(false);
            }
        }
    }
}

notify_setters!(FeaturedModel {
    set_loading: loading: bool => loading_changed,
    set_count: count: i32 => count_changed,
});

qml_element!(FeaturedModel, "FeaturedModel");

// Keep the existing Qt.UserRole numbers as well as the QML role names.
impl qtbridge::QModelItem for FeaturedItem {
    type T0 = String;
    type T1 = String;
    type T2 = String;
    type T3 = String;
    type T4 = String;
    type T5 = String;
    type T6 = String;
    type T7 = String;
    type T8 = String;
    type T9 = ();
    type T10 = ();
    type T11 = ();
    type T12 = ();
    type T13 = ();
    type T14 = ();
    fn get0(&self) -> Option<&String> {
        Some(&self.name)
    }
    fn get1(&self) -> Option<&String> {
        Some(&self.summary)
    }
    fn get2(&self) -> Option<&String> {
        Some(&self.iconUrl)
    }
    fn get3(&self) -> Option<&String> {
        Some(&self.appId)
    }
    fn get4(&self) -> Option<&String> {
        Some(&self.description)
    }
    fn get5(&self) -> Option<&String> {
        Some(&self.developer)
    }
    fn get6(&self) -> Option<&String> {
        Some(&self.screenshotUrl)
    }
    fn get7(&self) -> Option<&String> {
        Some(&self.brandColorLight)
    }
    fn get8(&self) -> Option<&String> {
        Some(&self.brandColorDark)
    }
    fn role_names() -> std::collections::HashMap<i32, String> {
        [
            (256, "name".into()),
            (257, "summary".into()),
            (258, "iconUrl".into()),
            (259, "appId".into()),
            (260, "description".into()),
            (261, "developer".into()),
            (262, "screenshotUrl".into()),
            (263, "brandColorLight".into()),
            (264, "brandColorDark".into()),
        ]
        .into()
    }
    fn get_role(&self, role: i32) -> qtbridge::qtbridge_type_lib::QVariant {
        match role {
            256 => qtbridge::QVariantConvertible::to_qvariant(&self.name),
            257 => qtbridge::QVariantConvertible::to_qvariant(&self.summary),
            258 => qtbridge::QVariantConvertible::to_qvariant(&self.iconUrl),
            259 => qtbridge::QVariantConvertible::to_qvariant(&self.appId),
            260 => qtbridge::QVariantConvertible::to_qvariant(&self.description),
            261 => qtbridge::QVariantConvertible::to_qvariant(&self.developer),
            262 => qtbridge::QVariantConvertible::to_qvariant(&self.screenshotUrl),
            263 => qtbridge::QVariantConvertible::to_qvariant(&self.brandColorLight),
            264 => qtbridge::QVariantConvertible::to_qvariant(&self.brandColorDark),
            _ => Default::default(),
        }
    }
}
