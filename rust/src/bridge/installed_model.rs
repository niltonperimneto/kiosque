use cxx_qt::{CxxQtType, Threading};

use crate::bridge::util;

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

pub struct InstalledEntry {
    pub name: String,
    pub app_id: String,
    pub version: String,
    pub size: String,
    pub origin: String,
    pub has_update: bool,
    pub is_checked_for_update: bool,
    pub is_runtime: bool,
}

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
}

impl Default for InstalledModelRust {
    fn default() -> Self {
        Self {
            items: Vec::new(),
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
        let filter = self.search_filter.to_lowercase();
        let updates_only = self.show_updates_only;
        let show_runtimes = self.show_runtimes;

        self.items
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
            .collect()
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
            Self::APP_ID_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.app_id)),
            Self::VERSION_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.version)),
            Self::SIZE_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.size)),
            Self::ORIGIN_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.origin)),
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
        roles.insert(Self::HAS_UPDATE_ROLE, cxx_qt_lib::QByteArray::from("hasUpdate"));
        roles.insert(Self::IS_CHECKED_ROLE, cxx_qt_lib::QByteArray::from("isChecked"));
        roles.insert(Self::IS_RUNTIME_ROLE, cxx_qt_lib::QByteArray::from("isRuntime"));
        roles.insert(Self::SECTION_GROUP_ROLE, cxx_qt_lib::QByteArray::from("sectionGroup"));
        roles
    }

    /// Sync the `updates_available_count` / `updates_checked_count` qproperties
    /// from `self.items`. Call this after any mutation that can change update state.
    fn sync_update_counts(mut self: std::pin::Pin<&mut Self>) {
        let available = self.items.iter().filter(|i| i.has_update).count() as i32;
        let checked = self.items.iter().filter(|i| i.has_update && i.is_checked_for_update).count() as i32;
        self.as_mut().set_updates_available_count(available);
        self.as_mut().set_updates_checked_count(checked);
    }

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let items = match crate::flatpak::cli::list_installed_cached().await {
                Ok(parsed) => {
                    klog!("InstalledModel::refresh: got {} installed apps", parsed.len());
                    parsed.into_iter().map(|app| InstalledEntry {
                        name: app.name,
                        app_id: app.application_id,
                        version: app.version.unwrap_or_default(),
                        size: app.installed_size.unwrap_or_default(),
                        origin: app.origin.unwrap_or_default(),
                        has_update: false,
                        is_checked_for_update: false,
                        is_runtime: false,
                    }).collect()
                }
                Err(e) => {
                    kerr!("InstalledModel::refresh: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                klog!("InstalledModel::refresh: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                {
                    let mut rust_mut = qobject.as_mut().rust_mut();
                    rust_mut.items = items;
                    let criterion = rust_mut.current_sort_criterion.clone();
                    let ascending = rust_mut.current_sort_ascending;
                    sort_items_helper(&mut rust_mut.items, &criterion, ascending);
                }
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
                qobject.as_mut().sync_update_counts();
            });
        });
    }

    pub fn sort_model(self: std::pin::Pin<&mut Self>, criterion: cxx_qt_lib::QString, ascending: bool) {
        let criterion_str = criterion.to_string();
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        {
            let mut rust_mut = qobject.as_mut().rust_mut();
            rust_mut.current_sort_criterion = criterion_str.clone();
            rust_mut.current_sort_ascending = ascending;
            let c = rust_mut.current_sort_criterion.clone();
            sort_items_helper(&mut rust_mut.items, &c, ascending);
        }
        qobject.as_mut().end_reset_model();
    }

    pub fn apply_search_filter(self: std::pin::Pin<&mut Self>, filter: cxx_qt_lib::QString) {
        let filter_str = filter.to_string();
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        qobject.as_mut().rust_mut().search_filter = filter_str;
        qobject.as_mut().end_reset_model();
    }

    pub fn apply_show_updates_only(self: std::pin::Pin<&mut Self>, enabled: bool) {
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        qobject.as_mut().rust_mut().show_updates_only = enabled;
        qobject.as_mut().end_reset_model();
    }

    pub fn apply_show_runtimes(self: std::pin::Pin<&mut Self>, enabled: bool) {
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        qobject.as_mut().rust_mut().show_runtimes = enabled;
        qobject.as_mut().end_reset_model();
        // Keep the qproperty in sync so QML bindings on show_runtimes still work
        qobject.as_mut().set_show_runtimes(enabled);
    }

    pub fn uninstall_app(mut self: std::pin::Pin<&mut Self>, index: i32) {
        let filtered = self.filtered_indices();
        let actual_idx = match filtered.get(index as usize) {
            Some(&i) => i,
            None => {
                kerr!("InstalledModel::uninstall_app: index {} out of filtered bounds", index);
                return;
            }
        };
        let app_id = self.items[actual_idx].app_id.clone();

        self.as_mut().set_uninstalling_app_id(cxx_qt_lib::QString::from(&app_id));
        self.as_mut().set_uninstall_progress(0.01);

        let qt_thread = self.qt_thread();
        klog!("InstalledModel::uninstall_app: uninstalling \"{}\"", app_id);

        let op_app_id = app_id.clone();
        util::run_with_progress(
            qt_thread,
            async move { crate::flatpak::cli::uninstall_app(&op_app_id).await },
            None,
            |q, p| q.set_uninstall_progress(p),
            |_| {},
            move |mut q, res| {
                q.as_mut().set_uninstalling_app_id(cxx_qt_lib::QString::from(""));
                q.as_mut().set_uninstall_progress(0.0);
                match res {
                    Ok(()) => {
                        klog!("InstalledModel::uninstall_app: successfully uninstalled \"{}\"", app_id);
                        q.as_mut().refresh();
                    }
                    Err(e) => {
                        kerr!("InstalledModel::uninstall_app: {}", e);
                    }
                }
            },
        );
    }

    pub fn launch_app(self: std::pin::Pin<&mut Self>, index: i32) {
        let filtered = self.filtered_indices();
        let actual_idx = match filtered.get(index as usize) {
            Some(&i) => i,
            None => {
                kerr!("InstalledModel::launch_app: index {} out of filtered bounds", index);
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
                    klog!("InstalledModel::check_for_updates: found {} updates", ids.len());
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
                        let is_runtime = up.ref_string.as_ref().map(|r| r.starts_with("runtime/")).unwrap_or(false);
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
            rust_mut.items[actual_idx].is_checked_for_update = !rust_mut.items[actual_idx].is_checked_for_update;
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
        let app_ids: Vec<String> = self.items.iter()
            .filter(|item| item.has_update && item.is_checked_for_update)
            .map(|item| item.app_id.clone())
            .collect();

        if app_ids.is_empty() {
            return;
        }

        self.as_mut().set_updating(true);
        self.as_mut().set_update_progress(0.01);
        self.as_mut().set_update_status_text(cxx_qt_lib::QString::from("Initializing update…"));
        let qt_thread = self.qt_thread();
        let qt_thread_status = qt_thread.clone();

        let operation = async move {
            crate::flatpak::cli::update_apps(&app_ids, move |line| {
                let line_qs = cxx_qt_lib::QString::from(&line);
                let _ = qt_thread_status.queue(move |mut qobject| {
                    qobject.as_mut().set_update_status_text(line_qs);
                });
            }).await
        };

        util::run_with_progress(
            qt_thread,
            operation,
            None,
            |q, p| q.set_update_progress(p),
            |_| {},
            |mut q, res| {
                q.as_mut().set_updating(false);
                q.as_mut().set_update_progress(0.0);
                q.as_mut().set_update_status_text(cxx_qt_lib::QString::from(""));
                match res {
                    Ok(()) => {
                        klog!("InstalledModel::update_selected_apps: successfully updated");
                    }
                    Err(e) => {
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
            return if a.is_runtime { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
        }
        // Items with updates sort before up-to-date items
        if a.has_update != b.has_update {
            return if a.has_update { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
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
