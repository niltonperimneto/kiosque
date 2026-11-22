use cxx_qt::{CxxQtType, Threading};
use std::sync::Arc;

use crate::bridge::detail_data::{self, DetailData, ReviewAction};
use crate::bridge::util::{self, JobControl, JobOutcome};
use crate::flatpak::{Operation, Scope};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, ref_key_sha256)]
        #[qproperty(bool, loading)]
        #[qproperty(QString, error_message)]
        #[qproperty(QString, detail_name)]
        #[qproperty(QString, detail_summary)]
        #[qproperty(QString, detail_description)]
        #[qproperty(QString, detail_icon_url)]
        #[qproperty(QString, detail_developer)]
        #[qproperty(QString, detail_license)]
        #[qproperty(bool, detail_is_installed)]
        #[qproperty(f64, install_progress)]
        #[qproperty(QString, detail_screenshots_json)]
        #[qproperty(QString, detail_permissions_json)]
        #[qproperty(QString, detail_urls_json)]
        #[qproperty(QString, detail_developer_apps_json)]
        #[qproperty(QString, detail_similar_apps_json)]
        #[qproperty(QString, detail_ratings_json)]
        #[qproperty(QString, detail_reviews_json)]
        type StoreController = super::StoreControllerRust;

        #[qinvokable]
        #[cxx_name = "loadAppDetails"]
        fn load_app_details(self: Pin<&mut StoreController>, app_id: QString);

        #[qinvokable]
        #[cxx_name = "installApp"]
        fn install_app(self: Pin<&mut StoreController>, app_id: QString);

        #[qinvokable]
        #[cxx_name = "uninstallApp"]
        fn uninstall_app(self: Pin<&mut StoreController>, app_id: QString);

        #[qinvokable]
        #[cxx_name = "cancelOperation"]
        fn cancel_operation(self: Pin<&mut StoreController>);

        /// Read a `.flatpakref` (a local path, a `file://` URL or an
        /// `http(s)://` URL) without installing it. The result arrives in
        /// `refFileInspected`; on success the file is kept pending until
        /// `confirmRefFileInstall` or `discardRefFile`.
        #[qinvokable]
        #[cxx_name = "inspectRefFile"]
        fn inspect_ref_file(self: Pin<&mut StoreController>, path: QString);

        /// Install the pending `.flatpakref` exactly as inspected. Progress and
        /// completion are reported through `install_progress` /
        /// `installFinished`, errors via `error_message`; `cancelOperation`
        /// cancels it.
        #[qinvokable]
        #[cxx_name = "confirmRefFileInstall"]
        fn confirm_ref_file_install(self: Pin<&mut StoreController>);

        /// Forget the pending `.flatpakref` without installing it.
        #[qinvokable]
        #[cxx_name = "discardRefFile"]
        fn discard_ref_file(self: Pin<&mut StoreController>);

        #[qsignal]
        #[cxx_name = "refFileInspected"]
        fn ref_file_inspected(
            self: Pin<&mut StoreController>,
            success: bool,
            title: QString,
            name: QString,
            url: QString,
            is_runtime: bool,
            error: QString,
        );

        #[qsignal]
        #[cxx_name = "detailsLoaded"]
        fn details_loaded(self: Pin<&mut StoreController>);

        #[qsignal]
        #[cxx_name = "installFinished"]
        fn install_finished(self: Pin<&mut StoreController>, success: bool);

        #[qinvokable]
        #[cxx_name = "submitReview"]
        fn submit_review(
            self: Pin<&mut StoreController>,
            app_id: QString,
            rating: i32,
            summary: QString,
            description: QString,
            version: QString,
            distro: QString,
            locale: QString,
            is_anonymous: bool,
        );

        #[qinvokable]
        #[cxx_name = "upvoteReview"]
        fn upvote_review(self: Pin<&mut StoreController>, review_id: i64);

        #[qinvokable]
        #[cxx_name = "downvoteReview"]
        fn downvote_review(self: Pin<&mut StoreController>, review_id: i64);

        #[qinvokable]
        #[cxx_name = "dismissReview"]
        fn dismiss_review(self: Pin<&mut StoreController>, review_id: i64);

        #[qinvokable]
        #[cxx_name = "removeReview"]
        fn remove_review(self: Pin<&mut StoreController>, review_id: i64);

        #[qsignal]
        #[cxx_name = "reviewSubmitted"]
        fn review_submitted(self: Pin<&mut StoreController>, success: bool, error: QString);

        #[qsignal]
        #[cxx_name = "reviewActionFinished"]
        fn review_action_finished(self: Pin<&mut StoreController>, success: bool, error: QString);

        #[qsignal]
        #[cxx_name = "reviewsLoaded"]
        fn reviews_loaded(self: Pin<&mut StoreController>);
    }

    impl cxx_qt::Threading for StoreController {}
}

