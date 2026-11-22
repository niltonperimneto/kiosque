//! Shared transaction list, cancellation, and completion notifications.
use kiosque_core::registry::{Job, JobState};
pub use object::JobsModel;
use qtbridge::qobject;
use std::collections::{HashMap, HashSet};
#[derive(Default, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct JobItem {
    pub jobId: i64,
    pub title: String,
    pub kind: String,
    pub jobState: String,
    pub progress: f64,
    pub status: String,
    pub error: String,
    pub appId: String,
    pub active: bool,
}
impl From<&Job> for JobItem {
    fn from(j: &Job) -> Self {
        Self {
            jobId: j.id as i64,
            title: j.title.clone(),
            kind: j.kind.as_str().into(),
            jobState: j.state.as_str().into(),
            progress: j.progress.min(1.0),
            status: j.status.clone(),
            error: j.state.error().into(),
            appId: j.app_ids.first().cloned().unwrap_or_default(),
            active: j.state.is_active(),
        }
    }
}
#[qobject(Base = QListModel, NoQmlElement)]
mod object {
    use super::*;
    use qtbridge::{QListModel, QListModelBase, QmlObject};
    // A proper list model retains role data, notifications and event ordering.
    #[derive(Default)]
    pub struct JobsModel {
        rows: Vec<JobItem>,
        pending: Option<Vec<JobItem>>,
        active_count: i32,
        finished_count: i32,
        dismissed: HashSet<u64>,
        seen: HashMap<u64, JobState>,
        watch: Option<tokio_util::sync::CancellationToken>,
    }
    impl Drop for JobsModel {
        fn drop(&mut self) {
            if let Some(w) = &self.watch {
                w.cancel();
            }
        }
    }
    impl QListModel for JobsModel {
        type Item = JobItem;
        fn len(&self) -> usize {
            self.rows.len()
        }
        fn get(&self, i: usize) -> Option<&JobItem> {
            self.rows.get(i)
        }
        fn reset_unnotified(&mut self) {
            if let Some(rows) = self.pending.take() {
                self.rows = rows;
            }
        }
    }
    impl JobsModel {
        qproperty!("active_count", Read = get_active_count, Notify = changed);
        qproperty!(
            "finished_count",
            Read = get_finished_count,
            Notify = changed
        );
        fn get_active_count(&self) -> i32 {
            self.active_count
        }
        fn get_finished_count(&self) -> i32 {
            self.finished_count
        }
        #[qsignal]
        pub fn changed(&mut self);
        #[qsignal(qml_name = "jobFinished")]
        pub fn job_finished(&mut self, title: String, kind: String, state: String, error: String);
        #[qslot]
        fn cancel(&mut self, id: i64) {
            if let Ok(id) = u64::try_from(id) {
                super::super::util::cancel_job(id);
            }
        }
        #[qslot(qml_name = "clearFinished")]
        fn clear_finished(&mut self) {
            for j in crate::services::services().registry.jobs() {
                if !j.state.is_active() {
                    self.dismissed.insert(j.id);
                }
            }
            self.sync();
        }
        #[qslot(qml_name = "notifyDesktop")]
        fn notify_desktop(&self, title: String, body: String, icon: String) {
            crate::bridge::spawn(async move {
                if let Err(e) = kiosque_core::notifications::notify(&title, &body, &icon).await {
                    klog!("notification: {}", e);
                }
            });
        }
        #[qslot]
        fn refresh(&mut self) {
            if self.watch.is_some() {
                self.sync();
                return;
            }
            let cancel = tokio_util::sync::CancellationToken::new();
            self.watch = Some(cancel.clone());
            let mut events = crate::services::services().registry.subscribe();
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                loop {
                    tokio::select! {
                     _=cancel.cancelled()=>break,
                     event=events.recv()=>{
                      use tokio::sync::broadcast::error::RecvError;
                      if matches!(event,Err(RecvError::Closed)) {break;}
                      handle.invoke_mut(move |m| {
                       let finished=if let Ok(kiosque_core::registry::Event::Job(j))=event {
                        let active=m.seen.get(&j.id).is_some_and(JobState::is_active);
                        m.seen.insert(j.id,j.state.clone());
                        (active&&!j.state.is_active()).then_some(j)
                       } else {None};
                       m.sync();
                       if let Some(j)=finished {m.job_finished(j.title,j.kind.as_str().into(),j.state.as_str().into(),j.state.error().into());}
                      });
                     }
                    }
                }
            });
            self.sync();
        }
        fn sync(&mut self) {
            let mut jobs = crate::services::services().registry.jobs();
            jobs.sort_by_key(|job| std::cmp::Reverse(job.id));
            let live: HashSet<_> = jobs.iter().map(|j| j.id).collect();
            self.seen.retain(|id, _| live.contains(id));
            self.dismissed.retain(|id| live.contains(id));
            jobs.retain(|j| j.state.is_active() || !self.dismissed.contains(&j.id));
            self.active_count = jobs.iter().filter(|j| j.state.is_active()).count() as i32;
            self.finished_count = jobs.len() as i32 - self.active_count;
            let rows: Vec<_> = jobs.iter().map(JobItem::from).collect();
            if self.rows != rows {
                self.pending = Some(rows);
                self.reset();
            }
            self.changed();
        }
    }
}
qml_element!(JobsModel, "JobsModel");

