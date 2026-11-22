//! The notifier's behaviour as a pure state machine: daemon events, tray
//! commands and notification actions in, effects out. All D-Bus work lives
//! in `tray.rs`/`notify.rs`, so this file is tested without a bus.

use kiosque_core::daemon::DaemonEvent;
use kiosque_core::l10n::{i18n, i18np};

/// How the process was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Print usage and exit without touching D-Bus.
    Help,
    /// Check and apply updates once, then exit with a status code.
    Check,
    /// Autostart: tray and scheduler, stays alive while idle.
    Background,
    /// On demand: tray and scheduler, exits when idle.
    Tray,
}

pub fn usage() -> String {
    format!(
        "{}\n\n  --check       {}\n  --background  {}\n  --help        {}\n",
        i18n(
            "Usage: kiosque-update [--check | --background | --help]",
            &[]
        ),
        i18n("Check for and install all updates once, then exit", &[]),
        i18n("Run as the autostarted background notifier", &[]),
        i18n("Show this help and exit", &[]),
    )
}

/// Parse the command line (without the program name).
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Mode, String> {
    let mut mode = Mode::Tray;
    for arg in args {
        let next = match arg.as_str() {
            "-h" | "--help" => return Ok(Mode::Help),
            "-c" | "--check" => Mode::Check,
            "--background" | "--daemon" => Mode::Background,
            other => return Err(format!("unknown argument {other:?}")),
        };
        if mode != Mode::Tray && mode != next {
            return Err("--check and --background are exclusive".into());
        }
        mode = next;
    }
    Ok(mode)
}

/// A tray menu entry or notification action chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    OpenStore,
    Check,
    UpdateAll,
    Quit,
}

