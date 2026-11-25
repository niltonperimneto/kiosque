//! Feature-parity adapter for the current storefront.
pub use object::{StoreController, read_ref_file, read_repo_file};
use qtbridge::qobject;
#[qobject(NoQmlElement)]
mod object {
    use qtbridge::QmlObject;
    use std::sync::Arc;

    use crate::bridge::detail_data::{self, DetailData, ReviewAction};
    use crate::bridge::util::{self, JobControl, JobOutcome};
    use crate::flatpak::{Operation, Scope};

    #[derive(Default)]
    pub struct StoreController {
        pub(super) loading: bool,
        pub(super) error_message: String,
        pub(super) detail_name: String,
        pub(super) detail_summary: String,
        pub(super) detail_description: String,
        pub(super) detail_icon_url: String,
        pub(super) detail_developer: String,
        pub(super) detail_license: String,
        pub(super) detail_is_installed: bool,
        /// Real registry progress of the most recent install/uninstall: `0` idle,
        /// negative while indeterminate, `0..=1` while running, `1` when done.
        pub(super) install_progress: f64,
        /// Status line of the current job, e.g. "Step 1 of 2: Installing …".
        pub(super) install_status: String,
        pub(super) detail_screenshots_json: String,
        pub(super) detail_permissions_json: String,
        pub(super) detail_urls_json: String,
        pub(super) detail_developer_apps_json: String,
        pub(super) detail_similar_apps_json: String,
        pub(super) detail_ratings_json: String,
        pub(super) detail_reviews_json: String,
        /// Handle of the in-flight install/uninstall job; `cancelOperation`
        /// cancels it through `Transactions::cancel(job_id)`.
        pub(super) job: Option<Arc<JobControl>>,
        /// Detail-load request generation; completions of superseded loads are
        /// dropped.
        pub(super) generation: u64,
        pub(super) watch: Option<tokio_util::sync::CancellationToken>,
        pub(super) watched_app_id: String,
        /// Contents and title of the inspected `.flatpakref` awaiting the user's
        /// confirmation.
        pub(super) ref_key_sha256: String,
        pub(super) pending_ref: Option<(Vec<u8>, String)>,
        /// Inspection request generation; superseded inspections are dropped.
        pub(super) ref_generation: u64,
    }

    impl Drop for StoreController {
        fn drop(&mut self) {
            if let Some(watch) = &self.watch {
                watch.cancel();
            }
        }
    }

    impl StoreController {
        /// Reset every detail-view property to its empty state before a fresh load,
        /// so the UI never shows leftovers from the previously viewed app.
        fn clear_detail_state(&mut self) {
            let empty = String::new;
            self.set_detail_name(empty());
            self.set_detail_summary(empty());
            self.set_detail_description(empty());
            self.set_detail_icon_url(empty());
            self.set_detail_developer(empty());
            self.set_detail_license(empty());
            self.set_detail_is_installed(false);
            self.set_error_message(empty());
            self.set_detail_screenshots_json(String::from("[]"));
            self.set_detail_permissions_json(String::from("{}"));
            self.set_detail_urls_json(String::from("{}"));
            self.set_detail_developer_apps_json(String::from("[]"));
            self.set_detail_similar_apps_json(String::from("[]"));
            self.set_detail_ratings_json(String::from("{}"));
            self.set_detail_reviews_json(String::from("[]"));
        }

