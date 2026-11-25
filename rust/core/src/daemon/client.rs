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
    fn read_installed(&self) -> zbus::Result<String>;
    fn read_updates(&self) -> zbus::Result<String>;
    fn read_remotes(&self) -> zbus::Result<String>;
    fn add_repository(&self, name: &str, data: &[u8], scope: &str) -> zbus::Result<()>;
    fn remove_repository(&self, name: &str, scope: &str) -> zbus::Result<()>;
    fn launch_app(&self, id: &str) -> zbus::Result<()>;
    fn refresh_metadata(&self, name: &str, scope: &str) -> zbus::Result<()>;
    fn ping(&self) -> zbus::Result<u32>;
    fn install(&self, app_id: &str, remote: &str, scope: &str, title: &str) -> zbus::Result<u64>;
    fn install_ref_file(&self, data: &[u8], scope: &str, title: &str) -> zbus::Result<u64>;
    fn uninstall(&self, app_id: &str, scope: &str, title: &str) -> zbus::Result<u64>;
    fn update(&self, app_ids: &[String], title: &str) -> zbus::Result<u64>;
    fn cancel(&self, job_id: u64) -> zbus::Result<bool>;
    fn get_jobs(&self) -> zbus::Result<Vec<JobWire>>;
    fn check_updates(&self) -> zbus::Result<u32>;
    fn configure_automatic_updates(
        &self,
        enabled: bool,
        frequency: &str,
        time: &str,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn api_version(&self) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn job_changed(&self, job: JobWire) -> zbus::Result<()>;
}

pub struct DaemonClient {
    proxy: DaemonProxy<'static>,
    forwarding: tokio::task::JoinHandle<()>,
}

impl Drop for DaemonClient {
    fn drop(&mut self) {
        self.forwarding.abort();
    }
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
        let monitor = proxy.clone();
        let forwarding = tokio::spawn(async move {
            let mut health = tokio::time::interval(Duration::from_secs(15));
            loop {
                tokio::select! {
                    signal = stream.next() => match signal {
                        Some(signal) => if let Ok(args) = signal.args() { registry.set_job(args.job.clone().into()); },
                        None => break,
                    },
                    _ = health.tick() => {
                        if !matches!(tokio::time::timeout(Duration::from_secs(5), monitor.ping()).await, Ok(Ok(1))) { break; }
                    }
                }
            }
            for mut job in registry.jobs() {
                if job.state.is_active() {
                    job.state =
                        crate::registry::JobState::Failed("Host companion disconnected".into());
                    registry.set_job(job);
                }
            }
        });
        Ok(Self { proxy, forwarding })
    }

    pub async fn enqueue(&self, op: Operation, title: &str) -> Result<u64, String> {
        crate::security::operation(&op, title)?;
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

/// Why the host companion cannot do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionError {
    /// Nothing provides `org.kiosque.Kiosque.Daemon` on the session bus.
    Missing,
    /// The companion is older than this application needs.
    Outdated {
        found: u32,
        required: u32,
    },
    Failed(String),
}

impl std::fmt::Display for CompanionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompanionError::Missing => write!(
                f,
                "The Kiosque companion is not installed on this computer. Install the \
                 kiosque-companion package from your distribution."
            ),
            CompanionError::Outdated { found, required } => write!(
                f,
                "The Kiosque companion on this computer is too old (interface version {found}, \
                 version {required} needed). Update the kiosque-companion package."
            ),
            CompanionError::Failed(error) => {
                write!(f, "The Kiosque companion could not be reached: {error}")
            }
        }
    }
}

impl std::error::Error for CompanionError {}

fn error_name(error: &zbus::Error) -> Option<String> {
    use zbus::DBusError;
    match error {
        zbus::Error::MethodError(name, _, _) => Some(name.to_string()),
        zbus::Error::FDO(error) => Some(error.name().to_string()),
        _ => None,
    }
}

