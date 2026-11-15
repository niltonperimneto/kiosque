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
    pub oauth_token: Option<String>,
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
            oauth_token: None,
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
        let mut settings = fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
            .unwrap_or_default();
        if settings.odrs_salt.is_empty() {
            settings.odrs_salt = AppSettings::default().odrs_salt;
            let _ = self.save(&settings);
        }
        settings
    }
    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())
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
/// autostart it (or not). This is the only scheduling mechanism; files
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
    options.insert(
        "commandline",
        Variant::from(vec!["kiosque-update", "--background"]),
    );
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
