//! `RepoModel` singleton: configured Flatpak remotes.

use qtbridge::{QModelItem, qobject};

pub use object::RepoModel;

#[derive(Debug, Default, Clone, PartialEq, QModelItem)]
pub struct RepoItem {
    pub name: String,
    pub title: String,
    pub url: String,
    pub description: String,
}

#[qobject(Base = QListModel, NoQmlElement)]
mod object {
    use qtbridge::{QListModel, QListModelBase, QmlObject};

    use super::*;
    use crate::view::ops;

    #[derive(Default)]
    pub struct RepoModel {
        pub(super) items: Vec<RepoItem>,
        pub(super) pending: Option<Vec<RepoItem>>,
        pub(super) loading: bool,
        pub(super) count: i32,
    }

    impl QListModel for RepoModel {
        type Item = RepoItem;
        fn len(&self) -> usize {
            self.items.len()
        }
        fn get(&self, index: usize) -> Option<&RepoItem> {
            self.items.get(index)
        }
        fn reset_unnotified(&mut self) {
            if let Some(items) = self.pending.take() {
                self.items = items;
            }
        }
    }

    impl RepoModel {
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
        #[qsignal(qml_name = "remoteAdded")]
        pub fn remote_added(&mut self, success: bool, message: String);
        #[qsignal(qml_name = "remoteRemoved")]
        pub fn remote_removed(&mut self, success: bool, message: String);

        #[qslot]
        fn refresh(&mut self) {
            self.set_loading(true);
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let remotes = ops::remotes().await.unwrap_or_else(|e| {
                    kerr!("RepoModel: {}", e);
                    vec![]
                });
                let items: Vec<RepoItem> = remotes
                    .into_iter()
                    .map(|r| RepoItem {
                        name: r.name,
                        title: r.title,
                        url: r.url,
                        description: r.description,
                    })
                    .collect();
                handle.invoke_mut(move |m| {
                    let count = items.len() as i32;
                    m.pending = Some(items);
                    m.reset();
                    m.set_count(count);
                    m.set_loading(false);
                });
            });
        }

        #[qslot(qml_name = "addRemote")]
        fn add_remote(&mut self, name: String, url: String) {
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let result = ops::add_remote(name.trim(), url.trim()).await;
                handle.invoke_mut(move |m| m.finish(result, true));
            });
        }

        #[qslot(qml_name = "removeRemote")]
        fn remove_remote(&mut self, name: String) {
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let result = ops::remove_remote(&name).await;
                handle.invoke_mut(move |m| m.finish(result, false));
            });
        }
    }

    impl RepoModel {
        fn finish(&mut self, result: Result<(), String>, added: bool) {
            let ok = result.is_ok();
            if ok {
                self.refresh();
            }
            let message = match result {
                Ok(()) if added => kiosque_core::l10n::i18n("Repository added successfully", &[]),
                Ok(()) => kiosque_core::l10n::i18n("Repository removed successfully", &[]),
                Err(e) => e,
            };
            if added {
                self.remote_added(ok, message);
            } else {
                self.remote_removed(ok, message);
            }
        }
    }
}

notify_setters!(RepoModel {
    set_loading: loading: bool => loading_changed,
    set_count: count: i32 => count_changed,
});

qml_element!(RepoModel, "RepoModel", singleton);
