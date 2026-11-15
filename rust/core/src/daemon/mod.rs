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
//! The C++ shell (`src/update_main.cpp`) provides the tray icon and desktop
//! notifications and drives this module through the C functions at the end
//! of this file.

pub mod client;
pub mod runner;
pub mod service;

use std::ffi::{CString, c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
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

// ── Events for the C++ shell ────────────────────────────────────────────────

/// `count` = number of available app updates.
pub const EVENT_UPDATES: i32 = 1;
/// `count` = 1 on success, 0 on failure; `text` = job title.
pub const EVENT_JOB_FINISHED: i32 = 2;
/// The daemon has been idle long enough and should quit.
pub const EVENT_IDLE_EXIT: i32 = 3;
/// `text` = summary of an automatic update run that started.
pub const EVENT_AUTO_UPDATE: i32 = 4;

pub type EventCallback =
    extern "C" fn(event: i32, count: i32, text: *const c_char, ctx: *mut c_void);

struct Callback {
    f: EventCallback,
    ctx: usize,
}

static CALLBACK: Mutex<Option<Callback>> = Mutex::new(None);

fn notify(event: i32, count: i32, text: &str) {
    if let Some(cb) = CALLBACK.lock().unwrap().as_ref() {
        let c = CString::new(text.replace('\0', "")).unwrap_or_default();
        (cb.f)(event, count, c.as_ptr(), cb.ctx as *mut c_void);
    }
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
    notify(EVENT_UPDATES, n as i32, "");
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
                notify(EVENT_AUTO_UPDATE, app_update_count() as i32, "");
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
            notify(EVENT_IDLE_EXIT, 0, "");
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
                JobState::Done => notify(EVENT_JOB_FINISHED, 1, &job.title),
                JobState::Failed(ref e) => {
                    notify(EVENT_JOB_FINISHED, 0, &format!("{}: {}", job.title, e))
                }
                _ => {}
            },
            Ok(Event::Updates) => notify(EVENT_UPDATES, app_update_count() as i32, ""),
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => break,
        }
    }
}

static CONNECTION: OnceLock<zbus::Connection> = OnceLock::new();

async fn start(background: bool) -> Result<(), String> {
    BACKGROUND.store(background, Ordering::SeqCst);
    touch();
    let conn = zbus::connection::Builder::session()
        .name(BUS_NAME)
        .serve_at(OBJECT_PATH, service::Service)
        .serve_at(RUNNER_PATH, runner::Runner)
        .build()
        .await
        .map_err(|e| e.to_string())?;
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

// ── C FFI for the C++ shell ─────────────────────────────────────────────────

/// Start the daemon services. `background` keeps it alive while idle (used
/// when autostarted); otherwise it asks to quit after [`IDLE_TIMEOUT`].
/// Returns false if the bus name is already owned or the bus is unreachable.
///
/// # Safety
/// `cb` must stay callable for the lifetime of the process; it is invoked
/// from worker threads and must marshal to the GUI thread itself.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kiosque_daemon_start(
    background: bool,
    cb: EventCallback,
    ctx: *mut c_void,
) -> bool {
    *CALLBACK.lock().unwrap() = Some(Callback {
        f: cb,
        ctx: ctx as usize,
    });
    match crate::runtime::runtime().block_on(start(background)) {
        Ok(()) => true,
        Err(e) => {
            kerr!("daemon: cannot start: {}", e);
            false
        }
    }
}

/// Trigger an update check (result arrives as an `EVENT_UPDATES`).
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_daemon_check_updates() {
    touch();
    crate::runtime::runtime().spawn(async {
        check_updates().await;
    });
}

/// Queue an update of everything.
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_daemon_update_all() {
    touch();
    crate::transactions::local_manager().enqueue(
        crate::flatpak::Operation::Update { ids: vec![] },
        "Update all",
    );
}

/// True while transactions are queued or running.
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_daemon_busy() -> bool {
    crate::transactions::local_manager().is_busy()
}

/// One-shot mode (`kiosque-update --check`): check and apply all updates in
/// process, returning a process exit code.
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_run_update_once() -> i32 {
    crate::runtime::runtime().block_on(run_update_once(&*crate::flatpak::backend()))
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