#[derive(Default)]
pub struct StoreControllerRust {
    pub loading: bool,
    pub error_message: cxx_qt_lib::QString,
    pub detail_name: cxx_qt_lib::QString,
    pub detail_summary: cxx_qt_lib::QString,
    pub detail_description: cxx_qt_lib::QString,
    pub detail_icon_url: cxx_qt_lib::QString,
    pub detail_developer: cxx_qt_lib::QString,
    pub detail_license: cxx_qt_lib::QString,
    pub detail_is_installed: bool,
    /// Real registry progress of the most recent install/uninstall: `0` idle,
    /// negative while indeterminate, `0..=1` while running, `1` when done.
    pub install_progress: f64,
    pub detail_screenshots_json: cxx_qt_lib::QString,
    pub detail_permissions_json: cxx_qt_lib::QString,
    pub detail_urls_json: cxx_qt_lib::QString,
    pub detail_developer_apps_json: cxx_qt_lib::QString,
    pub detail_similar_apps_json: cxx_qt_lib::QString,
    pub detail_ratings_json: cxx_qt_lib::QString,
    pub detail_reviews_json: cxx_qt_lib::QString,
    /// Handle of the in-flight install/uninstall job; `cancelOperation`
    /// cancels it through `Transactions::cancel(job_id)`.
    job: Option<Arc<JobControl>>,
    /// Detail-load request generation; completions of superseded loads are
    /// dropped.
    generation: u64,
    watch: Option<tokio_util::sync::CancellationToken>,
    watched_app_id: String,
    /// Contents and title of the inspected `.flatpakref` awaiting the user's
    /// confirmation.
    ref_key_sha256: cxx_qt_lib::QString,
    pending_ref: Option<(Vec<u8>, String)>,
    /// Inspection request generation; superseded inspections are dropped.
    ref_generation: u64,
}

impl Drop for StoreControllerRust {
    fn drop(&mut self) {
        if let Some(watch) = &self.watch {
            watch.cancel();
        }
    }
}

impl qobject::StoreController {
    /// Reset every detail-view property to its empty state before a fresh load,
    /// so the UI never shows leftovers from the previously viewed app.
    fn clear_detail_state(mut self: std::pin::Pin<&mut Self>) {
        let empty = cxx_qt_lib::QString::default;
        self.as_mut().set_detail_name(empty());
        self.as_mut().set_detail_summary(empty());
        self.as_mut().set_detail_description(empty());
        self.as_mut().set_detail_icon_url(empty());
        self.as_mut().set_detail_developer(empty());
        self.as_mut().set_detail_license(empty());
        self.as_mut().set_detail_is_installed(false);
        self.as_mut().set_error_message(empty());
        self.as_mut()
            .set_detail_screenshots_json(cxx_qt_lib::QString::from("[]"));
        self.as_mut()
            .set_detail_permissions_json(cxx_qt_lib::QString::from("{}"));
        self.as_mut()
            .set_detail_urls_json(cxx_qt_lib::QString::from("{}"));
        self.as_mut()
            .set_detail_developer_apps_json(cxx_qt_lib::QString::from("[]"));
        self.as_mut()
            .set_detail_similar_apps_json(cxx_qt_lib::QString::from("[]"));
        self.as_mut()
            .set_detail_ratings_json(cxx_qt_lib::QString::from("{}"));
        self.as_mut()
            .set_detail_reviews_json(cxx_qt_lib::QString::from("[]"));
    }

