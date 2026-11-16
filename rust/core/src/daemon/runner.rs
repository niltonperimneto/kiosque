//! KRunner D-Bus runner (`org.kde.krunner1`).
//!
//! Registered through `share/krunner/dbusplugins/org.kiosque.Kiosque.desktop`.
//! Matches come from the local AppStream catalog, so they work offline and
//! include every configured remote.

use std::collections::HashMap;

use zbus::interface;
use zbus::{OwnedVariant, Structure, Variant};

/// Plasma's `KRunner::QueryMatch::CategoryRelevance` values.
const RELEVANCE_HIGH: i32 = 100;
const RELEVANCE_MODERATE: i32 = 50;
const RELEVANCE_LOW: i32 = 30;

const ICON_SIZE: u32 = 64;
const MAX_MATCHES: usize = 8;

pub struct Runner;

/// `(id, text, icon name, category relevance, relevance, properties)`
pub type RemoteMatch = (
    String,
    String,
    String,
    i32,
    f64,
    HashMap<String, OwnedVariant>,
);

/// freedesktop `image-data`: (width, height, rowstride, has_alpha,
/// bits_per_sample, channels, data)
fn icon_data(icon_url: &str) -> Option<OwnedVariant> {
    let path = icon_url.strip_prefix("file://")?;
    use std::io::Read;
    let path = std::path::Path::new(path).canonicalize().ok()?;
    if !crate::image_cache::allowed_local_icon(&path) {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(crate::security::MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let img = crate::image_cache::decode(bytes).ok()?;
    let img = img
        .resize(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let (w, h) = img.dimensions();
    let s = Structure::from((
        w as i32,
        h as i32,
        (w * 4) as i32,
        true,
        8i32,
        4i32,
        img.into_raw(),
    ));
    OwnedVariant::try_from(Variant::from(s)).ok()
}

pub fn matches(query: &str) -> Vec<RemoteMatch> {
    let query = query.trim();
    if query.len() > 512 || query.chars().count() < 3 {
        return vec![];
    }
    let catalog = crate::catalog::catalog();
    let config = crate::config::config();
    let registry = crate::registry::registry();
    let q = query.to_lowercase();
    catalog
        .search(query, MAX_MATCHES * 2)
        .into_iter()
        .filter(|a| !config.blocklist.is_blocked(&a.id))
        // Installed apps are already offered by Plasma's own application runner.
        .filter(|a| !registry.is_installed(&a.id))
        .take(MAX_MATCHES)
        .enumerate()
        .map(|(i, app)| {
            let name = app.name.to_lowercase();
            let category = if name == q {
                RELEVANCE_HIGH
            } else if name.starts_with(&q) {
                RELEVANCE_MODERATE
            } else {
                RELEVANCE_LOW
            };
            let mut props: HashMap<String, OwnedVariant> = HashMap::new();
            let subtext = if app.summary.is_empty() {
                "Install with Kiosque".to_string()
            } else {
                format!("{} — Install with Kiosque", app.summary)
            };
            if let Ok(v) = OwnedVariant::try_from(Variant::from(subtext)) {
                props.insert("subtext".into(), v);
            }
            if let Some(icon) = icon_data(&app.icon) {
                props.insert("icon-data".into(), icon);
            }
            let relevance = 1.0 - (i as f64) * 0.05;
            (
                app.id,
                app.name,
                "system-software-install".to_string(),
                category,
                relevance,
                props,
            )
        })
        .collect()
}

#[interface(name = "org.kde.krunner1")]
impl Runner {
    async fn actions(&self) -> Vec<(String, String, String)> {
        vec![]
    }

    #[zbus(name = "Match")]
    async fn match_(&self, query: String) -> Vec<RemoteMatch> {
        super::touch();
        // The catalog may still be loading right after D-Bus activation.
        let _ = crate::catalog::loaded().await;
        matches(&query)
    }

    async fn run(&self, match_id: String, _action_id: String) {
        super::touch();
        if crate::security::app_ref(&match_id).is_err() || match_id.contains('/') {
            return;
        }
        if crate::catalog::catalog()
            .search(&match_id, 64)
            .iter()
            .all(|app| app.id != match_id)
        {
            return;
        }
        let url = format!("appstream://{}", match_id);
        let Ok(executable) = std::env::current_exe() else {
            return;
        };
        let Some(directory) = executable.parent() else {
            return;
        };
        let spawned = std::process::Command::new(directory.join("kiosque"))
            .arg(&url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Err(e) = spawned {
            kerr!("runner: cannot start kiosque {}: {}", url, e);
        }
    }

    async fn teardown(&self) {}
}