// Keep the existing Qt.UserRole numbers as well as the QML role names.
impl qtbridge::QModelItem for JobItem {
    type T0 = i64;
    type T1 = String;
    type T2 = String;
    type T3 = String;
    type T4 = f64;
    type T5 = String;
    type T6 = String;
    type T7 = String;
    type T8 = bool;
    type T9 = ();
    type T10 = ();
    type T11 = ();
    type T12 = ();
    type T13 = ();
    type T14 = ();
    fn get0(&self) -> Option<&i64> {
        Some(&self.jobId)
    }
    fn get1(&self) -> Option<&String> {
        Some(&self.title)
    }
    fn get2(&self) -> Option<&String> {
        Some(&self.kind)
    }
    fn get3(&self) -> Option<&String> {
        Some(&self.jobState)
    }
    fn get4(&self) -> Option<&f64> {
        Some(&self.progress)
    }
    fn get5(&self) -> Option<&String> {
        Some(&self.status)
    }
    fn get6(&self) -> Option<&String> {
        Some(&self.error)
    }
    fn get7(&self) -> Option<&String> {
        Some(&self.appId)
    }
    fn get8(&self) -> Option<&bool> {
        Some(&self.active)
    }
    fn role_names() -> std::collections::HashMap<i32, String> {
        [
            (256, "jobId".into()),
            (257, "title".into()),
            (258, "kind".into()),
            (259, "jobState".into()),
            (260, "progress".into()),
            (261, "status".into()),
            (262, "error".into()),
            (263, "appId".into()),
            (264, "active".into()),
        ]
        .into()
    }
    fn get_role(&self, role: i32) -> qtbridge::qtbridge_type_lib::QVariant {
        match role {
            256 => qtbridge::QVariantConvertible::to_qvariant(&self.jobId),
            257 => qtbridge::QVariantConvertible::to_qvariant(&self.title),
            258 => qtbridge::QVariantConvertible::to_qvariant(&self.kind),
            259 => qtbridge::QVariantConvertible::to_qvariant(&self.jobState),
            260 => qtbridge::QVariantConvertible::to_qvariant(&self.progress),
            261 => qtbridge::QVariantConvertible::to_qvariant(&self.status),
            262 => qtbridge::QVariantConvertible::to_qvariant(&self.error),
            263 => qtbridge::QVariantConvertible::to_qvariant(&self.appId),
            264 => qtbridge::QVariantConvertible::to_qvariant(&self.active),
            _ => Default::default(),
        }
    }
}
