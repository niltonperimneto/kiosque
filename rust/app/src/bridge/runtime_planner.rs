//! `RuntimePlanner`: what an install or removal does with runtimes, asked
//! before the user confirms it. One instance per page or dialog.

pub use object::RuntimePlanner;
use qtbridge::qobject;

#[qobject(NoQmlElement)]
mod object {
    use qtbridge::QmlObject;

    use crate::bridge::util::{QVariantList, QVariantMap};
    use crate::view::runtimes;

    pub struct RuntimePlanner {
        pub(super) loading: bool,
        pub(super) error_message: String,
        /// Install plan summary (see `view::runtimes::install_summary`).
        pub(super) summary: QVariantMap,
        /// Runtimes a removal would leave unused: `[{reference, name, size}]`.
        pub(super) leftovers: QVariantList,
        pub(super) removing: bool,
        /// Plan request generation: superseded answers are dropped.
        pub(super) generation: u64,
    }

    impl Default for RuntimePlanner {
        fn default() -> Self {
            Self {
                loading: false,
                error_message: String::new(),
                summary: serde_json::json!({}),
                leftovers: serde_json::json!([]),
                removing: false,
                generation: 0,
            }
        }
    }

    impl RuntimePlanner {
        qproperty!("loading", Read = get_loading, Notify = loading_changed);
        fn get_loading(&self) -> bool {
            self.loading
        }
        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);

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

        qproperty!("summary", Read = get_summary, Notify = summary_changed);
        fn get_summary(&self) -> serde_json::Value {
            self.summary.clone()
        }
        #[qsignal(qml_name = "summaryChanged")]
        pub fn summary_changed(&mut self);

        qproperty!(
            "leftovers",
            Read = get_leftovers,
            Notify = leftovers_changed
        );
        fn get_leftovers(&self) -> serde_json::Value {
            self.leftovers.clone()
        }
        #[qsignal(qml_name = "leftoversChanged")]
        pub fn leftovers_changed(&mut self);

        qproperty!("removing", Read = get_removing, Notify = removing_changed);
        fn get_removing(&self) -> bool {
            self.removing
        }
        #[qsignal(qml_name = "removingChanged")]
        pub fn removing_changed(&mut self);

        #[qsignal(qml_name = "removalFinished")]
        pub fn removal_finished(&mut self, success: bool, error: String);

        /// Forget the last plan (and ignore any answer still on its way).
        #[qslot]
        pub fn clear(&mut self) {
            self.generation += 1;
            self.summary = serde_json::json!({});
            self.summary_changed();
            self.leftovers = serde_json::json!([]);
            self.leftovers_changed();
            self.set_error_message(String::new());
            self.set_loading(false);
        }

        /// Plan installing `app_id` from `remote_hint`; `runtime` is the full
        /// ref of the app's runtime from its metadata, if known.
        #[qslot(qml_name = "planInstall")]
        pub fn plan_install(&mut self, app_id: String, remote_hint: String, runtime: String) {
            let generation = self.begin();
            let handle = std::sync::Arc::new(self.get_thread_handle());
            crate::bridge::spawn(async move {
                let result = runtimes::plan_install(&app_id, &remote_hint)
                    .await
                    .map(|plan| runtimes::install_summary(&plan, &runtime));
                handle.invoke_mut(move |p| {
                    if p.generation != generation {
                        return;
                    }
                    match result {
                        Ok(summary) => {
                            p.summary = summary;
                            p.summary_changed();
                        }
                        Err(e) => p.fail(e),
                    }
                    p.set_loading(false);
                });
            });
        }

        /// Plan removing `app_id`, listing the runtimes it would leave unused.
        #[qslot(qml_name = "planRemoval")]
        pub fn plan_removal(&mut self, app_id: String) {
            let generation = self.begin();
            let handle = std::sync::Arc::new(self.get_thread_handle());
            crate::bridge::spawn(async move {
                let result = runtimes::plan_removal(&app_id)
                    .await
                    .map(|plan| runtimes::removal_leftovers(&plan));
                handle.invoke_mut(move |p| {
                    if p.generation != generation {
                        return;
                    }
                    match result {
                        Ok(leftovers) => {
                            p.leftovers = leftovers.into();
                            p.leftovers_changed();
                        }
                        Err(e) => p.fail(e),
                    }
                    p.set_loading(false);
                });
            });
        }

        /// Queue removal of `refs` (runtime refs) after whatever is queued.
        #[qslot(qml_name = "removeRuntimes")]
        pub fn remove_runtimes(&mut self, refs: Vec<String>) {
            if refs.is_empty() {
                return;
            }
            self.set_removing(true);
            let handle = std::sync::Arc::new(self.get_thread_handle());
            crate::bridge::spawn(async move {
                let result = runtimes::remove(refs).await;
                if let Err(e) = &result {
                    kerr!("RuntimePlanner: removing runtimes failed: {}", e);
                }
                handle.invoke_mut(move |p| {
                    p.set_removing(false);
                    match result {
                        Ok(()) => p.removal_finished(true, String::new()),
                        Err(e) => p.removal_finished(false, e),
                    }
                });
            });
        }
    }

    impl RuntimePlanner {
        fn begin(&mut self) -> u64 {
            self.clear();
            self.set_loading(true);
            self.generation
        }

        fn fail(&mut self, error: String) {
            kerr!("RuntimePlanner: planning failed: {}", error);
            self.set_error_message(error);
        }
    }
}

notify_setters!(RuntimePlanner {
    set_loading: loading: bool => loading_changed,
    set_error_message: error_message: String => error_message_changed,
    set_removing: removing: bool => removing_changed,
});

qml_element!(RuntimePlanner, "RuntimePlanner");
