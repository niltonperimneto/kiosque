//! `FeaturedModel`: the shopfront hero carousel.

use kiosque_core::entries::FeaturedEntry;
use qtbridge::{QModelItem, qobject};

pub use object::FeaturedModel;

/// Seconds a cached featured list is shown without refetching.
const FEATURED_TTL: u64 = 300;

#[derive(Debug, Default, Clone, PartialEq, QModelItem)]
#[allow(non_snake_case)]
pub struct FeaturedItem {
    pub name: String,
    pub summary: String,
    pub iconUrl: String,
    pub appId: String,
    pub description: String,
    pub developer: String,
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
        qproperty!("loading", Read = loading_value, Notify = loading_changed);
        qproperty!("count", Read = count_value, Notify = count_changed);

        fn loading_value(&self) -> bool {
            self.loading
        }
        fn count_value(&self) -> i32 {
            self.count
        }

        #[qsignal]
        pub fn loading_changed(&mut self);
        #[qsignal]
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
                let day = chrono::Local::now().format("%Y-%m-%d").to_string();
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
            let items: Vec<FeaturedItem> = entries.into_iter().map(FeaturedItem::from).collect();
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
