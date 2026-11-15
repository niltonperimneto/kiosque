//! Small shared helpers for the cxx-qt bridge layer.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::flatpak::Operation;
pub use crate::flatpak::cli::JobOutcome;

/// Serialize `value` to a JSON `QString`, falling back to `fallback` (e.g. `"[]"`
/// or `"{}"`) if serialization fails. Replaces the repeated
/// `QString::from(&serde_json::to_string(..).unwrap_or_else(|_| "..".into()))`
/// boilerplate throughout the bridge models.
pub fn json_qstring<T: Serialize>(value: &T, fallback: &str) -> cxx_qt_lib::QString {
    let json = serde_json::to_string(value).unwrap_or_else(|_| fallback.to_string());
    cxx_qt_lib::QString::from(&json)
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
/// destroyed are dropped by `CxxQtThread::queue`.
pub fn run_job<T>(
    qt_thread: cxx_qt::CxxQtThread<T>,
    op: Operation,
    title: String,
    control: Arc<JobControl>,
    on_progress: impl Fn(Pin<&mut T>, f64, cxx_qt_lib::QString) + Send + Sync + Copy + 'static,
    on_finish: impl FnOnce(Pin<&mut T>, JobOutcome) + Send + 'static,
) where
    T: cxx_qt::Threading + 'static,
{
    crate::runtime::runtime().spawn(async move {
        let last: Mutex<Option<(f64, String)>> = Mutex::new(None);
        let progress_thread = qt_thread.clone();
        let progress_control = control.clone();
        let outcome = crate::flatpak::cli::run(op, &title, move |fraction, status, id| {
            progress_control.attach(id);
            {
                let mut last = last.lock().unwrap();
                if last.as_ref() == Some(&(fraction, status.clone())) {
                    return;
                }
                *last = Some((fraction, status.clone()));
            }
            let status = cxx_qt_lib::QString::from(&status);
            let _ = progress_thread.queue(move |qobject| on_progress(qobject, fraction, status));
        })
        .await;
        let _ = qt_thread.queue(move |qobject| on_finish(qobject, outcome));
    });
}

// ── JSON → QVariant conversions ─────────────────────────────────────────────

pub type QVariantMap = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
pub type QVariantList = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;

/// Convert a JSON value to the QVariant QML would get from `JSON.parse`,
/// except that arrays of strings become `QStringList`.
pub fn json_to_qvariant(value: &serde_json::Value) -> cxx_qt_lib::QVariant {
    use serde_json::Value;
    match value {
        Value::Null => cxx_qt_lib::QVariant::default(),
        Value::Bool(b) => cxx_qt_lib::QVariant::from(b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => cxx_qt_lib::QVariant::from(&i),
            None => cxx_qt_lib::QVariant::from(&n.as_f64().unwrap_or_default()),
        },
        Value::String(s) => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(s)),
        Value::Array(items) if items.iter().all(Value::is_string) => {
            cxx_qt_lib::QVariant::from(&string_list(items.iter().filter_map(Value::as_str)))
        }
        Value::Array(items) => cxx_qt_lib::QVariant::from(&json_to_qvariant_list(items)),
        Value::Object(_) => cxx_qt_lib::QVariant::from(&json_to_qvariant_map(value)),
    }
}

/// Convert a JSON object to a `QVariantMap` (non-objects give an empty map).
pub fn json_to_qvariant_map(value: &serde_json::Value) -> QVariantMap {
    let mut map = QVariantMap::default();
    if let Some(object) = value.as_object() {
        for (key, item) in object {
            map.insert(cxx_qt_lib::QString::from(key), json_to_qvariant(item));
        }
    }
    map
}

pub fn json_to_qvariant_list(items: &[serde_json::Value]) -> QVariantList {
    items.iter().map(json_to_qvariant).collect()
}

pub fn string_list<'a>(items: impl IntoIterator<Item = &'a str>) -> cxx_qt_lib::QStringList {
    items.into_iter().map(cxx_qt_lib::QString::from).collect()
}
