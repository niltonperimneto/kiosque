// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

//! List of queued, running and recently finished Flatpak transactions.
//!
//! Mirrors `Registry::jobs()` so the UI can show every operation without
//! blocking the window, whichever page started it. Finished jobs can be
//! dismissed locally; the registry keeps its own bounded history.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use kiosque_core::registry::{Event, Job, JobState};

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
        #[qproperty(i32, active_count)]
        #[qproperty(i32, finished_count)]
        type JobsModel = super::JobsModelRust;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &JobsModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "rowCount"]
        #[cxx_override]
        fn row_count(self: &JobsModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_name = "roleNames"]
        #[cxx_override]
        fn role_names(self: &JobsModel) -> QHash_i32_QByteArray;

        /// Cancel job `id` through the transaction client.
        #[qinvokable]
        fn cancel(self: Pin<&mut JobsModel>, id: i64);

        /// Show a desktop notification (for when the window is not active).
        #[qinvokable]
        #[cxx_name = "notifyDesktop"]
        fn notify_desktop(self: &JobsModel, title: QString, body: QString, icon: QString);

        /// Hide finished jobs from this list.
        #[qinvokable]
        #[cxx_name = "clearFinished"]
        fn clear_finished(self: Pin<&mut JobsModel>);

        /// A job this model saw queued or running has ended. `state` is
        /// "done", "failed" or "cancelled"; `kind` is "install",
        /// "uninstall" or "update".
        #[qsignal]
        #[cxx_name = "jobFinished"]
        fn job_finished(
            self: Pin<&mut JobsModel>,
            title: QString,
            kind: QString,
            state: QString,
            error: QString,
        );
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut JobsModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut JobsModel>);
    }

    impl cxx_qt::Threading for JobsModel {}
    impl cxx_qt::Initialize for JobsModel {}
}

/// `active_count`: queued or running jobs; `finished_count`: done, failed or
/// cancelled jobs still listed.
pub struct JobsModelRust {
    jobs: Vec<Job>,
    active_count: i32,
    finished_count: i32,
    /// Finished jobs the user cleared.
    dismissed: HashSet<u64>,
    /// Last state seen per job (from job events), to report each finish once.
    seen: HashMap<u64, JobState>,
    watch: Option<tokio_util::sync::CancellationToken>,
}

impl Default for JobsModelRust {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            active_count: 0,
            finished_count: 0,
            dismissed: HashSet::new(),
            seen: HashMap::new(),
            watch: None,
        }
    }
}

impl Drop for JobsModelRust {
    fn drop(&mut self) {
        if let Some(watch) = &self.watch {
            watch.cancel();
        }
    }
}

