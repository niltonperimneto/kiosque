//! Narrow sandbox-to-host API. No command names, paths or environments cross it.
use super::*;
use crate::daemon::client::DaemonProxy;

pub struct BrokerBackend;
fn failure(error: impl std::fmt::Display) -> FlatpakError {
    FlatpakError::Failed(format!(
        "The native Kiosque host companion is required: {error}"
    ))
}
async fn proxy() -> Result<DaemonProxy<'static>> {
    let connection = zbus::Connection::session().await.map_err(failure)?;
    DaemonProxy::new(&connection).await.map_err(failure)
}
fn decode<T: serde::de::DeserializeOwned>(data: String) -> Result<T> {
    if data.len() > crate::security::MAX_JSON_BYTES {
        return Err(failure("response too large"));
    }
    serde_json::from_str(&data).map_err(failure)
}
#[async_trait]
impl FlatpakBackend for BrokerBackend {
    fn name(&self) -> &'static str {
        "host-companion"
    }
    async fn list_installed(&self) -> Result<Vec<InstalledApp>> {
        decode(proxy().await?.read_installed().await.map_err(failure)?)
    }
    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        decode(proxy().await?.read_updates().await.map_err(failure)?)
    }
    async fn list_remotes(&self) -> Result<Vec<RemoteInfo>> {
        decode(proxy().await?.read_remotes().await.map_err(failure)?)
    }
    async fn add_remote(&self, name: &str, source: RemoteSource, scope: Scope) -> Result<()> {
        crate::security::remote_name(name).map_err(failure)?;
        let data = match source {
            #[cfg(feature = "test-util")]
            RemoteSource::LocalFixture { .. } => {
                return Err(failure("Local fixture source is unsupported"));
            }
            RemoteSource::FlatpakRepo(data) => data,
            RemoteSource::Url(url) => crate::flathub::client::HttpClient::default()
                .fetch_bytes_limited(&url, crate::security::MAX_REF_BYTES)
                .await
                .map_err(failure)?,
        };
        crate::security::repo_file(&data).map_err(failure)?;
        proxy()
            .await?
            .add_repository(name, &data, scope.as_str())
            .await
            .map_err(failure)
    }
    async fn remove_remote(&self, name: &str, scope: Option<Scope>) -> Result<()> {
        crate::security::remote_name(name).map_err(failure)?;
        proxy()
            .await?
            .remove_repository(name, scope.map(Scope::as_str).unwrap_or(""))
            .await
            .map_err(failure)
    }
    async fn plan(&self, op: &Operation) -> Result<OperationPlan> {
        crate::security::operation(op, "").map_err(failure)?;
        let proxy = proxy().await?;
        let data = match op {
            Operation::Install { remote, id, scope } => {
                proxy.plan_install(id, remote, scope.as_str()).await
            }
            Operation::Uninstall { id, scope } => {
                proxy
                    .plan_uninstall(id, scope.map(Scope::as_str).unwrap_or(""))
                    .await
            }
            _ => return Err(failure("this operation cannot be planned")),
        }
        .map_err(failure)?;
        decode(data)
    }
    async fn runtimes(&self) -> Result<Vec<InstalledRuntime>> {
        decode(proxy().await?.read_runtimes().await.map_err(failure)?)
    }
    async fn run(
        &self,
        op: Operation,
        progress: ProgressSink,
        cancel: CancellationToken,
    ) -> Result<()> {
        crate::security::operation(&op, "").map_err(failure)?;
        if cancel.is_cancelled() {
            return Err(FlatpakError::Cancelled);
        }
        let registry = Arc::new(crate::registry::Registry::default());
        let client = crate::daemon::client::DaemonClient::connect(registry.clone())
            .await
            .map_err(failure)?;
        let mut events = registry.subscribe();
        let id = client
            .enqueue(op, "Kiosque transaction")
            .await
            .map_err(failure)?;
        loop {
            if let Some(job) = registry.job(id) {
                progress(Progress {
                    fraction: job.progress,
                    status: job.status,
                    current_ref: String::new(),
                });
                match job.state {
                    crate::registry::JobState::Done => return Ok(()),
                    crate::registry::JobState::Failed(error) => {
                        return Err(FlatpakError::Failed(error));
                    }
                    crate::registry::JobState::Cancelled => return Err(FlatpakError::Cancelled),
                    _ => {}
                }
            }
            tokio::select! {
                _ = cancel.cancelled() => { client.cancel(id).await; return Err(FlatpakError::Cancelled); }
                event = events.recv() => { if matches!(event, Err(broadcast::error::RecvError::Closed)) { return Err(failure("connection closed")); } }
            }
        }
    }
    async fn launch(&self, id: &str) -> Result<()> {
        crate::security::app_ref(id).map_err(failure)?;
        proxy().await?.launch_app(id).await.map_err(failure)
    }
    async fn refresh_appstream(&self, remote: &RemoteInfo) -> Result<()> {
        proxy()
            .await?
            .refresh_metadata(&remote.name, remote.scope.as_str())
            .await
            .map_err(failure)
    }
}

#[cfg(not(feature = "libflatpak"))]
pub(super) struct OutsideSandbox;

#[cfg(not(feature = "libflatpak"))]
fn outside_sandbox<T>() -> Result<T> {
    Err(FlatpakError::Failed("This build requires a Flatpak sandbox. Install the native Kiosque package for host operation.".into()))
}

#[cfg(not(feature = "libflatpak"))]
#[async_trait]
impl FlatpakBackend for OutsideSandbox {
    fn name(&self) -> &'static str {
        "unavailable"
    }
    async fn list_installed(&self) -> Result<Vec<InstalledApp>> {
        outside_sandbox()
    }
    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        outside_sandbox()
    }
    async fn list_remotes(&self) -> Result<Vec<RemoteInfo>> {
        outside_sandbox()
    }
    async fn add_remote(&self, _: &str, _: RemoteSource, _: Scope) -> Result<()> {
        outside_sandbox()
    }
    async fn remove_remote(&self, _: &str, _: Option<Scope>) -> Result<()> {
        outside_sandbox()
    }
    async fn run(&self, _: Operation, _: ProgressSink, _: CancellationToken) -> Result<()> {
        outside_sandbox()
    }
    async fn launch(&self, _: &str) -> Result<()> {
        outside_sandbox()
    }
    async fn refresh_appstream(&self, _: &RemoteInfo) -> Result<()> {
        outside_sandbox()
    }
    fn spawn_monitor(&self, _: broadcast::Sender<()>) {}
}
