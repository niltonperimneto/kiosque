//! In-memory [`FlatpakBackend`] for unit tests.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::{
    FlatpakBackend, FlatpakError, InstalledApp, Operation, Progress, ProgressSink, RemoteInfo,
    RemoteSource, Result, Scope, UpdateInfo,
};

#[derive(Default)]
pub struct MockBackend {
    pub installed: Mutex<Vec<InstalledApp>>,
    pub updates: Mutex<Vec<UpdateInfo>>,
    pub list_updates_error: Mutex<Option<String>>,
    pub remotes: Mutex<Vec<RemoteInfo>>,
    /// Every operation passed to `run`, in order.
    pub ran: Mutex<Vec<Operation>>,
    /// When set, `run` takes this long (in 10 steps) so tests can cancel it.
    pub run_duration: Mutex<Option<Duration>>,
    /// When set, `run` fails with this message.
    pub fail_with: Mutex<Option<String>>,
    pub launched: Mutex<Vec<String>>,
    /// A permit releases one transaction; cancellation also releases the waiter.
    pub gate: Mutex<Option<std::sync::Arc<tokio::sync::Semaphore>>>,
}

impl MockBackend {
    pub fn with_installed(apps: Vec<InstalledApp>) -> Self {
        let m = Self::default();
        *m.installed.lock().unwrap() = apps;
        m
    }
}

#[async_trait]
impl FlatpakBackend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn list_installed(&self) -> Result<Vec<InstalledApp>> {
        Ok(self.installed.lock().unwrap().clone())
    }

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        if let Some(error) = self.list_updates_error.lock().unwrap().clone() {
            return Err(FlatpakError::Failed(error));
        }
        Ok(self.updates.lock().unwrap().clone())
    }

    async fn list_remotes(&self) -> Result<Vec<RemoteInfo>> {
        Ok(self.remotes.lock().unwrap().clone())
    }

    async fn add_remote(&self, name: &str, source: RemoteSource, scope: Scope) -> Result<()> {
        let url = match source {
            #[cfg(feature = "test-util")]
            RemoteSource::LocalFixture { .. } => {
                return Err(FlatpakError::Failed(
                    "Local fixture source is unsupported".into(),
                ));
            }
            RemoteSource::Url(u) => u,
            RemoteSource::FlatpakRepo(d) => super::keyfile_value(&d, "Url").unwrap_or_default(),
        };
        self.remotes.lock().unwrap().push(RemoteInfo {
            name: name.into(),
            url,
            scope,
            ..Default::default()
        });
        Ok(())
    }

    async fn remove_remote(&self, name: &str, scope: Option<Scope>) -> Result<()> {
        let mut r = self.remotes.lock().unwrap();
        let before = r.len();
        r.retain(|x| x.name != name || scope.is_some_and(|scope| x.scope != scope));
        if r.len() == before {
            Err(FlatpakError::Failed("no such remote".into()))
        } else {
            Ok(())
        }
    }

    async fn run(
        &self,
        op: Operation,
        progress: ProgressSink,
        cancel: CancellationToken,
    ) -> Result<()> {
        if cancel.is_cancelled() {
            return Err(FlatpakError::Cancelled);
        }
        self.ran.lock().unwrap().push(op.clone());
        let gate = self.gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            tokio::select! {
                _ = cancel.cancelled() => return Err(FlatpakError::Cancelled),
                permit = gate.acquire_owned() => { permit.map_err(|_| FlatpakError::Failed("gate closed".into()))?.forget(); }
            }
        }
        let duration = *self.run_duration.lock().unwrap();
        if let Some(d) = duration {
            for i in 1..=10 {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(FlatpakError::Cancelled),
                    _ = tokio::time::sleep(d / 10) => {}
                }
                progress(Progress {
                    fraction: i as f64 / 10.0,
                    status: format!("step {}", i),
                    current_ref: String::new(),
                });
            }
        } else {
            progress(Progress {
                fraction: 1.0,
                ..Default::default()
            });
        }
        if let Some(msg) = self.fail_with.lock().unwrap().clone() {
            return Err(FlatpakError::Failed(msg));
        }
        let mut installed = self.installed.lock().unwrap();
        match op {
            Operation::Install { id, remote, scope } => installed.push(InstalledApp {
                name: id.clone(),
                id,
                origin: remote,
                scope,
                ..Default::default()
            }),
            Operation::Uninstall { id, scope } => {
                installed.retain(|a| a.id != id || scope.is_some_and(|s| s != a.scope))
            }
            Operation::Update { ids } => {
                self.updates
                    .lock()
                    .unwrap()
                    .retain(|u| !ids.is_empty() && !ids.contains(&u.id));
            }
            Operation::InstallRefFile { data, scope } => installed.push(InstalledApp {
                id: super::keyfile_value(&data, "Name").unwrap_or_default(),
                scope,
                ..Default::default()
            }),
        }
        Ok(())
    }

    async fn launch(&self, id: &str) -> Result<()> {
        self.launched.lock().unwrap().push(id.into());
        Ok(())
    }

    async fn refresh_appstream(&self, _remote: &RemoteInfo) -> Result<()> {
        Ok(())
    }

    fn spawn_monitor(&self, _tx: broadcast::Sender<()>) {}
}
