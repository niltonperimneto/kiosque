use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_update: bool,
    pub update_frequency: String, // "Daily", "Weekly"
    pub update_time: String,      // "HH:MM"

    #[serde(default)]
    pub odrs_salt: String,

    #[serde(default)]
    pub oauth_provider: Option<String>,
    #[serde(default)]
    pub oauth_username: Option<String>,
    #[serde(default)]
    pub oauth_user_id: Option<String>,
    #[serde(default)]
    pub oauth_avatar_url: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let salt_bytes: [u8; 32] = rand::random();
        let salt_hex = salt_bytes.iter().map(|b| format!("{:02x}", b)).collect();

        Self {
            auto_update: false,
            update_frequency: "Daily".to_string(),
            update_time: "02:00".to_string(),
            odrs_salt: salt_hex,
            oauth_provider: None,
            oauth_username: None,
            oauth_user_id: None,
            oauth_avatar_url: None,
        }
    }
}

pub fn settings_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
    path.push("kiosque");
    fs::create_dir_all(&path).ok();
    path.push("settings.json");
    path
}

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn load(&self) -> AppSettings {
        match self.load_private() {
            Ok(settings) => settings,
            Err(_) => {
                kerr!("Settings could not be read securely; using defaults");
                AppSettings::default()
            }
        }
    }

    fn private_parent(&self) -> Result<(), String> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let parent = self.path.parent().ok_or("Invalid settings path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let metadata = fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err("Settings directory must be owned by this user".into());
        }
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())
    }

    fn load_private(&self) -> Result<AppSettings, String> {
        use std::{
            io::Read,
            os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        };
        self.private_parent()?;
        let mut file = match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&self.path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let settings = AppSettings::default();
                self.save(&settings)?;
                return Ok(settings);
            }
            Err(error) => return Err(error.to_string()),
        };
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.nlink() != 1
            || metadata.len() > 64 * 1024
        {
            return Err("Invalid settings file".into());
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        file.by_ref()
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("Settings file is too large".into());
        }
        let mut value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid settings JSON")?;
        let legacy_token = value
            .as_object_mut()
            .and_then(|object| object.remove("oauth_token"))
            .is_some();
        let mut settings: AppSettings =
            serde_json::from_value(value).map_err(|_| "Invalid settings data")?;
        let missing_salt = settings.odrs_salt.is_empty();
        if missing_salt {
            settings.odrs_salt = AppSettings::default().odrs_salt;
        }
        if legacy_token || missing_salt {
            self.save(&settings)?;
        }
        Ok(settings)
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        self.private_parent()?;
        if fs::symlink_metadata(&self.path).is_ok_and(|metadata| !metadata.is_file()) {
            return Err("Refusing to replace a non-regular settings file".into());
        }
        let temporary = self
            .path
            .with_extension(format!("{:016x}.tmp", rand::random::<u64>()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
                .open(&temporary)
                .map_err(|e| e.to_string())?;
            let json =
                serde_json::to_vec_pretty(settings).map_err(|_| "Could not encode settings")?;
            file.write_all(&json)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

pub fn load_settings() -> AppSettings {
    SettingsStore::new(settings_path()).load()
}
pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    SettingsStore::new(settings_path()).save(settings)
}

/// Apply the auto-update preference: the daemon (`kiosque-update`) runs the
/// schedule itself, so all that is needed is asking the Background portal to
/// autostart it (or not) at login. Callers also hand the schedule to the
/// running daemon (`daemon::client::configure_automatic_updates`) so the
/// change applies immediately. This is the only scheduling mechanism; files
/// written by older Kiosque versions (systemd user timer, manual autostart
/// entry) are removed.
pub async fn apply_autostart(settings: &AppSettings) -> Result<(), String> {
    request_background(settings.auto_update).await?;
    // Preserve the existing scheduler if the portal rejects the replacement.
    tokio::task::spawn_blocking(remove_legacy_schedulers)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn request_background(autostart: bool) -> Result<(), String> {
    use futures_util::StreamExt;
    use std::{collections::HashMap, time::Duration};
    use zbus::{OwnedObjectPath, OwnedVariant, zvariant::Variant};

    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let sender = conn
        .unique_name()
        .ok_or("session bus has no unique name")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!("kiosque_background_{:016x}", rand::random::<u64>());
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let mut request = zbus::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await
    .map_err(|e| e.to_string())?;
    // Subscribe before calling the portal: a granted request can finish immediately.
    let mut responses = request
        .receive_signal("Response")
        .await
        .map_err(|e| e.to_string())?;
    let mut options: HashMap<&str, Variant<'_>> = HashMap::new();
    options.insert("handle_token", Variant::from(token.as_str()));
    options.insert(
        "reason",
        Variant::from("Install Flatpak updates automatically in the background"),
    );
    options.insert("autostart", Variant::from(autostart));
    // Inside Flatpak the host companion does the work: the packaged app
    // only hands it the schedule at login (see `kiosque --update-companion`).
    let commandline = if crate::flatpak::in_sandbox() {
        vec!["kiosque", "--update-companion"]
    } else {
        vec!["kiosque-update", "--background"]
    };
    options.insert("commandline", Variant::from(commandline));
    options.insert("dbus-activatable", Variant::from(false));
    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Background"),
            "RequestBackground",
            &("", options),
        )
        .await
        .map_err(|e| e.to_string())?;
    let handle: OwnedObjectPath = reply.body().deserialize().map_err(|e| e.to_string())?;
    if handle.as_str() != path {
        request = zbus::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            handle.as_str(),
            "org.freedesktop.portal.Request",
        )
        .await
        .map_err(|e| e.to_string())?;
        responses = request
            .receive_signal("Response")
            .await
            .map_err(|e| e.to_string())?;
    }
    let message = match tokio::time::timeout(Duration::from_secs(120), responses.next()).await {
        Ok(Some(message)) => message,
        _ => {
            let _: zbus::Result<()> = request.call("Close", &()).await;
            return Err("Background permission request timed out or closed".into());
        }
    };
    let (response, results): (u32, HashMap<String, OwnedVariant>) =
        message.body().deserialize().map_err(|e| e.to_string())?;
    if response != 0 {
        return Err("Background permission was cancelled or denied".into());
    }
    let enabled = results
        .get("autostart")
        .and_then(|value| bool::try_from(value).ok());
    if enabled != Some(autostart) {
        return Err("The desktop did not apply the requested autostart setting".into());
    }
    klog!("Background portal: autostart={}", autostart);
    Ok(())
}

