//! `InstalledModel`: installed apps/runtimes and the update selection.
//!
//! Rows come from [`InstalledView`]; installed/update state comes from the
//! shared registry, so changes made by a detail page, the daemon or another
//! window show up here. Progress values are the registry's real job
//! progress (0…1).

use std::sync::Arc;

use qtbridge::qobject;
use tokio_util::sync::CancellationToken;

use crate::view::installed::{InstalledEntry, InstalledView};

pub use object::InstalledModel;

#[derive(Debug, Default, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct InstalledItem {
    pub name: String,
    pub appId: String,
    pub version: String,
    pub size: String,
    pub origin: String,
    pub hasUpdate: bool,
    pub isChecked: bool,
    pub isRuntime: bool,
    pub sectionGroup: String,
}

impl From<&InstalledEntry> for InstalledItem {
    fn from(e: &InstalledEntry) -> Self {
        Self {
            name: e.name.clone(),
            appId: e.app_id.clone(),
            version: e.version.clone(),
            size: e.size.clone(),
            origin: e.origin.clone(),
            hasUpdate: e.has_update,
            isChecked: e.is_checked_for_update,
            isRuntime: e.is_runtime,
            sectionGroup: if e.has_update { "updates" } else { "uptodate" }.into(),
        }
    }
}

#[qobject(Base = QListModel, NoQmlElement)]
mod object {
    use qtbridge::{QListModel, QListModelBase, QmlObject};

    use super::*;
    use crate::view::ops;

    #[derive(Default)]
    pub struct InstalledModel {
        pub(super) view: InstalledView,
        pub(super) rows: Vec<InstalledItem>,
        pub(super) loading: bool,
        pub(super) installed_error: String,
        pub(super) updates_error: String,
        pub(super) uninstall_error: String,
        pub(super) uninstall_control: Option<Arc<crate::bridge::util::JobControl>>,
        pub(super) update_control: Option<Arc<crate::bridge::util::JobControl>>,
        pub(super) count: i32,
        pub(super) uninstalling_app_id: String,
        pub(super) uninstall_progress: f64,
        pub(super) checking_updates: bool,
        pub(super) updating: bool,
        pub(super) update_progress: f64,
        pub(super) update_status_text: String,
        pub(super) updates_available_count: i32,
        pub(super) updates_checked_count: i32,
        pub(super) show_runtimes: bool,
        pub(super) watch: Option<CancellationToken>,
    }

    impl Drop for InstalledModel {
        fn drop(&mut self) {
            if let Some(w) = &self.watch {
                w.cancel();
            }
        }
    }

    impl QListModel for InstalledModel {
        type Item = InstalledItem;
        fn len(&self) -> usize {
            self.rows.len()
        }
        fn get(&self, index: usize) -> Option<&InstalledItem> {
            self.rows.get(index)
        }
        fn set_unnotified(&mut self, index: usize, value: InstalledItem) -> bool {
            match self.rows.get_mut(index) {
                Some(row) => {
                    *row = value;
                    true
                }
                None => false,
            }
        }
        fn reset_unnotified(&mut self) {
            self.rows = (0..self.view.len())
                .filter_map(|r| self.view.row(r).map(InstalledItem::from))
                .collect();
        }
    }

