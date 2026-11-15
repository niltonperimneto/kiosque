//! Single source of truth for per-app state.
//!
//! The [`Registry`] knows which refs are installed (and where from), which
//! have updates, and which transactions are queued or running. Every bridge
//! object (detail pages, the installed list, the transaction model) reads from
//! here and listens to [`Event`]s instead of keeping its own copy, so two
//! pages showing the same app can never disagree.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::flatpak::{FlatpakBackend, InstalledApp, UpdateInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    Install,
    Uninstall,
    Update,
    InstallRefFile,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            JobKind::Install => "install",
            JobKind::Uninstall => "uninstall",
            JobKind::Update => "update",
            JobKind::InstallRefFile => "install",
        }
    }

    pub fn parse(s: &str) -> JobKind {
        match s {
            "uninstall" => JobKind::Uninstall,
            "update" => JobKind::Update,
            _ => JobKind::Install,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

impl JobState {
    pub fn is_active(&self) -> bool {
        matches!(self, JobState::Queued | JobState::Running)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Done => "done",
            JobState::Failed(_) => "failed",
            JobState::Cancelled => "cancelled",
        }
    }

    pub fn from_parts(state: &str, error: &str) -> JobState {
        match state {
            "queued" => JobState::Queued,
            "running" => JobState::Running,
            "done" => JobState::Done,
            "cancelled" => JobState::Cancelled,
            _ => JobState::Failed(error.to_string()),
        }
    }

    pub fn error(&self) -> &str {
        match self {
            JobState::Failed(e) => e,
            _ => "",
        }
    }
}

/// A queued, running or recently finished transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub kind: JobKind,
    /// The apps this job touches; empty for "update everything".
    pub app_ids: Vec<String>,
    /// Display title (usually the app name).
    pub title: String,
    pub state: JobState,
    /// `0.0..=1.0`, or negative while indeterminate.
    pub progress: f64,
    pub status: String,
}

impl Job {
    pub fn touches(&self, app_id: &str) -> bool {
        self.app_ids.iter().any(|a| a == app_id)
            || (self.kind == JobKind::Update && self.app_ids.is_empty())
    }
}

/// Snapshot of everything known about one app id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppState {
    pub installed: Option<InstalledApp>,
    pub update: Option<UpdateInfo>,
    /// The active job touching this app, if any.
    pub job: Option<Job>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The installed set was reloaded.
    Installed,
    /// The set of available updates changed.
    Updates,
    /// A job was created or changed.
    Job(Job),
}

#[derive(Default)]
struct Inner {
    installed: HashMap<String, InstalledApp>,
    installed_loaded: bool,
    updates: HashMap<String, UpdateInfo>,
    updates_loaded: bool,
    jobs: BTreeMap<u64, Job>,
}

/// How many finished jobs are kept around for display.
const FINISHED_JOBS_KEPT: usize = 20;

pub struct Registry {
    inner: RwLock<Inner>,
    events: broadcast::Sender<Event>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            inner: RwLock::new(Inner::default()),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    pub fn state(&self, app_id: &str) -> AppState {
        let inner = self.inner.read().unwrap();
        AppState {
            installed: inner.installed.get(app_id).cloned(),
            update: inner.updates.get(app_id).cloned(),
            job: inner
                .jobs
                .values()
                .rev()
                .find(|j| j.state.is_active() && j.touches(app_id))
                .cloned(),
        }
    }

    pub fn is_installed(&self, app_id: &str) -> bool {
        self.inner.read().unwrap().installed.contains_key(app_id)
    }

    pub fn installed_loaded(&self) -> bool {
        self.inner.read().unwrap().installed_loaded
    }

    pub fn updates_loaded(&self) -> bool {
        self.inner.read().unwrap().updates_loaded
    }

    pub fn installed(&self) -> Vec<InstalledApp> {
        self.inner
            .read()
            .unwrap()
            .installed
            .values()
            .cloned()
            .collect()
    }

    pub fn updates(&self) -> Vec<UpdateInfo> {
        self.inner
            .read()
            .unwrap()
            .updates
            .values()
            .cloned()
            .collect()
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.inner.read().unwrap().jobs.values().cloned().collect()
    }

    pub fn job(&self, id: u64) -> Option<Job> {
        self.inner.read().unwrap().jobs.get(&id).cloned()
    }

    pub fn active_jobs(&self) -> Vec<Job> {
        self.inner
            .read()
            .unwrap()
            .jobs
            .values()
            .filter(|j| j.state.is_active())
            .cloned()
            .collect()
    }

    pub fn set_installed(&self, apps: Vec<InstalledApp>) {
        {
            let mut inner = self.inner.write().unwrap();
            // An app id may be installed in both installations; prefer the
            // app ref over a runtime of the same name and user over system.
            let mut map: HashMap<String, InstalledApp> = HashMap::new();
            for app in apps {
                match map.get(&app.id) {
                    Some(existing)
                        if existing.kind == crate::flatpak::RefKind::App
                            && app.kind != existing.kind => {}
                    Some(existing)
                        if existing.scope == crate::flatpak::Scope::User
                            && existing.kind == app.kind => {}
                    _ => {
                        map.insert(app.id.clone(), app);
                    }
                }
            }
            inner.installed = map;
            inner.installed_loaded = true;
            // Updates for refs that are gone no longer apply.
            let Inner {
                installed, updates, ..
            } = &mut *inner;
            updates.retain(|id, _| installed.contains_key(id));
        }
        self.emit(Event::Installed);
    }

