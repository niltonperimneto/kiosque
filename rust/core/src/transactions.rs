//! Transaction queue.
//!
//! [`TransactionManager`] owns a FIFO of [`Operation`]s and runs them one at a
//! time on a [`FlatpakBackend`], publishing every state change into the
//! [`Registry`]. It runs inside `kiosque-update` (the daemon) so jobs survive
//! the window closing; the GUI talks to it over D-Bus through
//! [`Transactions::Remote`], or runs it in-process
//! ([`Transactions::Local`]) when the daemon is unavailable.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::flatpak::{FlatpakBackend, FlatpakError, Operation, Scope};
use crate::registry::{Job, JobKind, JobState, Registry};

struct Queued {
    id: u64,
    op: Operation,
    token: CancellationToken,
}

pub struct TransactionManager {
    backend: Arc<dyn FlatpakBackend>,
    registry: Arc<Registry>,
    queue: mpsc::Sender<Queued>,
    tokens: Mutex<HashMap<u64, CancellationToken>>,
    next_id: AtomicU64,
    stopping: CancellationToken,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    submission: Mutex<()>,
    operations: Mutex<HashMap<u64, Operation>>,
}

fn kind_of(op: &Operation) -> JobKind {
    match op {
        Operation::Install { .. } => JobKind::Install,
        Operation::InstallRefFile { .. } => JobKind::InstallRefFile,
        Operation::Uninstall { .. } | Operation::RemoveRuntimes { .. } => JobKind::Uninstall,
        Operation::Update { .. } => JobKind::Update,
    }
}

