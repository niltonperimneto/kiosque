//! Small shared helpers for the cxx-qt bridge layer.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;

/// Serialize `value` to a JSON `QString`, falling back to `fallback` (e.g. `"[]"`
/// or `"{}"`) if serialization fails. Replaces the repeated
/// `QString::from(&serde_json::to_string(..).unwrap_or_else(|_| "..".into()))`
/// boilerplate throughout the bridge models.
pub fn json_qstring<T: Serialize>(value: &T, fallback: &str) -> cxx_qt_lib::QString {
    let json = serde_json::to_string(value).unwrap_or_else(|_| fallback.to_string());
    cxx_qt_lib::QString::from(&json)
}

/// Spawns `operation` on the async runtime and drives an indeterminate progress
/// bar on the owning QObject while it runs.
///
/// flatpak gives no real per-app progress, so the value ramps asymptotically
/// toward `0.95` on a 250 ms tick (purely to show motion) and is reported via
/// `set_progress`. The operation runs concurrently on the same task; when it
/// resolves, its result is handed to `on_finish` on the Qt thread. If `cancel`
/// is supplied and flips to `true`, the loop stops early and `on_cancel` runs
/// instead.
///
/// This consolidates the install / uninstall / update progress loops that were
/// previously copy-pasted across `store_controller` and `installed_model`.
pub fn run_with_progress<T, Fut, R>(
    qt_thread: cxx_qt::CxxQtThread<T>,
    operation: Fut,
    cancel: Option<Arc<AtomicBool>>,
    set_progress: impl Fn(Pin<&mut T>, f64) + Send + Copy + 'static,
    on_cancel: impl FnOnce(Pin<&mut T>) + Send + 'static,
    on_finish: impl FnOnce(Pin<&mut T>, R) + Send + 'static,
) where
    T: cxx_qt::Threading + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    crate::runtime::runtime().spawn(async move {
        let mut progress = 0.01_f64;
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        tokio::pin!(operation);

        loop {
            if let Some(ref flag) = cancel
                && flag.load(Ordering::SeqCst)
            {
                let _ = qt_thread.queue(on_cancel);
                return;
            }

            tokio::select! {
                result = &mut operation => {
                    let _ = qt_thread.queue(move |qobject| on_finish(qobject, result));
                    return;
                }
                _ = interval.tick() => {
                    progress += (0.95 - progress) * 0.05;
                    let p = progress;
                    let _ = qt_thread.queue(move |qobject| set_progress(qobject, p));
                }
            }
        }
    });
}
