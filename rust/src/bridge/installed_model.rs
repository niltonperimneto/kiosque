use cxx_qt::{CxxQtType, Threading};

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

        #[qinvokable]
        #[cxx_name = "updatesAvailableCount"]
        fn updates_available_count(self: &InstalledModel) -> i32;

        #[qinvokable]
        #[cxx_name = "updatesCheckedCount"]
        fn updates_checked_count(self: &InstalledModel) -> i32;
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

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        if !index.is_valid() {
            return cxx_qt_lib::QVariant::default();
        }
        let row = index.row() as usize;
        if let Some(item) = self.items.get(row) {
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
                    let group = if item.is_runtime {
                        "Platform Updates"
                    } else if item.has_update {
                        "Updates Available"
                    } else {
                        "Up to Date"
                    };
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(group))
                }
                _ => cxx_qt_lib::QVariant::default(),
            }
        } else {
            cxx_qt_lib::QVariant::default()
        }
    }

    pub fn row_count(&self, parent: &cxx_qt_lib::QModelIndex) -> i32 {
        if parent.is_valid() {
            return 0;
        }
        self.items.len() as i32
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

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            // Use the cached installed list instead of raw CLI call
            let items = match crate::flatpak::cli::list_installed_cached().await {
                Ok(parsed) => {
                    eprintln!("[kiosque] InstalledModel::refresh: got {} installed apps", parsed.len());
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
                    eprintln!("[kiosque] ERROR InstalledModel::refresh: {}", e);
                    vec![]
                }
            };
            
            let _ = qt_thread.queue(move |mut qobject| {
                eprintln!("[kiosque] InstalledModel::refresh: updating UI with {} items", items.len());
                qobject.as_mut().begin_reset_model();
                let mut rust_mut = qobject.as_mut().rust_mut();
                rust_mut.items = items;
                let criterion = rust_mut.current_sort_criterion.clone();
                let ascending = rust_mut.current_sort_ascending;
                sort_items_helper(&mut rust_mut.items, &criterion, ascending);
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn sort_model(self: std::pin::Pin<&mut Self>, criterion: cxx_qt_lib::QString, ascending: bool) {
        let criterion_str = criterion.to_string();
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        let mut rust_mut = qobject.as_mut().rust_mut();
        rust_mut.current_sort_criterion = criterion_str.clone();
        rust_mut.current_sort_ascending = ascending;
        let criterion = rust_mut.current_sort_criterion.clone();
        sort_items_helper(&mut rust_mut.items, &criterion, ascending);
        qobject.as_mut().end_reset_model();
    }

    pub fn uninstall_app(mut self: std::pin::Pin<&mut Self>, index: i32) {
        if index < 0 || index >= self.items.len() as i32 {
            eprintln!("[kiosque] ERROR InstalledModel::uninstall_app: index {} out of bounds (len {})", index, self.items.len());
            return;
        }
        let app_id = self.items[index as usize].app_id.clone();
        
        self.as_mut().set_uninstalling_app_id(cxx_qt_lib::QString::from(&app_id));
        self.as_mut().set_uninstall_progress(0.01);
        
        let qt_thread = self.qt_thread();
        
        eprintln!("[kiosque] InstalledModel::uninstall_app: uninstalling \"{}\"", app_id);
        crate::runtime::runtime().spawn(async move {
            let qt_thread_prog = qt_thread.clone();
            let mut progress = 0.01;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
            
            let (tx, mut rx) = tokio::sync::oneshot::channel();
            
            let app_id_clone = app_id.clone();
            tokio::spawn(async move {
                let res = crate::flatpak::cli::uninstall_app(&app_id_clone).await;
                let _ = tx.send(res);
            });

            loop {
                tokio::select! {
                    res_opt = &mut rx => {
                        let res = res_opt.unwrap_or_else(|_| Err("Uninstall task panicked".into()));
                        let _ = qt_thread.queue(move |mut qobject| {
                            qobject.as_mut().set_uninstalling_app_id(cxx_qt_lib::QString::from(""));
                            qobject.as_mut().set_uninstall_progress(0.0);
                            match res {
                                Ok(()) => {
                                    eprintln!("[kiosque] InstalledModel::uninstall_app: successfully uninstalled \"{}\"", app_id);
                                    qobject.as_mut().refresh();
                                }
                                Err(e) => {
                                    eprintln!("[kiosque] ERROR InstalledModel::uninstall_app: {}", e);
                                }
                            }
                        });
                        break;
                    }
                    _ = interval.tick() => {
                        progress += (0.95 - progress) * 0.05;
                        let p = progress;
                        let _ = qt_thread_prog.queue(move |mut qobject| {
                            qobject.as_mut().set_uninstall_progress(p);
                        });
                    }
                }
            }
        });
    }

    pub fn launch_app(self: std::pin::Pin<&mut Self>, index: i32) {
        if index < 0 || index >= self.items.len() as i32 {
            eprintln!("[kiosque] ERROR InstalledModel::launch_app: index {} out of bounds (len {})", index, self.items.len());
            return;
        }
        let app_id = self.items[index as usize].app_id.clone();
        eprintln!("[kiosque] InstalledModel::launch_app: launching \"{}\"", app_id);
        crate::runtime::runtime().spawn(async move {
            if let Err(e) = crate::flatpak::cli::launch_app(&app_id).await {
                eprintln!("[kiosque] ERROR InstalledModel::launch_app: {}", e);
            }
        });
    }

    pub fn check_for_updates(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_checking_updates(true);
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let updates = match crate::flatpak::cli::list_updates().await {
                Ok(ids) => {
                    eprintln!("[kiosque] InstalledModel::check_for_updates: found {} updates", ids.len());
                    ids
                }
                Err(e) => {
                    eprintln!("[kiosque] ERROR InstalledModel::check_for_updates: {}", e);
                    vec![]
                }
            };

            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().begin_reset_model();
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
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_checking_updates(false);
            });
        });
    }

    pub fn toggle_update_checked(self: std::pin::Pin<&mut Self>, index: i32) {
        let mut qobject = self;
        if index < 0 || index >= qobject.items.len() as i32 {
            return;
        }
        qobject.as_mut().begin_reset_model();
        let mut rust_mut = qobject.as_mut().rust_mut();
        let item = &mut rust_mut.items[index as usize];
        item.is_checked_for_update = !item.is_checked_for_update;
        qobject.as_mut().end_reset_model();
    }

    pub fn set_all_updates_checked(self: std::pin::Pin<&mut Self>, checked: bool) {
        let mut qobject = self;
        qobject.as_mut().begin_reset_model();
        let mut rust_mut = qobject.as_mut().rust_mut();
        for item in &mut rust_mut.items {
            if item.has_update {
                item.is_checked_for_update = checked;
            }
        }
        qobject.as_mut().end_reset_model();
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
        self.as_mut().set_update_status_text(cxx_qt_lib::QString::from("Initializing update..."));
        let qt_thread = self.qt_thread();

        crate::runtime::runtime().spawn(async move {
            let qt_thread_prog = qt_thread.clone();
            let qt_thread_status = qt_thread.clone();
            let mut progress = 0.01;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
            
            let (tx, mut rx) = tokio::sync::oneshot::channel();
            let app_ids_clone = app_ids.clone();
            
            tokio::spawn(async move {
                let res = crate::flatpak::cli::update_apps(&app_ids_clone, move |line| {
                    let line_qs = cxx_qt_lib::QString::from(&line);
                    let _ = qt_thread_status.queue(move |mut qobject| {
                        qobject.as_mut().set_update_status_text(line_qs);
                    });
                }).await;
                let _ = tx.send(res);
            });

            loop {
                tokio::select! {
                    res_opt = &mut rx => {
                        let res = res_opt.unwrap_or_else(|_| Err("Update task panicked".into()));
                        let _ = qt_thread.queue(move |mut qobject| {
                            qobject.as_mut().set_updating(false);
                            qobject.as_mut().set_update_progress(0.0);
                            qobject.as_mut().set_update_status_text(cxx_qt_lib::QString::from(""));
                            match res {
                                Ok(()) => {
                                    eprintln!("[kiosque] InstalledModel::update_selected_apps: successfully updated selected apps");
                                    qobject.as_mut().refresh();
                                }
                                Err(e) => {
                                    eprintln!("[kiosque] ERROR InstalledModel::update_selected_apps: {}", e);
                                    qobject.as_mut().refresh();
                                }
                            }
                        }); // Removed unwrap
                        break;
                    }
                    _ = interval.tick() => {
                        progress += (0.95 - progress) * 0.05;
                        let p = progress;
                        let _ = qt_thread_prog.queue(move |mut qobject| {
                            qobject.as_mut().set_update_progress(p);
                        }); // Removed unwrap
                    }
                }
            }
        });
    }

    pub fn updates_available_count(&self) -> i32 {
        self.items.iter().filter(|item| item.has_update).count() as i32
    }

    pub fn updates_checked_count(&self) -> i32 {
        self.items.iter().filter(|item| item.has_update && item.is_checked_for_update).count() as i32
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
        if a.is_runtime != b.is_runtime {
            return if a.is_runtime { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
        }
        if a.has_update != b.has_update {
            if a.has_update {
                return std::cmp::Ordering::Less;
            } else {
                return std::cmp::Ordering::Greater;
            }
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
