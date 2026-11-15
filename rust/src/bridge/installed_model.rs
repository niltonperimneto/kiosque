use cxx_qt::{CxxQtType, Threading};

use crate::bridge::util::{self, JobControl, JobOutcome};
use crate::flatpak::Operation;
use std::sync::Arc;

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
        #[qproperty(QString, uninstalling_app_id)]
        #[qproperty(f64, uninstall_progress)]
        #[qproperty(bool, checking_updates)]
        #[qproperty(bool, updating)]
        #[qproperty(f64, update_progress)]
        #[qproperty(QString, update_status_text)]
        // Reactive counters — QML bindings update automatically when these change
        #[qproperty(i32, updates_available_count)]
        #[qproperty(i32, updates_checked_count)]
        // Model-side filter state
        #[qproperty(bool, show_runtimes)]
        type InstalledModel = super::InstalledModelRust;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &InstalledModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "rowCount"]
        #[cxx_override]
        fn row_count(self: &InstalledModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_name = "roleNames"]
        #[cxx_override]
        fn role_names(self: &InstalledModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn refresh(self: Pin<&mut InstalledModel>);

        #[qinvokable]
        #[cxx_name = "uninstallApp"]
        fn uninstall_app(self: Pin<&mut InstalledModel>, index: i32);

        #[qinvokable]
        #[cxx_name = "launchApp"]
        fn launch_app(self: Pin<&mut InstalledModel>, index: i32);

        #[qinvokable]
        #[cxx_name = "sortModel"]
        fn sort_model(self: Pin<&mut InstalledModel>, criterion: QString, ascending: bool);

        #[qinvokable]
        #[cxx_name = "checkForUpdates"]
        fn check_for_updates(self: Pin<&mut InstalledModel>);

        #[qinvokable]
        #[cxx_name = "toggleUpdateChecked"]
        fn toggle_update_checked(self: Pin<&mut InstalledModel>, index: i32);

        #[qinvokable]
        #[cxx_name = "updateSelectedApps"]
        fn update_selected_apps(self: Pin<&mut InstalledModel>);

        /// Cancel the running uninstall via `Transactions::cancel(job_id)`.
        #[qinvokable]
        #[cxx_name = "cancelUninstall"]
        fn cancel_uninstall(self: Pin<&mut InstalledModel>);

        /// Cancel the running update via `Transactions::cancel(job_id)`.
        #[qinvokable]
        #[cxx_name = "cancelUpdate"]
        fn cancel_update(self: Pin<&mut InstalledModel>);

        #[qinvokable]
        #[cxx_name = "setAllUpdatesChecked"]
        fn set_all_updates_checked(self: Pin<&mut InstalledModel>, checked: bool);

        // Filter setters — call begin/endResetModel around the state change
        #[qinvokable]
        #[cxx_name = "applySearchFilter"]
        fn apply_search_filter(self: Pin<&mut InstalledModel>, filter: QString);

        #[qinvokable]
        #[cxx_name = "applyShowUpdatesOnly"]
        fn apply_show_updates_only(self: Pin<&mut InstalledModel>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyShowRuntimes"]
        fn apply_show_runtimes(self: Pin<&mut InstalledModel>, enabled: bool);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut InstalledModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut InstalledModel>);
    }

    impl cxx_qt::Threading for InstalledModel {}
}

pub use kiosque_core::entries::InstalledEntry;

pub struct InstalledModelRust {
    pub items: Vec<InstalledEntry>,
    pub current_sort_criterion: String,
    pub current_sort_ascending: bool,
    pub loading: bool,
    pub uninstalling_app_id: cxx_qt_lib::QString,
    pub uninstall_progress: f64,
    pub checking_updates: bool,
    pub updating: bool,
    pub update_progress: f64,
    pub update_status_text: cxx_qt_lib::QString,
    pub updates_available_count: i32,
    pub updates_checked_count: i32,
    // Filter state (not exposed as qproperty — set via invokables)
    pub search_filter: String,
    pub show_updates_only: bool,
    // show_runtimes is a qproperty so the section toggle in QML can bind to it
    pub show_runtimes: bool,
    pub filtered_indices_cache: std::cell::RefCell<Option<Vec<usize>>>,
    watch: Option<tokio_util::sync::CancellationToken>,
    uninstall_job: Option<Arc<JobControl>>,
    update_job: Option<Arc<JobControl>>,
}