        /// Follow registry changes so `detail_is_installed` tracks the watched app.
        fn ensure_watch(&mut self) {
            if self.watch.is_some() {
                return;
            }
            let cancel = tokio_util::sync::CancellationToken::new();
            self.watch = Some(cancel.clone());
            let mut events = crate::services::services().registry.subscribe();
            let thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = events.recv() => {
                        if matches!(event, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            break;
                        }
                        thread.invoke_mut(|object| {
                            let installed = crate::services::services()
                                .registry
                                .is_installed(&object.watched_app_id);
                            object.set_detail_is_installed(installed);
                        });
                    }
                }
            }
        });
        }

        #[qslot(qml_name = "loadAppDetails")]
        pub fn load_app_details(&mut self, app_id: String) {
            self.set_loading(true);
            self.clear_detail_state();
            self.watched_app_id = app_id.to_string();
            self.ensure_watch();
            self.generation += 1;
            let generation = self.generation;
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
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
                        odrs_qt_thread.invoke_mut(move |qobject| {
                            if qobject.generation != generation {
                                return;
                            }
                            qobject.set_detail_ratings_json(util::json_qstring(&ratings, "{}"));
                            qobject.set_detail_reviews_json(util::json_qstring(&reviews, "[]"));
                            qobject.reviews_loaded();
                        });
                    });
                }

                qt_thread.invoke_mut(move |qobject| {
                    if qobject.generation != generation {
                        return;
                    }
                    match details_res {
                        Ok(details) => apply_details(qobject, details, installed),
                        Err(e) => {
                            kerr!("StoreController::load_app_details: {}", e);
                            qobject.set_error_message(String::from(&e));
                            qobject.reviews_loaded();
                        }
                    }
                    qobject.set_loading(false);
                    qobject.details_loaded();
                });
            });
        }

        #[qslot(qml_name = "cancelOperation")]
        pub fn cancel_operation(&mut self) {
            if let Some(job) = self.job.take() {
                job.cancel();
            }
        }

        /// Start `op` as the controller's current job, reporting real progress
        /// through `install_progress` and completion through `installFinished`.
        fn start_job(
            &mut self,
            op: Operation,
            title: String,
            on_success: impl FnOnce(&mut Self) + Send + 'static,
        ) {
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
                            on_success(q);
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

        #[qslot(qml_name = "installApp")]
        pub fn install_app(&mut self, app_id: String) {
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

        #[qslot(qml_name = "uninstallApp")]
        pub fn uninstall_app(&mut self, app_id: String) {
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
                |_| {},
            );
        }

        #[qslot(qml_name = "discardRefFile")]
        pub fn discard_ref_file(&mut self) {
            self.pending_ref = None;
            // Drop any inspection still in flight as well.
            self.ref_generation += 1;
        }

        #[allow(clippy::too_many_arguments)]
        #[qslot(qml_name = "submitReview")]
        pub fn submit_review(
            &mut self,
            app_id: String,
            rating: i32,
            summary: String,
            description: String,
            version: String,
            distro: String,
            locale: String,
            is_anonymous: bool,
        ) {
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
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
                qt_thread.invoke_mut(move |qobject| {
                    qobject.review_submitted(ok, String::from(&error));
                });
            });
        }

        fn run_review_action(&mut self, action: ReviewAction, review_id: i64) {
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                let (ok, error) = match detail_data::review_action(action, review_id).await {
                    Ok(()) => (true, String::new()),
                    Err(e) => (false, e),
                };
                qt_thread.invoke_mut(move |qobject| {
                    qobject.review_action_finished(ok, String::from(&error));
                });
            });
        }

        #[qslot(qml_name = "upvoteReview")]
        pub fn upvote_review(&mut self, review_id: i64) {
            self.run_review_action(ReviewAction::Upvote, review_id);
        }

        #[qslot(qml_name = "downvoteReview")]
        pub fn downvote_review(&mut self, review_id: i64) {
            self.run_review_action(ReviewAction::Downvote, review_id);
        }

        #[qslot(qml_name = "dismissReview")]
        pub fn dismiss_review(&mut self, review_id: i64) {
            self.run_review_action(ReviewAction::Dismiss, review_id);
        }

        #[qslot(qml_name = "removeReview")]
        pub fn remove_review(&mut self, review_id: i64) {
            self.run_review_action(ReviewAction::Remove, review_id);
        }
    }

    /// Marshal loaded details into the StoreController's legacy detail properties
    /// on the Qt thread.
    fn apply_details(qobject: &mut StoreController, details: DetailData, installed: bool) {
        klog!(
            "StoreController::load_app_details: OK — \"{}\" (installed={})",
            details.name,
            installed
        );
        let q = |s: &str| String::from(s);
        qobject.set_detail_name(q(&details.name));
        qobject.set_detail_summary(q(&details.summary));
        qobject.set_detail_description(q(&details.description));
        qobject.set_detail_icon_url(q(&crate::image_cache::image_url(&details.icon)));
        qobject.set_detail_developer(q(&details.developer));
        qobject.set_detail_license(q(&details.license));
        qobject.set_detail_is_installed(installed);
        qobject.set_error_message(q(""));

        let screenshots: Vec<_> = details
            .screenshots
            .iter()
            .map(|url| crate::image_cache::image_url(url))
            .collect();
        qobject.set_detail_screenshots_json(util::json_qstring(&screenshots, "[]"));
        qobject.set_detail_permissions_json(util::json_qstring(&details.permissions, "{}"));
        qobject.set_detail_urls_json(util::json_qstring(&details.urls, "{}"));
        qobject.set_detail_developer_apps_json(util::json_qstring(
            &detail_data::apps_to_json(&details.developer_apps),
            "[]",
        ));
        qobject.set_detail_similar_apps_json(util::json_qstring(
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
        qproperty!("loading", Read = get_loading, Notify = loading_changed);
        fn get_loading(&self) -> bool {
            self.loading
        }
        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);
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
            "detail_name",
            Read = get_detail_name,
            Notify = detail_name_changed
        );
        fn get_detail_name(&self) -> String {
            self.detail_name.clone()
        }
        #[qsignal(qml_name = "detail_nameChanged")]
        pub fn detail_name_changed(&mut self);
        qproperty!(
            "detail_summary",
            Read = get_detail_summary,
            Notify = detail_summary_changed
        );
        fn get_detail_summary(&self) -> String {
            self.detail_summary.clone()
        }
        #[qsignal(qml_name = "detail_summaryChanged")]
        pub fn detail_summary_changed(&mut self);
        qproperty!(
            "detail_description",
            Read = get_detail_description,
            Notify = detail_description_changed
        );
        fn get_detail_description(&self) -> String {
            self.detail_description.clone()
        }
        #[qsignal(qml_name = "detail_descriptionChanged")]
        pub fn detail_description_changed(&mut self);
        qproperty!(
            "detail_icon_url",
            Read = get_detail_icon_url,
            Notify = detail_icon_url_changed
        );
        fn get_detail_icon_url(&self) -> String {
            self.detail_icon_url.clone()
        }
        #[qsignal(qml_name = "detail_icon_urlChanged")]
        pub fn detail_icon_url_changed(&mut self);
        qproperty!(
            "detail_developer",
            Read = get_detail_developer,
            Notify = detail_developer_changed
        );
        fn get_detail_developer(&self) -> String {
            self.detail_developer.clone()
        }
        #[qsignal(qml_name = "detail_developerChanged")]
        pub fn detail_developer_changed(&mut self);
        qproperty!(
            "detail_license",
            Read = get_detail_license,
            Notify = detail_license_changed
        );
        fn get_detail_license(&self) -> String {
            self.detail_license.clone()
        }
        #[qsignal(qml_name = "detail_licenseChanged")]
        pub fn detail_license_changed(&mut self);
        qproperty!(
            "detail_is_installed",
            Read = get_detail_is_installed,
            Notify = detail_is_installed_changed
        );
        fn get_detail_is_installed(&self) -> bool {
            self.detail_is_installed
        }
        #[qsignal(qml_name = "detail_is_installedChanged")]
        pub fn detail_is_installed_changed(&mut self);
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
        qproperty!(
            "detail_screenshots_json",
            Read = get_detail_screenshots_json,
            Notify = detail_screenshots_json_changed
        );
        fn get_detail_screenshots_json(&self) -> String {
            self.detail_screenshots_json.clone()
        }
        #[qsignal(qml_name = "detail_screenshots_jsonChanged")]
        pub fn detail_screenshots_json_changed(&mut self);
        qproperty!(
            "detail_permissions_json",
            Read = get_detail_permissions_json,
            Notify = detail_permissions_json_changed
        );
        fn get_detail_permissions_json(&self) -> String {
            self.detail_permissions_json.clone()
        }
        #[qsignal(qml_name = "detail_permissions_jsonChanged")]
        pub fn detail_permissions_json_changed(&mut self);
        qproperty!(
            "detail_urls_json",
            Read = get_detail_urls_json,
            Notify = detail_urls_json_changed
        );
        fn get_detail_urls_json(&self) -> String {
            self.detail_urls_json.clone()
        }
        #[qsignal(qml_name = "detail_urls_jsonChanged")]
        pub fn detail_urls_json_changed(&mut self);
        qproperty!(
            "detail_developer_apps_json",
            Read = get_detail_developer_apps_json,
            Notify = detail_developer_apps_json_changed
        );
        fn get_detail_developer_apps_json(&self) -> String {
            self.detail_developer_apps_json.clone()
        }
        #[qsignal(qml_name = "detail_developer_apps_jsonChanged")]
        pub fn detail_developer_apps_json_changed(&mut self);
        qproperty!(
            "detail_similar_apps_json",
            Read = get_detail_similar_apps_json,
            Notify = detail_similar_apps_json_changed
        );
        fn get_detail_similar_apps_json(&self) -> String {
            self.detail_similar_apps_json.clone()
        }
        #[qsignal(qml_name = "detail_similar_apps_jsonChanged")]
        pub fn detail_similar_apps_json_changed(&mut self);
        qproperty!(
            "detail_ratings_json",
            Read = get_detail_ratings_json,
            Notify = detail_ratings_json_changed
        );
        fn get_detail_ratings_json(&self) -> String {
            self.detail_ratings_json.clone()
        }
        #[qsignal(qml_name = "detail_ratings_jsonChanged")]
        pub fn detail_ratings_json_changed(&mut self);
        qproperty!(
            "detail_reviews_json",
            Read = get_detail_reviews_json,
            Notify = detail_reviews_json_changed
        );
        fn get_detail_reviews_json(&self) -> String {
            self.detail_reviews_json.clone()
        }
        #[qsignal(qml_name = "detail_reviews_jsonChanged")]
        pub fn detail_reviews_json_changed(&mut self);
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
        #[qsignal(qml_name = "detailsLoaded")]
        pub fn details_loaded(&mut self);
        #[qsignal(qml_name = "installFinished")]
        pub fn install_finished(&mut self, success: bool);
        #[qsignal(qml_name = "reviewSubmitted")]
        pub fn review_submitted(&mut self, success: bool, error: String);
        #[qsignal(qml_name = "reviewActionFinished")]
        pub fn review_action_finished(&mut self, success: bool, error: String);
        #[qsignal(qml_name = "reviewsLoaded")]
        pub fn reviews_loaded(&mut self);
    }
}
notify_setters!(StoreController {
set_ref_key_sha256: ref_key_sha256: String => ref_key_sha256_changed,
set_loading: loading: bool => loading_changed,
set_error_message: error_message: String => error_message_changed,
set_detail_name: detail_name: String => detail_name_changed,
set_detail_summary: detail_summary: String => detail_summary_changed,
set_detail_description: detail_description: String => detail_description_changed,
set_detail_icon_url: detail_icon_url: String => detail_icon_url_changed,
set_detail_developer: detail_developer: String => detail_developer_changed,
set_detail_license: detail_license: String => detail_license_changed,
set_detail_is_installed: detail_is_installed: bool => detail_is_installed_changed,
set_install_progress: install_progress: f64 => install_progress_changed,
set_install_status: install_status: String => install_status_changed,
set_detail_screenshots_json: detail_screenshots_json: String => detail_screenshots_json_changed,
set_detail_permissions_json: detail_permissions_json: String => detail_permissions_json_changed,
set_detail_urls_json: detail_urls_json: String => detail_urls_json_changed,
set_detail_developer_apps_json: detail_developer_apps_json: String => detail_developer_apps_json_changed,
set_detail_similar_apps_json: detail_similar_apps_json: String => detail_similar_apps_json_changed,
set_detail_ratings_json: detail_ratings_json: String => detail_ratings_json_changed,
set_detail_reviews_json: detail_reviews_json: String => detail_reviews_json_changed
});
qml_element!(StoreController, "StoreController", singleton);
