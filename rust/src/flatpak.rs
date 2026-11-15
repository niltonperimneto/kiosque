//! Legacy UI conversions over the shared backend/transaction services.
pub use kiosque_core::flatpak::*;

pub mod cli {
    use super::*;
    use crate::services::services;
    pub use kiosque_core::flatpak::cli::{CliBackend, flatpak_command};
    use std::sync::Arc;

    pub struct UiApp {
        pub application_id: String,
        pub name: String,
        pub version: Option<String>,
        pub installed_size: Option<String>,
        pub origin: Option<String>,
        pub options: Option<String>,
        pub ref_string: Option<String>,
    }
    fn app(app: InstalledApp) -> UiApp {
        UiApp {
            application_id: app.id.clone(),
            name: app.name.clone(),
            version: Some(app.version.clone()),
            installed_size: Some(format!("{} B", app.installed_size)),
            origin: Some(app.origin.clone()),
            options: Some(if app.is_runtime() { "runtime" } else { "app" }.into()),
            ref_string: Some(app.ref_string()),
        }
    }
    pub async fn list_installed_cached() -> std::result::Result<Vec<UiApp>, String> {
        services()
            .registry
            .reload_installed(&*services().backend)
            .await
            .map_err(|e| e.to_string())?;
        Ok(services()
            .registry
            .installed()
            .into_iter()
            .map(app)
            .collect())
    }
    pub async fn is_installed_cached(id: &str) -> bool {
        if !services().registry.installed_loaded() {
            let _ = list_installed_cached().await;
        }
        services().registry.is_installed(id)
    }
    pub async fn list_updates() -> std::result::Result<Vec<UiApp>, String> {
        services()
            .registry
            .reload_updates(&*services().backend)
            .await
            .map_err(|e| e.to_string())?;
        Ok(services()
            .registry
            .updates()
            .into_iter()
            .map(|u| UiApp {
                application_id: u.id,
                name: u.name,
                version: Some(u.version),
                installed_size: None,
                origin: Some(u.origin),
                options: None,
                ref_string: Some(u.ref_string),
            })
            .collect())
    }
    pub async fn list_remotes() -> std::result::Result<Vec<RemoteInfo>, String> {
        services()
            .backend
            .list_remotes()
            .await
            .map_err(|e| e.to_string())
    }
    pub async fn add_repository(name: &str, url: &str) -> std::result::Result<(), String> {
        services()
            .backend
            .add_remote(name, RemoteSource::Url(url.into()), Scope::User)
            .await
            .map_err(|e| e.to_string())
    }
    pub async fn remove_repository(name: &str) -> std::result::Result<(), String> {
        services()
            .backend
            .remove_remote(name, None)
            .await
            .map_err(|e| e.to_string())
    }
    pub async fn launch_app(id: &str) -> std::result::Result<(), String> {
        services()
            .backend
            .launch(id)
            .await
            .map_err(|e| e.to_string())
    }

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

    impl JobOutcome {
        pub fn into_result(self) -> std::result::Result<(), String> {
            match self {
                JobOutcome::Done => Ok(()),
                JobOutcome::Failed(error) => Err(error),
                JobOutcome::Cancelled => Err("operation cancelled".into()),
            }
        }
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
    pub async fn install_app(id: &str) -> std::result::Result<(), String> {
        let (remote, scope) = services().install_target(id);
        run(
            Operation::Install {
                remote,
                id: id.into(),
                scope,
            },
            "",
            |_, _, _| {},
        )
        .await
        .into_result()
    }
    pub async fn uninstall_app(id: &str) -> std::result::Result<(), String> {
        run(
            Operation::Uninstall {
                id: id.into(),
                scope: None,
            },
            "",
            |_, _, _| {},
        )
        .await
        .into_result()
    }
    pub async fn update_apps(
        ids: &[String],
        progress: impl Fn(f64, String, u64) + Send,
    ) -> std::result::Result<(), String> {
        run(Operation::Update { ids: ids.to_vec() }, "", progress)
            .await
            .into_result()
    }
}