    pub fn set_updates(&self, updates: Vec<UpdateInfo>) {
        {
            let mut inner = self.inner.write().unwrap();
            inner.updates = updates.into_iter().map(|u| (u.id.clone(), u)).collect();
            inner.updates_loaded = true;
        }
        self.emit(Event::Updates);
    }

    /// Forget updates for `ids` (all of them when `ids` is empty).
    pub fn clear_updates(&self, ids: &[String]) {
        {
            let mut inner = self.inner.write().unwrap();
            if ids.is_empty() {
                inner.updates.clear();
            } else {
                inner.updates.retain(|id, _| !ids.contains(id));
            }
        }
        self.emit(Event::Updates);
    }

    /// Insert or replace a job and notify subscribers.
    pub fn set_job(&self, job: Job) {
        {
            let mut inner = self.inner.write().unwrap();
            inner.jobs.insert(job.id, job.clone());
            let finished: Vec<u64> = inner
                .jobs
                .values()
                .filter(|j| !j.state.is_active())
                .map(|j| j.id)
                .collect();
            if finished.len() > FINISHED_JOBS_KEPT {
                for id in &finished[..finished.len() - FINISHED_JOBS_KEPT] {
                    inner.jobs.remove(id);
                }
            }
        }
        self.emit(Event::Job(job));
    }

    /// Update just the progress of a running job.
    pub fn set_job_progress(&self, id: u64, progress: f64, status: &str) {
        let job = {
            let mut inner = self.inner.write().unwrap();
            let Some(job) = inner.jobs.get_mut(&id) else {
                return;
            };
            if progress >= 0.0 || job.progress < 0.0 {
                job.progress = progress;
            }
            if !status.is_empty() {
                job.status = status.to_string();
            }
            job.clone()
        };
        self.emit(Event::Job(job));
    }

    pub async fn reload_installed(
        &self,
        backend: &dyn FlatpakBackend,
    ) -> crate::flatpak::Result<()> {
        let apps = backend.list_installed().await?;
        self.set_installed(apps);
        Ok(())
    }

    pub async fn reload_updates(&self, backend: &dyn FlatpakBackend) -> crate::flatpak::Result<()> {
        let ups = backend.list_updates().await?;
        self.set_updates(ups);
        Ok(())
    }
}

/// The process-wide registry. On first use it loads the installed set and
/// starts following installation changes (from any tool) via the backend's
/// monitor.
pub fn registry() -> Arc<Registry> {
    static REG: OnceLock<Arc<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        let reg = Arc::new(Registry::new());
        let r = reg.clone();
        crate::runtime::runtime().spawn(async move {
            let backend = crate::flatpak::backend();
            if let Err(e) = r.reload_installed(&*backend).await {
                kerr!("registry: initial installed load failed: {}", e);
            }
            let mut changes = crate::flatpak::installation_changes();
            loop {
                match changes.recv().await {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
                // Debounce bursts of file events from a single transaction.
                tokio::time::sleep(Duration::from_millis(400)).await;
                while changes.try_recv().is_ok() {}
                klog!("registry: installation changed, reloading");
                if let Err(e) = r.reload_installed(&*backend).await {
                    kerr!("registry: reload failed: {}", e);
                }
            }
        });
        crate::flatpak::start_monitor();
        reg
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatpak::{RefKind, Scope};

    fn app(id: &str, scope: Scope, kind: RefKind) -> InstalledApp {
        InstalledApp {
            id: id.into(),
            name: id.into(),
            scope,
            kind,
            ..Default::default()
        }
    }

    #[test]
    fn installed_prefers_app_and_user() {
        let r = Registry::new();
        r.set_installed(vec![
            app("a", Scope::System, RefKind::App),
            app("a", Scope::User, RefKind::App),
            app("b", Scope::User, RefKind::App),
            app("b", Scope::User, RefKind::Runtime),
        ]);
        assert_eq!(r.state("a").installed.unwrap().scope, Scope::User);
        assert_eq!(r.state("b").installed.unwrap().kind, RefKind::App);
        assert!(r.state("c").installed.is_none());
    }

    #[test]
    fn job_state_is_shared() {
        let r = Registry::new();
        let mut rx = r.subscribe();
        let job = Job {
            id: 1,
            kind: JobKind::Install,
            app_ids: vec!["a".into()],
            title: "A".into(),
            state: JobState::Running,
            progress: 0.5,
            status: String::new(),
        };
        r.set_job(job.clone());
        assert_eq!(r.state("a").job, Some(job.clone()));
        assert!(r.state("b").job.is_none());
        assert_eq!(rx.try_recv().unwrap(), Event::Job(job));
        r.set_job_progress(1, -1.0, "x");
        // indeterminate reports never erase known progress
        assert_eq!(r.job(1).unwrap().progress, 0.5);
        assert_eq!(r.job(1).unwrap().status, "x");
    }

    #[test]
    fn finished_jobs_are_pruned() {
        let r = Registry::new();
        for id in 0..(FINISHED_JOBS_KEPT as u64 + 5) {
            r.set_job(Job {
                id,
                kind: JobKind::Install,
                app_ids: vec![],
                title: String::new(),
                state: JobState::Done,
                progress: 1.0,
                status: String::new(),
            });
        }
        assert_eq!(r.jobs().len(), FINISHED_JOBS_KEPT);
    }

    #[test]
    fn updates_follow_installed_set() {
        let r = Registry::new();
        r.set_installed(vec![app("a", Scope::User, RefKind::App)]);
        r.set_updates(vec![UpdateInfo {
            id: "a".into(),
            ..Default::default()
        }]);
        assert!(r.state("a").update.is_some());
        r.set_installed(vec![]);
        assert!(r.state("a").update.is_none());
    }
}