    /// Follow registry changes so `detail_is_installed` tracks the watched app.
    fn ensure_watch(mut self: std::pin::Pin<&mut Self>) {
        if self.watch.is_some() {
            return;
        }
        let cancel = tokio_util::sync::CancellationToken::new();
        self.as_mut().rust_mut().watch = Some(cancel.clone());
        let mut events = crate::services::services().registry.subscribe();
        let thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = events.recv() => {
                        if matches!(event, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            break;
                        }
                        if thread.queue(|mut object| {
                            let installed = crate::services::services()
                                .registry
                                .is_installed(&object.watched_app_id);
                            object.as_mut().set_detail_is_installed(installed);
                        }).is_err() { break; }
                    }
                }
            }
        });
    }

    pub fn load_app_details(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        self.as_mut().clear_detail_state();
        self.as_mut().rust_mut().watched_app_id = app_id.to_string();
        self.as_mut().ensure_watch();
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();

        klog!(
            "StoreController::load_app_details: fetching \"{}\"",
            app_id_str
        );
        crate::runtime::runtime().spawn(async move {
            let (details_res, installed) = tokio::join!(
                detail_data::load(&app_id_str),
                crate::flatpak::ui::is_installed_cached(&app_id_str),
            );

            // ODRS ratings & reviews run in a separate task so slow ODRS responses
            // never hold up the main detail view.
            if details_res.is_ok() {
                let odrs_app_id = app_id_str.clone();
                let odrs_qt_thread = qt_thread.clone();
                crate::runtime::runtime().spawn(async move {
                    let (ratings, reviews) = detail_data::load_reviews(&odrs_app_id).await;
                    let _ = odrs_qt_thread.queue(move |mut qobject| {
                        if qobject.generation != generation {
                            return;
                        }
                        qobject
                            .as_mut()
                            .set_detail_ratings_json(util::json_qstring(&ratings, "{}"));
                        qobject
                            .as_mut()
                            .set_detail_reviews_json(util::json_qstring(&reviews, "[]"));
                        qobject.as_mut().reviews_loaded();
                    });
                });
            }

            let _ = qt_thread.queue(move |mut qobject| {
                if qobject.generation != generation {
                    return;
                }
                match details_res {
                    Ok(details) => apply_details(qobject.as_mut(), details, installed),
                    Err(e) => {
                        kerr!("StoreController::load_app_details: {}", e);
                        qobject
                            .as_mut()
                            .set_error_message(cxx_qt_lib::QString::from(&e));
                        qobject.as_mut().reviews_loaded();
                    }
                }
                qobject.as_mut().set_loading(false);
                qobject.as_mut().details_loaded();
            });
        });
    }

    pub fn cancel_operation(mut self: std::pin::Pin<&mut Self>) {
        if let Some(job) = self.as_mut().rust_mut().job.take() {
            job.cancel();
        }
    }

    /// Start `op` as the controller's current job, reporting real progress
    /// through `install_progress` and completion through `installFinished`.
    fn start_job(
        mut self: std::pin::Pin<&mut Self>,
        op: Operation,
        title: String,
        on_success: impl FnOnce(std::pin::Pin<&mut Self>) + Send + 'static,
    ) {
        self.as_mut().set_install_progress(-1.0);
        self.as_mut()
            .set_error_message(cxx_qt_lib::QString::default());
        let control = JobControl::new();
        self.as_mut().rust_mut().job = Some(control.clone());
        let qt_thread = self.qt_thread();
        util::run_job(
            qt_thread,
            op,
            title,
            control.clone(),
            |q, p, _| q.set_install_progress(p),
            move |mut q, outcome| {
                let current = q.job.as_ref().is_some_and(|job| Arc::ptr_eq(job, &control));
                if current {
                    q.as_mut().rust_mut().job = None;
                }
                match outcome {
                    JobOutcome::Done => {
                        klog!("StoreController: job finished");
                        q.as_mut().set_install_progress(1.0);
                        on_success(q.as_mut());
                        q.as_mut().install_finished(true);
                    }
                    JobOutcome::Cancelled => {
                        klog!("StoreController: job cancelled");
                        q.as_mut().set_install_progress(0.0);
                        q.as_mut().install_finished(false);
                    }
                    JobOutcome::Failed(e) => {
                        kerr!("StoreController: job failed: {}", e);
                        q.as_mut().set_install_progress(0.0);
                        q.as_mut().set_error_message(cxx_qt_lib::QString::from(&e));
                        q.as_mut().install_finished(false);
                    }
                }
            },
        );
    }

    pub fn install_app(self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        let app_id = app_id.to_string();
        klog!("StoreController::install_app: installing \"{}\"", app_id);
        let (remote, scope) = crate::services::services().install_target(&app_id);
        let watched = app_id.clone();
        self.start_job(
            Operation::Install {
                remote,
                id: app_id,
                scope,
            },
            String::new(),
            move |q| {
                if q.watched_app_id == watched {
                    q.set_detail_is_installed(true);
                }
            },
        );
    }

    pub fn uninstall_app(self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        let app_id = app_id.to_string();
        klog!(
            "StoreController::uninstall_app: uninstalling \"{}\"",
            app_id
        );
        let watched = app_id.clone();
        self.start_job(
            Operation::Uninstall {
                id: app_id,
                scope: None,
            },
            String::new(),
            move |q| {
                if q.watched_app_id == watched {
                    q.set_detail_is_installed(false);
                }
            },
        );
    }

    pub fn inspect_ref_file(mut self: std::pin::Pin<&mut Self>, path: cxx_qt_lib::QString) {
        let source = path.to_string();
        klog!("StoreController::inspect_ref_file: \"{}\"", source);
        self.as_mut().rust_mut().pending_ref = None;
        self.as_mut().rust_mut().ref_generation += 1;
        let generation = self.ref_generation;
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let result = read_ref_file(&source).await;
            let _ = qt_thread.queue(move |mut q| {
                if q.ref_generation != generation {
                    return;
                }
                let qs = |s: &str| cxx_qt_lib::QString::from(s);
                match result {
                    Ok(data) => {
                        let validated =
                            kiosque_core::security::ref_file(&data).expect("validated reference");
                        q.as_mut().set_ref_key_sha256(qs(&validated.key_sha256));
                        let mut summary = crate::flatpak::summarize_keyfile(&data);
                        summary.url = validated.origin;
                        q.as_mut().rust_mut().pending_ref = Some((data, summary.title.clone()));
                        q.as_mut().ref_file_inspected(
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
                        q.as_mut()
                            .ref_file_inspected(false, qs(""), qs(""), qs(""), false, qs(&e));
                    }
                }
            });
        });
    }

    pub fn confirm_ref_file_install(mut self: std::pin::Pin<&mut Self>) {
        let Some((data, title)) = self.as_mut().rust_mut().pending_ref.take() else {
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
            |_| {},
        );
    }

    pub fn discard_ref_file(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().rust_mut().pending_ref = None;
        // Drop any inspection still in flight as well.
        self.as_mut().rust_mut().ref_generation += 1;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_review(
        self: std::pin::Pin<&mut Self>,
        app_id: cxx_qt_lib::QString,
        rating: i32,
        summary: cxx_qt_lib::QString,
        description: cxx_qt_lib::QString,
        version: cxx_qt_lib::QString,
        distro: cxx_qt_lib::QString,
        locale: cxx_qt_lib::QString,
        is_anonymous: bool,
    ) {
        let qt_thread = self.qt_thread();
        let future = detail_data::submit_review(
            app_id.to_string(),
            rating,
            summary.to_string(),
            description.to_string(),
            version.to_string(),
            distro.to_string(),
            locale.to_string(),
            is_anonymous,
        );
        crate::runtime::runtime().spawn(async move {
            let (ok, error) = match future.await {
                Ok(()) => (true, String::new()),
                Err(e) => (false, e),
            };
            let _ = qt_thread.queue(move |mut qobject| {
                qobject
                    .as_mut()
                    .review_submitted(ok, cxx_qt_lib::QString::from(&error));
            });
        });
    }

    fn run_review_action(self: std::pin::Pin<&mut Self>, action: ReviewAction, review_id: i64) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let (ok, error) = match detail_data::review_action(action, review_id).await {
                Ok(()) => (true, String::new()),
                Err(e) => (false, e),
            };
            let _ = qt_thread.queue(move |mut qobject| {
                qobject
                    .as_mut()
                    .review_action_finished(ok, cxx_qt_lib::QString::from(&error));
            });
        });
    }

    pub fn upvote_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        self.run_review_action(ReviewAction::Upvote, review_id);
    }

    pub fn downvote_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        self.run_review_action(ReviewAction::Downvote, review_id);
    }

    pub fn dismiss_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        self.run_review_action(ReviewAction::Dismiss, review_id);
    }

    pub fn remove_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        self.run_review_action(ReviewAction::Remove, review_id);
    }
}

