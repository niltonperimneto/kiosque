//! Who keeps Flatpak applications updated on this computer.
//!
//! The Flatpak build cannot update other applications itself; that work
//! belongs to the native host companion (`kiosque-companion`). This module
//! reports whether the companion can do it, notices when the system already
//! updates Flatpaks on a timer (Fedora Atomic and Universal Blue images ship
//! one), and keeps the Kiosque Flatpak itself current through the Flatpak
//! portal's update monitor, which needs no companion.

use std::path::PathBuf;

use crate::daemon::client::{self, CompanionError};

/// Whether the host companion can run automatic updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionState {
    Ready { version: u32 },
    Outdated { found: u32, required: u32 },
    Missing,
    Unreachable(String),
}

impl CompanionState {
    /// Stable identifier for QML: `ready`, `outdated`, `missing`, `unreachable`.
    pub fn id(&self) -> &'static str {
        match self {
            CompanionState::Ready { .. } => "ready",
            CompanionState::Outdated { .. } => "outdated",
            CompanionState::Missing => "missing",
            CompanionState::Unreachable(_) => "unreachable",
        }
    }
}

impl From<Result<u32, CompanionError>> for CompanionState {
    fn from(result: Result<u32, CompanionError>) -> Self {
        match result {
            Ok(version) => CompanionState::Ready { version },
            Err(CompanionError::Outdated { found, required }) => {
                CompanionState::Outdated { found, required }
            }
            Err(CompanionError::Missing) => CompanionState::Missing,
            Err(CompanionError::Failed(error)) => CompanionState::Unreachable(error),
        }
    }
}

/// Ask the host companion (D-Bus activating it) whether it is usable.
pub async fn companion_state() -> CompanionState {
    client::companion_version().await.into()
}

/// Timers known to update Flatpak applications, besides any timer whose name
/// mentions Flatpak.
const KNOWN_UPDATERS: [&str; 2] = ["uupd.timer", "ublue-update.timer"];

fn updates_flatpaks(timer: &str) -> bool {
    timer.ends_with(".timer")
        && (timer.to_ascii_lowercase().contains("flatpak") || KNOWN_UPDATERS.contains(&timer))
}

/// The first enabled systemd timer under `roots` that updates Flatpak
/// applications. Each root is a configuration tree such as `/etc` or
/// `/usr/lib`; enabled timers are links in its
/// `systemd/{system,user}/timers.target.wants`.
pub fn system_auto_updater(roots: &[PathBuf]) -> Option<String> {
    let mut found: Vec<String> = roots
        .iter()
        .flat_map(|root| {
            ["system", "user"]
                .into_iter()
                .map(move |kind| root.join("systemd").join(kind).join("timers.target.wants"))
        })
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| updates_flatpaks(name))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Configuration trees to search for host timers. Inside the sandbox the
/// host's `/etc` and `/usr` are visible read-only under `/run/host`.
pub fn host_unit_roots() -> Vec<PathBuf> {
    if crate::flatpak::in_sandbox() {
        vec![
            PathBuf::from("/run/host/etc"),
            PathBuf::from("/run/host/usr/lib"),
        ]
    } else {
        let mut roots = vec![PathBuf::from("/etc"), PathBuf::from("/usr/lib")];
        if let Some(config) = dirs::config_dir() {
            roots.push(config);
        }
        roots
    }
}

/// The detected host updater, if any (blocking filesystem reads).
pub fn detect_system_auto_updater() -> Option<String> {
    system_auto_updater(&host_unit_roots())
}

// ── Self-update of the Kiosque Flatpak ──────────────────────────────────────

/// `status` of the portal's `UpdateMonitor.Progress` signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfUpdateProgress {
    Running,
    Empty,
    Done,
    Error,
}