    impl InstalledModel {
        qproperty!(
            "installed_error",
            Read = get_installed_error,
            Notify = installed_error_changed
        );
        fn get_installed_error(&self) -> String {
            self.installed_error.clone()
        }
        #[qsignal(qml_name = "installed_errorChanged")]
        pub fn installed_error_changed(&mut self);
        qproperty!(
            "updates_error",
            Read = get_updates_error,
            Write = set_updates_error,
            Notify = updates_error_changed
        );
        fn get_updates_error(&self) -> String {
            self.updates_error.clone()
        }
        pub fn set_updates_error(&mut self, error: String) {
            if self.updates_error != error {
                self.updates_error = error;
                self.updates_error_changed();
            }
        }
        qproperty!(
            "uninstall_error",
            Read = get_uninstall_error,
            Notify = uninstall_error_changed
        );
        fn get_uninstall_error(&self) -> String {
            self.uninstall_error.clone()
        }
        #[qsignal(qml_name = "updates_errorChanged")]
        pub fn updates_error_changed(&mut self);
        #[qsignal(qml_name = "uninstall_errorChanged")]
        pub fn uninstall_error_changed(&mut self);
        #[qslot(qml_name = "cancelUninstall")]
        fn cancel_uninstall(&mut self) {
            if let Some(c) = &self.uninstall_control {
                c.cancel();
            }
        }
        #[qslot(qml_name = "cancelUpdate")]
        fn cancel_update(&mut self) {
            if let Some(c) = &self.update_control {
                c.cancel();
            }
        }
        qproperty!("loading", Read = get_loading, Notify = loading_changed);
        qproperty!("count", Read = get_count, Notify = count_changed);
        qproperty!(
            "uninstalling_app_id",
            Read = get_uninstalling_app_id,
            Notify = uninstalling_app_id_changed
        );
        qproperty!(
            "uninstall_progress",
            Read = get_uninstall_progress,
            Notify = uninstall_progress_changed
        );
        qproperty!(
            "checking_updates",
            Read = get_checking_updates,
            Notify = checking_updates_changed
        );
        qproperty!("updating", Read = get_updating, Notify = updating_changed);
        qproperty!(
            "update_progress",
            Read = get_update_progress,
            Notify = update_progress_changed
        );
        qproperty!(
            "update_status_text",
            Read = get_update_status_text,
            Notify = update_status_text_changed
        );
        qproperty!(
            "updates_available_count",
            Read = get_updates_available_count,
            Notify = updates_available_count_changed
        );
        qproperty!(
            "updates_checked_count",
            Read = get_updates_checked_count,
            Notify = updates_checked_count_changed
        );
        qproperty!(
            "show_runtimes",
            Read = get_show_runtimes,
            Notify = show_runtimes_changed
        );

        fn get_loading(&self) -> bool {
            self.loading
        }
        fn get_count(&self) -> i32 {
            self.count
        }
        fn get_uninstalling_app_id(&self) -> String {
            self.uninstalling_app_id.clone()
        }
        fn get_uninstall_progress(&self) -> f64 {
            self.uninstall_progress
        }
        fn get_checking_updates(&self) -> bool {
            self.checking_updates
        }
        fn get_updating(&self) -> bool {
            self.updating
        }
        fn get_update_progress(&self) -> f64 {
            self.update_progress
        }
        fn get_update_status_text(&self) -> String {
            self.update_status_text.clone()
        }
        fn get_updates_available_count(&self) -> i32 {
            self.updates_available_count
        }
        fn get_updates_checked_count(&self) -> i32 {
            self.updates_checked_count
        }
        fn get_show_runtimes(&self) -> bool {
            self.show_runtimes
        }

        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);
        #[qsignal(qml_name = "countChanged")]
        pub fn count_changed(&mut self);
        #[qsignal(qml_name = "uninstalling_app_idChanged")]
        pub fn uninstalling_app_id_changed(&mut self);
        #[qsignal(qml_name = "uninstall_progressChanged")]
        pub fn uninstall_progress_changed(&mut self);
        #[qsignal(qml_name = "checking_updatesChanged")]
        pub fn checking_updates_changed(&mut self);
        #[qsignal(qml_name = "updatingChanged")]
        pub fn updating_changed(&mut self);
        #[qsignal(qml_name = "update_progressChanged")]
        pub fn update_progress_changed(&mut self);
        #[qsignal(qml_name = "update_status_textChanged")]
        pub fn update_status_text_changed(&mut self);
        #[qsignal(qml_name = "updates_available_countChanged")]
        pub fn updates_available_count_changed(&mut self);
        #[qsignal(qml_name = "updates_checked_countChanged")]
        pub fn updates_checked_count_changed(&mut self);
        #[qsignal(qml_name = "show_runtimesChanged")]
        pub fn show_runtimes_changed(&mut self);

