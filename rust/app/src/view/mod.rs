//! Qt-free view state behind the QML bridge. Everything here is plain Rust
//! and tested with `cargo test`; `bridge/` only adapts it to Qt Bridge types.

pub mod app_list;
pub mod details;
pub mod installed;