/// Classify a failed `ApiVersion` read: no companion at all, or one that
/// predates the property (version 1).
pub fn classify_version_error(error: &zbus::Error) -> Result<u32, CompanionError> {
    match error_name(error).as_deref() {
        Some(
            "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.Spawn.ServiceNotFound"
            | "org.freedesktop.DBus.Error.Spawn.ExecFailed"
            | "org.freedesktop.DBus.Error.Spawn.ChildExited",
        ) => Err(CompanionError::Missing),
        Some(
            "org.freedesktop.DBus.Error.UnknownProperty"
            | "org.freedesktop.DBus.Error.UnknownInterface"
            | "org.freedesktop.DBus.Error.InvalidArgs",
        ) => Ok(1),
        _ => Err(CompanionError::Failed(error.to_string())),
    }
}

/// Connect to the host companion (D-Bus activating it) and check that it is
/// recent enough for this application.
pub async fn companion() -> Result<DaemonProxy<'static>, CompanionError> {
    connect_checked().await.map(|(proxy, _)| proxy)
}

/// The companion's interface version, if it is installed and recent enough.
pub async fn companion_version() -> Result<u32, CompanionError> {
    connect_checked().await.map(|(_, version)| version)
}

async fn connect_checked() -> Result<(DaemonProxy<'static>, u32), CompanionError> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| CompanionError::Failed(e.to_string()))?;
    let proxy = DaemonProxy::builder(&conn)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .map_err(|e| CompanionError::Failed(e.to_string()))?;
    let version = match tokio::time::timeout(Duration::from_secs(30), proxy.api_version()).await {
        Ok(Ok(version)) => version,
        Ok(Err(error)) => classify_version_error(&error)?,
        Err(_) => {
            return Err(CompanionError::Failed(
                "it did not answer in time".to_string(),
            ));
        }
    };
    if version < super::REQUIRED_API_VERSION {
        return Err(CompanionError::Outdated {
            found: version,
            required: super::REQUIRED_API_VERSION,
        });
    }
    Ok((proxy, version))
}

/// Send the automatic-update settings to the daemon (D-Bus activating it),
/// which saves them and stays running in the background while they are on.
pub async fn configure_automatic_updates(
    settings: &crate::settings::AppSettings,
) -> Result<(), CompanionError> {
    let proxy = companion().await?;
    tokio::time::timeout(
        Duration::from_secs(30),
        proxy.configure_automatic_updates(
            settings.auto_update,
            &settings.update_frequency,
            &settings.update_time,
        ),
    )
    .await
    .map_err(|_| CompanionError::Failed("it did not answer in time".to_string()))?
    .map_err(|e| CompanionError::Failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fdo(error: zbus::fdo::Error) -> zbus::Error {
        zbus::Error::FDO(Box::new(error))
    }

    #[test]
    fn missing_and_outdated_companions_are_told_apart() {
        assert_eq!(
            classify_version_error(&fdo(zbus::fdo::Error::ServiceUnknown("x".into()))),
            Err(CompanionError::Missing)
        );
        assert_eq!(
            classify_version_error(&fdo(zbus::fdo::Error::NameHasNoOwner("x".into()))),
            Err(CompanionError::Missing)
        );
        // Companions from before the property was added.
        assert_eq!(
            classify_version_error(&fdo(zbus::fdo::Error::UnknownProperty("x".into()))),
            Ok(1)
        );
        assert_eq!(
            classify_version_error(&fdo(zbus::fdo::Error::InvalidArgs("x".into()))),
            Ok(1)
        );
        assert!(matches!(
            classify_version_error(&fdo(zbus::fdo::Error::AccessDenied("x".into()))),
            Err(CompanionError::Failed(_))
        ));
    }

    #[test]
    fn companion_errors_name_the_package_to_install_or_update() {
        assert!(
            CompanionError::Missing
                .to_string()
                .contains("kiosque-companion")
        );
        let outdated = CompanionError::Outdated {
            found: 1,
            required: 2,
        }
        .to_string();
        assert!(outdated.contains("too old") && outdated.contains("kiosque-companion"));
    }
}