impl Default for InstalledModelRust {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            watch: None,
            current_sort_criterion: "name".to_string(),
            current_sort_ascending: true,
            loading: false,
            uninstalling_app_id: cxx_qt_lib::QString::from(""),
            uninstall_progress: 0.0,
            checking_updates: false,
            updating: false,
            update_progress: 0.0,
            update_status_text: cxx_qt_lib::QString::from(""),
            updates_available_count: 0,
            updates_checked_count: 0,
            search_filter: String::new(),
            show_updates_only: false,
            show_runtimes: false,
            filtered_indices_cache: std::cell::RefCell::new(None),
            uninstall_job: None,
            update_job: None,
        }
    }
}

impl Drop for InstalledModelRust {
    fn drop(&mut self) {
        if let Some(watch) = &self.watch {
            watch.cancel();
        }
    }
}

impl qobject::InstalledModel {
    pub const NAME_ROLE: i32 = 256;
    pub const APP_ID_ROLE: i32 = 257;
    pub const VERSION_ROLE: i32 = 258;
    pub const SIZE_ROLE: i32 = 259;
    pub const ORIGIN_ROLE: i32 = 260;
    pub const HAS_UPDATE_ROLE: i32 = 261;
    pub const IS_CHECKED_ROLE: i32 = 262;
    pub const IS_RUNTIME_ROLE: i32 = 263;
    pub const SECTION_GROUP_ROLE: i32 = 264;

