// FFI entry points for the `kiosque-update` companion binary. These reuse the
// flatpak helpers in `crate::flatpak::cli` so update logic lives in one place
// instead of being reimplemented in C++ via QProcess.

/// One-shot update: check for available updates and apply them if any exist.
///
/// Returns a process exit code: `0` when there is nothing to update or the
/// update succeeded, non-zero on failure. Runs entirely on the shared tokio
/// runtime and needs no Qt event loop, so the `kiosque-update --check` path can
/// call it directly from `main`.
#[unsafe(no_mangle)]
pub extern "C" fn kiosque_oneshot_update() -> i32 {
    crate::runtime::runtime().block_on(async {
        match crate::flatpak::cli::list_updates().await {
            Ok(updates) if updates.is_empty() => {
                eprintln!("[kiosque-update] No updates available.");
                0
            }
            Ok(updates) => {
                eprintln!(
                    "[kiosque-update] {} update(s) available, running flatpak update...",
                    updates.len()
                );
                match crate::flatpak::cli::update_all().await {
                    Ok(()) => {
                        eprintln!("[kiosque-update] Updates completed successfully.");
                        0
                    }
                    Err(e) => {
                        eprintln!("[kiosque-update] Update failed: {}", e);
                        1
                    }
                }
            }
            Err(e) => {
                eprintln!("[kiosque-update] Update check failed: {}", e);
                1
            }
        }
    })
}