impl SelfUpdateProgress {
    pub fn from_status(status: u32) -> Option<Self> {
        match status {
            0 => Some(Self::Running),
            1 => Some(Self::Empty),
            2 => Some(Self::Done),
            3 => Some(Self::Error),
            _ => None,
        }
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Flatpak",
    default_service = "org.freedesktop.portal.Flatpak",
    default_path = "/org/freedesktop/portal/Flatpak"
)]
trait FlatpakPortal {
    fn create_update_monitor(
        &self,
        options: std::collections::HashMap<&str, zbus::Variant<'_>>,
    ) -> zbus::Result<zbus::OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Flatpak.UpdateMonitor",
    default_service = "org.freedesktop.portal.Flatpak"
)]
trait UpdateMonitor {
    fn update(
        &self,
        parent_window: &str,
        options: std::collections::HashMap<&str, zbus::Variant<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn update_available(
        &self,
        update_info: std::collections::HashMap<String, zbus::OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn progress(
        &self,
        info: std::collections::HashMap<String, zbus::OwnedVariant>,
    ) -> zbus::Result<()>;
}

/// Inside Flatpak, install new versions of Kiosque itself as the portal
/// announces them. Runs until the session bus goes away; failures are
/// logged and leave Kiosque at its current version.
pub async fn keep_kiosque_current() {
    if !crate::flatpak::in_sandbox() {
        return;
    }
    if let Err(e) = watch_self_updates().await {
        kerr!("self-update: {}", e);
    }
}

async fn watch_self_updates() -> zbus::Result<()> {
    use futures_util::StreamExt;
    let conn = zbus::Connection::session().await?;
    let portal = FlatpakPortalProxy::new(&conn).await?;
    let path = portal
        .create_update_monitor(std::collections::HashMap::new())
        .await?;
    let monitor = UpdateMonitorProxy::builder(&conn)
        .path(path)
        .build()
        .await?;
    let mut available = monitor.receive_update_available().await?;
    let mut progress = monitor.receive_progress().await?;
    klog!("self-update: watching for new Kiosque versions");
    loop {
        tokio::select! {
            Some(_) = available.next() => {
                klog!("self-update: a new Kiosque version is available; installing it");
                if let Err(e) = monitor.update("", std::collections::HashMap::new()).await {
                    kerr!("self-update: {}", e);
                }
            }
            Some(signal) = progress.next() => {
                let status = signal
                    .args()
                    .ok()
                    .and_then(|args| args.info.get("status").and_then(|v| u32::try_from(v).ok()))
                    .and_then(SelfUpdateProgress::from_status);
                match status {
                    Some(SelfUpdateProgress::Done) => {
                        klog!("self-update: installed; it takes effect when Kiosque restarts")
                    }
                    Some(SelfUpdateProgress::Error) => kerr!("self-update: the update failed"),
                    _ => {}
                }
            }
            else => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn unit_dir(root: &Path, kind: &str) -> PathBuf {
        root.join("systemd").join(kind).join("timers.target.wants")
    }

    #[test]
    fn flatpak_updating_timers_are_recognised() {
        assert!(updates_flatpaks("flatpak-system-update.timer"));
        assert!(updates_flatpaks("flatpak-user-update.timer"));
        assert!(updates_flatpaks("uupd.timer"));
        assert!(!updates_flatpaks("rpm-ostreed-automatic.timer"));
        assert!(!updates_flatpaks("flatpak-system-update.service"));
    }

    #[test]
    fn enabled_timers_are_found_under_any_root() {
        let etc = tempfile::tempdir().unwrap();
        let usr = tempfile::tempdir().unwrap();
        let roots = vec![etc.path().to_path_buf(), usr.path().to_path_buf()];
        assert_eq!(system_auto_updater(&roots), None);

        std::fs::create_dir_all(unit_dir(etc.path(), "system")).unwrap();
        std::fs::write(
            unit_dir(etc.path(), "system").join("rpm-ostreed-automatic.timer"),
            "",
        )
        .unwrap();
        assert_eq!(system_auto_updater(&roots), None, "not a Flatpak updater");

        std::fs::create_dir_all(unit_dir(usr.path(), "user")).unwrap();
        std::fs::write(unit_dir(usr.path(), "user").join("uupd.timer"), "").unwrap();
        assert_eq!(system_auto_updater(&roots).as_deref(), Some("uupd.timer"));
    }

    #[test]
    fn companion_errors_map_to_states() {
        assert_eq!(
            CompanionState::from(Ok(2)),
            CompanionState::Ready { version: 2 }
        );
        assert_eq!(
            CompanionState::from(Err(CompanionError::Missing)).id(),
            "missing"
        );
        assert_eq!(
            CompanionState::from(Err(CompanionError::Outdated {
                found: 1,
                required: 2
            }))
            .id(),
            "outdated"
        );
        assert_eq!(
            CompanionState::from(Err(CompanionError::Failed("x".into()))).id(),
            "unreachable"
        );
    }

    #[test]
    fn portal_progress_statuses() {
        assert_eq!(
            SelfUpdateProgress::from_status(2),
            Some(SelfUpdateProgress::Done)
        );
        assert_eq!(
            SelfUpdateProgress::from_status(3),
            Some(SelfUpdateProgress::Error)
        );
        assert_eq!(SelfUpdateProgress::from_status(9), None);
    }
}