fn remove_legacy_schedulers() {
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
    let timer = config.join("systemd/user/kiosque-update.timer");
    if timer.exists() {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "kiosque-update.timer"])
            .status();
        let _ = fs::remove_file(&timer);
        let _ = fs::remove_file(config.join("systemd/user/kiosque-update.service"));
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
    }
    let _ = fs::remove_file(config.join("autostart/com.kiosque.app.update.desktop"));
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn legacy_credentials_are_removed_and_storage_is_private() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("app/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut legacy = serde_json::to_value(AppSettings::default()).unwrap();
        legacy["oauth_token"] = "synthetic-secret".into();
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let store = SettingsStore::new(&path);
        let settings = store.load();
        assert!(!settings.odrs_salt.is_empty());
        assert!(
            !fs::read_to_string(&path)
                .unwrap()
                .contains("synthetic-secret")
        );
        assert!(!fs::read_to_string(&path).unwrap().contains("oauth_token"));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(settings.odrs_salt, store.load().odrs_salt);
    }
    #[test]
    fn settings_never_follow_symlinks_or_modify_hard_link_targets() {
        let root = tempfile::tempdir().unwrap();
        let victim = root.path().join("victim");
        fs::write(&victim, "preserve").unwrap();
        let path = root.path().join("app/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&victim, &path).unwrap();
        let store = SettingsStore::new(&path);
        assert!(store.load_private().is_err());
        assert!(store.save(&AppSettings::default()).is_err());
        fs::remove_file(&path).unwrap();
        fs::hard_link(&victim, &path).unwrap();
        assert!(store.load_private().is_err());
        assert_eq!(fs::read_to_string(victim).unwrap(), "preserve");
    }
}
