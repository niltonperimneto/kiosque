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
//! The Rust shell (`rust/updater`) provides the tray icon and desktop
//! notifications and consumes the typed daemon event stream.

pub mod auth;
pub mod client;
pub mod runner;
pub mod service;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use jiff::{
    ToSpan, Zoned,
    civil::{Time, Weekday},
    tz::AmbiguousOffset,
};
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

// ── Events for the Rust shell ────────────────────────────────────────────────

/// `count` = number of available app updates.
pub const EVENT_UPDATES: i32 = 1;
/// `count` = 1 on success, 0 on failure; `text` = job title.
pub const EVENT_JOB_FINISHED: i32 = 2;
/// The daemon has been idle long enough and should quit.
pub const EVENT_IDLE_EXIT: i32 = 3;
/// `text` = summary of an automatic update run that started.
pub const EVENT_AUTO_UPDATE: i32 = 4;

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
pub async fn check_updates() -> crate::flatpak::Result<usize> {
    let reg = crate::registry::registry();
    let backend = crate::flatpak::backend();
    if let Err(e) = reg.reload_updates(&*backend).await {
        kerr!("daemon: update check failed: {}", e);
        return Err(e);
    }
    let n = app_update_count();
    klog!("daemon: {} app updates available", n);
    notify(DaemonEvent::Updates(n));
    Ok(n)
}

/// The next time automatic updates should run according to the settings.
pub fn next_auto_update(settings: &crate::settings::AppSettings, now: &Zoned) -> Option<Zoned> {
    if !settings.auto_update {
        return None;
    }
    let time =
        Time::strptime("%H:%M", &settings.update_time).unwrap_or(Time::new(2, 0, 0, 0).ok()?);
    let weekly = settings.update_frequency.eq_ignore_ascii_case("weekly");
    (0..=7).find_map(|d| {
        let day = now.date().checked_add(d.days()).ok()?;
        if weekly && day.weekday() != Weekday::Monday {
            return None;
        }
        let ambiguous = now.time_zone().to_ambiguous_zoned(day.to_datetime(time));
        // Preserve existing semantics: skip nonexistent times and choose the first fold.
        if matches!(ambiguous.offset(), AmbiguousOffset::Gap { .. }) {
            return None;
        }
        let at = ambiguous.earlier().ok()?;
        (&at > now).then_some(at)
    })
}