        /// Reload installed refs; the model follows the registry afterwards.
        #[qslot]
        fn refresh(&mut self) {
            self.set_loading(true);
            self.ensure_watch();
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let error = ops::reload_installed().await.err().unwrap_or_default();
                handle.invoke_mut(|m| {
                    m.sync_registry();
                    m.set_installed_error(error);
                    m.set_loading(false);
                });
            });
        }

        #[qslot(qml_name = "checkForUpdates")]
        fn check_for_updates(&mut self) {
            if self.checking_updates {
                return;
            }
            self.set_checking_updates(true);
            self.ensure_watch();
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let error = ops::reload_updates().await.err().unwrap_or_default();
                handle.invoke_mut(|m| {
                    m.sync_registry();
                    m.set_updates_error(error);
                    m.set_checking_updates(false);
                });
            });
        }

        #[qslot(qml_name = "sortModel")]
        fn sort_model(&mut self, criterion: String, ascending: bool) {
            self.view.sort(&criterion, ascending);
            self.publish();
        }

        #[qslot(qml_name = "applySearchFilter")]
        fn apply_search_filter(&mut self, filter: String) {
            let (updates, runtimes) = (self.view.updates_only, self.view.show_runtimes);
            self.view.set_filter(&filter, updates, runtimes);
            self.publish();
        }

        #[qslot(qml_name = "applyShowUpdatesOnly")]
        fn apply_show_updates_only(&mut self, enabled: bool) {
            let search = self.view.search.clone();
            let runtimes = self.view.show_runtimes;
            self.view.set_filter(&search, enabled, runtimes);
            self.publish();
        }

        #[qslot(qml_name = "applyShowRuntimes")]
        fn apply_show_runtimes(&mut self, enabled: bool) {
            let search = self.view.search.clone();
            let updates = self.view.updates_only;
            self.view.set_filter(&search, updates, enabled);
            self.publish();
            self.set_show_runtimes(enabled);
        }

        /// Flip one row's checkbox: a `dataChanged` for that row, no reset.
        #[qslot(qml_name = "toggleUpdateChecked")]
        fn toggle_update_checked(&mut self, index: i32) {
            let Ok(row) = usize::try_from(index) else {
                return;
            };
            if self.view.toggle_checked(row)
                && let Some(item) = self.view.row(row).map(InstalledItem::from)
            {
                self.set(row, item);
                self.sync_counts();
            }
        }

        #[qslot(qml_name = "setAllUpdatesChecked")]
        fn set_all_updates_checked(&mut self, checked: bool) {
            for row in self.view.set_all_checked(checked) {
                if let Some(item) = self.view.row(row).map(InstalledItem::from) {
                    self.set(row, item);
                }
            }
            self.sync_counts();
        }

        #[qslot(qml_name = "updateSelectedApps")]
        fn update_selected_apps(&mut self) {
            let ids = self.view.selected_updates();
            if ids.is_empty() || self.updating {
                return;
            }
            self.set_update_progress(0.0);
            self.set_update_status_text(kiosque_core::l10n::i18n("Initializing update…", &[]));
            self.set_updating(true);
            self.set_updates_error(String::new());
            let control = crate::bridge::util::JobControl::new();
            self.update_control = Some(control.clone());
            crate::bridge::util::run_job(
                self.get_thread_handle(),
                kiosque_core::flatpak::Operation::Update { ids },
                "Update".into(),
                control,
                |m, fraction, status| {
                    m.set_update_progress(fraction);
                    m.set_update_status_text(status);
                },
                |m, result| {
                    m.update_control = None;
                    m.sync_registry();
                    let error = match result {
                        crate::bridge::util::JobOutcome::Done => String::new(),
                        crate::bridge::util::JobOutcome::Failed(e) => e,
                        crate::bridge::util::JobOutcome::Cancelled => String::new(),
                    };
                    m.set_updates_error(error);
                    m.set_update_progress(0.0);
                    m.set_update_status_text(String::new());
                    m.set_updating(false);
                },
            );
        }
        #[qslot(qml_name = "uninstallApp")]
        fn uninstall_app(&mut self, index: i32) {
            if !self.uninstalling_app_id.is_empty() {
                return;
            }
            let Some(id) = self.app_id_at(index) else {
                return;
            };
            self.set_uninstall_error(String::new());
            self.set_uninstall_progress(-1.0);
            self.set_uninstalling_app_id(id.clone());
            let control = crate::bridge::util::JobControl::new();
            self.uninstall_control = Some(control.clone());
            crate::bridge::util::run_job(
                self.get_thread_handle(),
                kiosque_core::flatpak::Operation::Uninstall {
                    id: id.clone(),
                    scope: None,
                },
                id,
                control,
                |m, fraction, _| m.set_uninstall_progress(fraction),
                |m, result| {
                    m.uninstall_control = None;
                    m.sync_registry();
                    let error = match result {
                        crate::bridge::util::JobOutcome::Done => String::new(),
                        crate::bridge::util::JobOutcome::Failed(e) => e,
                        crate::bridge::util::JobOutcome::Cancelled => String::new(),
                    };
                    m.set_uninstall_error(error);
                    m.set_uninstall_progress(0.0);
                    m.set_uninstalling_app_id(String::new());
                },
            );
        }

        #[qslot(qml_name = "launchApp")]
        fn launch_app(&mut self, index: i32) {
            let Some(id) = self.app_id_at(index) else {
                return;
            };
            crate::bridge::spawn(async move {
                if let Err(e) = ops::launch(&id).await {
                    kerr!("InstalledModel::launch_app: {}", e);
                }
            });
        }
    }

    impl InstalledModel {
        fn app_id_at(&self, index: i32) -> Option<String> {
            let row = usize::try_from(index).ok()?;
            self.view.row(row).map(|e| e.app_id.clone())
        }

        /// Reset the rows from the view, then publish derived counters.
        fn publish(&mut self) {
            self.reset();
            self.set_count(self.rows.len() as i32);
            self.sync_counts();
        }

        fn sync_counts(&mut self) {
            let (available, checked) = self.view.update_counts();
            self.set_updates_available_count(available as i32);
            self.set_updates_checked_count(checked as i32);
        }

        fn sync_registry(&mut self) {
            self.view.set_items(ops::installed_entries());
            self.publish();
        }

        /// Follow registry changes for the lifetime of this object.
        fn ensure_watch(&mut self) {
            if self.watch.is_some() {
                return;
            }
            let cancel = CancellationToken::new();
            self.watch = Some(cancel.clone());
            let mut events = crate::services::services().registry.subscribe();
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                use kiosque_core::registry::Event;
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        event = events.recv() => match event {
                            Ok(Event::Installed | Event::Updates) => {
                                handle.invoke_mut(|m| m.sync_registry());
                            }
                            Ok(Event::Job(_)) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                handle.invoke_mut(|m| m.sync_registry());
                            }
                            Err(_) => break,
                        }
                    }
                }
            });
        }
    }
}

