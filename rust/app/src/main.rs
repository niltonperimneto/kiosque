//! `kiosque`: the storefront window.

use std::time::Duration;

use kiosque::bridge::app_shell;

const USAGE: &str = "Usage: kiosque [--app ID | appstream://ID | FILE.flatpakref | FILE.flatpakrepo] [--check-ui] [--version] [--help]\n\n  \
    --check-ui  Load the packaged UI, print KIOSQUE_UI_READY and exit\n  \
    --update-companion  Hand the automatic-update schedule to the host companion and exit\n  \
    --version   Print the version and exit\n  \
    --help      Show this help and exit\n";

/// Give up on `--check-ui` if QML never reports back.
const CHECK_UI_TIMEOUT: Duration = Duration::from_secs(30);

fn main() {
    let mut check_ui = false;
    // Other arguments (e.g. `-platform offscreen`) are Qt's.
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return;
            }
            "-v" | "--version" => {
                println!("kiosque {}", app_shell::version());
                return;
            }
            "--check-ui" => check_ui = true,
            // Login autostart inside Flatpak: no window, no Qt.
            "--update-companion" => {
                std::process::exit(
                    kiosque_core::runtime::runtime()
                        .block_on(kiosque_core::daemon::start_update_companion()),
                );
            }
            _ => {}
        }
    }
    app_shell::set_check_ui(check_ui);

    kiosque::register_resources();
    kiosque::configure_style();
    let mut app = qtbridge::QApp::new();
    cxx_qt_lib::QGuiApplication::set_desktop_file_name(&cxx_qt_lib::QString::from(
        "org.kiosque.Kiosque",
    ));
    app.application_name("Kiosque");
    kiosque_core::l10n::init();
    kiosque::services::init();
    kiosque::bridge::register_all(&mut app);

    if check_ui {
        std::thread::spawn(|| {
            std::thread::sleep(CHECK_UI_TIMEOUT);
            eprintln!("kiosque: --check-ui timed out after {CHECK_UI_TIMEOUT:?}");
            std::process::exit(1);
        });
    }
    let code = app.load_qml_from_file("qrc:/qml/Main.qml").run();
    // A QML load failure quits the event loop with status 0; only a
    // reported start counts.
    if check_ui && !app_shell::ui_ready() {
        eprintln!("kiosque: the UI did not start");
        std::process::exit(1);
    }
    drop(app);
    std::process::exit(code);
}
