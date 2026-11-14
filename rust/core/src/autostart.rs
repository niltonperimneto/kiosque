//! Autostart of the update notifier (`kiosque-update --background`).
//!
//! [`apply`] is the only writer. It is called when the user saves settings,
//! never by the updater itself, so a disabled preference stays disabled.
//!
//! * Inside a Flatpak only the Background portal is used; the sandbox cannot
//!   write host autostart entries.
//! * On a host install the portal is tried first (with the host Registry
//!   announcing our app id). If it refuses or is unavailable, an XDG autostart
//!   file `~/.config/autostart/org.kiosque.Kiosque.Update.desktop` is written
//!   or removed instead.
//!
//! Files written by earlier versions (systemd user timer, the C++ notifier's
//! `com.kiosque.app.update.desktop`) are removed by [`migrate_legacy`].

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use zbus::{OwnedVariant, Variant};

pub const APP_ID: &str = "org.kiosque.Kiosque";
pub const AUTOSTART_FILE: &str = "org.kiosque.Kiosque.Update.desktop";
pub const COMMANDLINE: [&str; 2] = ["kiosque-update", "--background"];
const LEGACY_AUTOSTART: &str = "com.kiosque.app.update.desktop";
/// The portal may show a permission dialog; give the user time to answer.
const PORTAL_TIMEOUT: Duration = Duration::from_secs(120);

/// What [`apply`] ended up doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The Background portal accepted the request.
    Portal { autostart: bool },
    /// The XDG autostart file was written (`true`) or removed (`false`).
    File { autostart: bool },
}

pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"))
}

/// Contents of the host-install fallback autostart entry.
pub fn desktop_entry() -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Kiosque Update Notifier\n\
         Comment=Checks for Flatpak updates in the background\n\
         Exec={}\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n\
         X-KDE-autostart-phase=2\n",
        COMMANDLINE.join(" ")
    )
}

