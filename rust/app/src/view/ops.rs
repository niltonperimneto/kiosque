//! Flatpak operations as the views use them: everything goes through the
//! shared transaction client, and progress comes from the registry's job
//! state (the same numbers the Installed page and the daemon see).

use std::sync::Arc;

use kiosque_core::entries::InstalledEntry;
use kiosque_core::flatpak::{Operation, RemoteInfo, RemoteSource, Scope};
use kiosque_core::registry::JobState;
use kiosque_core::transactions::Transactions;

use crate::services::services;

/// Cancels the queued/running job if the waiting future is dropped early.
struct CancelOnDrop {
    client: Arc<Transactions>,
    id: u64,
    finished: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if !self.finished {
            let client = self.client.clone();
            let id = self.id;
            kiosque_core::runtime::runtime().spawn(async move {
                client.cancel(id).await;
            });
        }
    }
}

/// Job progress as reported to views: `fraction` in 0…1 (negative while
/// unknown) and a status line.
#[derive(Debug, Clone, PartialEq)]
pub struct JobProgress {
    pub id: u64,
    pub fraction: f64,
    pub status: String,
}

/// Enqueue `op` and wait for its terminal state, reporting progress.
pub async fn run(
    op: Operation,
    title: &str,
    progress: impl Fn(JobProgress) + Send,
) -> Result<(), String> {
    let mut events = services().registry.subscribe();
    let client = services().transactions().await;
    let id = client.enqueue(op, title).await?;
    let mut guard = CancelOnDrop {
        client,
        id,
        finished: false,
    };
    loop {
        if let Some(job) = services().registry.job(id) {
            match job.state {
                JobState::Done => {
                    guard.finished = true;
                    return Ok(());
                }
                JobState::Failed(error) => {
                    guard.finished = true;
                    return Err(error);
                }
                JobState::Cancelled => {
                    guard.finished = true;
                    return Err("operation cancelled".into());
                }
                _ => progress(JobProgress {
                    id,
                    fraction: job.progress,
                    status: job.status.clone(),
                }),
            }
        }
        match events.recv().await {
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => return Err("transaction registry closed".into()),
        }
    }
}

/// Cancel a job started by [`run`].
pub async fn cancel(id: u64) {
    services().transactions().await.cancel(id).await;
}

pub async fn install(id: &str, progress: impl Fn(JobProgress) + Send) -> Result<(), String> {
    let (remote, scope) = services().install_target(id);
    run(
        Operation::Install {
            remote,
            id: id.into(),
            scope,
        },
        id,
        progress,
    )
    .await
}

pub async fn uninstall(id: &str, progress: impl Fn(JobProgress) + Send) -> Result<(), String> {
    run(
        Operation::Uninstall {
            id: id.into(),
            scope: None,
        },
        id,
        progress,
    )
    .await
}

pub async fn update(ids: &[String], progress: impl Fn(JobProgress) + Send) -> Result<(), String> {
    run(Operation::Update { ids: ids.to_vec() }, "Update", progress).await
}

pub async fn launch(id: &str) -> Result<(), String> {
    services().backend.launch(id).await.map_err(|e| e.to_string())
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

pub async fn is_installed(id: &str) -> bool {
    if !services().registry.installed_loaded() {
        let _ = reload_installed().await;
    }
    services().registry.is_installed(id)
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

/// The registry's job for `app_id`, if one is queued or running.
pub fn active_job(app_id: &str) -> Option<JobProgress> {
    services()
        .registry
        .jobs()
        .into_iter()
        .find(|j| {
            j.app_ids.iter().any(|a| a == app_id)
                && matches!(j.state, JobState::Queued | JobState::Running)
        })
        .map(|j| JobProgress {
            id: j.id,
            fraction: j.progress,
            status: j.status,
        })
}
