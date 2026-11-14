//! The Kiosque daemon (`kiosque-update`).
//!
//! Owns the transaction queue, update checks and automatic updates, and
//! exposes them on the session bus as `org.kiosque.Kiosque.Daemon`:
//!
//! * `/org/kiosque/Kiosque/Daemon`, interface `org.kiosque.Kiosque.Daemon1`
//!   — install/uninstall/update/cancel, `JobChanged` and `UpdatesChanged`
//!   signals. The GUI is a client of this, so transactions keep running
//!   after the window is closed.
//! * `/runner`, interface `org.kde.krunner1` — a KRunner D-Bus runner that
//!   searches the local AppStream catalog.
//!
//! The `kiosque-update` binary provides the tray icon and desktop
//! notifications; it drives this module through [`start`], [`subscribe`],
//! [`check_updates_now`], [`update_all`] and [`run_once`].

pub mod client;
pub mod runner;
pub mod service;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use chrono::{Datelike, Local, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use zbus::Type;

use crate::registry::{Event, Job, JobKind, JobState};

pub const BUS_NAME: &str = "org.kiosque.Kiosque.Daemon";
pub const OBJECT_PATH: &str = "/org/kiosque/Kiosque/Daemon";
pub const INTERFACE: &str = "org.kiosque.Kiosque.Daemon1";
pub const RUNNER_PATH: &str = "/runner";

/// How long an on-demand (D-Bus activated) daemon lingers without work.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How often available updates are checked.
const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

/// D-Bus representation of a [`Job`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct JobWire {
    pub id: u64,
    pub kind: String,
    pub app_ids: Vec<String>,
    pub title: String,
    pub state: String,
    pub error: String,
    pub progress: f64,
    pub status: String,
}

impl From<&Job> for JobWire {
    fn from(j: &Job) -> Self {
        JobWire {
            id: j.id,
            kind: match j.kind {
                JobKind::InstallRefFile => "install-ref".into(),
                k => k.as_str().into(),
            },
            app_ids: j.app_ids.clone(),
            title: j.title.clone(),
            state: j.state.as_str().into(),
            error: j.state.error().into(),
            progress: j.progress,
            status: j.status.clone(),
        }
    }
}

impl From<JobWire> for Job {
    fn from(w: JobWire) -> Self {
        Job {
            id: w.id,
            kind: if w.kind == "install-ref" {
                JobKind::InstallRefFile
            } else {
                JobKind::parse(&w.kind)
            },
            app_ids: w.app_ids,
            title: w.title,
            state: JobState::from_parts(&w.state, &w.error),
            progress: w.progress,
            status: w.status,
        }
    }
}

// ── Events for the shell ────────────────────────────────────────────────────

/// Something the tray/notification shell should react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    /// Number of available app (not runtime) updates.
    Updates(usize),
    /// A transaction ended; `title` includes the error on failure.
    JobFinished { ok: bool, title: String },
    /// A scheduled automatic update of this many apps started.
    AutoUpdate(usize),
    /// The daemon has been idle long enough and should quit.
    IdleExit,
}

fn events() -> &'static tokio::sync::broadcast::Sender<DaemonEvent> {
    static EVENTS: OnceLock<tokio::sync::broadcast::Sender<DaemonEvent>> = OnceLock::new();
    EVENTS.get_or_init(|| tokio::sync::broadcast::channel(64).0)
}

/// Receive [`DaemonEvent`]s. Subscribe before [`start`] to see every event.
pub fn subscribe() -> tokio::sync::broadcast::Receiver<DaemonEvent> {
    events().subscribe()
}

fn notify(event: DaemonEvent) {
    let _ = events().send(event);
}

static LAST_ACTIVITY: AtomicU64 = AtomicU64::new(0);
static BACKGROUND: AtomicBool = AtomicBool::new(false);

/// Record client activity (resets the idle timer).
pub fn touch() {
    LAST_ACTIVITY.store(crate::disk_cache::now_secs(), Ordering::SeqCst);
}

/// Number of app (not runtime) updates currently known.
pub fn app_update_count() -> usize {
    crate::registry::registry()
        .updates()
        .iter()
        .filter(|u| u.kind == crate::flatpak::RefKind::App)
        .count()
}

/// Re-check available updates and publish the result.
pub async fn check_updates() -> usize {
    let reg = crate::registry::registry();
    let backend = crate::flatpak::backend();
    if let Err(e) = reg.reload_updates(&*backend).await {
        kerr!("daemon: update check failed: {}", e);
    }
    let n = app_update_count();
    klog!("daemon: {} app updates available", n);
    notify(DaemonEvent::Updates(n));
    n
}

