//! `AppListModel`: one list of apps (a collection, a search or a category).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use kiosque_core::entries::AppEntry;
use qtbridge::qobject;

use crate::view::app_list::{self, AppSource};

pub use object::AppListModel;

/// Seconds a cached popular list is shown without refetching.
const POPULAR_TTL: u64 = 300;

#[derive(Debug, Default, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct AppItem {
    pub name: String,
    pub summary: String,
    pub iconUrl: String,
    pub appId: String,
}

impl From<AppEntry> for AppItem {
    fn from(e: AppEntry) -> Self {
        Self {
            name: e.name,
            summary: e.summary,
            iconUrl: e.icon_url,
            appId: e.app_id,
        }
    }
}

#[qobject(Base = QListModel, NoQmlElement)]
mod object {
    use qtbridge::{QListModel, QmlObject};

    use super::*;

    #[derive(Default)]
    pub struct AppListModel {
        pub(super) items: Vec<AppItem>,
        pub(super) pending: Option<Vec<AppItem>>,
        pub(super) loading: bool,
        pub(super) count: i32,
        pub(super) error: String,
        pub(super) query: crate::view::app_list::AppListQuery,
        /// Mirrors the current generation for tasks, so a superseded
        /// (debounced) request is never sent.
        pub(super) latest: Arc<AtomicU64>,
    }

    impl QListModel for AppListModel {
        type Item = AppItem;
        fn len(&self) -> usize {
            self.items.len()
        }
        fn get(&self, index: usize) -> Option<&AppItem> {
            self.items.get(index)
        }
        fn reset_unnotified(&mut self) {
            if let Some(items) = self.pending.take() {
                self.items = items;
            }
        }
    }

    impl AppListModel {
        qproperty!("loading", Read = loading_value, Notify = loading_changed);
        qproperty!("count", Read = count_value, Notify = count_changed);
        qproperty!("error_message", Read = error_value, Notify = error_changed);
        qproperty!("errorString", Read = error_value, Notify = error_changed);

        fn loading_value(&self) -> bool {
            self.loading
        }
        fn count_value(&self) -> i32 {
            self.count
        }
        fn error_value(&self) -> String {
            self.error.clone()
        }

        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);
        #[qsignal(qml_name = "countChanged")]
        pub fn count_changed(&mut self);
        #[qsignal]
        pub fn error_changed(&mut self);

        /// `source`: `popular`, `new`, `updated`, `search` or `category`;
        /// `arg`: the query or category id. Searches are debounced and only
        /// the newest request's result is applied.
        #[qslot]
        fn load(&mut self, source: String, arg: String) {
            let Some(source) = AppSource::parse(&source, &arg) else {
                kerr!("AppListModel: unknown source {:?}", source);
                return;
            };
            let generation = self.query.begin(source.clone());
            self.latest.store(generation, Ordering::SeqCst);
            self.set_loading(true);
            let handle = self.get_thread_handle();
            let latest = self.latest.clone();
            crate::bridge::spawn(async move {
                let client = &crate::services::services().flathub;
                if let AppSource::Search(query) = &source
                    && let Some(apps) = client.cached_search(query).await
                {
                    let entries = apps.into_iter().map(app_list::entry).collect();
                    handle.invoke_mut(move |m| m.apply(generation, Ok(entries), false));
                    return;
                }
                if let Some(delay) = source.debounce() {
                    tokio::time::sleep(delay).await;
                    if latest.load(Ordering::SeqCst) != generation {
                        return;
                    }
                }
                if source == AppSource::Popular
                    && let Some((stamp, cached)) = crate::disk_cache::load_popular_cache().await
                {
                    let fresh =
                        kiosque_core::disk_cache::now_secs().saturating_sub(stamp) <= POPULAR_TTL;
                    let items = cached.clone();
                    handle.invoke_mut(move |m| m.apply(generation, Ok(items), !fresh));
                    if fresh {
                        return;
                    }
                }
                let result = app_list::fetch(client, &source).await;
                if source == AppSource::Popular
                    && let Ok(items) = &result
                    && !items.is_empty()
                {
                    let _ = crate::disk_cache::save_popular_cache(items).await;
                }
                handle.invoke_mut(move |m| m.apply(generation, result, false));
            });
        }