notify_setters!(InstalledModel {
    set_installed_error: installed_error: String => installed_error_changed,
    set_uninstall_error: uninstall_error: String => uninstall_error_changed,
    set_loading: loading: bool => loading_changed,
    set_count: count: i32 => count_changed,
    set_uninstalling_app_id: uninstalling_app_id: String => uninstalling_app_id_changed,
    set_uninstall_progress: uninstall_progress: f64 => uninstall_progress_changed,
    set_checking_updates: checking_updates: bool => checking_updates_changed,
    set_updating: updating: bool => updating_changed,
    set_update_progress: update_progress: f64 => update_progress_changed,
    set_update_status_text: update_status_text: String => update_status_text_changed,
    set_updates_available_count: updates_available_count: i32 => updates_available_count_changed,
    set_updates_checked_count: updates_checked_count: i32 => updates_checked_count_changed,
    set_show_runtimes: show_runtimes: bool => show_runtimes_changed,
});

qml_element!(InstalledModel, "InstalledModel");

// Keep the existing Qt.UserRole numbers as well as the QML role names.
impl qtbridge::QModelItem for InstalledItem {
    type T0 = String;
    type T1 = String;
    type T2 = String;
    type T3 = String;
    type T4 = String;
    type T5 = bool;
    type T6 = bool;
    type T7 = bool;
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
        Some(&self.appId)
    }
    fn get2(&self) -> Option<&String> {
        Some(&self.version)
    }
    fn get3(&self) -> Option<&String> {
        Some(&self.size)
    }
    fn get4(&self) -> Option<&String> {
        Some(&self.origin)
    }
    fn get5(&self) -> Option<&bool> {
        Some(&self.hasUpdate)
    }
    fn get6(&self) -> Option<&bool> {
        Some(&self.isChecked)
    }
    fn get7(&self) -> Option<&bool> {
        Some(&self.isRuntime)
    }
    fn get8(&self) -> Option<&String> {
        Some(&self.sectionGroup)
    }
    fn role_names() -> std::collections::HashMap<i32, String> {
        [
            (256, "name".into()),
            (257, "appId".into()),
            (258, "version".into()),
            (259, "size".into()),
            (260, "origin".into()),
            (261, "hasUpdate".into()),
            (262, "isChecked".into()),
            (264, "isRuntime".into()),
            (263, "sectionGroup".into()),
        ]
        .into()
    }
    fn get_role(&self, role: i32) -> qtbridge::qtbridge_type_lib::QVariant {
        match role {
            256 => qtbridge::QVariantConvertible::to_qvariant(&self.name),
            257 => qtbridge::QVariantConvertible::to_qvariant(&self.appId),
            258 => qtbridge::QVariantConvertible::to_qvariant(&self.version),
            259 => qtbridge::QVariantConvertible::to_qvariant(&self.size),
            260 => qtbridge::QVariantConvertible::to_qvariant(&self.origin),
            261 => qtbridge::QVariantConvertible::to_qvariant(&self.hasUpdate),
            262 => qtbridge::QVariantConvertible::to_qvariant(&self.isChecked),
            264 => qtbridge::QVariantConvertible::to_qvariant(&self.isRuntime),
            263 => qtbridge::QVariantConvertible::to_qvariant(&self.sectionGroup),
            _ => Default::default(),
        }
    }
}
