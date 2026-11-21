#[macro_use]
extern crate kiosque_core;

pub use kiosque_core::{
    cache, catalog, config, daemon, flathub, registry, runtime, settings, transactions,
};
pub mod bridge;
pub mod disk_cache;
pub mod flatpak;
pub mod image_cache;
pub mod services;

/// Start the GUI's background services: the installed-state registry (and
/// its installation monitor), the transaction client and the AppStream
/// catalog. Called once from `main.cpp` before QML is loaded.
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_gui_init() {
    let _ = registry::registry();
    let _ = config::config();
    runtime::runtime().spawn(async {
        let _ = transactions::client().await;
    });
    runtime::runtime().spawn(async {
        catalog::loaded().await;
        catalog::refresh_if_stale().await;
    });
}

/// Read installed state through the production backend for package diagnostics.
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_check_backend() -> i32 {
    let backend = flatpak::backend();
    match runtime::runtime().block_on(backend.list_installed()) {
        Ok(refs) => {
            println!("KIOSQUE_BACKEND_READY {} {}", backend.name(), refs.len());
            0
        }
        Err(error) => {
            eprintln!("KIOSQUE_BACKEND_ERROR {}: {error}", backend.name());
            1
        }
    }
}