async fn scheduler() {
    let mut next_check = tokio::time::Instant::now();
    let mut next_auto = next_auto_update(&crate::settings::load_settings(), &Zoned::now());
    let mut catalog_refreshed = tokio::time::Instant::now() - Duration::from_secs(86400);
    loop {
        let now = tokio::time::Instant::now();
        if now >= next_check {
            let _ = check_updates().await;
            next_check = now + CHECK_INTERVAL;
        }
        if now.duration_since(catalog_refreshed) > Duration::from_secs(12 * 3600) {
            crate::catalog::refresh_if_stale().await;
            catalog_refreshed = now;
        }

        // Settings may change while we run; re-read every minute.
        let settings = crate::settings::load_settings();
        let computed = next_auto_update(&settings, &Zoned::now());
        if computed.is_none() || next_auto.is_none() {
            next_auto = computed;
        }
        if let Some(at) = &next_auto
            && &Zoned::now() >= at
        {
            if check_updates().await.is_ok() && !crate::registry::registry().updates().is_empty() {
                klog!("daemon: running scheduled automatic update");
                notify(DaemonEvent::AutoUpdate(app_update_count()));
                crate::transactions::local_manager().enqueue(
                    crate::flatpak::Operation::Update { ids: vec![] },
                    "Automatic update",
                );
            }
            next_auto = next_auto_update(
                &settings,
                &Zoned::now()
                    .checked_add(1.minutes())
                    .expect("current time plus one minute"),
            );
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
                    title: job.title,
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
pub fn connection() -> Option<zbus::Connection> {
    CONNECTION.get().cloned()
}

pub async fn start(background: bool) -> Result<(), String> {
    if crate::flatpak::in_sandbox() {
        return Err(
            "Install the native Kiosque companion on the host to manage applications".into(),
        );
    }
    BACKGROUND.store(background, Ordering::SeqCst);
    touch();
    let conn = zbus::connection::Builder::session()
        .serve_at(OBJECT_PATH, service::Service)
        .serve_at(RUNNER_PATH, runner::Runner)
        .build()
        .await
        .map_err(|e| e.to_string())?;
    match conn
        .request_name_with_flags(BUS_NAME, zbus::fdo::RequestNameFlags::DO_NOT_QUEUE)
        .await
        .map_err(|e| e.to_string())?
    {
        zbus::fdo::RequestNameReply::PrimaryOwner | zbus::fdo::RequestNameReply::AlreadyOwner => {}
        _ => return Err(format!("{BUS_NAME} is already owned")),
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

pub fn check_updates_now() {
    touch();
    crate::runtime::runtime().spawn(async {
        let _ = check_updates().await;
    });
}
pub fn update_all() {
    touch();
    crate::transactions::local_manager().enqueue(
        crate::flatpak::Operation::Update { ids: vec![] },
        "Update all",
    );
}
pub fn is_busy() -> bool {
    crate::transactions::local_manager().is_busy()
}
pub async fn run_once() -> i32 {
    run_update_once(&*crate::flatpak::backend()).await
}
/// Check and apply updates using the selected backend. Does not start Qt or D-Bus.
pub async fn run_update_once(backend: &dyn crate::flatpak::FlatpakBackend) -> i32 {
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
        let now = "2026-11-11T10:00:00[UTC]".parse::<Zoned>().unwrap();
        assert_eq!(
            next_auto_update(&settings(false, "Daily", "02:00"), &now),
            None
        );
        let daily = next_auto_update(&settings(true, "Daily", "02:00"), &now).unwrap();
        assert_eq!(daily, "2026-11-12T02:00:00[UTC]".parse::<Zoned>().unwrap());
        let later_today = next_auto_update(&settings(true, "Daily", "23:30"), &now).unwrap();
        assert_eq!(
            later_today,
            "2026-11-11T23:30:00[UTC]".parse::<Zoned>().unwrap()
        );
        let weekly = next_auto_update(&settings(true, "Weekly", "02:00"), &now).unwrap();
        assert_eq!(weekly, "2026-11-16T02:00:00[UTC]".parse::<Zoned>().unwrap());
    }

    #[test]
    fn schedule_preserves_dst_gap_and_fold_behavior() {
        let gap = "2026-03-08T00:00:00-05:00[America/New_York]"
            .parse::<Zoned>()
            .unwrap();
        let next = next_auto_update(&settings(true, "Daily", "02:30"), &gap).unwrap();
        assert_eq!(
            next,
            "2026-03-09T02:30:00-04:00[America/New_York]"
                .parse::<Zoned>()
                .unwrap()
        );
        let fold = "2026-11-01T00:00:00-04:00[America/New_York]"
            .parse::<Zoned>()
            .unwrap();
        let next = next_auto_update(&settings(true, "Daily", "01:30"), &fold).unwrap();
        assert_eq!(
            next,
            "2026-11-01T01:30:00-04:00[America/New_York]"
                .parse::<Zoned>()
                .unwrap()
        );
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
    #[tokio::test]
    async fn one_shot_reports_no_updates_success_and_backend_failures() {
        use crate::flatpak::{Operation, UpdateInfo, mock::MockBackend};
        let backend = MockBackend::default();
        assert_eq!(run_update_once(&backend).await, 0);
        assert!(backend.ran.lock().unwrap().is_empty());
        backend.updates.lock().unwrap().push(UpdateInfo::default());
        assert_eq!(run_update_once(&backend).await, 0);
        assert_eq!(
            *backend.ran.lock().unwrap(),
            vec![Operation::Update { ids: vec![] }]
        );
        backend.updates.lock().unwrap().push(UpdateInfo::default());
        *backend.fail_with.lock().unwrap() = Some("fixture failed".into());
        assert_eq!(run_update_once(&backend).await, 1);
        *backend.list_updates_error.lock().unwrap() = Some("fixture unavailable".into());
        let before = backend.ran.lock().unwrap().len();
        assert_eq!(run_update_once(&backend).await, 1);
        assert_eq!(backend.ran.lock().unwrap().len(), before);
    }
}
