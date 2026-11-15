//! GUI-side client of the daemon.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use zbus::proxy;

use super::JobWire;
use crate::flatpak::Operation;
use crate::registry::Registry;

#[proxy(
    interface = "org.kiosque.Kiosque.Daemon1",
    default_service = "org.kiosque.Kiosque.Daemon",
    default_path = "/org/kiosque/Kiosque/Daemon"
)]
pub trait Daemon {
    fn ping(&self) -> zbus::Result<u32>;
    fn install(&self, app_id: &str, remote: &str, scope: &str, title: &str) -> zbus::Result<u64>;
    fn install_ref_file(&self, data: &[u8], scope: &str, title: &str) -> zbus::Result<u64>;
    fn uninstall(&self, app_id: &str, scope: &str, title: &str) -> zbus::Result<u64>;
    fn update(&self, app_ids: &[String], title: &str) -> zbus::Result<u64>;
    fn cancel(&self, job_id: u64) -> zbus::Result<bool>;
    fn get_jobs(&self) -> zbus::Result<Vec<JobWire>>;
    fn check_updates(&self) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn job_changed(&self, job: JobWire) -> zbus::Result<()>;
}

pub struct DaemonClient {
    proxy: DaemonProxy<'static>,
}

impl DaemonClient {
    /// Connect (D-Bus activating the daemon if needed) and mirror its jobs
    /// into `registry`.
    pub async fn connect(registry: Arc<Registry>) -> Result<Self, String> {
        if std::env::var_os("KIOSQUE_NO_DAEMON").is_some() {
            return Err("disabled by KIOSQUE_NO_DAEMON".into());
        }
        let conn = zbus::Connection::session()
            .await
            .map_err(|e| e.to_string())?;
        let proxy = DaemonProxy::new(&conn).await.map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(10), proxy.ping())
            .await
            .map_err(|_| "timed out".to_string())?
            .map_err(|e| e.to_string())?;

        let mut stream = proxy
            .receive_job_changed()
            .await
            .map_err(|e| e.to_string())?;
        for job in proxy.get_jobs().await.unwrap_or_default() {
            registry.set_job(job.into());
        }
        tokio::spawn(async move {
            while let Some(sig) = stream.next().await {
                if let Ok(args) = sig.args() {
                    registry.set_job(args.job.clone().into());
                }
            }
        });
        Ok(Self { proxy })
    }

    pub async fn enqueue(&self, op: Operation, title: &str) -> Result<u64, String> {
        let res = match &op {
            Operation::Install { remote, id, scope } => {
                self.proxy.install(id, remote, scope.as_str(), title).await
            }
            Operation::InstallRefFile { data, scope } => {
                self.proxy
                    .install_ref_file(data, scope.as_str(), title)
                    .await
            }
            Operation::Uninstall { id, scope } => {
                self.proxy
                    .uninstall(id, scope.map(|s| s.as_str()).unwrap_or(""), title)
                    .await
            }
            Operation::Update { ids } => self.proxy.update(ids, title).await,
        };
        res.map_err(|e| e.to_string())
    }

    pub async fn cancel(&self, id: u64) -> bool {
        self.proxy.cancel(id).await.unwrap_or(false)
    }
}
