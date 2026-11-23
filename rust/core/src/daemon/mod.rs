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
pub mod schedule;
pub mod service;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use jiff::ZonedDateTime;
use serde::{Deserialize, Serialize};
use zbus::Type;

use crate::registry::{Event, Job, JobKind, JobState};

pub const BUS_NAME: &str = "org.kiosque.Kiosque.Daemon";
pub const OBJECT_PATH: &str = "/org/kiosque/Kiosque/Daemon";
pub const INTERFACE: &str = "org.kiosque.Kiosque.Daemon1";
pub const RUNNER_PATH: &str = "/runner";

/// Version of the `org.kiosque.Kiosque.Daemon1` interface, published as its
/// `ApiVersion` property. Companions that predate the property are version 1.
///
/// * 2 — `ConfigureAutomaticUpdates`
pub const API_VERSION: u32 = 2;
/// The oldest companion this application works with.
pub const REQUIRED_API_VERSION: u32 = 2;

/// The application ID of the Kiosque Flatpak.
pub const FLATPAK_APP_ID: &str = "org.kiosque.Kiosque";

/// How to start the storefront from the host companion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreLauncher {
    /// A native `kiosque` next to this executable.
    Native(std::path::PathBuf),
    /// The Kiosque Flatpak (the companion is installed without a native GUI).
    Flatpak,
    /// Whatever `kiosque` is on `PATH`.
    Path,
}

impl StoreLauncher {
    /// Prefer a native storefront installed alongside, then the Flatpak.
    pub fn choose(sibling: Option<std::path::PathBuf>, flatpak_installed: bool) -> Self {
        match sibling {
            Some(path) => StoreLauncher::Native(path),
            None if flatpak_installed => StoreLauncher::Flatpak,
            None => StoreLauncher::Path,
        }
    }

    fn detect() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let sibling = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("kiosque")))
            .filter(|path| {
                std::fs::metadata(path)
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            });
        let desktop = format!("exports/share/applications/{FLATPAK_APP_ID}.desktop");
        let flatpak_installed = crate::flatpak::installation_paths()
            .iter()
            .any(|(_, path)| path.join(&desktop).is_file());
        Self::choose(sibling, flatpak_installed)
    }

    pub fn command(&self, args: &[String]) -> std::process::Command {
        let mut command = match self {
            StoreLauncher::Native(path) => std::process::Command::new(path),
            StoreLauncher::Flatpak => {
                let mut c = std::process::Command::new("flatpak");
                c.args(["run", FLATPAK_APP_ID]);
                c
            }
            StoreLauncher::Path => std::process::Command::new("kiosque"),
        };
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }
}

/// Start the storefront (native or Flatpak, whichever is installed),
/// optionally with an `appstream://` link to open.
pub fn open_store(args: &[String]) -> Result<(), String> {
    let launcher = StoreLauncher::detect();
    let mut child = launcher
        .command(args)
        .spawn()
        .map_err(|e| format!("cannot start the store ({launcher:?}): {e}"))?;
    // Reap the launcher in the background (flatpak run exits quickly).
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// How long an on-demand (D-Bus activated) daemon lingers without work.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How often available updates are checked.
const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);
/// Delay before a missed automatic update runs, so it does not compete with
/// the rest of the session starting up.
const STARTUP_DELAY: Duration = Duration::from_secs(5 * 60);
/// Wait before retrying an automatic update that failed.
const RETRY_DELAY: Duration = Duration::from_secs(60 * 60);

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
pub fn next_auto_update(settings: &crate::settings::AppSettings, now: &ZonedDateTime) -> Option<ZonedDateTime> {
    schedule::next_slot(settings, now)
}

/// Keep running while idle (automatic updates enabled) or exit when idle.
pub fn set_background(background: bool) {
    if BACKGROUND.swap(background, Ordering::SeqCst) != background {
        klog!(
            "daemon: background mode {}",
            if background { "on" } else { "off" }
        );
    }
    touch();
}

/// Apply automatic-update settings received from a client: persist the
/// schedule for this daemon and switch background mode accordingly, so the
/// change takes effect without waiting for the next login.
pub fn configure_automatic_updates(
    enabled: bool,
    frequency: &str,
    time: &str,
) -> Result<(), String> {
    let frequency = match frequency.to_ascii_lowercase().as_str() {
        "daily" => "Daily",
        "weekly" => "Weekly",
        _ => return Err("Unknown update frequency".into()),
    };
    if time.len() != 5 || jiff::civil::Time::strptime("%H:%M", time).is_err() {
        return Err("Invalid update time".into());
    }
    let mut settings = crate::settings::load_settings();
    if settings.auto_update != enabled
        || settings.update_frequency != frequency
        || settings.update_time != time
    {
        settings.auto_update = enabled;
        settings.update_frequency = frequency.into();
        settings.update_time = time.into();
        crate::settings::save_settings(&settings)?;
    }
    record_schedule(&settings);
    set_background(enabled);
    Ok(())
}

/// Update the automatic-update state for the current settings.
fn record_schedule(settings: &crate::settings::AppSettings) -> schedule::State {
    let path = schedule::state_path();
    let mut state = schedule::load(&path);
    if state.observe(settings, jiff::Timestamp::now().as_second())
        && let Err(e) = schedule::save(&path, &state)
    {
        kerr!("daemon: cannot save the automatic-update state: {}", e);
    }
    state
}