/// Marshal loaded details into the StoreController's legacy detail properties
/// on the Qt thread.
fn apply_details(
    mut qobject: std::pin::Pin<&mut qobject::StoreController>,
    details: DetailData,
    installed: bool,
) {
    klog!(
        "StoreController::load_app_details: OK — \"{}\" (installed={})",
        details.name,
        installed
    );
    let q = |s: &str| cxx_qt_lib::QString::from(s);
    qobject.as_mut().set_detail_name(q(&details.name));
    qobject.as_mut().set_detail_summary(q(&details.summary));
    qobject
        .as_mut()
        .set_detail_description(q(&details.description));
    qobject
        .as_mut()
        .set_detail_icon_url(q(&crate::image_cache::image_url(&details.icon)));
    qobject.as_mut().set_detail_developer(q(&details.developer));
    qobject.as_mut().set_detail_license(q(&details.license));
    qobject.as_mut().set_detail_is_installed(installed);
    qobject.as_mut().set_error_message(q(""));

    let screenshots: Vec<_> = details
        .screenshots
        .iter()
        .map(|url| crate::image_cache::image_url(url))
        .collect();
    qobject
        .as_mut()
        .set_detail_screenshots_json(util::json_qstring(&screenshots, "[]"));
    qobject
        .as_mut()
        .set_detail_permissions_json(util::json_qstring(&details.permissions, "{}"));
    qobject
        .as_mut()
        .set_detail_urls_json(util::json_qstring(&details.urls, "{}"));
    qobject
        .as_mut()
        .set_detail_developer_apps_json(util::json_qstring(
            &detail_data::apps_to_json(&details.developer_apps),
            "[]",
        ));
    qobject
        .as_mut()
        .set_detail_similar_apps_json(util::json_qstring(
            &detail_data::apps_to_json(&details.similar_apps),
            "[]",
        ));
}

/// Largest `.flatpakref`/`.flatpakrepo` accepted; real ones are a few KiB
/// (mostly the GPG key).
const MAX_REF_FILE_SIZE: usize = 1024 * 1024;

/// Read a `.flatpakref` from a local path, `file://` URL or `http(s)://` URL
/// and check that it looks like one.
pub async fn read_ref_file(source: &str) -> Result<Vec<u8>, String> {
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
    let text = std::str::from_utf8(&data).map_err(|_| format!("{source} is not a text file"))?;
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
