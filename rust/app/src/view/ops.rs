//! Backend operations and registry snapshots used by the views.

use kiosque_core::entries::InstalledEntry;
use kiosque_core::flatpak::{Operation, RemoteInfo, RemoteSource, Scope};
use std::sync::Arc;

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
/// Terminal state of a registry job driven by [`run`].
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
pub async fn run(
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

pub async fn launch(id: &str) -> Result<(), String> {
    services()
        .backend
        .launch(id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn remotes() -> Result<Vec<RemoteInfo>, String> {
    services()
        .backend
        .list_remotes()
        .await
        .map_err(|e| e.to_string())
}

pub async fn add_remote(name: &str, url: &str) -> Result<(), String> {
    services()
        .backend
        .add_remote(name, RemoteSource::Url(url.into()), Scope::User)
        .await
        .map_err(|e| e.to_string())
}

pub async fn remove_remote(name: &str) -> Result<(), String> {
    services()
        .backend
        .remove_remote(name, None)
        .await
        .map_err(|e| e.to_string())
}

/// Reload installed refs into the registry.
pub async fn reload_installed() -> Result<(), String> {
    let s = services();
    s.registry
        .reload_installed(&*s.backend)
        .await
        .map_err(|e| e.to_string())
}

/// Reload available updates into the registry.
pub async fn reload_updates() -> Result<(), String> {
    let s = services();
    s.registry
        .reload_updates(&*s.backend)
        .await
        .map_err(|e| e.to_string())
}

/// Installed version of `id`, if known.
pub fn installed_version(id: &str) -> Option<String> {
    services()
        .registry
        .installed()
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.version)
}

/// The registry's installed refs and updates as view entries.
pub fn installed_entries() -> Vec<InstalledEntry> {
    let registry = &services().registry;
    let mut entries: Vec<InstalledEntry> = registry
        .installed()
        .into_iter()
        .map(|app| {
            let has_update = registry.state(&app.id).update.is_some();
            InstalledEntry {
                is_runtime: app.is_runtime(),
                size: super::installed::format_size(app.installed_size),
                size_bytes: app.installed_size,
                name: app.name,
                app_id: app.id,
                version: app.version,
                origin: app.origin,
                has_update,
                is_checked_for_update: has_update,
            }
        })
        .collect();
    // Runtime updates for refs not listed as installed (e.g. extensions).
    for update in registry.updates() {
        if !entries.iter().any(|e| e.app_id == update.id) {
            entries.push(InstalledEntry {
                is_runtime: update.kind == kiosque_core::flatpak::RefKind::Runtime,
                name: update.name,
                app_id: update.id,
                version: update.version,
                origin: update.origin,
                has_update: true,
                is_checked_for_update: true,
                ..Default::default()
            });
        }
    }
    entries
}
