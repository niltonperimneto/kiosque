//! Per-page app details and actions.
pub use object::AppDetails;
use qtbridge::qobject;
#[qobject(NoQmlElement)]
mod object {
    //! Per-page app detail object.
    //!
    //! Each `AppDetails` instance holds
    //! the state of one app id, so several detail pages can coexist. Metadata is
    //! loaded from Flathub/ODRS when `appId` is set (superseded loads are
    //! dropped by request generation); installed/busy/progress/statusText are
    //! derived solely from the shared [`Registry`](crate::registry::Registry)
    //! through a subscription that stops when the QObject is destroyed, so two
    //! instances showing the same app always agree.

    use qtbridge::QmlObject;

    use std::sync::Arc;

    use crate::bridge::detail_data::{self, DetailData, ReviewAction};
    use crate::bridge::util::{self, JobControl, JobOutcome, QVariantList, QVariantMap};
    use crate::flathub::odrs::{OdrsRatings, OdrsReview};
    use crate::flathub::types::FlathubApp;
    use crate::flatpak::Operation;

    pub struct AppDetails {
        pub(super) app_id: String,
        pub(super) loading: bool,
        pub(super) error_message: String,
        pub(super) name: String,
        pub(super) summary: String,
        pub(super) description: String,
        pub(super) icon_url: String,
        pub(super) developer: String,
        pub(super) license: String,
        pub(super) installed: bool,
        pub(super) busy: bool,
        pub(super) progress: f64,
        pub(super) status_text: String,
        pub(super) screenshots: serde_json::Value,
        pub(super) urls: QVariantMap,
        pub(super) permissions_known: bool,
        pub(super) permissions: QVariantMap,
        pub(super) ratings: QVariantMap,
        pub(super) developer_apps: QVariantList,
        pub(super) similar_apps: QVariantList,
        pub(super) reviews: QVariantList,
        pub(super) reviews_loading: bool,
        pub(super) latest_release: QVariantMap,
        /// `{name, title, scope, url}` of the source repository; empty when
        /// the app is only known from the Flathub web API.
        pub(super) origin: QVariantMap,
        /// The app's catalog entry when it was loaded, to reload on changes.
        pub(super) catalog_entry: Option<kiosque_core::catalog::CatalogApp>,
        /// Load request generation: completions of superseded loads are dropped.
        pub(super) generation: u64,
        /// The job this object started (for `operationFinished` and early cancel).
        pub(super) job: Option<Arc<JobControl>>,
        /// Stops the registry subscription when the QObject is destroyed.
        pub(super) watch: Option<tokio_util::sync::CancellationToken>,
    }

    impl Default for AppDetails {
        fn default() -> Self {
            Self {
                app_id: Default::default(),
                loading: false,
                error_message: Default::default(),
                name: Default::default(),
                summary: Default::default(),
                description: Default::default(),
                icon_url: Default::default(),
                developer: Default::default(),
                license: Default::default(),
                installed: false,
                busy: false,
                progress: 0.0,
                status_text: Default::default(),
                screenshots: serde_json::json!([]),
                urls: serde_json::json!({}),
                permissions_known: false,
                permissions: serde_json::json!({}),
                ratings: serde_json::json!({}),
                developer_apps: serde_json::json!([]),
                similar_apps: serde_json::json!([]),
                reviews: serde_json::json!([]),
                reviews_loading: false,
                latest_release: serde_json::json!({}),
                origin: serde_json::json!({}),
                catalog_entry: None,
                generation: 0,
                job: None,
                watch: None,
            }
        }
    }

    impl Drop for AppDetails {
        fn drop(&mut self) {
            if let Some(watch) = &self.watch {
                watch.cancel();
            }
        }
    }

    /// Assign a field and emit its change signal if the value differs.
    macro_rules! update {
        ($obj:expr, $field:ident, $signal:ident, $value:expr) => {{
            let value = $value;
            if $obj.$field != value {
                $obj.$field = value;
                $obj.$signal();
            }
        }};
    }

    /// Assign a field and always emit its change signal (for container types).
    macro_rules! replace {
        ($obj:expr, $field:ident, $signal:ident, $value:expr) => {{
            $obj.$field = $value;
            $obj.$signal();
        }};
    }

