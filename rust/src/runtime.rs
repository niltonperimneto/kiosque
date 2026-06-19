use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// Returns a reference to the shared tokio async runtime.
///
/// The runtime is created once on first access and lives for the entire
/// process lifetime. It uses a multi-threaded scheduler with 4 worker
/// threads (sufficient for our I/O-bound workload of HTTP requests and
/// subprocess management).
pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("kiosque-async")
            .enable_all()
            .build()
            .expect("[kiosque] FATAL: Failed to create tokio runtime")
    })
}
