//! `RepoModel` singleton: configured Flatpak remotes.

use qtbridge::qobject;

pub use object::RepoModel;

#[derive(Debug, Default, Clone, PartialEq)]
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
        pub(super) repo_key_sha256: String,
        pub(super) pending_repo: Option<Vec<u8>>,
        pub(super) repo_generation: u64,
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
        qproperty!("repo_key_sha256", Read = get_key, Notify = key_changed);
        fn get_key(&self) -> String {
            self.repo_key_sha256.clone()
        }
        fn set_repo_key_sha256(&mut self, key: String) {
            self.repo_key_sha256 = key;
            self.key_changed();
        }
        #[qsignal]
        pub fn key_changed(&mut self);
        #[qsignal(qml_name = "repoFileInspected")]
        pub fn repo_file_inspected(
            &mut self,
            success: bool,
            suggested_name: String,
            title: String,
            url: String,
            error: String,
        );
        #[qslot(qml_name = "inspectRepoFile")]
        pub fn inspect_repo_file(&mut self, path: String) {
            let source = path.to_string();
            klog!("RepoModel::inspect_repo_file: \"{}\"", source);
            self.pending_repo = None;
            self.repo_generation += 1;
            let generation = self.repo_generation;
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                let result = crate::bridge::store_controller::read_repo_file(&source).await;
                qt_thread.invoke_mut(move |q| {
                    if q.repo_generation != generation {
                        return;
                    }
                    let qs = |s: &str| String::from(s);
                    match result {
                        Ok(data) => {
                            let validated = kiosque_core::security::repo_file(&data)
                                .expect("validated repository descriptor");
                            q.set_repo_key_sha256(qs(&validated.key_sha256));
                            let mut summary = crate::flatpak::summarize_keyfile(&data);
                            summary.url = validated.origin;
                            q.pending_repo = Some(data);
                            q.repo_file_inspected(
                                true,
                                qs(&summary.suggested_remote_name),
                                qs(&summary.title),
                                qs(&summary.url),
                                qs(""),
                            );
                        }
                        Err(e) => {
                            kerr!("RepoModel::inspect_repo_file: {}", e);
                            q.repo_file_inspected(false, qs(""), qs(""), qs(""), qs(&e));
                        }
                    }
                });
            });
        }

        #[qslot(qml_name = "addInspectedRemote")]
        pub fn add_inspected_remote(&mut self, name: String) {
            let Some(data) = self.pending_repo.take() else {
                kerr!("RepoModel::add_inspected_remote: no inspected .flatpakrepo");
                return;
            };
            let name = name.to_string();
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                let result = crate::flatpak::ui::add_repository_file(&name, data).await;
                let (success, msg) = match result {
                    Ok(_) => (true, "Repository added successfully".to_string()),
                    Err(e) => (false, e),
                };
                qt_thread.invoke_mut(move |qobject| {
                    qobject.remote_added(success, String::from(&msg));
                    if success {
                        qobject.refresh();
                    }
                });
            });
        }

        #[qslot(qml_name = "discardRepoFile")]
        pub fn discard_repo_file(&mut self) {
            self.pending_repo = None;
            self.repo_generation += 1;
        }

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
        #[qsignal(qml_name = "remoteAdded")]
        pub fn remote_added(&mut self, success: bool, message: String);
        #[qsignal(qml_name = "remoteRemoved")]
        pub fn remote_removed(&mut self, success: bool, message: String);

        #[qslot]
        fn refresh(&mut self) {
            self.set_loading(true);
            let handle = std::sync::Arc::new(self.get_thread_handle());
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
            let handle = std::sync::Arc::new(self.get_thread_handle());
            crate::bridge::spawn(async move {
                let result = ops::add_remote(name.trim(), url.trim()).await;
                handle.invoke_mut(move |m| m.finish(result, true));
            });
        }

        #[qslot(qml_name = "removeRemote")]
        fn remove_remote(&mut self, name: String) {
            let handle = std::sync::Arc::new(self.get_thread_handle());
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

// Keep the existing Qt.UserRole numbers as well as the QML role names.
impl qtbridge::QModelItem for RepoItem {
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
        Some(&self.title)
    }
    fn get2(&self) -> Option<&String> {
        Some(&self.url)
    }
    fn get3(&self) -> Option<&String> {
        Some(&self.description)
    }
    fn role_names() -> std::collections::HashMap<i32, String> {
        [
            (256, "name".into()),
            (257, "title".into()),
            (258, "url".into()),
            (259, "description".into()),
        ]
        .into()
    }
    fn get_role(&self, role: i32) -> qtbridge::qtbridge_type_lib::QVariant {
        match role {
            256 => qtbridge::QVariantConvertible::to_qvariant(&self.name),
            257 => qtbridge::QVariantConvertible::to_qvariant(&self.title),
            258 => qtbridge::QVariantConvertible::to_qvariant(&self.url),
            259 => qtbridge::QVariantConvertible::to_qvariant(&self.description),
            _ => Default::default(),
        }
    }
}