impl cxx_qt::Initialize for qobject::JobsModel {
    fn initialize(mut self: Pin<&mut Self>) {
        let cancel = tokio_util::sync::CancellationToken::new();
        self.as_mut().rust_mut().watch = Some(cancel.clone());
        let mut events = crate::services::services().registry.subscribe();
        let thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = events.recv() => {
                        // Job events carry the job as it was at that moment, so
                        // short jobs that finish before the next sync are still
                        // seen going from active to finished.
                        let job = match event {
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            Ok(Event::Job(job)) => Some(job),
                            _ => None,
                        };
                        let queued = thread.queue(move |mut object| {
                            if let Some(job) = job {
                                object.as_mut().note_job(job);
                            }
                            object.sync();
                        });
                        if queued.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        self.sync();
    }
}

impl qobject::JobsModel {
    pub const ID_ROLE: i32 = 256;
    pub const TITLE_ROLE: i32 = 257;
    pub const KIND_ROLE: i32 = 258;
    pub const STATE_ROLE: i32 = 259;
    pub const PROGRESS_ROLE: i32 = 260;
    pub const STATUS_ROLE: i32 = 261;
    pub const ERROR_ROLE: i32 = 262;
    pub const APP_ID_ROLE: i32 = 263;
    pub const ACTIVE_ROLE: i32 = 264;

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        let Some(job) = usize::try_from(index.row())
            .ok()
            .and_then(|row| self.jobs.get(row))
        else {
            return cxx_qt_lib::QVariant::default();
        };
        let text = |s: &str| cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(s));
        match role {
            Self::ID_ROLE => cxx_qt_lib::QVariant::from(&(job.id as i64)),
            Self::TITLE_ROLE => text(&job.title),
            Self::KIND_ROLE => text(job.kind.as_str()),
            Self::STATE_ROLE => text(job.state.as_str()),
            Self::PROGRESS_ROLE => cxx_qt_lib::QVariant::from(&job.progress.min(1.0)),
            Self::STATUS_ROLE => text(&job.status),
            Self::ERROR_ROLE => text(job.state.error()),
            Self::APP_ID_ROLE => text(job.app_ids.first().map(String::as_str).unwrap_or("")),
            Self::ACTIVE_ROLE => cxx_qt_lib::QVariant::from(&job.state.is_active()),
            _ => cxx_qt_lib::QVariant::default(),
        }
    }

    pub fn row_count(&self, parent: &cxx_qt_lib::QModelIndex) -> i32 {
        if parent.is_valid() {
            return 0;
        }
        self.jobs.len() as i32
    }

    pub fn role_names(&self) -> cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray> {
        let mut roles = cxx_qt_lib::QHash::<cxx_qt_lib::QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (Self::ID_ROLE, "jobId"),
            (Self::TITLE_ROLE, "title"),
            (Self::KIND_ROLE, "kind"),
            (Self::STATE_ROLE, "jobState"),
            (Self::PROGRESS_ROLE, "progress"),
            (Self::STATUS_ROLE, "status"),
            (Self::ERROR_ROLE, "error"),
            (Self::APP_ID_ROLE, "appId"),
            (Self::ACTIVE_ROLE, "active"),
        ] {
            roles.insert(role, cxx_qt_lib::QByteArray::from(name));
        }
        roles
    }

    pub fn cancel(self: Pin<&mut Self>, id: i64) {
        if let Ok(id) = u64::try_from(id) {
            super::util::cancel_job(id);
        }
    }

    pub fn notify_desktop(
        &self,
        title: cxx_qt_lib::QString,
        body: cxx_qt_lib::QString,
        icon: cxx_qt_lib::QString,
    ) {
        let (title, body, icon) = (title.to_string(), body.to_string(), icon.to_string());
        crate::runtime::runtime().spawn(async move {
            if let Err(e) = kiosque_core::notifications::notify(&title, &body, &icon).await {
                klog!("JobsModel::notify_desktop: {}", e);
            }
        });
    }

    pub fn clear_finished(mut self: Pin<&mut Self>) {
        let finished: Vec<u64> = self
            .jobs
            .iter()
            .filter(|job| !job.state.is_active())
            .map(|job| job.id)
            .collect();
        self.as_mut().rust_mut().dismissed.extend(finished);
        self.sync();
    }

    /// Record one job event and report the job if it just stopped being active.
    fn note_job(mut self: Pin<&mut Self>, job: Job) {
        let was_active = self.seen.get(&job.id).is_some_and(JobState::is_active);
        let now_active = job.state.is_active();
        self.as_mut()
            .rust_mut()
            .seen
            .insert(job.id, job.state.clone());
        if was_active && !now_active {
            let qs = |s: &str| cxx_qt_lib::QString::from(s);
            self.as_mut().job_finished(
                qs(&job.title),
                qs(job.kind.as_str()),
                qs(job.state.as_str()),
                qs(job.state.error()),
            );
        }
    }

    /// Reload the job list from the registry.
    fn sync(mut self: Pin<&mut Self>) {
        let mut jobs = crate::services::services().registry.jobs();
        // Newest first.
        jobs.sort_by(|a, b| b.id.cmp(&a.id));
        {
            // Forget jobs the registry dropped from its history.
            let live: HashSet<u64> = jobs.iter().map(|job| job.id).collect();
            let mut rust = self.as_mut().rust_mut();
            rust.seen.retain(|id, _| live.contains(id));
            rust.dismissed.retain(|id| live.contains(id));
        }
        let dismissed = &self.dismissed;
        jobs.retain(|job| job.state.is_active() || !dismissed.contains(&job.id));

        if self.jobs != jobs {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().jobs = jobs;
            self.as_mut().end_reset_model();
        }
        let active = self.jobs.iter().filter(|job| job.state.is_active()).count() as i32;
        let done = self.jobs.len() as i32 - active;
        self.as_mut().set_active_count(active);
        self.as_mut().set_finished_count(done);
    }
}
