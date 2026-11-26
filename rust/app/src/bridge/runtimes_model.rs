//! `RuntimesModel`: installed runtimes, which applications use them, and
//! the unused ones that can be removed.

use qtbridge::qobject;

use crate::view::runtimes::RuntimeRow;

pub use object::RuntimesModel;

#[qobject(Base = QListModel, NoQmlElement)]
mod object {
    use qtbridge::{QListModel, QListModelBase, QmlObject};

    use super::*;
    use crate::bridge::util::QVariantList;
    use crate::view::runtimes;

    pub struct RuntimesModel {
        pub(super) items: Vec<RuntimeRow>,
        pub(super) pending: Option<Vec<RuntimeRow>>,
        pub(super) loading: bool,
        pub(super) removing: bool,
        pub(super) count: i32,
        pub(super) error_message: String,
        /// `[{reference, name, size}]` of the unused runtimes.
        pub(super) unused: QVariantList,
        pub(super) generation: u64,
    }

    impl Default for RuntimesModel {
        fn default() -> Self {
            Self {
                items: vec![],
                pending: None,
                loading: false,
                removing: false,
                count: 0,
                error_message: String::new(),
                unused: serde_json::json!([]),
                generation: 0,
            }
        }
    }

    impl QListModel for RuntimesModel {
        type Item = RuntimeRow;
        fn len(&self) -> usize {
            self.items.len()
        }
        fn get(&self, index: usize) -> Option<&RuntimeRow> {
            self.items.get(index)
        }
        fn reset_unnotified(&mut self) {
            if let Some(items) = self.pending.take() {
                self.items = items;
            }
        }
    }

    impl RuntimesModel {
        qproperty!("loading", Read = get_loading, Notify = loading_changed);
        fn get_loading(&self) -> bool {
            self.loading
        }
        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);

        qproperty!("removing", Read = get_removing, Notify = removing_changed);
        fn get_removing(&self) -> bool {
            self.removing
        }
        #[qsignal(qml_name = "removingChanged")]
        pub fn removing_changed(&mut self);

        qproperty!("count", Read = get_count, Notify = count_changed);
        fn get_count(&self) -> i32 {
            self.count
        }
        #[qsignal(qml_name = "countChanged")]
        pub fn count_changed(&mut self);

        qproperty!(
            "errorMessage",
            Read = get_error_message,
            Notify = error_message_changed
        );
        fn get_error_message(&self) -> String {
            self.error_message.clone()
        }
        #[qsignal(qml_name = "errorMessageChanged")]
        pub fn error_message_changed(&mut self);

        qproperty!("unused", Read = get_unused, Notify = unused_changed);
        fn get_unused(&self) -> serde_json::Value {
            self.unused.clone()
        }
        #[qsignal(qml_name = "unusedChanged")]
        pub fn unused_changed(&mut self);

        #[qsignal(qml_name = "removalFinished")]
        pub fn removal_finished(&mut self, success: bool, error: String);

        #[qslot]
        pub fn refresh(&mut self) {
            self.generation += 1;
            let generation = self.generation;
            self.set_loading(true);
            let handle = std::sync::Arc::new(self.get_thread_handle());
            crate::bridge::spawn(async move {
                let result = runtimes::installed_runtimes().await;
                handle.invoke_mut(move |m| {
                    if m.generation != generation {
                        return;
                    }
                    let rows = match result {
                        Ok(rows) => {
                            m.set_error_message(String::new());
                            rows
                        }
                        Err(e) => {
                            kerr!("RuntimesModel: {}", e);
                            m.set_error_message(e);
                            vec![]
                        }
                    };
                    let unused: Vec<serde_json::Value> = rows
                        .iter()
                        .filter(|r| r.unused)
                        .map(|r| {
                            serde_json::json!({
                                "reference": r.reference,
                                "name": r.name,
                                "size": r.size,
                            })
                        })
                        .collect();
                    let count = rows.len() as i32;
                    m.pending = Some(rows);
                    m.reset();
                    m.unused = unused.into();
                    m.unused_changed();
                    m.set_count(count);
                    m.set_loading(false);
                });
            });
        }

        /// Remove every runtime currently listed as unused.
        #[qslot(qml_name = "removeUnused")]
        pub fn remove_unused(&mut self) {
            let refs: Vec<String> = self
                .items
                .iter()
                .filter(|r| r.unused)
                .map(|r| r.reference.clone())
                .collect();
            if refs.is_empty() || self.removing {
                return;
            }
            self.set_removing(true);
            let handle = std::sync::Arc::new(self.get_thread_handle());
            crate::bridge::spawn(async move {
                let result = runtimes::remove(refs).await;
                handle.invoke_mut(move |m| {
                    m.set_removing(false);
                    m.refresh();
                    match result {
                        Ok(()) => m.removal_finished(true, String::new()),
                        Err(e) => {
                            kerr!("RuntimesModel: removing unused runtimes failed: {}", e);
                            m.removal_finished(false, e)
                        }
                    }
                });
            });
        }
    }
}

notify_setters!(RuntimesModel {
    set_loading: loading: bool => loading_changed,
    set_removing: removing: bool => removing_changed,
    set_count: count: i32 => count_changed,
    set_error_message: error_message: String => error_message_changed,
});

qml_element!(RuntimesModel, "RuntimesModel");

impl qtbridge::QModelItem for RuntimeRow {
    type T0 = String;
    type T1 = String;
    type T2 = String;
    type T3 = String;
    type T4 = bool;
    type T5 = String;
    type T6 = String;
    type T7 = ();
    type T8 = ();
    type T9 = ();
    type T10 = ();
    type T11 = ();
    type T12 = ();
    type T13 = ();
    type T14 = ();
    fn get0(&self) -> Option<&String> {
        Some(&self.reference)
    }
    fn get1(&self) -> Option<&String> {
        Some(&self.name)
    }
    fn get2(&self) -> Option<&String> {
        Some(&self.size)
    }
    fn get3(&self) -> Option<&String> {
        Some(&self.used_by)
    }
    fn get4(&self) -> Option<&bool> {
        Some(&self.unused)
    }
    fn get5(&self) -> Option<&String> {
        Some(&self.eol)
    }
    fn get6(&self) -> Option<&String> {
        Some(&self.scope)
    }
    fn role_names() -> std::collections::HashMap<i32, String> {
        [
            (256, "reference".into()),
            (257, "name".into()),
            (258, "size".into()),
            (259, "usedBy".into()),
            (260, "unused".into()),
            (261, "eol".into()),
            (262, "scope".into()),
        ]
        .into()
    }
    fn get_role(&self, role: i32) -> qtbridge::qtbridge_type_lib::QVariant {
        use qtbridge::QVariantConvertible;
        match role {
            256 => self.reference.to_qvariant(),
            257 => self.name.to_qvariant(),
            258 => self.size.to_qvariant(),
            259 => self.used_by.to_qvariant(),
            260 => self.unused.to_qvariant(),
            261 => self.eol.to_qvariant(),
            262 => self.scope.to_qvariant(),
            _ => Default::default(),
        }
    }
}
