//! Kiosque's Qt Quick front end: `bridge/` (Qt Bridge types), `view/`
//! (Qt-free view state) and the service composition root.

#[macro_use]
extern crate kiosque_core;

pub use kiosque_core::{
    cache, catalog, config, daemon, flathub, flatpak, registry, runtime, settings, transactions,
};
#[macro_use]
pub mod bridge;
pub mod disk_cache;
#[cfg(feature = "test-util")]
pub mod quicktest_ffi;
pub mod services;
pub mod view;

/// Register the embedded `qml/` tree as `qrc:/qml/…`.
pub fn register_resources() {
    include!(concat!(env!("OUT_DIR"), "/qml_resources.rs"));
}

/// Use the Plasma style unless the user chose one. Must run before the QML
/// engine loads Qt Quick Controls.
pub fn configure_style() {
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        cxx_qt_lib::QQuickStyle::set_style(&cxx_qt_lib::QString::from("org.kde.desktop"));
    }
}

#[cfg(feature = "test-util")]
pub mod test_support;
