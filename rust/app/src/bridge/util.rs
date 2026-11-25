//! Shared helpers for Qt adapter jobs and JSON data.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::flatpak::Operation;
use crate::services::services;

struct CancelOnDrop {
    client: Arc<crate::transactions::Transactions>,
    id: u64,
    finished: bool,
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if !self.finished {
            let client = self.client.clone();
            let id = self.id;
            crate::runtime::runtime().spawn(async move {
                client.cancel(id).await;
            });
        }
    }
}
/// Terminal state of a registry job driven by [`follow_job`].
#[derive(Debug, Clone, PartialEq)]
pub enum JobOutcome {
    Done,
    Failed(String),
    Cancelled,
}

/// Enqueue `op` on the shared transaction client and follow its registry
/// job until it reaches a terminal state.
///
/// `progress` receives `(job.progress, job.status, job.id)` on every
/// change of the job (progress is negative while indeterminate). It is
/// called once with `(-1.0, "", id)` straight after enqueueing when the
/// registry does not know the job yet, so callers learn the id needed for
/// [`crate::transactions::Transactions::cancel`] immediately.
/// Dropping the returned future cancels the job.
async fn follow_job(
    op: Operation,
    title: &str,
    progress: impl Fn(f64, String, u64) + Send,
) -> JobOutcome {
    use crate::registry::JobState;
    let mut events = services().registry.subscribe();
    let client = services().transactions().await;
    let id = match client.enqueue(op, title).await {
        Ok(id) => id,
        Err(error) => return JobOutcome::Failed(error),
    };
    let mut guard = CancelOnDrop {
        client,
        id,
        finished: false,
    };
    if services().registry.job(id).is_none() {
        progress(-1.0, String::new(), id);
    }
    loop {
        if let Some(job) = services().registry.job(id) {
            let outcome = match job.state {
                JobState::Done => Some(JobOutcome::Done),
                JobState::Failed(ref error) => Some(JobOutcome::Failed(error.clone())),
                JobState::Cancelled => Some(JobOutcome::Cancelled),
                JobState::Queued | JobState::Running => None,
            };
            progress(job.progress, job.status.clone(), job.id);
            if let Some(outcome) = outcome {
                guard.finished = true;
                return outcome;
            }
        }
        match events.recv().await {
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => {
                guard.finished = true;
                return JobOutcome::Failed("transaction registry closed".into());
            }
        }
    }
}

/// Ask the transaction client (daemon or in-process) to cancel job `id`.
pub fn cancel_job(id: u64) {
    crate::runtime::runtime().spawn(async move {
        let client = crate::services::services().transactions().await;
        if !client.cancel(id).await {
            klog!("cancel_job: job {} was not cancellable", id);
        }
    });
}

/// Cancellation handle for one job started through [`run_job`].
///
/// The registry job id is only known once the operation has been enqueued,
/// so a cancel requested before that is remembered and forwarded to
/// `Transactions::cancel` as soon as the id arrives.
#[derive(Default)]
pub struct JobControl {
    id: AtomicU64,
    cancel_requested: AtomicBool,
}

impl JobControl {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The registry job id, once known.
    pub fn job_id(&self) -> Option<u64> {
        match self.id.load(Ordering::SeqCst) {
            0 => None,
            id => Some(id),
        }
    }

    /// Request cancellation of the job via `Transactions::cancel(job_id)`.
    pub fn cancel(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
        if let Some(id) = self.job_id() {
            cancel_job(id);
        }
    }

    fn attach(&self, id: u64) {
        if id == 0 {
            return;
        }
        if self.id.swap(id, Ordering::SeqCst) != id && self.cancel_requested.load(Ordering::SeqCst)
        {
            cancel_job(id);
        }
    }
}

/// Enqueue `op` on the transaction client and report its real registry
/// progress to the owning QObject.
///
/// `on_progress(qobject, fraction, status)` runs on the Qt thread whenever
/// the job's progress or status changes; `fraction` is negative while
/// flatpak reports no measurable progress (QML shows an indeterminate bar).
/// `on_finish` runs on the Qt thread with the terminal state. `control` lets
/// the caller cancel the job by id. Results queued after the QObject is
/// destroyed are dropped by `QThreadHandle::invoke_mut`.
pub fn run_job<T>(
    qt_thread: qtbridge::QThreadHandle<T>,
    op: Operation,
    title: String,
    control: Arc<JobControl>,
    on_progress: impl Fn(&mut T, f64, String) + Send + Sync + Copy + 'static,
    on_finish: impl FnOnce(&mut T, JobOutcome) + Send + 'static,
) where
    T: qtbridge::QObjectHolder + 'static,
{
    crate::runtime::runtime().spawn(async move {
        let qt_thread = Arc::new(qt_thread);
        let last: Mutex<Option<(f64, String)>> = Mutex::new(None);
        let progress_thread = qt_thread.clone();
        let progress_control = control.clone();
        let outcome = follow_job(op, &title, move |fraction, status, id| {
            progress_control.attach(id);
            {
                let mut last = last.lock().unwrap();
                if last.as_ref() == Some(&(fraction, status.clone())) {
                    return;
                }
                *last = Some((fraction, status.clone()));
            }
            let status = String::from(&status);
            progress_thread.invoke_mut(move |qobject| on_progress(qobject, fraction, status));
        })
        .await;
        qt_thread.invoke_mut(move |qobject| on_finish(qobject, outcome));
    });
}

pub type QVariantMap = serde_json::Value;
pub type QVariantList = serde_json::Value;
pub fn json_to_qvariant_map(value: &serde_json::Value) -> serde_json::Value {
    if value.is_object() {
        value.clone()
    } else {
        serde_json::json!({})
    }
}
pub fn json_to_qvariant_list(items: &[serde_json::Value]) -> serde_json::Value {
    items.into()
}
pub fn string_list<'a>(items: impl IntoIterator<Item = &'a str>) -> serde_json::Value {
    items.into_iter().collect::<Vec<_>>().into()
}
