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
    async fn run(
        op: Operation,
        progress: impl Fn(String) + Send,
    ) -> std::result::Result<(), String> {
        use crate::registry::JobState;
        let mut events = services().registry.subscribe();
        let client = services().transactions().await;
        let id = client.enqueue(op, "").await?;
        let mut guard = CancelOnDrop {
            client,
            id,
            finished: false,
        };
        loop {
            if let Some(job) = services().registry.job(id) {
                progress(job.status.clone());
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
                    _ => {}
                }
            }
            match events.recv().await {
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => return Err("transaction registry closed".into()),
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
            |_| {},
        )
        .await
    }
    pub async fn uninstall_app(id: &str) -> std::result::Result<(), String> {
        run(
            Operation::Uninstall {
                id: id.into(),
                scope: None,
            },
            |_| {},
        )
        .await
    }
    pub async fn update_apps(
        ids: &[String],
        progress: impl Fn(String) + Send,
    ) -> std::result::Result<(), String> {
        run(Operation::Update { ids: ids.to_vec() }, progress).await
    }
}