impl TransactionManager {
    /// Create a manager and start its worker on the shared runtime.
    pub fn new(backend: Arc<dyn FlatpakBackend>, registry: Arc<Registry>) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(crate::security::MAX_OPERATIONS);
        let mgr = Arc::new(Self {
            backend,
            registry,
            queue: tx,
            tokens: Mutex::new(HashMap::new()),
            stopping: CancellationToken::new(),
            worker: Mutex::new(None),
            submission: Mutex::new(()),
            operations: Mutex::new(HashMap::new()),
            // Start ids at the current time so ids from a restarted daemon
            // never collide with ones a GUI still remembers.
            next_id: AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(1),
            ),
        });
        let weak = Arc::downgrade(&mgr);
        let stopping = mgr.stopping.clone();
        let worker = crate::runtime::runtime().spawn(async move {
            let mut rx = rx;
            loop {
                let queued = tokio::select! {
                    biased;
                    _ = stopping.cancelled() => break,
                    item = rx.recv() => match item { Some(item) => item, None => break },
                };
                let Some(manager) = weak.upgrade() else { break };
                manager.process(queued).await;
            }
            if let Some(manager) = weak.upgrade() {
                while let Ok(queued) = rx.try_recv() {
                    if let Some(mut job) = manager.registry.job(queued.id) {
                        job.state = JobState::Cancelled;
                        manager.finish(job);
                    }
                }
            }
        });
        *mgr.worker.lock().unwrap() = Some(worker);
        mgr
    }

    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    pub fn backend(&self) -> &Arc<dyn FlatpakBackend> {
        &self.backend
    }

    /// Queue `op`. If an identical operation is already queued or running,
    /// its id is returned instead of queueing a duplicate.
    pub fn enqueue(&self, op: Operation, title: &str) -> u64 {
        self.try_enqueue(op, title).unwrap_or_else(|error| {
            kerr!("Transaction rejected: {}", error);
            0
        })
    }

    /// Bounded submission for every external/UI caller. A rejected operation
    /// has no job id and never reaches a backend.
    pub fn try_enqueue(&self, op: Operation, title: &str) -> Result<u64, String> {
        crate::security::operation(&op, title)?;
        let _submission = self.submission.lock().unwrap();
        let kind = kind_of(&op);
        let app_ids = op.app_ids();
        if let Some((&existing, _)) = self
            .operations
            .lock()
            .unwrap()
            .iter()
            .find(|(_, existing)| **existing == op)
        {
            return Ok(existing);
        }

        if self.operations.lock().unwrap().len() >= crate::security::MAX_OPERATIONS {
            return Err("Transaction queue is full; wait for existing operations".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let token = CancellationToken::new();
        self.tokens.lock().unwrap().insert(id, token.clone());
        self.operations.lock().unwrap().insert(id, op.clone());
        let title = if title.is_empty() {
            app_ids.join(", ")
        } else {
            title.to_string()
        };
        self.registry.set_job(Job {
            id,
            kind,
            app_ids,
            title,
            state: JobState::Queued,
            progress: -1.0,
            status: String::new(),
        });
        if self.stopping.is_cancelled() {
            if let Some(mut job) = self.registry.job(id) {
                job.state = JobState::Cancelled;
                self.finish(job);
            }
        } else {
            if self.queue.try_send(Queued { id, op, token }).is_err() {
                if let Some(mut job) = self.registry.job(id) {
                    job.state = JobState::Failed("Transaction queue is unavailable".into());
                    self.finish(job);
                }
                return Err("Transaction queue is unavailable".into());
            }
        }
        Ok(id)
    }

    /// Cancel a queued or running job. Returns false if it is unknown or
    /// already finished.
    pub fn cancel(&self, id: u64) -> bool {
        match self.tokens.lock().unwrap().get(&id) {
            Some(t) => {
                t.cancel();
                true
            }
            None => false,
        }
    }

    pub fn is_busy(&self) -> bool {
        !self.registry.active_jobs().is_empty()
    }

    /// Cancel pending work and wait for the worker to exit.
    pub async fn shutdown(&self) {
        {
            let _submission = self.submission.lock().unwrap();
            self.stopping.cancel();
            for token in self.tokens.lock().unwrap().values() {
                token.cancel();
            }
        }
        let worker = self.worker.lock().unwrap().take();
        if let Some(worker) = worker {
            let _ = worker.await;
        }
    }

    async fn process(&self, Queued { id, op, token }: Queued) {
        let Some(mut job) = self.registry.job(id) else {
            return;
        };

        if token.is_cancelled() {
            job.state = JobState::Cancelled;
            self.finish(job);
            return;
        }

        job.state = JobState::Running;
        self.registry.set_job(job.clone());
        klog!(
            "transaction {}: running {:?} for {:?}",
            id,
            job.kind,
            job.app_ids
        );

        let reg = self.registry.clone();
        let sink: crate::flatpak::ProgressSink =
            Arc::new(move |p| reg.set_job_progress(id, p.fraction, &p.status));

        let result = self.backend.run(op.clone(), sink, token.clone()).await;

        let mut job = self.registry.job(id).unwrap_or(job);
        match result {
            Ok(()) => {
                job.state = JobState::Done;
                job.progress = 1.0;
                if let Operation::Update { ids } = &op {
                    self.registry.clear_updates(ids);
                }
            }
            Err(FlatpakError::Cancelled) => job.state = JobState::Cancelled,
            Err(FlatpakError::Failed(e)) => {
                kerr!("transaction {} failed: {}", id, e);
                job.state = JobState::Failed(e);
            }
        }
        // Reload before announcing completion so listeners that re-read
        // the registry on "done" see the new installed state.
        if let Err(e) = self.registry.reload_installed(&*self.backend).await {
            kerr!("transaction {}: reloading installed refs failed: {}", id, e);
        }
        self.finish(job);
    }

    fn finish(&self, job: Job) {
        klog!("transaction {}: {}", job.id, job.state.as_str());
        self.operations.lock().unwrap().remove(&job.id);
        self.tokens.lock().unwrap().remove(&job.id);
        self.registry.set_job(job);
    }
}

impl Drop for TransactionManager {
    fn drop(&mut self) {
        self.stopping.cancel();
        for token in self.tokens.get_mut().unwrap().values() {
            token.cancel();
        }
    }
}

// ── Client used by the GUI ──────────────────────────────────────────────────

/// How the GUI submits transactions: to the daemon over D-Bus when it is
/// reachable, otherwise in-process.
pub enum Transactions {
    Local(Arc<TransactionManager>),
    Remote(crate::daemon::client::DaemonClient),
}

impl Transactions {
    pub async fn enqueue(&self, op: Operation, title: &str) -> Result<u64, String> {
        match self {
            Transactions::Local(m) => m.try_enqueue(op, title),
            Transactions::Remote(c) => c.enqueue(op, title).await,
        }
    }

    pub async fn cancel(&self, id: u64) -> bool {
        match self {
            Transactions::Local(m) => m.cancel(id),
            Transactions::Remote(c) => c.cancel(id).await,
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Transactions::Remote(_))
    }
}

static CLIENT: OnceLock<Arc<Transactions>> = OnceLock::new();

/// The GUI's transaction client. Connects to the daemon on first use and
/// falls back to an in-process queue.
pub async fn client() -> Arc<Transactions> {
    if let Some(c) = CLIENT.get() {
        return c.clone();
    }
    let t = match crate::daemon::client::DaemonClient::connect(crate::registry::registry()).await {
        Ok(c) => {
            klog!("transactions: using kiosque-update daemon");
            Transactions::Remote(c)
        }
        Err(e) => {
            klog!(
                "transactions: daemon unavailable ({}), running in-process",
                e
            );
            Transactions::Local(local_manager())
        }
    };
    CLIENT.get_or_init(|| Arc::new(t)).clone()
}

/// The in-process manager (used by the daemon itself and as GUI fallback).
pub fn local_manager() -> Arc<TransactionManager> {
    static MGR: OnceLock<Arc<TransactionManager>> = OnceLock::new();
    MGR.get_or_init(|| {
        TransactionManager::new(crate::flatpak::backend(), crate::registry::registry())
    })
    .clone()
}

/// Pick the remote and scope to install `app_id` from: the origin of an
/// existing installation, else the given remote, else the first enumerable
/// remote that carries the app in its AppStream data, else `flathub`.
pub fn install_target(app_id: &str, remote_hint: &str) -> (String, Scope) {
    let reg = crate::registry::registry();
    if let Some(app) = reg.state(app_id).installed {
        return (app.origin, app.scope);
    }
    let catalog = crate::catalog::catalog();
    let entry = catalog.get(app_id);
    let remote = if !remote_hint.is_empty() {
        remote_hint.to_string()
    } else if let Some(e) = &entry {
        e.remote.clone()
    } else {
        "flathub".to_string()
    };
    let scope = catalog
        .remote_scope(&remote)
        .unwrap_or_else(|| entry.map(|e| e.scope).unwrap_or(Scope::User));
    (remote, scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatpak::mock::MockBackend;
    use std::time::Duration;

    async fn wait_for(reg: &Registry, id: u64) -> Job {
        let mut events = reg.subscribe();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(job) = reg.job(id).filter(|job| !job.state.is_active()) {
                    return job;
                }
                let _ = events.recv().await.expect("registry closed");
            }
        })
        .await
        .expect("job did not finish")
    }

    fn setup(mock: MockBackend) -> (Arc<MockBackend>, Arc<Registry>, Arc<TransactionManager>) {
        let mock = Arc::new(mock);
        let reg = Arc::new(Registry::new());
        let mgr = TransactionManager::new(mock.clone(), reg.clone());
        (mock, reg, mgr)
    }

    #[tokio::test]
    async fn install_updates_registry() {
        let (mock, reg, mgr) = setup(MockBackend::default());
        let id = mgr.enqueue(
            Operation::Install {
                remote: "beta".into(),
                id: "org.a.A".into(),
                scope: Scope::User,
            },
            "A",
        );
        let job = wait_for(&reg, id).await;
        assert_eq!(job.state, JobState::Done);
        assert_eq!(job.progress, 1.0);
        let installed = reg.state("org.a.A").installed.unwrap();
        assert_eq!(installed.origin, "beta");
        assert_eq!(mock.ran.lock().unwrap().len(), 1);
        mgr.shutdown().await;
    }

    #[tokio::test]
    async fn jobs_run_in_order_and_dedupe() {
        let mock = MockBackend::default();
        *mock.run_duration.lock().unwrap() = Some(Duration::from_millis(50));
        let (mock, reg, mgr) = setup(mock);
        let op = |id: &str| Operation::Install {
            remote: "flathub".into(),
            id: id.into(),
            scope: Scope::User,
        };
        let a = mgr.enqueue(op("a"), "");
        let a2 = mgr.enqueue(op("a"), "");
        let b = mgr.enqueue(op("b"), "");
        assert_eq!(a, a2, "duplicate operation must be merged");
        assert_eq!(reg.job(b).unwrap().state, JobState::Queued);
        wait_for(&reg, b).await;
        let ran = mock.ran.lock().unwrap().clone();
        assert_eq!(ran, vec![op("a"), op("b")]);
        mgr.shutdown().await;
    }

    #[tokio::test]
    async fn cancel_running_and_queued() {
        let mock = MockBackend::default();
        *mock.run_duration.lock().unwrap() = Some(Duration::from_secs(5));
        let (mock, reg, mgr) = setup(mock);
        let op = |id: &str| Operation::Install {
            remote: "flathub".into(),
            id: id.into(),
            scope: Scope::User,
        };
        let a = mgr.enqueue(op("a"), "");
        let b = mgr.enqueue(op("b"), "");
        let mut events = reg.subscribe();
        tokio::time::timeout(Duration::from_secs(3), async {
            while reg.job(a).unwrap().state != JobState::Running {
                let _ = events.recv().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert!(mgr.cancel(b));
        assert!(mgr.cancel(a));
        assert_eq!(wait_for(&reg, a).await.state, JobState::Cancelled);
        assert_eq!(wait_for(&reg, b).await.state, JobState::Cancelled);
        // The queued job never reached the backend.
        assert!(
            mock.ran
                .lock()
                .unwrap()
                .iter()
                .all(|op| op.app_ids() == vec!["a"])
        );
        assert!(!reg.is_installed("a"));
        assert!(!mgr.cancel(a), "finished jobs cannot be cancelled");
        mgr.shutdown().await;
    }

    #[tokio::test]
    async fn failure_is_reported() {
        let mock = MockBackend::default();
        *mock.fail_with.lock().unwrap() = Some("no space left".into());
        let (_mock, reg, mgr) = setup(mock);
        let id = mgr.enqueue(
            Operation::Uninstall {
                id: "a".into(),
                scope: None,
            },
            "",
        );
        assert_eq!(
            wait_for(&reg, id).await.state,
            JobState::Failed("no space left".into())
        );
        mgr.shutdown().await;
    }

    #[tokio::test]
    async fn update_clears_updates() {
        let mock = MockBackend::with_installed(vec![crate::flatpak::InstalledApp {
            id: "a".into(),
            ..Default::default()
        }]);
        let (mock, reg, mgr) = setup(mock);
        reg.reload_installed(&*mock).await.unwrap();
        reg.set_updates(vec![crate::flatpak::UpdateInfo {
            id: "a".into(),
            ..Default::default()
        }]);
        let id = mgr.enqueue(
            Operation::Update {
                ids: vec!["a".into()],
            },
            "",
        );
        wait_for(&reg, id).await;
        assert!(reg.state("a").update.is_none());
        mgr.shutdown().await;
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::flatpak::mock::MockBackend;

    #[tokio::test]
    async fn shutdown_cancels_and_joins_pending_work_and_scope_is_not_deduplicated() {
        let backend = Arc::new(MockBackend::default());
        *backend.gate.lock().unwrap() = Some(Arc::new(tokio::sync::Semaphore::new(0)));
        let registry = Arc::new(Registry::new());
        let manager = TransactionManager::new(backend, registry.clone());
        let operation = |scope| Operation::Install {
            remote: "fixture".into(),
            id: "org.kiosque.Fixture".into(),
            scope,
        };
        let user = manager.enqueue(operation(Scope::User), "user");
        let system = manager.enqueue(operation(Scope::System), "system");
        assert_ne!(user, system);
        tokio::time::timeout(std::time::Duration::from_secs(3), manager.shutdown())
            .await
            .unwrap();
        assert!(registry.active_jobs().is_empty());
        assert_eq!(registry.job(user).unwrap().state, JobState::Cancelled);
        assert_eq!(registry.job(system).unwrap().state, JobState::Cancelled);
        let after_shutdown = manager.enqueue(operation(Scope::User), "closed");
        assert_eq!(
            registry.job(after_shutdown).unwrap().state,
            JobState::Cancelled
        );
    }
    #[tokio::test]
    async fn untrusted_submissions_are_bounded_before_backend_execution() {
        let mock = Arc::new(crate::flatpak::mock::MockBackend::default());
        *mock.gate.lock().unwrap() = Some(Arc::new(tokio::sync::Semaphore::new(0)));
        let op = |id: &str| Operation::Uninstall {
            id: id.into(),
            scope: Some(Scope::User),
        };
        let reg = Arc::new(Registry::default());
        let manager = TransactionManager::new(mock.clone(), reg.clone());
        for index in 0..crate::security::MAX_OPERATIONS {
            manager
                .try_enqueue(op(&format!("app{index}")), "bounded")
                .unwrap();
        }
        assert!(manager.try_enqueue(op("overflow"), "").is_err());
        assert!(manager.try_enqueue(op("--command=sh"), "").is_err());
        assert_eq!(reg.jobs().len(), crate::security::MAX_OPERATIONS);
        manager.shutdown().await;
        assert!(reg.jobs().iter().all(|job| !job.state.is_active()));
    }
}
