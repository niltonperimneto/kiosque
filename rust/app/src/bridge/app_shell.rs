//! `AppShell` singleton: process-level facts for QML and the `--check-ui`
//! handshake.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use qtbridge::qobject;

pub use object::AppShell;

/// Set once by `main` before QML loads.
static CHECK_UI: OnceLock<bool> = OnceLock::new();
static READY: AtomicBool = AtomicBool::new(false);

pub const READY_MARKER: &str = "KIOSQUE_UI_READY";

pub fn set_check_ui(check: bool) {
    let _ = CHECK_UI.set(check);
}

/// Whether QML reported a successful start (`AppShell.uiReady(true)`).
pub fn ui_ready() -> bool {
    READY.load(Ordering::SeqCst)
}

pub fn version() -> &'static str {
    option_env!("KIOSQUE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

#[qobject(NoQmlElement)]
mod object {
    use super::*;

    pub struct AppShell {
        pub(super) check_ui: bool,
        pub(super) version: String,
    }

    impl Default for AppShell {
        fn default() -> Self {
            Self {
                check_ui: CHECK_UI.get().copied().unwrap_or(false),
                version: version().to_string(),
            }
        }
    }

    impl AppShell {
        qproperty!("checkUi", Member = check_ui, Constant);
        qproperty!("version", Member = version, Constant);

        /// Called by Main.qml once the window and its packaged resources are
        /// up. In `--check-ui` mode this prints the readiness marker; QML
        /// then quits.
        #[qslot(qml_name = "uiReady")]
        fn ui_ready(&self, ok: bool) {
            if ok && !READY.swap(true, Ordering::SeqCst) && self.check_ui {
                println!("{READY_MARKER}");
            }
            if !ok {
                kerr!("UI self-check failed");
            }
        }
    }
}

qml_element!(AppShell, "AppShell", singleton);