    /// Returns the indices into `self.items` that pass the current filter.
    fn filtered_indices(&self) -> Vec<usize> {
        if let Ok(cache) = self.filtered_indices_cache.try_borrow() {
            if let Some(ref indices) = *cache {
                return indices.clone();
            }
        }

        let filter = self.search_filter.to_lowercase();
        let updates_only = self.show_updates_only;
        let show_runtimes = self.show_runtimes;

        let indices: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                // Runtimes are hidden unless the user has toggled them on
                if item.is_runtime && !show_runtimes {
                    return false;
                }
                let matches_search = filter.is_empty()
                    || item.name.to_lowercase().contains(&filter)
                    || item.app_id.to_lowercase().contains(&filter);
                let matches_updates = !updates_only || item.has_update;
                matches_search && matches_updates
            })
            .map(|(i, _)| i)
            .collect();

        if let Ok(mut cache) = self.filtered_indices_cache.try_borrow_mut() {
            *cache = Some(indices.clone());
        }
        indices
    }

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        if !index.is_valid() {
            return cxx_qt_lib::QVariant::default();
        }
        let filtered = self.filtered_indices();
        let row = index.row() as usize;
        let actual_idx = match filtered.get(row) {
            Some(&i) => i,
            None => return cxx_qt_lib::QVariant::default(),
        };
        let item = match self.items.get(actual_idx) {
            Some(i) => i,
            None => return cxx_qt_lib::QVariant::default(),
        };

        match role {
            Self::NAME_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.name)),
            Self::APP_ID_ROLE => {
                cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.app_id))
            }
            Self::VERSION_ROLE => {
                cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.version))
            }
            Self::SIZE_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.size)),
            Self::ORIGIN_ROLE => {
                cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.origin))
            }
            Self::HAS_UPDATE_ROLE => cxx_qt_lib::QVariant::from(&item.has_update),
            Self::IS_CHECKED_ROLE => cxx_qt_lib::QVariant::from(&item.is_checked_for_update),
            Self::IS_RUNTIME_ROLE => cxx_qt_lib::QVariant::from(&item.is_runtime),
            Self::SECTION_GROUP_ROLE => {
                let group = if item.has_update {
                    "updates"
                } else {
                    "uptodate"
                };
                cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(group))
            }
            _ => cxx_qt_lib::QVariant::default(),
        }
    }

    pub fn row_count(&self, parent: &cxx_qt_lib::QModelIndex) -> i32 {
        if parent.is_valid() {
            return 0;
        }
        self.filtered_indices().len() as i32
    }

    pub fn role_names(&self) -> cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray> {
        let mut roles = cxx_qt_lib::QHash::<cxx_qt_lib::QHashPair_i32_QByteArray>::default();
        roles.insert(Self::NAME_ROLE, cxx_qt_lib::QByteArray::from("name"));
        roles.insert(Self::APP_ID_ROLE, cxx_qt_lib::QByteArray::from("appId"));
        roles.insert(Self::VERSION_ROLE, cxx_qt_lib::QByteArray::from("version"));
        roles.insert(Self::SIZE_ROLE, cxx_qt_lib::QByteArray::from("size"));
        roles.insert(Self::ORIGIN_ROLE, cxx_qt_lib::QByteArray::from("origin"));
        roles.insert(
            Self::HAS_UPDATE_ROLE,
            cxx_qt_lib::QByteArray::from("hasUpdate"),
        );
        roles.insert(
            Self::IS_CHECKED_ROLE,
            cxx_qt_lib::QByteArray::from("isChecked"),
        );
        roles.insert(
            Self::IS_RUNTIME_ROLE,
            cxx_qt_lib::QByteArray::from("isRuntime"),
        );
        roles.insert(
            Self::SECTION_GROUP_ROLE,
            cxx_qt_lib::QByteArray::from("sectionGroup"),
        );
        roles
    }

    /// Sync the `updates_available_count` / `updates_checked_count` qproperties
    /// from `self.items`. Call this after any mutation that can change update state.
    fn sync_update_counts(mut self: std::pin::Pin<&mut Self>) {
        let available = self.items.iter().filter(|i| i.has_update).count() as i32;
        let checked = self
            .items
            .iter()
            .filter(|i| i.has_update && i.is_checked_for_update)
            .count() as i32;
        self.as_mut().set_updates_available_count(available);
        self.as_mut().set_updates_checked_count(checked);
    }

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        if self.watch.is_none() {
            let cancel = tokio_util::sync::CancellationToken::new();
            self.as_mut().rust_mut().watch = Some(cancel.clone());
            let mut events = crate::services::services().registry.subscribe();
            let thread = self.qt_thread();
            crate::runtime::runtime().spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        event = events.recv() => {
                            if matches!(event, Ok(crate::registry::Event::Installed | crate::registry::Event::Updates)) {
                                if thread.queue(|mut object| {
                                    object.as_mut().sync_registry();
                                }).is_err() { break; }
                            }
                        }
                    }
                }
            });
        }
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let services = crate::services::services();
            if let Err(error) = services.registry.reload_installed(&*services.backend).await {
                kerr!("InstalledModel::refresh: {}", error);
            }
            let _ = qt_thread.queue(|mut object| {
                object.as_mut().sync_registry();
                object.as_mut().set_loading(false);
            });
        });
    }

    fn sync_registry(mut self: std::pin::Pin<&mut Self>) {
        let registry = &crate::services::services().registry;
        let items = registry
            .installed()
            .into_iter()
            .map(|app| {
                let has_update = registry.state(&app.id).update.is_some();
                let is_runtime = app.is_runtime();
                InstalledEntry {
                    name: app.name,
                    app_id: app.id,
                    version: app.version,
                    size: format!("{} B", app.installed_size),
                    origin: app.origin,
                    has_update,
                    is_checked_for_update: has_update,
                    is_runtime,
                }
            })
            .collect();
        self.as_mut().begin_reset_model();
        {
            let mut state = self.as_mut().rust_mut();
            state.items = items;
            state.filtered_indices_cache.replace(None);
            let criterion = state.current_sort_criterion.clone();
            let ascending = state.current_sort_ascending;
            sort_items_helper(&mut state.items, &criterion, ascending);
        }
        self.as_mut().end_reset_model();
        self.as_mut().sync_update_counts();
    }

    pub fn sort_model(
        self: std::pin::Pin<&mut Self>,
        criterion: cxx_qt_lib::QString,
        ascending: bool,
    ) {
        let criterion_str = criterion.to_string();
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            rust_mut.current_sort_criterion = criterion_str.clone();
            rust_mut.current_sort_ascending = ascending;
            rust_mut.filtered_indices_cache.replace(None);
            let c = rust_mut.current_sort_criterion.clone();
            sort_items_helper(&mut rust_mut.items, &c, ascending);
        }
        qobject.as_mut().end_reset_model();
    }

    pub fn apply_search_filter(self: std::pin::Pin<&mut Self>, filter: cxx_qt_lib::QString) {
        let filter_str = filter.to_string();
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            rust_mut.search_filter = filter_str;
            rust_mut.filtered_indices_cache.replace(None);
        }
        qobject.as_mut().end_reset_model();
    }

    pub fn apply_show_updates_only(self: std::pin::Pin<&mut Self>, enabled: bool) {
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            rust_mut.show_updates_only = enabled;
            rust_mut.filtered_indices_cache.replace(None);
        }
        qobject.as_mut().end_reset_model();
    }

    pub fn apply_show_runtimes(self: std::pin::Pin<&mut Self>, enabled: bool) {
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            rust_mut.show_runtimes = enabled;
            rust_mut.filtered_indices_cache.replace(None);
        }
        qobject.as_mut().end_reset_model();
        // Keep the qproperty in sync so QML bindings on show_runtimes still work
        qobject.as_mut().set_show_runtimes(enabled);
    }

    pub fn uninstall_app(mut self: std::pin::Pin<&mut Self>, index: i32) {
        let filtered = self.filtered_indices();
        let actual_idx = match filtered.get(index as usize) {
            Some(&i) => i,
            None => {
                kerr!(
                    "InstalledModel::uninstall_app: index {} out of filtered bounds",
                    index
                );
                return;
            }
        };
        let app_id = self.items[actual_idx].app_id.clone();

        self.as_mut()
            .set_uninstalling_app_id(cxx_qt_lib::QString::from(&app_id));
        self.as_mut().set_uninstall_progress(-1.0);

        let qt_thread = self.qt_thread();
        klog!("InstalledModel::uninstall_app: uninstalling \"{}\"", app_id);

        let control = JobControl::new();
        self.as_mut().rust_mut().uninstall_job = Some(control.clone());
        util::run_job(
            qt_thread,
            Operation::Uninstall {
                id: app_id.clone(),
                scope: None,
            },
            String::new(),
            control.clone(),
            |q, p, _| q.set_uninstall_progress(p),
            move |mut q, outcome| {
                if q.uninstall_job
                    .as_ref()
                    .is_some_and(|job| Arc::ptr_eq(job, &control))
                {
                    q.as_mut().rust_mut().uninstall_job = None;
                    q.as_mut()
                        .set_uninstalling_app_id(cxx_qt_lib::QString::from(""));
                    q.as_mut().set_uninstall_progress(0.0);
                }
                match outcome {
                    JobOutcome::Done => {
                        klog!(
                            "InstalledModel::uninstall_app: successfully uninstalled \"{}\"",
                            app_id
                        );
                        q.as_mut().refresh();
                    }
                    JobOutcome::Cancelled => {
                        klog!("InstalledModel::uninstall_app: cancelled \"{}\"", app_id);
                    }
                    JobOutcome::Failed(e) => {
                        kerr!("InstalledModel::uninstall_app: {}", e);
                    }
                }
            },
        );
    }

    pub fn cancel_uninstall(mut self: std::pin::Pin<&mut Self>) {
        if let Some(job) = self.as_mut().rust_mut().uninstall_job.as_ref() {
            job.cancel();
        }
    }

    pub fn cancel_update(mut self: std::pin::Pin<&mut Self>) {
        if let Some(job) = self.as_mut().rust_mut().update_job.as_ref() {
            job.cancel();
        }
    }

    pub fn launch_app(self: std::pin::Pin<&mut Self>, index: i32) {
        let filtered = self.filtered_indices();
        let actual_idx = match filtered.get(index as usize) {
            Some(&i) => i,
            None => {
                kerr!(
                    "InstalledModel::launch_app: index {} out of filtered bounds",
                    index
                );
                return;
            }
        };
        let app_id = self.items[actual_idx].app_id.clone();
        klog!("InstalledModel::launch_app: launching \"{}\"", app_id);
        crate::runtime::runtime().spawn(async move {
            if let Err(e) = crate::flatpak::cli::launch_app(&app_id).await {
                kerr!("InstalledModel::launch_app: {}", e);
            }
        });
    }

    pub fn check_for_updates(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_checking_updates(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let updates = match crate::flatpak::cli::list_updates().await {
                Ok(ids) => {
                    klog!(
                        "InstalledModel::check_for_updates: found {} updates",
                        ids.len()
                    );
                    ids
                }
                Err(e) => {
                    kerr!("InstalledModel::check_for_updates: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().begin_reset_model();
                {
                    let mut rust_mut = qobject.as_mut().rust_mut();
                    rust_mut.filtered_indices_cache.replace(None);

                    let mut found_app_updates = std::collections::HashSet::new();

                    for item in &mut rust_mut.items {
                        if updates.iter().any(|u| u.application_id == item.app_id) {
                            item.has_update = true;
                            item.is_checked_for_update = true;
                            found_app_updates.insert(item.app_id.clone());
                        } else {
                            item.has_update = false;
                            item.is_checked_for_update = false;
                        }
                    }

                    for up in updates {
                        let is_runtime = up
                            .ref_string
                            .as_ref()
                            .map(|r| r.starts_with("runtime/"))
                            .unwrap_or(false);
                        if is_runtime && !found_app_updates.contains(&up.application_id) {
                            if !rust_mut.items.iter().any(|i| i.app_id == up.application_id) {
                                rust_mut.items.push(InstalledEntry {
                                    name: up.name.clone(),
                                    app_id: up.application_id.clone(),
                                    version: up.version.clone().unwrap_or_default(),
                                    size: up.installed_size.clone().unwrap_or_default(),
                                    origin: up.origin.clone().unwrap_or_default(),
                                    has_update: true,
                                    is_checked_for_update: true,
                                    is_runtime: true,
                                });
                            }
                        }
                    }

                    let criterion = rust_mut.current_sort_criterion.clone();
                    let ascending = rust_mut.current_sort_ascending;
                    sort_items_helper(&mut rust_mut.items, &criterion, ascending);
                }
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_checking_updates(false);
                qobject.as_mut().sync_update_counts();
            });
        });
    }

    pub fn toggle_update_checked(self: std::pin::Pin<&mut Self>, index: i32) {
        let filtered = self.filtered_indices();
        let actual_idx = match filtered.get(index as usize) {
            Some(&i) => i,
            None => return,
        };
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            rust_mut.items[actual_idx].is_checked_for_update =
                !rust_mut.items[actual_idx].is_checked_for_update;
        }
        qobject.as_mut().end_reset_model();
        qobject.as_mut().sync_update_counts();
    }

    pub fn set_all_updates_checked(self: std::pin::Pin<&mut Self>, checked: bool) {
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            for item in &mut rust_mut.items {
                if item.has_update {
                    item.is_checked_for_update = checked;
                }
            }
        }
        qobject.as_mut().end_reset_model();
        qobject.as_mut().sync_update_counts();
    }

    pub fn update_selected_apps(mut self: std::pin::Pin<&mut Self>) {
        let app_ids: Vec<String> = self
            .items
            .iter()
            .filter(|item| item.has_update && item.is_checked_for_update)
            .map(|item| item.app_id.clone())
            .collect();

        if app_ids.is_empty() {
            return;
        }

        self.as_mut().set_updating(true);
        self.as_mut().set_update_progress(-1.0);
        self.as_mut()
            .set_update_status_text(cxx_qt_lib::QString::from("Initializing update…"));
        let qt_thread = self.qt_thread();
        let control = JobControl::new();
        self.as_mut().rust_mut().update_job = Some(control.clone());

        util::run_job(
            qt_thread,
            Operation::Update { ids: app_ids },
            String::new(),
            control.clone(),
            |mut q, p, status| {
                q.as_mut().set_update_progress(p);
                if !status.is_empty() {
                    q.as_mut().set_update_status_text(status);
                }
            },
            move |mut q, outcome| {
                if q.update_job
                    .as_ref()
                    .is_some_and(|job| Arc::ptr_eq(job, &control))
                {
                    q.as_mut().rust_mut().update_job = None;
                }
                q.as_mut().set_updating(false);
                q.as_mut().set_update_progress(0.0);
                q.as_mut()
                    .set_update_status_text(cxx_qt_lib::QString::from(""));
                match outcome {
                    JobOutcome::Done => {
                        klog!("InstalledModel::update_selected_apps: successfully updated");
                    }
                    JobOutcome::Cancelled => {
                        klog!("InstalledModel::update_selected_apps: cancelled");
                    }
                    JobOutcome::Failed(e) => {
                        kerr!("InstalledModel::update_selected_apps: {}", e);
                    }
                }
                q.as_mut().refresh();
            },
        );
    }
}

