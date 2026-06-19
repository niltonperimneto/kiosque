use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_update: bool,
    pub update_frequency: String, // "Daily", "Weekly"
    pub update_time: String,      // "HH:MM"
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_update: false,
            update_frequency: "Daily".to_string(),
            update_time: "02:00".to_string(),
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

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    if let Ok(contents) = fs::read_to_string(&path)
        && let Ok(settings) = serde_json::from_str(&contents) {
            return settings;
        }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    
    // Configure systemd timer based on settings
    configure_systemd_timer(settings)?;
    
    Ok(())
}

fn configure_systemd_timer(settings: &AppSettings) -> Result<(), String> {
    let mut systemd_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
    systemd_dir.push("systemd");
    systemd_dir.push("user");
    
    if let Err(e) = fs::create_dir_all(&systemd_dir) {
        eprintln!("[kiosque] WARNING: Could not create systemd user dir: {}", e);
        return Ok(());
    }
    
    let timer_path = systemd_dir.join("kiosque-update.timer");
    let service_path = systemd_dir.join("kiosque-update.service");
    
    if !settings.auto_update {
        // Disable and stop the timer
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "kiosque-update.timer"])
            .status();
        return Ok(());
    }

    // Build the systemd service file
    let service_content = r#"[Unit]
Description=Kiosque Flatpak Auto Updater

[Service]
Type=oneshot
ExecStart=/usr/bin/kiosque-update --check
"#;

    // Build the systemd timer file
    // Frequency "Daily" -> *-*-* HH:MM:00
    // Frequency "Weekly" -> Mon *-*-* HH:MM:00 (Arbitrarily Monday)
    let calendar = if settings.update_frequency.eq_ignore_ascii_case("weekly") {
        format!("Mon *-*-* {}:00", settings.update_time)
    } else {
        format!("*-*-* {}:00", settings.update_time)
    };

    let timer_content = format!(r#"[Unit]
Description=Timer for Kiosque Flatpak Auto Updater

[Timer]
OnCalendar={}
Persistent=true

[Install]
WantedBy=timers.target
"#, calendar);

    fs::write(&service_path, service_content).map_err(|e| e.to_string())?;
    fs::write(&timer_path, timer_content).map_err(|e| e.to_string())?;

    // Reload daemon and enable timer
    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    
    if let Ok(status) = reload
        && status.success() {
            let _ = Command::new("systemctl")
                .args(["--user", "enable", "--now", "kiosque-update.timer"])
                .status();
        }

    Ok(())
}
