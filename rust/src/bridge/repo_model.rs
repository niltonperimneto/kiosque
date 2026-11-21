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
        #[qml_singleton]
        #[qproperty(QString, repo_key_sha256)]
        #[qproperty(bool, loading)]
        type RepoModel = super::RepoModelRust;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &RepoModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "rowCount"]
        #[cxx_override]
        fn row_count(self: &RepoModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_name = "roleNames"]
        #[cxx_override]
        fn role_names(self: &RepoModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn refresh(self: Pin<&mut RepoModel>);

        #[qinvokable]
        #[cxx_name = "addRemote"]
        fn add_remote(self: Pin<&mut RepoModel>, name: QString, url: QString);

        #[qinvokable]
        #[cxx_name = "removeRemote"]
        fn remove_remote(self: Pin<&mut RepoModel>, name: QString);

        /// Read a `.flatpakrepo` (local path, `file://` or `http(s)://` URL)
        /// without adding it. The result arrives in `repoFileInspected`; on
        /// success the file stays pending for `addInspectedRemote`.
        #[qinvokable]
        #[cxx_name = "inspectRepoFile"]
        fn inspect_repo_file(self: Pin<&mut RepoModel>, path: QString);

        /// Add the pending `.flatpakrepo` under `name`; reports through
        /// `remoteAdded`.
        #[qinvokable]
        #[cxx_name = "addInspectedRemote"]
        fn add_inspected_remote(self: Pin<&mut RepoModel>, name: QString);

        /// Forget the pending `.flatpakrepo`.
        #[qinvokable]
        #[cxx_name = "discardRepoFile"]
        fn discard_repo_file(self: Pin<&mut RepoModel>);

        #[qsignal]
        #[cxx_name = "repoFileInspected"]
        fn repo_file_inspected(
            self: Pin<&mut RepoModel>,
            success: bool,
            suggested_name: QString,
            title: QString,
            url: QString,
            error: QString,
        );

        #[qsignal]
        #[cxx_name = "remoteAdded"]
        fn remote_added(self: Pin<&mut RepoModel>, success: bool, message: QString);

        #[qsignal]
        #[cxx_name = "remoteRemoved"]
        fn remote_removed(self: Pin<&mut RepoModel>, success: bool, message: QString);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut RepoModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut RepoModel>);
    }

    impl cxx_qt::Threading for RepoModel {}
}

pub struct RepoEntry {
    pub name: String,
    pub title: String,
    pub url: String,
    pub description: String,
}

#[derive(Default)]
pub struct RepoModelRust {
    repo_key_sha256: cxx_qt_lib::QString,
    pub items: Vec<RepoEntry>,
    pub loading: bool,
    /// Inspected `.flatpakrepo` contents awaiting confirmation.
    pending_repo: Option<Vec<u8>>,
    repo_generation: u64,
}