/// Write or remove the fallback autostart file below `config`.
pub fn set_autostart_file(config: &Path, enable: bool) -> std::io::Result<()> {
    let path = config.join("autostart").join(AUTOSTART_FILE);
    if enable {
        fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("desktop.part");
        fs::write(&tmp, desktop_entry())?;
        fs::rename(&tmp, &path)
    } else {
        match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// Remove schedulers written by earlier versions. Returns true if a systemd
/// timer unit was found (the caller then disables it). Idempotent.
pub fn migrate_legacy(config: &Path) -> bool {
    let _ = fs::remove_file(config.join("autostart").join(LEGACY_AUTOSTART));
    let units = config.join("systemd/user");
    let timer = units.join("kiosque-update.timer");
    let had_timer = timer.exists();
    let _ = fs::remove_file(&timer);
    let _ = fs::remove_file(units.join("kiosque-update.service"));
    let _ = fs::remove_file(units.join("timers.target.wants/kiosque-update.timer"));
    had_timer
}

/// Decide the fallback after a portal attempt.
pub fn decide(
    flatpak: bool,
    enable: bool,
    portal: Result<bool, String>,
) -> Result<Outcome, String> {
    match portal {
        Ok(granted) if granted || !enable => Ok(Outcome::Portal {
            autostart: enable && granted,
        }),
        Ok(_) if flatpak => Err("the Background portal denied autostart".into()),
        Err(e) if flatpak => Err(e),
        _ => Ok(Outcome::File { autostart: enable }),
    }
}

/// Apply the autostart preference. Call only from an explicit settings save.
pub async fn apply(enable: bool) -> Result<Outcome, String> {
    let flatpak = in_flatpak();
    let config = config_dir();
    if !flatpak && migrate_legacy(&config) {
        let _ = tokio::process::Command::new("systemctl")
            .args(["--user", "disable", "--now", "kiosque-update.timer"])
            .status()
            .await;
        let _ = tokio::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()
            .await;
    }
    let portal = request_background(enable, !flatpak).await;
    if let Err(e) = &portal {
        klog!("autostart: Background portal unavailable: {}", e);
    }
    let outcome = decide(flatpak, enable, portal)?;
    if !flatpak {
        // Keep exactly one mechanism: drop the fallback file when the portal
        // took over, write/remove it when the portal did not.
        let file = matches!(outcome, Outcome::File { autostart: true });
        set_autostart_file(&config, file).map_err(|e| e.to_string())?;
    }
    klog!("autostart: {:?}", outcome);
    Ok(outcome)
}

/// Ask the Background portal for `autostart = enable` and wait for its
/// `Response`. Returns whether background running/autostart was granted.
async fn request_background(enable: bool, register_host: bool) -> Result<bool, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    if register_host {
        // Host apps must announce their app id before any other portal call.
        let options: HashMap<&str, Variant<'_>> = HashMap::new();
        conn.call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/host/portal/Registry",
            Some("org.freedesktop.host.portal.Registry"),
            "Register",
            &(APP_ID, options),
        )
        .await
        .map_err(|e| format!("host registry: {e}"))?;
    }
    let token = format!("kiosque_bg_{}", std::process::id());
    let sender = conn
        .unique_name()
        .ok_or("no unique bus name")?
        .trim_start_matches(':')
        .replace('.', "_");
    let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Request")
        .member("Response")
        .path(request_path.as_str())
        .build()
        .map_err(|e| e.to_string())?;
    let mut responses = zbus::MessageStream::for_match_rule(rule, &conn, None)
        .await
        .map_err(|e| e.to_string())?;

    let mut options: HashMap<&str, Variant<'_>> = HashMap::new();
    options.insert("handle_token", Variant::from(token.as_str()));
    options.insert(
        "reason",
        Variant::from("Check for and install Flatpak updates in the background"),
    );
    options.insert("autostart", Variant::from(enable));
    options.insert("commandline", Variant::from(COMMANDLINE.to_vec()));
    options.insert("dbus-activatable", Variant::from(false));
    conn.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.Background"),
        "RequestBackground",
        &("", options),
    )
    .await
    .map_err(|e| e.to_string())?;

    let message = tokio::time::timeout(PORTAL_TIMEOUT, responses.next())
        .await
        .map_err(|_| "timed out waiting for the Background portal".to_string())?
        .ok_or("portal connection closed")?
        .map_err(|e| e.to_string())?;
    let (code, results): (u32, HashMap<String, OwnedVariant>) =
        message.body().deserialize().map_err(|e| e.to_string())?;
    if code != 0 {
        return Ok(false);
    }
    let flag = |k: &str| results.get(k).and_then(|v| bool::try_from(v).ok());
    Ok(flag("background").unwrap_or(true) && (!enable || flag("autostart").unwrap_or(false)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_decisions() {
        // Sandbox: the portal is the only mechanism.
        assert_eq!(
            decide(true, true, Ok(true)),
            Ok(Outcome::Portal { autostart: true })
        );
        assert!(decide(true, true, Ok(false)).is_err());
        assert!(decide(true, true, Err("no portal".into())).is_err());
        assert_eq!(
            decide(true, false, Ok(true)),
            Ok(Outcome::Portal { autostart: false })
        );
        // Host: refusal or absence falls back to the XDG file.
        assert_eq!(
            decide(false, true, Ok(false)),
            Ok(Outcome::File { autostart: true })
        );
        assert_eq!(
            decide(false, true, Err("no registry".into())),
            Ok(Outcome::File { autostart: true })
        );
        assert_eq!(
            decide(false, false, Err("no portal".into())),
            Ok(Outcome::File { autostart: false })
        );
        assert_eq!(
            decide(false, true, Ok(true)),
            Ok(Outcome::Portal { autostart: true })
        );
    }

    #[test]
    fn autostart_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        set_autostart_file(dir.path(), true).unwrap();
        let path = dir.path().join("autostart").join(AUTOSTART_FILE);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Exec=kiosque-update --background\n"));
        assert!(text.starts_with("[Desktop Entry]\n"));
        set_autostart_file(dir.path(), false).unwrap();
        assert!(!path.exists());
        set_autostart_file(dir.path(), false).unwrap();
    }

    #[test]
    fn legacy_schedulers_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let units = dir.path().join("systemd/user");
        fs::create_dir_all(units.join("timers.target.wants")).unwrap();
        fs::create_dir_all(dir.path().join("autostart")).unwrap();
        for f in [
            "kiosque-update.timer",
            "kiosque-update.service",
            "timers.target.wants/kiosque-update.timer",
        ] {
            fs::write(units.join(f), "").unwrap();
        }
        let legacy = dir.path().join("autostart").join(LEGACY_AUTOSTART);
        fs::write(&legacy, "").unwrap();
        let ours = dir.path().join("autostart").join(AUTOSTART_FILE);
        fs::write(&ours, "keep").unwrap();
        assert!(migrate_legacy(dir.path()));
        assert!(!legacy.exists());
        assert!(!units.join("kiosque-update.timer").exists());
        assert!(!units.join("kiosque-update.service").exists());
        assert!(ours.exists(), "the current autostart entry is not legacy");
        assert!(!migrate_legacy(dir.path()));
    }
}