        #[qslot]
        fn refresh(&mut self) {
            self.load("popular".into(), String::new());
        }
        #[qslot(qml_name = "loadNew")]
        fn load_new(&mut self) {
            self.load("new".into(), String::new());
        }
        #[qslot(qml_name = "loadUpdated")]
        fn load_updated(&mut self) {
            self.load("updated".into(), String::new());
        }
        #[qslot(qml_name = "loadPopular")]
        fn load_popular(&mut self) {
            self.load("popular".into(), String::new());
        }
        #[qslot(qml_name = "loadRecentlyAdded")]
        fn load_recently_added(&mut self) {
            self.load("new".into(), String::new());
        }
        #[qslot(qml_name = "loadRecentlyUpdated")]
        fn load_recently_updated(&mut self) {
            self.load("updated".into(), String::new());
        }
        #[qslot]
        fn search(&mut self, q: String) {
            self.load("search".into(), q);
        }
        #[qslot(qml_name = "loadSearch")]
        fn load_search(&mut self, q: String) {
            self.load("search".into(), q);
        }
        #[qslot(qml_name = "loadCategory")]
        fn load_category(&mut self, q: String) {
            self.load("category".into(), q);
        }
        #[qslot(qml_name = "loadCategorySorted")]
        fn load_category_sorted(&mut self, q: String, sort: String) {
            self.load("category-sorted".into(), format!("{q}:{sort}"));
        }
        #[qslot(qml_name = "loadMobile")]
        fn load_mobile(&mut self) {
            self.load("mobile".into(), String::new());
        }
        /// Drop the items and invalidate in-flight requests.
        #[qslot]
        fn clear(&mut self) {
            self.query.cancel();
            self.latest.store(self.query.generation(), Ordering::SeqCst);
            self.replace(vec![]);
            self.set_loading(false);
        }
    }
}

notify_setters!(AppListModel {
    set_loading: loading: bool => loading_changed,
    set_count: count: i32 => count_changed,
    set_error: error: String => error_changed,
});

impl AppListModel {
    fn replace(&mut self, items: Vec<AppItem>) {
        use qtbridge::QListModelBase;
        if items == self.items {
            return;
        }
        let count = items.len() as i32;
        self.pending = Some(items);
        self.reset();
        self.set_count(count);
    }

    /// Apply a response if it belongs to the current request. `more` keeps
    /// the list loading (a cached list shown while a fresh one is fetched).
    fn apply(&mut self, generation: u64, result: Result<Vec<AppEntry>, String>, more: bool) {
        if !self.query.is_current(generation) {
            return;
        }
        match result {
            Ok(entries) => {
                self.replace(entries.into_iter().map(AppItem::from).collect());
                self.set_error(String::new());
            }
            Err(e) => {
                kerr!("AppListModel: {}", e);
                self.replace(vec![]);
                self.set_error(e);
            }
        }
        if !more {
            self.set_loading(false);
        }
    }

    /// Fill the list from Rust (detail-page child lists).
    pub fn set_entries(&mut self, entries: Vec<AppEntry>) {
        self.query.cancel();
        self.replace(entries.into_iter().map(AppItem::from).collect());
        self.set_loading(false);
    }
}

qml_element!(AppListModel, "AppListModel");

// Keep the existing Qt.UserRole numbers as well as the QML role names.
impl qtbridge::QModelItem for AppItem {
    type T0 = String;
    type T1 = String;
    type T2 = String;
    type T3 = String;
    type T4 = ();
    type T5 = ();
    type T6 = ();
    type T7 = ();
    type T8 = ();
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
    fn role_names() -> std::collections::HashMap<i32, String> {
        [
            (256, "name".into()),
            (257, "summary".into()),
            (258, "iconUrl".into()),
            (259, "appId".into()),
        ]
        .into()
    }
    fn get_role(&self, role: i32) -> qtbridge::qtbridge_type_lib::QVariant {
        match role {
            256 => qtbridge::QVariantConvertible::to_qvariant(&self.name),
            257 => qtbridge::QVariantConvertible::to_qvariant(&self.summary),
            258 => qtbridge::QVariantConvertible::to_qvariant(&self.iconUrl),
            259 => qtbridge::QVariantConvertible::to_qvariant(&self.appId),
            _ => Default::default(),
        }
    }
}
