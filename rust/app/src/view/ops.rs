//! Backend operations and registry snapshots used by the views.

use kiosque_core::entries::InstalledEntry;
use kiosque_core::flatpak::{RemoteInfo, RemoteSource, Scope};

use crate::services::services;

pub async fn launch(id: &str) -> Result<(), String> {
    services()
        .backend
        .launch(id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn remotes() -> Result<Vec<RemoteInfo>, String> {
    services()
        .backend
        .list_remotes()
        .await
        .map_err(|e| e.to_string())
}

pub async fn add_remote(name: &str, url: &str) -> Result<(), String> {
    services()
        .backend
        .add_remote(name, RemoteSource::Url(url.into()), Scope::User)
        .await
        .map_err(|e| e.to_string())
}

pub async fn remove_remote(name: &str) -> Result<(), String> {
    services()
        .backend
        .remove_remote(name, None)
        .await
        .map_err(|e| e.to_string())
}

/// Reload installed refs into the registry.
pub async fn reload_installed() -> Result<(), String> {
    let s = services();
    s.registry
        .reload_installed(&*s.backend)
        .await
        .map_err(|e| e.to_string())
}

/// Reload available updates into the registry.
pub async fn reload_updates() -> Result<(), String> {
    let s = services();
    s.registry
        .reload_updates(&*s.backend)
        .await
        .map_err(|e| e.to_string())
}

/// Installed version of `id`, if known.
pub fn installed_version(id: &str) -> Option<String> {
    services()
        .registry
        .installed()
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.version)
}

/// The registry's installed refs and updates as view entries.
pub fn installed_entries() -> Vec<InstalledEntry> {
    let registry = &services().registry;
    let mut entries: Vec<InstalledEntry> = registry
        .installed()
        .into_iter()
        .map(|app| {
            let has_update = registry.state(&app.id).update.is_some();
            InstalledEntry {
                is_runtime: app.is_runtime(),
                size: super::installed::format_size(app.installed_size),
                size_bytes: app.installed_size,
                name: app.name,
                app_id: app.id,
                version: app.version,
                origin: app.origin,
                has_update,
                is_checked_for_update: has_update,
            }
        })
        .collect();
    // Runtime updates for refs not listed as installed (e.g. extensions).
    for update in registry.updates() {
        if !entries.iter().any(|e| e.app_id == update.id) {
            entries.push(InstalledEntry {
                is_runtime: update.kind == kiosque_core::flatpak::RefKind::Runtime,
                name: update.name,
                app_id: update.id,
                version: update.version,
                origin: update.origin,
                has_update: true,
                is_checked_for_update: true,
                ..Default::default()
            });
        }
    }
    entries
}