    impl AppDetails {
        #[qslot]
        fn dispose(&mut self) {
            self.generation += 1;
            if let Some(w) = self.watch.take() {
                w.cancel();
            }
        }
        pub fn set_app_id(&mut self, app_id: String) {
            if self.app_id == app_id {
                return;
            }
            self.app_id = app_id;
            self.app_id_changed();
            // A job started for the previous app keeps running, but its
            // completion no longer belongs to this page.
            self.job = None;
            self.ensure_watch();
            self.sync_registry();
            self.start_load();
        }

        #[qslot(qml_name = "reload")]
        pub fn reload(&mut self) {
            self.start_load();
        }

        /// Subscribe to registry events once per object.
        fn ensure_watch(&mut self) {
            if self.watch.is_some() {
                return;
            }
            let cancel = tokio_util::sync::CancellationToken::new();
            self.watch = Some(cancel.clone());
            let services = crate::services::services();
            let mut events = services.registry.subscribe();
            let mut catalog_changes = kiosque_core::catalog::changes();
            let thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                use tokio::sync::broadcast::error::RecvError;
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        event = events.recv() => {
                            if matches!(event, Err(RecvError::Closed)) {
                                break;
                            }
                            thread.invoke_mut(|object| object.sync_registry());
                        }
                        change = catalog_changes.recv() => {
                            if matches!(change, Err(RecvError::Closed)) {
                                break;
                            }
                            // Reload when this app's catalog entry changed.
                            thread.invoke_mut(|object| {
                                let current = kiosque_core::catalog::catalog().get(&object.app_id);
                                if !object.loading && current != object.catalog_entry {
                                    object.start_load();
                                }
                            });
                        }
                    }
                }
            });
            if !services.registry.installed_loaded() {
                // The reload publishes `Event::Installed`, which syncs us.
                crate::runtime::runtime().spawn(async move {
                    let services = crate::services::services();
                    if let Err(e) = services.registry.reload_installed(&*services.backend).await {
                        kerr!("AppDetails: loading installed refs failed: {}", e);
                    }
                });
            }
        }

        /// Recompute installed/busy/progress/statusText from the registry.
        fn sync_registry(&mut self) {
            let app_id = self.app_id.to_string();
            let state = if app_id.is_empty() {
                Default::default()
            } else {
                crate::services::services().registry.state(&app_id)
            };
            update!(
                self,
                installed,
                installed_changed,
                state.installed.is_some()
            );
            let (busy, progress, status) = match &state.job {
                Some(job) => (true, job.progress.min(1.0), job.status.as_str()),
                None => (false, 0.0, ""),
            };
            update!(self, busy, busy_changed, busy);
            update!(self, progress, progress_changed, progress);
            update!(self, status_text, status_text_changed, String::from(status));
        }

        fn clear_details(&mut self) {
            let empty = String::new;
            update!(self, error_message, error_message_changed, empty());
            update!(self, name, name_changed, empty());
            update!(self, summary, summary_changed, empty());
            update!(self, description, description_changed, empty());
            update!(self, icon_url, icon_url_changed, empty());
            update!(self, developer, developer_changed, empty());
            update!(self, license, license_changed, empty());
            replace!(
                self,
                screenshots,
                screenshots_changed,
                serde_json::json!([])
            );
            replace!(self, urls, urls_changed, serde_json::json!({}));
            replace!(
                self,
                latest_release,
                latest_release_changed,
                serde_json::json!({})
            );
            replace!(self, origin, origin_changed, serde_json::json!({}));
            update!(self, permissions_known, permissions_known_changed, false);
            replace!(
                self,
                permissions,
                permissions_changed,
                serde_json::json!({})
            );
            replace!(self, ratings, ratings_changed, serde_json::json!({}));
            replace!(
                self,
                developer_apps,
                developer_apps_changed,
                serde_json::json!([])
            );
            replace!(
                self,
                similar_apps,
                similar_apps_changed,
                serde_json::json!([])
            );
            replace!(self, reviews, reviews_changed, serde_json::json!([]));
        }

        fn start_load(&mut self) {
            self.generation += 1;
            let generation = self.generation;
            self.clear_details();
            let app_id = self.app_id.to_string();
            self.catalog_entry = kiosque_core::catalog::catalog().get(&app_id);
            if app_id.is_empty() {
                update!(self, loading, loading_changed, false);
                update!(self, reviews_loading, reviews_loading_changed, false);
                return;
            }
            update!(self, loading, loading_changed, true);
            update!(self, reviews_loading, reviews_loading_changed, true);
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            klog!("AppDetails: loading \"{}\"", app_id);
            crate::runtime::runtime().spawn(async move {
                let result = detail_data::load(&app_id).await;
                let reviews_wanted = result.is_ok();
                let details_thread = qt_thread.clone();
                details_thread.invoke_mut(move |object| {
                    if object.generation != generation {
                        return;
                    }
                    match result {
                        Ok(details) => object.apply_details(details),
                        Err(e) => {
                            kerr!("AppDetails: loading details failed: {}", e);
                            update!(
                                object,
                                error_message,
                                error_message_changed,
                                String::from(&e)
                            );
                        }
                    }
                    update!(object, loading, loading_changed, false);
                    object.loaded();
                });

                if !reviews_wanted {
                    qt_thread.invoke_mut(move |object| {
                        if object.generation != generation {
                            return;
                        }
                        update!(object, reviews_loading, reviews_loading_changed, false);
                        object.reviews_loaded();
                    });
                    return;
                }
                let (ratings, reviews) = detail_data::load_reviews(&app_id).await;
                qt_thread.invoke_mut(move |object| {
                    if object.generation != generation {
                        return;
                    }
                    object.apply_reviews(&ratings, &reviews);
                    update!(object, reviews_loading, reviews_loading_changed, false);
                    object.reviews_loaded();
                });
            });
        }

        fn apply_details(&mut self, details: DetailData) {
            let q = |s: &str| String::from(s);
            update!(self, name, name_changed, q(&details.name));
            update!(self, summary, summary_changed, q(&details.summary));
            update!(
                self,
                description,
                description_changed,
                q(&details.description)
            );
            update!(self, icon_url, icon_url_changed, q(&details.icon));
            update!(self, developer, developer_changed, q(&details.developer));
            update!(self, license, license_changed, q(&details.license));
            replace!(
                self,
                screenshots,
                screenshots_changed,
                util::string_list(details.screenshots.iter().map(String::as_str))
            );
            replace!(
                self,
                urls,
                urls_changed,
                util::json_to_qvariant_map(&details.urls)
            );
            update!(
                self,
                permissions_known,
                permissions_known_changed,
                details.permissions.is_object()
            );
            replace!(
                self,
                permissions,
                permissions_changed,
                util::json_to_qvariant_map(&details.permissions)
            );
            replace!(
                self,
                latest_release,
                latest_release_changed,
                util::json_to_qvariant_map(&details.latest_release)
            );
            replace!(
                self,
                origin,
                origin_changed,
                util::json_to_qvariant_map(&details.origin)
            );
            replace!(
                self,
                developer_apps,
                developer_apps_changed,
                apps_list(&details.developer_apps)
            );
            replace!(
                self,
                similar_apps,
                similar_apps_changed,
                apps_list(&details.similar_apps)
            );
        }

        fn apply_reviews(&mut self, ratings: &OdrsRatings, reviews: &[OdrsReview]) {
            let ratings = serde_json::to_value(ratings).unwrap_or_default();
            replace!(
                self,
                ratings,
                ratings_changed,
                util::json_to_qvariant_map(&ratings)
            );
            replace!(self, reviews, reviews_changed, reviews_list(reviews));
        }

        fn start_job(&mut self, op: Operation, title: String) {
            let control = JobControl::new();
            self.job = Some(control.clone());
            util::run_job(
                self.get_thread_handle(),
                op,
                title,
                control.clone(),
                // Progress is published from the registry subscription only.
                |_, _, _| {},
                move |object, outcome| {
                    if !object
                        .job
                        .as_ref()
                        .is_some_and(|job| Arc::ptr_eq(job, &control))
                    {
                        return;
                    }
                    object.job = None;
                    let (success, error) = match outcome {
                        JobOutcome::Done => (true, String::new()),
                        JobOutcome::Cancelled => (false, "operation cancelled".to_string()),
                        JobOutcome::Failed(e) => (false, e),
                    };
                    object.sync_registry();
                    object.operation_finished(success, String::from(&error));
                },
            );
        }

        #[qslot(qml_name = "install")]
        pub fn install(&mut self) {
            let app_id = self.app_id.to_string();
            if app_id.is_empty() {
                return;
            }
            klog!("AppDetails: installing \"{}\"", app_id);
            // Install from the repository the page showed.
            let origin = self
                .origin
                .get("name")
                .and_then(|name| name.as_str())
                .unwrap_or_default()
                .to_string();
            let (remote, scope) = crate::services::services().install_target_from(&app_id, &origin);
            let title = self.name.to_string();
            self.start_job(
                Operation::Install {
                    remote,
                    id: app_id,
                    scope,
                },
                title,
            );
        }

        #[qslot(qml_name = "uninstall")]
        pub fn uninstall(&mut self) {
            let app_id = self.app_id.to_string();
            if app_id.is_empty() {
                return;
            }
            klog!("AppDetails: uninstalling \"{}\"", app_id);
            let title = self.name.to_string();
            self.start_job(
                Operation::Uninstall {
                    id: app_id,
                    scope: None,
                },
                title,
            );
        }

        #[qslot(qml_name = "launch")]
        pub fn launch(&self) {
            let app_id = self.app_id.to_string();
            if app_id.is_empty() || !self.installed {
                return;
            }
            crate::runtime::runtime().spawn(async move {
                if let Err(e) = crate::view::ops::launch(&app_id).await {
                    kerr!("AppDetails::launch: {}", e);
                }
            });
        }

        /// Cancel the active job for this app (whoever started it) through
        /// `Transactions::cancel(job_id)`.
        #[qslot(qml_name = "cancel")]
        pub fn cancel(&mut self) {
            let app_id = self.app_id.to_string();
            let registry_job = crate::services::services()
                .registry
                .state(&app_id)
                .job
                .map(|job| job.id);
            match (&self.job, registry_job) {
                (Some(own), _) => own.cancel(),
                (None, Some(id)) => util::cancel_job(id),
                (None, None) => {}
            }
        }

        #[qslot(qml_name = "submitReview")]
        pub fn submit_review(
            &mut self,
            rating: i32,
            summary: String,
            description: String,
            is_anonymous: bool,
        ) {
            let future = detail_data::submit_review(
                self.app_id.to_string(),
                rating,
                summary.to_string(),
                description.to_string(),
                String::new(),
                String::new(),
                String::new(),
                is_anonymous,
            );
            self.finish_review_action(future);
        }

        fn finish_review_action(
            &mut self,
            future: impl std::future::Future<Output = Result<(), String>> + Send + 'static,
        ) {
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                let (ok, error) = match future.await {
                    Ok(()) => (true, String::new()),
                    Err(e) => (false, e),
                };
                qt_thread.invoke_mut(move |object| {
                    object.review_action_finished(ok, String::from(&error));
                });
            });
        }

        #[qslot(qml_name = "upvoteReview")]
        pub fn upvote_review(&mut self, review_id: i64) {
            self.finish_review_action(detail_data::review_action(ReviewAction::Upvote, review_id));
        }

        #[qslot(qml_name = "downvoteReview")]
        pub fn downvote_review(&mut self, review_id: i64) {
            self.finish_review_action(detail_data::review_action(
                ReviewAction::Downvote,
                review_id,
            ));
        }

        #[qslot(qml_name = "dismissReview")]
        pub fn dismiss_review(&mut self, review_id: i64) {
            self.finish_review_action(detail_data::review_action(ReviewAction::Dismiss, review_id));
        }

        #[qslot(qml_name = "removeReview")]
        pub fn remove_review(&mut self, review_id: i64) {
            self.finish_review_action(detail_data::review_action(ReviewAction::Remove, review_id));
        }
    }

    /// `[{appId, name, summary, iconUrl}]`, the keys the related-app rows
    /// (`HorizontalAppList`) read.
    fn apps_list(apps: &[FlathubApp]) -> QVariantList {
        apps.iter()
            .map(|app| {
                serde_json::json!({
                    "appId": app.app_id,
                    "name": app.name,
                    "summary": app.summary.clone().unwrap_or_default(),
                    "iconUrl": app.icon.clone().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>()
            .into()
    }
    fn reviews_list(reviews: &[OdrsReview]) -> QVariantList {
        serde_json::Value::Array(reviews.iter().map(|r| serde_json::json!({
  "userName":r.user_display.as_deref().unwrap_or_default(), "dateCreated":r.date_created,
  "dateStr":jiff::Timestamp::from_second(r.date_created as i64).map(|d|d.strftime("%Y-%m-%d").to_string()).unwrap_or_default(),
  "rating":r.rating,"summary":r.summary,"description":r.description,"version":r.version.as_deref().unwrap_or_default(),
  "reviewId":r.review_id,"userHash":r.user_hash.as_deref().unwrap_or_default(),"karmaUp":r.karma_up,"karmaDown":r.karma_down
 })).collect())
    }

    impl AppDetails {
        qproperty!(
            "appId",
            Read = get_app_id,
            Write = set_app_id,
            Notify = app_id_changed
        );
        fn get_app_id(&self) -> String {
            self.app_id.clone()
        }
        #[qsignal(qml_name = "appIdChanged")]
        pub fn app_id_changed(&mut self);
        qproperty!("loading", Read = get_loading, Notify = loading_changed);
        fn get_loading(&self) -> bool {
            self.loading
        }
        #[qsignal(qml_name = "loadingChanged")]
        pub fn loading_changed(&mut self);
        qproperty!(
            "errorMessage",
            Read = get_error_message,
            Notify = error_message_changed
        );
        fn get_error_message(&self) -> String {
            self.error_message.clone()
        }
        #[qsignal(qml_name = "errorMessageChanged")]
        pub fn error_message_changed(&mut self);
        qproperty!("name", Read = get_name, Notify = name_changed);
        fn get_name(&self) -> String {
            self.name.clone()
        }
        #[qsignal(qml_name = "nameChanged")]
        pub fn name_changed(&mut self);
        qproperty!("summary", Read = get_summary, Notify = summary_changed);
        fn get_summary(&self) -> String {
            self.summary.clone()
        }
        #[qsignal(qml_name = "summaryChanged")]
        pub fn summary_changed(&mut self);
        qproperty!(
            "description",
            Read = get_description,
            Notify = description_changed
        );
        fn get_description(&self) -> String {
            self.description.clone()
        }
        #[qsignal(qml_name = "descriptionChanged")]
        pub fn description_changed(&mut self);
        qproperty!("iconUrl", Read = get_icon_url, Notify = icon_url_changed);
        fn get_icon_url(&self) -> String {
            self.icon_url.clone()
        }
        #[qsignal(qml_name = "iconUrlChanged")]
        pub fn icon_url_changed(&mut self);
        qproperty!(
            "developer",
            Read = get_developer,
            Notify = developer_changed
        );
        fn get_developer(&self) -> String {
            self.developer.clone()
        }
        #[qsignal(qml_name = "developerChanged")]
        pub fn developer_changed(&mut self);
        qproperty!("license", Read = get_license, Notify = license_changed);
        fn get_license(&self) -> String {
            self.license.clone()
        }
        #[qsignal(qml_name = "licenseChanged")]
        pub fn license_changed(&mut self);
        qproperty!(
            "installed",
            Read = get_installed,
            Notify = installed_changed
        );
        fn get_installed(&self) -> bool {
            self.installed
        }
        #[qsignal(qml_name = "installedChanged")]
        pub fn installed_changed(&mut self);
        qproperty!("busy", Read = get_busy, Notify = busy_changed);
        fn get_busy(&self) -> bool {
            self.busy
        }
        #[qsignal(qml_name = "busyChanged")]
        pub fn busy_changed(&mut self);
        qproperty!("progress", Read = get_progress, Notify = progress_changed);
        fn get_progress(&self) -> f64 {
            self.progress
        }
        #[qsignal(qml_name = "progressChanged")]
        pub fn progress_changed(&mut self);
        qproperty!(
            "statusText",
            Read = get_status_text,
            Notify = status_text_changed
        );
        fn get_status_text(&self) -> String {
            self.status_text.clone()
        }
        #[qsignal(qml_name = "statusTextChanged")]
        pub fn status_text_changed(&mut self);
        qproperty!(
            "screenshots",
            Read = get_screenshots,
            Notify = screenshots_changed
        );
        fn get_screenshots(&self) -> serde_json::Value {
            self.screenshots.clone()
        }
        #[qsignal(qml_name = "screenshotsChanged")]
        pub fn screenshots_changed(&mut self);
        qproperty!("urls", Read = get_urls, Notify = urls_changed);
        fn get_urls(&self) -> serde_json::Value {
            self.urls.clone()
        }
        #[qsignal(qml_name = "urlsChanged")]
        pub fn urls_changed(&mut self);
        qproperty!(
            "permissionsKnown",
            Read = get_permissions_known,
            Notify = permissions_known_changed
        );
        fn get_permissions_known(&self) -> bool {
            self.permissions_known
        }
        #[qsignal(qml_name = "permissionsKnownChanged")]
        pub fn permissions_known_changed(&mut self);
        qproperty!(
            "permissions",
            Read = get_permissions,
            Notify = permissions_changed
        );
        fn get_permissions(&self) -> serde_json::Value {
            self.permissions.clone()
        }
        #[qsignal(qml_name = "permissionsChanged")]
        pub fn permissions_changed(&mut self);
        qproperty!("ratings", Read = get_ratings, Notify = ratings_changed);
        fn get_ratings(&self) -> serde_json::Value {
            self.ratings.clone()
        }
        #[qsignal(qml_name = "ratingsChanged")]
        pub fn ratings_changed(&mut self);
        qproperty!(
            "developerApps",
            Read = get_developer_apps,
            Notify = developer_apps_changed
        );
        fn get_developer_apps(&self) -> serde_json::Value {
            self.developer_apps.clone()
        }
        #[qsignal(qml_name = "developerAppsChanged")]
        pub fn developer_apps_changed(&mut self);
        qproperty!(
            "similarApps",
            Read = get_similar_apps,
            Notify = similar_apps_changed
        );
        fn get_similar_apps(&self) -> serde_json::Value {
            self.similar_apps.clone()
        }
        #[qsignal(qml_name = "similarAppsChanged")]
        pub fn similar_apps_changed(&mut self);
        qproperty!("reviews", Read = get_reviews, Notify = reviews_changed);
        fn get_reviews(&self) -> serde_json::Value {
            self.reviews.clone()
        }
        #[qsignal(qml_name = "reviewsChanged")]
        pub fn reviews_changed(&mut self);
        qproperty!(
            "reviewsLoading",
            Read = get_reviews_loading,
            Notify = reviews_loading_changed
        );
        fn get_reviews_loading(&self) -> bool {
            self.reviews_loading
        }
        #[qsignal(qml_name = "reviewsLoadingChanged")]
        pub fn reviews_loading_changed(&mut self);
        qproperty!(
            "latestRelease",
            Read = get_latest_release,
            Notify = latest_release_changed
        );
        fn get_latest_release(&self) -> serde_json::Value {
            self.latest_release.clone()
        }
        #[qsignal(qml_name = "latestReleaseChanged")]
        pub fn latest_release_changed(&mut self);
        qproperty!("origin", Read = get_origin, Notify = origin_changed);
        fn get_origin(&self) -> serde_json::Value {
            self.origin.clone()
        }
        #[qsignal(qml_name = "originChanged")]
        pub fn origin_changed(&mut self);
        #[qsignal(qml_name = "loaded")]
        pub fn loaded(&mut self);
        #[qsignal(qml_name = "reviewsLoaded")]
        pub fn reviews_loaded(&mut self);
        #[qsignal(qml_name = "operationFinished")]
        pub fn operation_finished(&mut self, success: bool, error: String);
        #[qsignal(qml_name = "reviewActionFinished")]
        pub fn review_action_finished(&mut self, success: bool, error: String);
    }
}
notify_setters!(AppDetails {
set_loading: loading: bool => loading_changed,
set_error_message: error_message: String => error_message_changed,
set_name: name: String => name_changed,
set_summary: summary: String => summary_changed,
set_description: description: String => description_changed,
set_icon_url: icon_url: String => icon_url_changed,
set_developer: developer: String => developer_changed,
set_license: license: String => license_changed,
set_installed: installed: bool => installed_changed,
set_busy: busy: bool => busy_changed,
set_progress: progress: f64 => progress_changed,
set_status_text: status_text: String => status_text_changed,
set_screenshots: screenshots: serde_json::Value => screenshots_changed,
set_urls: urls: serde_json::Value => urls_changed,
set_permissions_known: permissions_known: bool => permissions_known_changed,
set_permissions: permissions: serde_json::Value => permissions_changed,
set_ratings: ratings: serde_json::Value => ratings_changed,
set_developer_apps: developer_apps: serde_json::Value => developer_apps_changed,
set_similar_apps: similar_apps: serde_json::Value => similar_apps_changed,
set_reviews: reviews: serde_json::Value => reviews_changed,
set_reviews_loading: reviews_loading: bool => reviews_loading_changed,
set_latest_release: latest_release: serde_json::Value => latest_release_changed
});
qml_element!(AppDetails, "AppDetails");
