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
pub fn apply_autostart(settings: &AppSettings) {
    remove_legacy_schedulers();
    let enable = settings.auto_update;
    crate::runtime::runtime().spawn(async move {
        if let Err(e) = request_background(enable).await {
            kerr!("Background portal request failed: {}", e);
        }
    });
}

async fn request_background(autostart: bool) -> zbus::Result<()> {
    use std::collections::HashMap;
    use zbus::Variant;

    let conn = zbus::Connection::session().await?;
    let mut options: HashMap<&str, Variant<'_>> = HashMap::new();
    options.insert("handle_token", Variant::from("kiosque_background"));
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
    conn.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.Background"),
        "RequestBackground",
        &("", options),
    )
    .await?;
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