impl qobject::RepoModel {
    pub const NAME_ROLE: i32 = 256;
    pub const TITLE_ROLE: i32 = 257;
    pub const URL_ROLE: i32 = 258;
    pub const DESCRIPTION_ROLE: i32 = 259;

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        if !index.is_valid() {
            return cxx_qt_lib::QVariant::default();
        }
        let row = index.row() as usize;
        if let Some(item) = self.items.get(row) {
            match role {
                Self::NAME_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.name))
                }
                Self::TITLE_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.title))
                }
                Self::URL_ROLE => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.url)),
                Self::DESCRIPTION_ROLE => {
                    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&item.description))
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
        roles.insert(Self::TITLE_ROLE, cxx_qt_lib::QByteArray::from("title"));
        roles.insert(Self::URL_ROLE, cxx_qt_lib::QByteArray::from("url"));
        roles.insert(
            Self::DESCRIPTION_ROLE,
            cxx_qt_lib::QByteArray::from("description"),
        );
        roles
    }

    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread = self.qt_thread();

        crate::runtime::runtime().spawn(async move {
            let remotes = crate::flatpak::ui::list_remotes().await.unwrap_or_default();

            let items: Vec<RepoEntry> = remotes
                .into_iter()
                .map(|r| RepoEntry {
                    name: r.name,
                    title: r.title,
                    url: r.url,
                    description: r.description,
                })
                .collect();

            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().begin_reset_model();
                qobject.as_mut().rust_mut().items = items;
                qobject.as_mut().end_reset_model();
                qobject.as_mut().set_loading(false);
            });
        });
    }

    pub fn add_remote(
        self: std::pin::Pin<&mut Self>,
        name: cxx_qt_lib::QString,
        url: cxx_qt_lib::QString,
    ) {
        let qt_thread = self.qt_thread();
        let name_str = name.to_string();
        let url_str = url.to_string();

        crate::runtime::runtime().spawn(async move {
            let result = crate::flatpak::ui::add_repository(&name_str, &url_str).await;
            let (success, msg) = match result {
                Ok(_) => (true, "Repository added successfully".to_string()),
                Err(e) => (false, e),
            };

            let _ = qt_thread.queue(move |mut qobject| {
                qobject
                    .as_mut()
                    .remote_added(success, cxx_qt_lib::QString::from(&msg));
                if success {
                    qobject.as_mut().refresh();
                }
            });
        });
    }

    pub fn inspect_repo_file(mut self: std::pin::Pin<&mut Self>, path: cxx_qt_lib::QString) {
        let source = path.to_string();
        klog!("RepoModel::inspect_repo_file: \"{}\"", source);
        self.as_mut().rust_mut().pending_repo = None;
        self.as_mut().rust_mut().repo_generation += 1;
        let generation = self.repo_generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let result = crate::bridge::store_controller::read_repo_file(&source).await;
            let _ = qt_thread.queue(move |mut q| {
                if q.repo_generation != generation {
                    return;
                }
                let qs = |s: &str| cxx_qt_lib::QString::from(s);
                match result {
                    Ok(data) => {
                        let validated = kiosque_core::security::repo_file(&data)
                            .expect("validated repository descriptor");
                        q.as_mut().set_repo_key_sha256(qs(&validated.key_sha256));
                        let mut summary = crate::flatpak::summarize_keyfile(&data);
                        summary.url = validated.origin;
                        q.as_mut().rust_mut().pending_repo = Some(data);
                        q.as_mut().repo_file_inspected(
                            true,
                            qs(&summary.suggested_remote_name),
                            qs(&summary.title),
                            qs(&summary.url),
                            qs(""),
                        );
                    }
                    Err(e) => {
                        kerr!("RepoModel::inspect_repo_file: {}", e);
                        q.as_mut()
                            .repo_file_inspected(false, qs(""), qs(""), qs(""), qs(&e));
                    }
                }
            });
        });
    }

    pub fn add_inspected_remote(mut self: std::pin::Pin<&mut Self>, name: cxx_qt_lib::QString) {
        let Some(data) = self.as_mut().rust_mut().pending_repo.take() else {
            kerr!("RepoModel::add_inspected_remote: no inspected .flatpakrepo");
            return;
        };
        let name = name.to_string();
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let result = crate::flatpak::ui::add_repository_file(&name, data).await;
            let (success, msg) = match result {
                Ok(_) => (true, "Repository added successfully".to_string()),
                Err(e) => (false, e),
            };
            let _ = qt_thread.queue(move |mut qobject| {
                qobject
                    .as_mut()
                    .remote_added(success, cxx_qt_lib::QString::from(&msg));
                if success {
                    qobject.as_mut().refresh();
                }
            });
        });
    }

    pub fn discard_repo_file(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().rust_mut().pending_repo = None;
        self.as_mut().rust_mut().repo_generation += 1;
    }

    pub fn remove_remote(self: std::pin::Pin<&mut Self>, name: cxx_qt_lib::QString) {
        let qt_thread = self.qt_thread();
        let name_str = name.to_string();

        crate::runtime::runtime().spawn(async move {
            let result = crate::flatpak::ui::remove_repository(&name_str).await;
            let (success, msg) = match result {
                Ok(_) => (true, "Repository removed successfully".to_string()),
                Err(e) => (false, e),
            };

            let _ = qt_thread.queue(move |mut qobject| {
                qobject
                    .as_mut()
                    .remote_removed(success, cxx_qt_lib::QString::from(&msg));
                if success {
                    qobject.as_mut().refresh();
                }
            });
        });
    }
}