fn parse_size_to_bytes(size_str: &str) -> u64 {
    let size_str = size_str.trim().to_lowercase();
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.is_empty() {
        return 0;
    }
    let num_val: f64 = parts[0].parse().unwrap_or(0.0);
    if parts.len() < 2 {
        return num_val as u64;
    }
    let unit = parts[1];
    if unit.contains("gb") || unit.contains("g") {
        (num_val * 1024.0 * 1024.0 * 1024.0) as u64
    } else if unit.contains("mb") || unit.contains("m") {
        (num_val * 1024.0 * 1024.0) as u64
    } else if unit.contains("kb") || unit.contains("k") {
        (num_val * 1024.0) as u64
    } else {
        num_val as u64
    }
}

fn sort_items_helper(items: &mut [InstalledEntry], criterion: &str, ascending: bool) {
    items.sort_by(|a, b| {
        // Runtimes always sort last regardless of other criteria
        if a.is_runtime != b.is_runtime {
            return if a.is_runtime {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        // Items with updates sort before up-to-date items
        if a.has_update != b.has_update {
            return if a.has_update {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        match criterion {
            "name" | "" => {
                let res = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                if ascending { res } else { res.reverse() }
            }
            "app_id" => {
                let res = a.app_id.to_lowercase().cmp(&b.app_id.to_lowercase());
                if ascending { res } else { res.reverse() }
            }
            "size" => {
                let size_a = parse_size_to_bytes(&a.size);
                let size_b = parse_size_to_bytes(&b.size);
                let res = size_a.cmp(&size_b);
                if ascending { res } else { res.reverse() }
            }
            _ => std::cmp::Ordering::Equal,
        }
    });
}