/// The next time automatic updates should run according to the settings.
pub fn next_auto_update(
    settings: &crate::settings::AppSettings,
    now: chrono::DateTime<Local>,
) -> Option<chrono::DateTime<Local>> {
    if !settings.auto_update {
        return None;
    }
    let time = NaiveTime::parse_from_str(&settings.update_time, "%H:%M")
        .unwrap_or(NaiveTime::from_hms_opt(2, 0, 0)?);
    let weekly = settings.update_frequency.eq_ignore_ascii_case("weekly");
    (0..=7).find_map(|d| {
        let day = now.date_naive() + chrono::Duration::days(d);
        if weekly && day.weekday() != chrono::Weekday::Mon {
            return None;
        }
        let at = Local.from_local_datetime(&day.and_time(time)).earliest()?;
        (at > now).then_some(at)
    })
}

async fn scheduler() {
    let mut next_check = tokio::time::Instant::now();
    let mut next_auto = next_auto_update(&crate::settings::load_settings(), Local::now());
    let mut catalog_refreshed = tokio::time::Instant::now() - Duration::from_secs(86400);
    loop {
        let now = tokio::time::Instant::now();
        if now >= next_check {
            check_updates().await;
            next_check = now + CHECK_INTERVAL;
        }
        if now.duration_since(catalog_refreshed) > Duration::from_secs(12 * 3600) {
            crate::catalog::refresh_if_stale().await;
            catalog_refreshed = now;
        }

        // Settings may change while we run; re-read every minute.
        let settings = crate::settings::load_settings();
        let computed = next_auto_update(&settings, Local::now());
        if computed.is_none() || next_auto.is_none() {
            next_auto = computed;
        }
        if let Some(at) = next_auto
            && Local::now() >= at
        {
            if check_updates().await > 0 || !crate::registry::registry().updates().is_empty() {
                klog!("daemon: running scheduled automatic update");
                notify(DaemonEvent::AutoUpdate(app_update_count()));
                crate::transactions::local_manager().enqueue(
                    crate::flatpak::Operation::Update { ids: vec![] },
                    "Automatic update",
                );
            }
            next_auto = next_auto_update(&settings, Local::now() + chrono::Duration::minutes(1));
        }

        // Idle exit for on-demand instances.
        let busy = crate::transactions::local_manager().is_busy();
        if busy {
            touch();
        }
        let idle =
            crate::disk_cache::now_secs().saturating_sub(LAST_ACTIVITY.load(Ordering::SeqCst));
        if !BACKGROUND.load(Ordering::SeqCst) && !busy && idle >= IDLE_TIMEOUT.as_secs() {
            klog!("daemon: idle for {}s, exiting", idle);
            notify(DaemonEvent::IdleExit);
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Forward finished jobs to the shell for notifications.
async fn job_notifier() {
    let mut rx = crate::registry::registry().subscribe();
    loop {
        match rx.recv().await {
            Ok(Event::Job(job)) => match job.state {
                JobState::Done => notify(DaemonEvent::JobFinished {
                    ok: true,
                    title: job.title.clone(),
                }),
                JobState::Failed(ref e) => notify(DaemonEvent::JobFinished {
                    ok: false,
                    title: format!("{}: {}", job.title, e),
                }),
                _ => {}
            },
            Ok(Event::Updates) => notify(DaemonEvent::Updates(app_update_count())),
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => break,
        }
    }
}

static CONNECTION: OnceLock<zbus::Connection> = OnceLock::new();

/// The daemon's session bus connection, once [`start`] has succeeded.
pub fn connection() -> Option<zbus::Connection> {
    CONNECTION.get().cloned()
}

/// Own the daemon bus name and start the scheduler. `background` keeps it
/// alive while idle (autostart); otherwise it emits [`DaemonEvent::IdleExit`]
/// after [`IDLE_TIMEOUT`]. Fails if the name is taken or the bus is
/// unreachable. Must run inside the shared runtime.
pub async fn start(background: bool) -> Result<(), String> {
    BACKGROUND.store(background, Ordering::SeqCst);
    touch();
    let conn = zbus::connection::Builder::session()
        .serve_at(OBJECT_PATH, service::Service)
        .serve_at(RUNNER_PATH, runner::Runner)
        .build()
        .await
        .map_err(|e| e.to_string())?;
    // Single instance: neither replace a running daemon nor wait in line
    // (zbus' defaults would do the former).
    match conn
        .request_name_with_flags(BUS_NAME, zbus::fdo::RequestNameFlags::DO_NOT_QUEUE)
        .await
        .map_err(|e| e.to_string())?
    {
        zbus::fdo::RequestNameReply::PrimaryOwner | zbus::fdo::RequestNameReply::AlreadyOwner => {}
        _ => {
            return Err(format!(
                "{BUS_NAME} is already owned; another instance is running"
            ));
        }
    }
    let _ = CONNECTION.set(conn.clone());
    klog!("daemon: serving {} (background={})", BUS_NAME, background);

    // Make sure the manager exists before anything is queued.
    let _ = crate::transactions::local_manager();
    tokio::spawn(service::forward_events(conn));
    tokio::spawn(job_notifier());
    tokio::spawn(scheduler());
    tokio::spawn(async {
        crate::catalog::loaded().await;
    });
    Ok(())
}

// ── Shell entry points ──────────────────────────────────────────────────────

/// Trigger an update check; the result arrives as [`DaemonEvent::Updates`].
pub fn check_updates_now() {
    touch();
    crate::runtime::runtime().spawn(async {
        check_updates().await;
    });
}

/// Queue an update of everything.
pub fn update_all() {
    touch();
    crate::transactions::local_manager().enqueue(
        crate::flatpak::Operation::Update { ids: vec![] },
        "Update all",
    );
}

/// True while transactions are queued or running.
pub fn is_busy() -> bool {
    crate::transactions::local_manager().is_busy()
}

/// One-shot mode (`kiosque-update --check`): check and apply all updates in
/// process. Returns the process exit code: 0 when up to date or updated,
/// 1 when the check or the update failed.
pub async fn run_once() -> i32 {
    run_once_with(&*crate::flatpak::backend()).await
}

/// [`run_once`] against an explicit backend.
pub async fn run_once_with(backend: &dyn crate::flatpak::FlatpakBackend) -> i32 {
    let updates = match backend.list_updates().await {
        Ok(u) => u,
        Err(e) => {
            kerr!("update check failed: {}", e);
            return 1;
        }
    };
    if updates.is_empty() {
        klog!("no updates available");
        return 0;
    }
    klog!("updating {} refs", updates.len());
    let sink: crate::flatpak::ProgressSink = std::sync::Arc::new(|p| {
        if !p.status.is_empty() {
            klog!("{:>3.0}% {}", p.fraction.max(0.0) * 100.0, p.status);
        }
    });
    match backend
        .run(
            crate::flatpak::Operation::Update { ids: vec![] },
            sink,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
    {
        Ok(()) => 0,
        Err(e) => {
            kerr!("update failed: {}", e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;

    fn settings(auto: bool, freq: &str, time: &str) -> AppSettings {
        AppSettings {
            auto_update: auto,
            update_frequency: freq.into(),
            update_time: time.into(),
            ..Default::default()
        }
    }

    #[test]
    fn auto_update_schedule() {
        // Wednesday 2026-11-11 10:00 local
        let now = Local.with_ymd_and_hms(2026, 11, 11, 10, 0, 0).unwrap();
        assert_eq!(
            next_auto_update(&settings(false, "Daily", "02:00"), now),
            None
        );
        let daily = next_auto_update(&settings(true, "Daily", "02:00"), now).unwrap();
        assert_eq!(
            daily,
            Local.with_ymd_and_hms(2026, 11, 12, 2, 0, 0).unwrap()
        );
        let later_today = next_auto_update(&settings(true, "Daily", "23:30"), now).unwrap();
        assert_eq!(
            later_today,
            Local.with_ymd_and_hms(2026, 11, 11, 23, 30, 0).unwrap()
        );
        let weekly = next_auto_update(&settings(true, "Weekly", "02:00"), now).unwrap();
        assert_eq!(
            weekly,
            Local.with_ymd_and_hms(2026, 11, 16, 2, 0, 0).unwrap()
        );
    }

    #[tokio::test]
    async fn run_once_exit_codes() {
        use crate::flatpak::{UpdateInfo, mock::MockBackend};
        let up_to_date = MockBackend::default();
        assert_eq!(run_once_with(&up_to_date).await, 0);
        assert!(up_to_date.ran.lock().unwrap().is_empty());

        let pending = MockBackend::default();
        pending.updates.lock().unwrap().push(UpdateInfo {
            id: "org.example.App".into(),
            ..Default::default()
        });
        assert_eq!(run_once_with(&pending).await, 0);
        assert_eq!(
            pending.ran.lock().unwrap().as_slice(),
            [crate::flatpak::Operation::Update { ids: vec![] }]
        );

        let failing = MockBackend::default();
        failing.updates.lock().unwrap().push(UpdateInfo::default());
        *failing.fail_with.lock().unwrap() = Some("disk full".into());
        assert_eq!(run_once_with(&failing).await, 1);
    }

    #[tokio::test]
    async fn events_reach_every_subscriber() {
        let mut a = subscribe();
        let mut b = subscribe();
        notify(DaemonEvent::Updates(3));
        assert_eq!(a.recv().await.unwrap(), DaemonEvent::Updates(3));
        assert_eq!(b.recv().await.unwrap(), DaemonEvent::Updates(3));
    }

    #[test]
    fn job_wire_roundtrip() {
        let job = Job {
            id: 7,
            kind: JobKind::InstallRefFile,
            app_ids: vec!["a".into()],
            title: "A".into(),
            state: JobState::Failed("boom".into()),
            progress: 0.5,
            status: "s".into(),
        };
        let wire = JobWire::from(&job);
        assert_eq!(JobWire::SIGNATURE.to_string(), "(tsassssds)");
        let message = zbus::Message::signal(OBJECT_PATH, INTERFACE, "JobChanged")
            .build(&(wire,))
            .unwrap();
        let (decoded,): (JobWire,) = message.body().deserialize().unwrap();
        let back: Job = decoded.into();
        assert_eq!(back, job);
    }
}
