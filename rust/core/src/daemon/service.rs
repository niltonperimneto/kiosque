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
    async fn ping(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<u32> {
        super::auth::authorize(&header, conn).await?;
        super::touch();
        Ok(1)
    }
    async fn install(
        &self,
        app_id: String,
        remote: String,
        scope: String,
        title: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<u64> {
        super::auth::authorize(&header, conn).await?;
        if !scope.is_empty() && Scope::parse(&scope).is_none() {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Invalid installation scope".into(),
            ));
        }
        let (remote, scope) = scope_or_default(&app_id, &remote, &scope);
        queue(
            Operation::Install {
                remote,
                id: app_id,
                scope,
            },
            &title,
        )
    }
    async fn install_ref_file(
        &self,
        data: Vec<u8>,
        scope: String,
        title: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<u64> {
        super::auth::authorize(&header, conn).await?;
        let scope = parse_scope(&scope)?;
        queue(Operation::InstallRefFile { data, scope }, &title)
    }
    async fn uninstall(
        &self,
        app_id: String,
        scope: String,
        title: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<u64> {
        super::auth::authorize(&header, conn).await?;
        let scope = if scope.is_empty() {
            None
        } else {
            Some(parse_scope(&scope)?)
        };
        queue(Operation::Uninstall { id: app_id, scope }, &title)
    }
    async fn update(
        &self,
        app_ids: Vec<String>,
        title: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<u64> {
        super::auth::authorize(&header, conn).await?;
        queue(Operation::Update { ids: app_ids }, &title)
    }
    async fn cancel(
        &self,
        job_id: u64,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<bool> {
        super::auth::authorize(&header, conn).await?;
        super::touch();
        Ok(crate::transactions::local_manager().cancel(job_id))
    }
    async fn get_jobs(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<Vec<JobWire>> {
        super::auth::authorize(&header, conn).await?;
        super::touch();
        Ok(crate::registry::registry()
            .jobs()
            .iter()
            .map(JobWire::from)
            .collect())
    }
    async fn check_updates(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<u32> {
        super::auth::authorize(&header, conn).await?;
        super::touch();
        Ok(super::check_updates().await as u32)
    }
    async fn read_installed(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<String> {
        super::auth::authorize(&header, conn).await?;
        json(
            crate::flatpak::backend()
                .list_installed()
                .await
                .map_err(failed)?,
        )
    }
    async fn read_updates(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<String> {
        super::auth::authorize(&header, conn).await?;
        json(
            crate::flatpak::backend()
                .list_updates()
                .await
                .map_err(failed)?,
        )
    }
    async fn read_remotes(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<String> {
        super::auth::authorize(&header, conn).await?;
        json(
            crate::flatpak::backend()
                .list_remotes()
                .await
                .map_err(failed)?,
        )
    }
    async fn add_repository(
        &self,
        name: String,
        data: Vec<u8>,
        scope: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        super::auth::authorize(&header, conn).await?;
        crate::security::remote_name(&name).map_err(zbus::fdo::Error::InvalidArgs)?;
        crate::security::repo_file(&data).map_err(zbus::fdo::Error::InvalidArgs)?;
        crate::flatpak::backend()
            .add_remote(
                &name,
                crate::flatpak::RemoteSource::FlatpakRepo(data),
                parse_scope(&scope)?,
            )
            .await
            .map_err(failed)
    }
    async fn remove_repository(
        &self,
        name: String,
        scope: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        super::auth::authorize(&header, conn).await?;
        crate::security::remote_name(&name).map_err(zbus::fdo::Error::InvalidArgs)?;
        let scope = if scope.is_empty() {
            None
        } else {
            Some(parse_scope(&scope)?)
        };
        crate::flatpak::backend()
            .remove_remote(&name, scope)
            .await
            .map_err(failed)
    }
    async fn launch_app(
        &self,
        id: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        super::auth::authorize(&header, conn).await?;
        crate::security::app_ref(&id).map_err(zbus::fdo::Error::InvalidArgs)?;
        crate::flatpak::backend().launch(&id).await.map_err(failed)
    }
    async fn refresh_metadata(
        &self,
        name: String,
        scope: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        super::auth::authorize(&header, conn).await?;
        crate::security::remote_name(&name).map_err(zbus::fdo::Error::InvalidArgs)?;
        crate::flatpak::backend()
            .refresh_appstream(&crate::flatpak::RemoteInfo {
                name,
                scope: parse_scope(&scope)?,
                ..Default::default()
            })
            .await
            .map_err(failed)
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

fn failed(error: crate::flatpak::FlatpakError) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}
fn parse_scope(scope: &str) -> zbus::fdo::Result<Scope> {
    Scope::parse(scope)
        .ok_or_else(|| zbus::fdo::Error::InvalidArgs("Invalid installation scope".into()))
}
fn queue(operation: Operation, title: &str) -> zbus::fdo::Result<u64> {
    super::touch();
    crate::transactions::local_manager()
        .try_enqueue(operation, title)
        .map_err(zbus::fdo::Error::LimitsExceeded)
}
fn json(value: impl serde::Serialize) -> zbus::fdo::Result<String> {
    let data = serde_json::to_string(&value)
        .map_err(|_| zbus::fdo::Error::Failed("Cannot serialize backend state".into()))?;
    if data.len() > crate::security::MAX_JSON_BYTES {
        return Err(zbus::fdo::Error::LimitsExceeded(
            "Backend state exceeds the response limit".into(),
        ));
    }
    Ok(data)
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
