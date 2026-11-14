//! `kiosque-update`: Kiosque's background update notifier. Qt-free.
//!
//! * `--help` prints usage without touching D-Bus.
//! * `--check` checks and applies all updates once and exits (0 on success).
//! * `--background` is the autostart mode: tray, notifications and the
//!   scheduler, kept alive while idle.
//! * No flag: the same, exiting when idle.
//!
//! The process owns `org.kiosque.Kiosque.Daemon`, which makes it single
//! instance. It never enables autostart itself; only Kiosque's settings do.

mod notify;
mod shell;
mod tray;

use std::sync::{Arc, Mutex};

use kiosque_core::{daemon, kerr, klog};
use shell::{Command, Effect, Mode, Shell};

fn main() {
    let mode = match shell::parse_args(std::env::args().skip(1)) {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("kiosque-update: {e}\n");
            eprint!("{}", shell::usage());
            std::process::exit(2);
        }
    };
    let code = match mode {
        Mode::Help => {
            print!("{}", shell::usage());
            0
        }
        Mode::Check => kiosque_core::runtime::runtime().block_on(daemon::run_once()),
        Mode::Background => kiosque_core::runtime::runtime().block_on(run(true)),
        Mode::Tray => kiosque_core::runtime::runtime().block_on(run(false)),
    };
    std::process::exit(code);
}

async fn run(background: bool) -> i32 {
    let mut events = daemon::subscribe();
    if let Err(e) = daemon::start(background).await {
        // Most likely another instance owns the bus name already.
        kerr!("kiosque-update: {}", e);
        return 1;
    }
    let Some(conn) = daemon::connection() else {
        kerr!("kiosque-update: no bus connection");
        return 1;
    };
    let (commands, mut command_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut shell = Shell::default();
    let shared = Arc::new(tray::Shared {
        state: Mutex::new(shell.tray()),
        revision: Mutex::new(1),
        commands: commands.clone(),
    });
    if let Err(e) = tray::serve(&conn, shared.clone()).await {
        kerr!("tray: {}", e);
    }
    let notifier = match notify::Notifier::new(&conn, commands.clone()).await {
        Ok(n) => Some(n),
        Err(e) => {
            klog!("notifications unavailable: {}", e);
            None
        }
    };
    let mut terminate =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                kerr!("kiosque-update: {}", e);
                return 1;
            }
        };
    daemon::check_updates_now();

    loop {
        let effects = tokio::select! {
            event = events.recv() => match event {
                Ok(event) => shell.on_event(event),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => vec![Effect::Exit],
            },
            Some(command) = command_rx.recv() => shell.on_command(command),
            _ = tokio::signal::ctrl_c() => vec![Effect::Exit],
            _ = terminate.recv() => vec![Effect::Exit],
        };
        for effect in effects {
            match effect {
                Effect::Notify(n) => {
                    if let Some(notifier) = &notifier {
                        notifier.send(&n).await;
                    }
                }
                Effect::Tray(state) => tray::update(&conn, &shared, state).await,
                Effect::CheckUpdates => daemon::check_updates_now(),
                Effect::UpdateAll => daemon::update_all(),
                Effect::OpenStore => open_store(),
                Effect::Exit => {
                    if daemon::is_busy() {
                        klog!("kiosque-update: waiting for running transactions");
                        let _ = commands.send(Command::Quit);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    return 0;
                }
            }
        }
    }
}

/// Start the store next to this binary (same prefix or sandbox), falling
/// back to `PATH`.
fn open_store() {
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("kiosque")))
        .filter(|p| p.is_file());
    let program = sibling.unwrap_or_else(|| "kiosque".into());
    if let Err(e) = std::process::Command::new(&program)
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        kerr!("cannot start {}: {}", program.display(), e);
    }
}
