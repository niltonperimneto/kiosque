//! `org.kiosque.Kiosque.Daemon1` D-Bus interface.

use zbus::interface;

use super::JobWire;
use crate::flatpak::{Operation, Scope};
use crate::registry::Event;

pub struct Service;

fn scope_or_default(app_id: &str, remote: &str, scope: &str) -> (String, Scope) {
    let (default_remote, default_scope) = crate::transactions::install_target(app_id, remote);
    (default_remote, Scope::parse(scope).unwrap_or(default_scope))
}

#[interface(name = "org.kiosque.Kiosque.Daemon1")]
impl Service {
    /// No-op used by clients to check that the daemon is reachable.
    async fn ping(&self) -> u32 {
        super::touch();
        1
    }

    /// Queue an install. Empty `remote`/`scope` pick the app's origin from
    /// the catalog. Returns the job id.
    async fn install(&self, app_id: String, remote: String, scope: String, title: String) -> u64 {
        super::touch();
        let (remote, scope) = scope_or_default(&app_id, &remote, &scope);
        crate::transactions::local_manager().enqueue(
            Operation::Install {
                remote,
                id: app_id,
                scope,
            },
            &title,
        )
    }

    /// Queue an install from the contents of a `.flatpakref` file.
    async fn install_ref_file(&self, data: Vec<u8>, scope: String, title: String) -> u64 {
        super::touch();
        let scope = Scope::parse(&scope).unwrap_or(Scope::User);
        crate::transactions::local_manager()
            .enqueue(Operation::InstallRefFile { data, scope }, &title)
    }

    async fn uninstall(&self, app_id: String, scope: String, title: String) -> u64 {
        super::touch();
        crate::transactions::local_manager().enqueue(
            Operation::Uninstall {
                id: app_id,
                scope: Scope::parse(&scope),
            },
            &title,
        )
    }

    /// Update the given ids, or everything when the list is empty.
    async fn update(&self, app_ids: Vec<String>, title: String) -> u64 {
        super::touch();
        crate::transactions::local_manager().enqueue(Operation::Update { ids: app_ids }, &title)
    }

    async fn cancel(&self, job_id: u64) -> bool {
        super::touch();
        crate::transactions::local_manager().cancel(job_id)
    }

    /// All queued, running and recently finished jobs.
    async fn get_jobs(&self) -> Vec<JobWire> {
        super::touch();
        crate::registry::registry()
            .jobs()
            .iter()
            .map(JobWire::from)
            .collect()
    }

    /// Re-check for updates; returns the number of app updates.
    async fn check_updates(&self) -> u32 {
        super::touch();
        super::check_updates().await as u32
    }

    #[zbus(property)]
    async fn busy(&self) -> bool {
        crate::transactions::local_manager().is_busy()
    }

    #[zbus(signal)]
    async fn job_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        job: JobWire,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn updates_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        count: u32,
    ) -> zbus::Result<()>;
}

/// Re-emit registry events as D-Bus signals.
pub async fn forward_events(conn: zbus::Connection) {
    let mut rx = crate::registry::registry().subscribe();
    let emitter = match zbus::object_server::SignalEmitter::new(&conn, super::OBJECT_PATH) {
        Ok(e) => e,
        Err(e) => {
            kerr!("daemon: cannot create signal emitter: {}", e);
            return;
        }
    };
    loop {
        let res = match rx.recv().await {
            Ok(Event::Job(job)) => Service::job_changed(&emitter, JobWire::from(&job)).await,
            Ok(Event::Updates) => {
                Service::updates_changed(&emitter, super::app_update_count() as u32).await
            }
            Ok(Event::Installed) => Ok(()),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => Ok(()),
            Err(_) => break,
        };
        if let Err(e) = res {
            kerr!("daemon: emitting signal failed: {}", e);
        }
    }
}