/// Check for updates and install them all, waiting for the result.
async fn run_automatic_update() -> Result<(), String> {
    check_updates().await.map_err(|e| e.to_string())?;
    if crate::registry::registry().updates().is_empty() {
        klog!("daemon: automatic update: nothing to update");
        return Ok(());
    }
    klog!("daemon: running automatic update");
    notify(DaemonEvent::AutoUpdate(app_update_count()));
    let registry = crate::registry::registry();
    let mut events = registry.subscribe();
    let id = crate::transactions::local_manager().enqueue(
        crate::flatpak::Operation::Update { ids: vec![] },
        "Automatic update",
    );
    loop {
        if let Some(job) = registry.job(id).filter(|job| !job.state.is_active()) {
            return match job.state {
                JobState::Done => Ok(()),
                state => Err(format!("automatic update {}", state.as_str())),
            };
        }
        match events.recv().await {
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                return Err("registry closed".into());
            }
            _ => continue,
        }
    }
}

async fn scheduler() {
    let started = tokio::time::Instant::now();
    let mut next_check = started;
    let mut retry_after: Option<tokio::time::Instant> = None;
    let mut catalog_refreshed = started - Duration::from_secs(86400);
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

        // Settings may change while we run; re-read every minute. A
        // scheduled time that passed since the last run (also while the
        // computer was off or asleep) is due.
        let settings = crate::settings::load_settings();
        let state = record_schedule(&settings);
        if settings.auto_update {
            set_background(true);
        }
        let due = schedule::is_due(&settings, &state, &ZonedDateTime::now());
        if due
            && now.duration_since(started) >= STARTUP_DELAY
            && retry_after.is_none_or(|t| now >= t)
            && !crate::transactions::local_manager().is_busy()
        {
            match run_automatic_update().await {
                Ok(()) => {
                    retry_after = None;
                    let path = schedule::state_path();
                    let mut state = schedule::load(&path);
                    state.last_run = Some(jiff::Timestamp::now().as_second());
                    if let Err(e) = schedule::save(&path, &state) {
                        kerr!("daemon: cannot save the automatic-update state: {}", e);
                    }
                    if let Some(next) = schedule::next_slot(&settings, &ZonedDateTime::now()) {
                        klog!("daemon: next automatic update at {}", next);
                    }
                }
                Err(e) => {
                    kerr!("daemon: automatic update failed: {}; retrying later", e);
                    retry_after = Some(tokio::time::Instant::now() + RETRY_DELAY);
                }
            }
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

/// True while the daemon stays alive when idle.
pub fn is_background() -> bool {
    BACKGROUND.load(Ordering::SeqCst)
}

/// Sandboxed autostart (`kiosque --update-companion`): hand this app's
/// automatic-update settings to the host companion, which then keeps running
/// and applies them. Returns a process exit code.
pub async fn start_update_companion() -> i32 {
    let settings = crate::settings::load_settings();
    match client::configure_automatic_updates(&settings).await {
        Ok(()) => 0,
        Err(e) => {
            kerr!("automatic updates: {}", e);
            1
        }
    }
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
        let now = "2026-11-11T10:00:00[UTC]".parse::<ZonedDateTime>().unwrap();
        assert_eq!(
            next_auto_update(&settings(false, "Daily", "02:00"), &now),
            None
        );
        let daily = next_auto_update(&settings(true, "Daily", "02:00"), &now).unwrap();
        assert_eq!(daily, "2026-11-12T02:00:00[UTC]".parse::<ZonedDateTime>().unwrap());
        let later_today = next_auto_update(&settings(true, "Daily", "23:30"), &now).unwrap();
        assert_eq!(
            later_today,
            "2026-11-11T23:30:00[UTC]".parse::<ZonedDateTime>().unwrap()
        );
        let weekly = next_auto_update(&settings(true, "Weekly", "02:00"), &now).unwrap();
        assert_eq!(weekly, "2026-11-16T02:00:00[UTC]".parse::<ZonedDateTime>().unwrap());
    }

    #[test]
    fn schedule_preserves_dst_gap_and_fold_behavior() {
        let gap = "2026-03-08T00:00:00-05:00[America/New_York]"
            .parse::<ZonedDateTime>()
            .unwrap();
        let next = next_auto_update(&settings(true, "Daily", "02:30"), &gap).unwrap();
        assert_eq!(
            next,
            "2026-03-09T02:30:00-04:00[America/New_York]"
                .parse::<ZonedDateTime>()
                .unwrap()
        );
        let fold = "2026-11-01T00:00:00-04:00[America/New_York]"
            .parse::<ZonedDateTime>()
            .unwrap();
        let next = next_auto_update(&settings(true, "Daily", "01:30"), &fold).unwrap();
        assert_eq!(
            next,
            "2026-11-01T01:30:00-04:00[America/New_York]"
                .parse::<ZonedDateTime>()
                .unwrap()
        );
    }

    #[test]
    fn the_store_opens_natively_or_as_the_flatpak() {
        let native = std::path::PathBuf::from("/usr/bin/kiosque");
        assert_eq!(
            StoreLauncher::choose(Some(native.clone()), true),
            StoreLauncher::Native(native),
            "a native storefront installed alongside wins"
        );
        assert_eq!(StoreLauncher::choose(None, true), StoreLauncher::Flatpak);
        assert_eq!(StoreLauncher::choose(None, false), StoreLauncher::Path);
        let command = StoreLauncher::Flatpak.command(&["appstream://org.example.App".into()]);
        assert_eq!(command.get_program(), "flatpak");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            ["run", "org.kiosque.Kiosque", "appstream://org.example.App"]
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