impl Command {
    /// Notification action keys (`org.freedesktop.Notifications.ActionInvoked`).
    pub fn from_action(key: &str) -> Option<Self> {
        match key {
            "open" | "default" => Some(Self::OpenStore),
            "update" => Some(Self::UpdateAll),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub summary: String,
    pub body: String,
    /// `(key, label)` pairs.
    pub actions: Vec<(&'static str, String)>,
}

/// What the tray should look like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayState {
    /// SNI status: `Passive` (hidden by most hosts), `Active` or `NeedsAttention`.
    pub status: &'static str,
    pub tooltip: String,
    pub can_update: bool,
    pub can_check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Notify(Notification),
    Tray(TrayState),
    CheckUpdates,
    UpdateAll,
    OpenStore,
    Exit,
}

#[derive(Debug, Default)]
pub struct Shell {
    updates: usize,
    /// Count last announced, so a periodic re-check does not re-notify.
    announced: usize,
    updating: bool,
    checking: bool,
}

impl Shell {
    pub fn tray(&self) -> TrayState {
        let tooltip = if self.updating {
            i18n("Kiosque: installing updates…", &[])
        } else if self.updates > 0 {
            i18np(
                "Kiosque: %1 update available",
                "Kiosque: %1 updates available",
                self.updates as i64,
                &[],
            )
        } else {
            i18n("Kiosque: applications are up to date", &[])
        };
        TrayState {
            status: if self.updating {
                "Active"
            } else if self.updates > 0 {
                "NeedsAttention"
            } else {
                "Passive"
            },
            tooltip,
            can_update: self.updates > 0 && !self.updating,
            can_check: !self.checking && !self.updating,
        }
    }

    pub fn on_event(&mut self, event: DaemonEvent) -> Vec<Effect> {
        let mut out = vec![];
        match event {
            DaemonEvent::Updates(n) => {
                self.checking = false;
                self.updates = n;
                if n > 0 && n != self.announced && !self.updating {
                    out.push(Effect::Notify(Notification {
                        summary: i18n("Software Updates Available", &[]),
                        body: i18np(
                            "%1 application update is available.",
                            "%1 application updates are available.",
                            n as i64,
                            &[],
                        ),
                        actions: vec![
                            ("update", i18n("Update All", &[])),
                            ("open", i18n("Open Kiosque", &[])),
                        ],
                    }));
                }
                self.announced = n;
            }
            DaemonEvent::AutoUpdate(n) => {
                self.updating = true;
                out.push(Effect::Notify(Notification {
                    summary: i18n("Updating Applications", &[]),
                    body: i18np(
                        "Installing %1 update in the background.",
                        "Installing %1 updates in the background.",
                        n as i64,
                        &[],
                    ),
                    actions: vec![],
                }));
            }
            DaemonEvent::JobFinished { ok, title } => {
                if !self.updating {
                    return out;
                }
                self.updating = false;
                out.push(Effect::Notify(if ok {
                    Notification {
                        summary: i18n("Updates Completed", &[]),
                        body: i18n("All applications have been updated.", &[]),
                        actions: vec![],
                    }
                } else {
                    Notification {
                        summary: i18n("Updates Failed", &[]),
                        body: title,
                        actions: vec![("open", i18n("Open Kiosque", &[]))],
                    }
                }));
                // The registry refreshes updates after a transaction; ask anyway
                // so the tray never shows a stale count.
                self.checking = true;
                out.push(Effect::CheckUpdates);
            }
            DaemonEvent::IdleExit => {
                if !self.updating {
                    out.push(Effect::Exit);
                }
                return out;
            }
        }
        out.push(Effect::Tray(self.tray()));
        out
    }

    pub fn on_command(&mut self, command: Command) -> Vec<Effect> {
        let mut out = match command {
            Command::OpenStore => return vec![Effect::OpenStore],
            Command::Quit => return vec![Effect::Exit],
            Command::Check if self.checking || self.updating => return vec![],
            Command::Check => {
                self.checking = true;
                vec![Effect::CheckUpdates]
            }
            Command::UpdateAll if self.updating => return vec![],
            Command::UpdateAll => {
                self.updating = true;
                vec![
                    Effect::UpdateAll,
                    Effect::Notify(Notification {
                        summary: i18n("Updating Applications", &[]),
                        body: i18n(
                            "Kiosque is installing Flatpak updates in the background.",
                            &[],
                        ),
                        actions: vec![],
                    }),
                ]
            }
        };
        out.push(Effect::Tray(self.tray()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Result<Mode, String> {
        parse_args(a.iter().map(|s| s.to_string()))
    }

    fn notifications(effects: &[Effect]) -> Vec<&Notification> {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::Notify(n) => Some(n),
                _ => None,
            })
            .collect()
    }

    fn tray(effects: &[Effect]) -> &TrayState {
        effects
            .iter()
            .rev()
            .find_map(|e| match e {
                Effect::Tray(t) => Some(t),
                _ => None,
            })
            .expect("tray update")
    }

    #[test]
    fn command_line() {
        assert_eq!(args(&[]), Ok(Mode::Tray));
        assert_eq!(args(&["--help"]), Ok(Mode::Help));
        assert_eq!(args(&["--check", "--help"]), Ok(Mode::Help));
        assert_eq!(args(&["-c"]), Ok(Mode::Check));
        assert_eq!(args(&["--background"]), Ok(Mode::Background));
        assert_eq!(
            args(&["--daemon"]),
            Ok(Mode::Background),
            "legacy autostart entries"
        );
        assert!(args(&["--check", "--background"]).is_err());
        assert!(args(&["--command=/bin/sh"]).is_err());
        assert!(usage().contains("--background"));
    }

    #[test]
    fn announces_each_new_count_once() {
        let mut s = Shell::default();
        let e = s.on_event(DaemonEvent::Updates(2));
        let n = notifications(&e);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].actions[0].0, "update");
        assert_eq!(tray(&e).status, "NeedsAttention");
        assert!(tray(&e).can_update);
        assert!(notifications(&s.on_event(DaemonEvent::Updates(2))).is_empty());
        assert_eq!(notifications(&s.on_event(DaemonEvent::Updates(3))).len(), 1);
        let none = s.on_event(DaemonEvent::Updates(0));
        assert!(notifications(&none).is_empty());
        assert_eq!(tray(&none).status, "Passive");
        assert!(!tray(&none).can_update);
    }

    #[test]
    fn update_all_flow() {
        let mut s = Shell::default();
        s.on_event(DaemonEvent::Updates(1));
        let start = s.on_command(Command::UpdateAll);
        assert_eq!(start[0], Effect::UpdateAll);
        assert!(!tray(&start).can_update && !tray(&start).can_check);
        assert!(
            s.on_command(Command::UpdateAll).is_empty(),
            "no double start"
        );
        assert!(s.on_command(Command::Check).is_empty());
        assert!(!s.on_event(DaemonEvent::IdleExit).contains(&Effect::Exit));
        let done = s.on_event(DaemonEvent::JobFinished {
            ok: false,
            title: "Update all: no space".into(),
        });
        assert_eq!(notifications(&done)[0].body, "Update all: no space");
        assert!(done.contains(&Effect::CheckUpdates));
        assert_eq!(s.on_event(DaemonEvent::IdleExit), vec![Effect::Exit]);
    }

    #[test]
    fn unrelated_jobs_are_silent() {
        let mut s = Shell::default();
        assert!(
            s.on_event(DaemonEvent::JobFinished {
                ok: true,
                title: "Install GIMP".into()
            })
            .is_empty()
        );
    }

    #[test]
    fn notification_actions_and_menu() {
        assert_eq!(Command::from_action("update"), Some(Command::UpdateAll));
        assert_eq!(Command::from_action("open"), Some(Command::OpenStore));
        assert_eq!(Command::from_action("default"), Some(Command::OpenStore));
        assert_eq!(Command::from_action("rm -rf"), None);
        let mut s = Shell::default();
        assert_eq!(s.on_command(Command::OpenStore), vec![Effect::OpenStore]);
        assert_eq!(s.on_command(Command::Quit), vec![Effect::Exit]);
        assert_eq!(s.on_command(Command::Check)[0], Effect::CheckUpdates);
        assert!(
            s.on_command(Command::Check).is_empty(),
            "one check at a time"
        );
    }
}
