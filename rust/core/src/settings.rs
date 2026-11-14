use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
