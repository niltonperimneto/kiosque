//! Inspection, confirmation and installation of Flatpak reference files.
pub use object::{StoreController, read_repo_file};
use qtbridge::qobject;
#[qobject(NoQmlElement)]
mod object {
    use qtbridge::QmlObject;
    use std::sync::Arc;

    use crate::bridge::util::{self, JobControl, JobOutcome};
    use crate::flatpak::{Operation, Scope};

    #[derive(Default)]
    pub struct StoreController {
        pub(super) error_message: String,
        pub(super) install_progress: f64,
        /// Status line of the current reference-file installation.
        pub(super) install_status: String,
        pub(super) job: Option<Arc<JobControl>>,
        pub(super) ref_key_sha256: String,
        pub(super) pending_ref: Option<(Vec<u8>, String)>,
        /// Inspection request generation; superseded inspections are dropped.
        pub(super) ref_generation: u64,
    }

    impl StoreController {
        #[qslot(qml_name = "cancelOperation")]
        pub fn cancel_operation(&mut self) {
            if let Some(job) = self.job.take() {
                job.cancel();
            }
        }

        /// Start `op` as the controller's current job, reporting real progress
        /// through `install_progress` and completion through `installFinished`.
        fn start_job(&mut self, op: Operation, title: String) {
            self.set_install_progress(-1.0);
            self.set_install_status(String::new());
            self.set_error_message(String::new());
            let control = JobControl::new();
            self.job = Some(control.clone());
            let qt_thread = self.get_thread_handle();
            util::run_job(
                qt_thread,
                op,
                title,
                control.clone(),
                |q, p, status| {
                    q.set_install_progress(p);
                    q.set_install_status(status);
                },
                move |q, outcome| {
                    let current = q.job.as_ref().is_some_and(|job| Arc::ptr_eq(job, &control));
                    if current {
                        q.job = None;
                    }
                    match outcome {
                        JobOutcome::Done => {
                            klog!("StoreController: job finished");
                            q.set_install_progress(1.0);
                            q.install_finished(true);
                        }
                        JobOutcome::Cancelled => {
                            klog!("StoreController: job cancelled");
                            q.set_install_progress(0.0);
                            q.install_finished(false);
                        }
                        JobOutcome::Failed(e) => {
                            kerr!("StoreController: job failed: {}", e);
                            q.set_install_progress(0.0);
                            q.set_error_message(String::from(&e));
                            q.install_finished(false);
                        }
                    }
                },
            );
        }
        #[qslot(qml_name = "inspectRefFile")]
        pub fn inspect_ref_file(&mut self, path: String) {
            let source = path.to_string();
            klog!("StoreController::inspect_ref_file: \"{}\"", source);
            self.pending_ref = None;
            self.ref_generation += 1;
            let generation = self.ref_generation;
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                let result = read_ref_file(&source).await;
                qt_thread.invoke_mut(move |q| {
                    if q.ref_generation != generation {
                        return;
                    }
                    let qs = |s: &str| String::from(s);
                    match result {
                        Ok(data) => {
                            let validated = kiosque_core::security::ref_file(&data)
                                .expect("validated reference");
                            q.set_ref_key_sha256(qs(&validated.key_sha256));
                            let mut summary = crate::flatpak::summarize_keyfile(&data);
                            summary.url = validated.origin;
                            q.pending_ref = Some((data, summary.title.clone()));
                            q.ref_file_inspected(
                                true,
                                qs(&summary.title),
                                qs(&summary.name),
                                qs(&summary.url),
                                summary.is_runtime,
                                qs(""),
                            );
                        }
                        Err(e) => {
                            kerr!("StoreController::inspect_ref_file: {}", e);
                            q.ref_file_inspected(false, qs(""), qs(""), qs(""), false, qs(&e));
                        }
                    }
                });
            });
        }

        #[qslot(qml_name = "confirmRefFileInstall")]
        pub fn confirm_ref_file_install(&mut self) {
            let Some((data, title)) = self.pending_ref.take() else {
                kerr!("StoreController::confirm_ref_file_install: no inspected .flatpakref");
                return;
            };
            klog!("StoreController::confirm_ref_file_install: \"{}\"", title);
            self.start_job(
                Operation::InstallRefFile {
                    data,
                    scope: Scope::User,
                },
                title,
            );
        }

        #[qslot(qml_name = "discardRefFile")]
        pub fn discard_ref_file(&mut self) {
            self.pending_ref = None;
            // Drop any inspection still in flight as well.
            self.ref_generation += 1;
        }
    }

    /// Largest `.flatpakref`/`.flatpakrepo` accepted; real ones are a few KiB
    /// (mostly the GPG key).
    const MAX_REF_FILE_SIZE: usize = 1024 * 1024;

    /// Read a `.flatpakref` from a local path, `file://` URL or `http(s)://` URL
    /// and check that it looks like one.
    async fn read_ref_file(source: &str) -> Result<Vec<u8>, String> {
        read_keyfile(source, ".flatpakref", "[Flatpak Ref]").await
    }

    /// Read a `.flatpakrepo` from a local path, `file://` URL or `http(s)://` URL
    /// and check that it looks like one.
    pub async fn read_repo_file(source: &str) -> Result<Vec<u8>, String> {
        read_keyfile(source, ".flatpakrepo", "[Flatpak Repo]").await
    }

    async fn read_keyfile(source: &str, kind: &str, group: &str) -> Result<Vec<u8>, String> {
        let source = source.trim();
        if source.is_empty() {
            return Err(format!("no {kind} file given"));
        }
        let lower = source.to_ascii_lowercase();
        let data = if lower.starts_with("http://") || lower.starts_with("https://") {
            crate::services::services()
                .http
                .fetch_bytes_limited(source, MAX_REF_FILE_SIZE)
                .await
                .map_err(|e| format!("could not download {source}: {e}"))?
        } else {
            let path = if lower.starts_with("file://") {
                let rest = &source["file://".len()..];
                // file://host/path: only the local host is meaningful.
                let rest = rest.strip_prefix("localhost").unwrap_or(rest);
                if !rest.starts_with('/') {
                    return Err(format!("unsupported file URL: {source}"));
                }
                percent_decode(rest)?
            } else if lower.contains("://") {
                return Err(format!("unsupported {kind} location: {source}"));
            } else {
                source.to_string()
            };
            let path = std::path::PathBuf::from(path);
            tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
                let metadata = std::fs::metadata(&path)
                    .map_err(|e| format!("could not read {}: {e}", path.display()))?;
                if !metadata.is_file() {
                    return Err(format!("{} is not a file", path.display()));
                }
                if metadata.len() > MAX_REF_FILE_SIZE as u64 {
                    return Err(format!("{} is too large", path.display()));
                }
                kiosque_core::security::read_local_file(&path, MAX_REF_FILE_SIZE)
            })
            .await
            .map_err(|e| e.to_string())??
        };
        if data.len() > MAX_REF_FILE_SIZE {
            return Err(format!("{source} is too large"));
        }
        let text =
            std::str::from_utf8(&data).map_err(|_| format!("{source} is not a text file"))?;
        if !text.lines().any(|l| l.trim() == group) {
            return Err(format!("{source} is not a {kind} file"));
        }
        if group == "[Flatpak Ref]" {
            kiosque_core::security::ref_file(&data)?;
        } else {
            kiosque_core::security::repo_file(&data)?;
        }
        Ok(data)
    }

    /// Decode `%XX` escapes of a `file://` URL path.
    fn percent_decode(input: &str) -> Result<String, String> {
        let bytes = input.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                let hex = input
                    .get(i + 1..i + 3)
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                    .ok_or_else(|| format!("invalid escape in file URL: {input}"))?;
                out.push(hex);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8(out).map_err(|_| format!("file URL is not UTF-8: {input}"))
    }

    impl StoreController {
        qproperty!(
            "ref_key_sha256",
            Read = get_ref_key_sha256,
            Notify = ref_key_sha256_changed
        );
        fn get_ref_key_sha256(&self) -> String {
            self.ref_key_sha256.clone()
        }
        #[qsignal(qml_name = "ref_key_sha256Changed")]
        pub fn ref_key_sha256_changed(&mut self);
        qproperty!(
            "error_message",
            Read = get_error_message,
            Notify = error_message_changed
        );
        fn get_error_message(&self) -> String {
            self.error_message.clone()
        }
        #[qsignal(qml_name = "error_messageChanged")]
        pub fn error_message_changed(&mut self);
        qproperty!(
            "install_progress",
            Read = get_install_progress,
            Notify = install_progress_changed
        );
        fn get_install_progress(&self) -> f64 {
            self.install_progress
        }
        #[qsignal(qml_name = "install_progressChanged")]
        pub fn install_progress_changed(&mut self);
        qproperty!(
            "install_status",
            Read = get_install_status,
            Notify = install_status_changed
        );
        fn get_install_status(&self) -> String {
            self.install_status.clone()
        }
        #[qsignal(qml_name = "install_statusChanged")]
        pub fn install_status_changed(&mut self);
        #[qsignal(qml_name = "refFileInspected")]
        pub fn ref_file_inspected(
            &mut self,
            success: bool,
            title: String,
            name: String,
            url: String,
            is_runtime: bool,
            error: String,
        );
        #[qsignal(qml_name = "installFinished")]
        pub fn install_finished(&mut self, success: bool);
    }
}
notify_setters!(StoreController {
set_ref_key_sha256: ref_key_sha256: String => ref_key_sha256_changed,
set_error_message: error_message: String => error_message_changed,
set_install_progress: install_progress: f64 => install_progress_changed,
set_install_status: install_status: String => install_status_changed
});
qml_element!(StoreController, "StoreController", singleton);
