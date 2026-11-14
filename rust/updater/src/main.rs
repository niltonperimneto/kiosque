//! `kiosque-update`: background update notifier. Qt-free.
//!
//! The tray, notifications and scheduler move here from the C++ shell; until
//! then this binary only answers `--help`.

const USAGE: &str = "Usage: kiosque-update [--check | --background | --help]\n";

fn main() {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return;
    }
    eprint!("{USAGE}");
    std::process::exit(2);
}
