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
    env!("KIOSQUE_VERSION")
}

#[qobject(NoQmlElement)]
mod object {
    use super::*;

    pub struct AppShell {
        pub(super) check_ui: bool,
        pub(super) version: String,
        pub(super) initial_app_id: String,
        pub(super) initial_ref_file: String,
        pub(super) initial_repo_file: String,
    }

    impl Default for AppShell {
        fn default() -> Self {
            Self {
                check_ui: CHECK_UI.get().copied().unwrap_or(false),
                version: version().to_string(),
                initial_app_id: launch_targets().0,
                initial_ref_file: launch_targets().1,
                initial_repo_file: launch_targets().2,
            }
        }
    }

    impl AppShell {
        qproperty!("checkUi", Member = check_ui, Constant);
        qproperty!("version", Member = version, Constant);
        qproperty!("initialAppId", Member = initial_app_id, Constant);
        qproperty!("initialRefFile", Member = initial_ref_file, Constant);
        qproperty!("initialRepoFile", Member = initial_repo_file, Constant);

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

fn launch_targets() -> (String, String, String) {
    let mut app = String::new();
    let mut reference = String::new();
    let mut repo = String::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--app" {
            app = args.next().unwrap_or_default();
        } else if let Some(id) = arg.strip_prefix("appstream://") {
            app = id.trim_end_matches('/').trim_end_matches(".desktop").into();
        } else if arg
            .split('?')
            .next()
            .is_some_and(|s| s.ends_with(".flatpakref"))
        {
            reference = arg;
        } else if arg
            .split('?')
            .next()
            .is_some_and(|s| s.ends_with(".flatpakrepo"))
        {
            repo = arg;
        }
    }
    (app, reference, repo)
}
