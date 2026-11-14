//! Lightweight stderr logging with a consistent `[kiosque]` prefix.
//!
//! These macros replace the hand-written `eprintln!("[kiosque] ...")` /
//! `eprintln!("[kiosque] ERROR ...")` calls that were scattered across the
//! crate, so the prefix is defined in exactly one place. Output format is
//! intentionally identical to the previous calls.

/// Log an informational line: `[kiosque] <message>`.
#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => {
        eprintln!("[kiosque] {}", format_args!($($arg)*))
    };
}

/// Log an error line: `[kiosque] ERROR <message>`.
#[macro_export]
macro_rules! kerr {
    ($($arg:tt)*) => {
        eprintln!("[kiosque] ERROR {}", format_args!($($arg)*))
    };
}
