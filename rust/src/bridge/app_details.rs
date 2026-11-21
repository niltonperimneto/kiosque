//! Per-page app detail object.
//!
//! Unlike the `StoreController` singleton, every `AppDetails` instance holds
//! the state of one app id, so several detail pages can coexist. Metadata is
//! loaded from Flathub/ODRS when `appId` is set (superseded loads are
//! dropped by request generation); installed/busy/progress/statusText are
//! derived solely from the shared [`Registry`](crate::registry::Registry)
//! through a subscription that stops when the QObject is destroyed, so two
//! instances showing the same app always agree.

use cxx_qt::{CxxQtType, Threading};
use std::pin::Pin;
use std::sync::Arc;

use crate::bridge::detail_data::{self, DetailData, ReviewAction};
use crate::bridge::util::{self, JobControl, JobOutcome, QVariantList, QVariantMap};
use crate::flathub::odrs::{OdrsRatings, OdrsReview};
use crate::flathub::types::FlathubApp;
use crate::flatpak::Operation;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;

        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, app_id, READ, WRITE = set_app_id, NOTIFY)]
        #[qproperty(bool, loading, READ, NOTIFY)]
        #[qproperty(QString, error_message, READ, NOTIFY)]
        #[qproperty(QString, name, READ, NOTIFY)]
        #[qproperty(QString, summary, READ, NOTIFY)]
        #[qproperty(QString, description, READ, NOTIFY)]
        #[qproperty(QString, icon_url, READ, NOTIFY)]
        #[qproperty(QString, developer, READ, NOTIFY)]
        #[qproperty(QString, license, READ, NOTIFY)]
        #[qproperty(bool, installed, READ, NOTIFY)]
        #[qproperty(bool, busy, READ, NOTIFY)]
        #[qproperty(f64, progress, READ, NOTIFY)]
        #[qproperty(QString, status_text, READ, NOTIFY)]
        #[qproperty(QStringList, screenshots, READ, NOTIFY)]
        #[qproperty(QMap_QString_QVariant, urls, READ, NOTIFY)]
        #[qproperty(bool, permissions_known, READ, NOTIFY)]
        #[qproperty(QMap_QString_QVariant, permissions, READ, NOTIFY)]
        #[qproperty(QMap_QString_QVariant, ratings, READ, NOTIFY)]
        #[qproperty(QList_QVariant, developer_apps, READ, NOTIFY)]
        #[qproperty(QList_QVariant, similar_apps, READ, NOTIFY)]
        #[qproperty(QList_QVariant, reviews, READ, NOTIFY)]
        #[qproperty(bool, reviews_loading, READ, NOTIFY)]
        type AppDetails = super::AppDetailsRust;

        fn set_app_id(self: Pin<&mut AppDetails>, app_id: QString);

        #[qinvokable]
        fn install(self: Pin<&mut AppDetails>);

        #[qinvokable]
        fn uninstall(self: Pin<&mut AppDetails>);

        #[qinvokable]
        fn cancel(self: Pin<&mut AppDetails>);

        #[qinvokable]
        fn reload(self: Pin<&mut AppDetails>);

        #[qinvokable]
        fn submit_review(
            self: Pin<&mut AppDetails>,
            rating: i32,
            summary: QString,
            description: QString,
            is_anonymous: bool,
        );

        #[qinvokable]
        fn upvote_review(self: Pin<&mut AppDetails>, review_id: i64);

        #[qinvokable]
        fn downvote_review(self: Pin<&mut AppDetails>, review_id: i64);

        #[qinvokable]
        fn dismiss_review(self: Pin<&mut AppDetails>, review_id: i64);

        #[qinvokable]
        fn remove_review(self: Pin<&mut AppDetails>, review_id: i64);

        #[qsignal]
        fn loaded(self: Pin<&mut AppDetails>);

        #[qsignal]
        fn reviews_loaded(self: Pin<&mut AppDetails>);

        #[qsignal]
        fn operation_finished(self: Pin<&mut AppDetails>, success: bool, error: QString);

        #[qsignal]
        fn review_action_finished(self: Pin<&mut AppDetails>, success: bool, error: QString);
    }

    impl cxx_qt::Threading for AppDetails {}
}

pub struct AppDetailsRust {
    app_id: cxx_qt_lib::QString,
    loading: bool,
    error_message: cxx_qt_lib::QString,
    name: cxx_qt_lib::QString,
    summary: cxx_qt_lib::QString,
    description: cxx_qt_lib::QString,
    icon_url: cxx_qt_lib::QString,
    developer: cxx_qt_lib::QString,
    license: cxx_qt_lib::QString,
    installed: bool,
    busy: bool,
    progress: f64,
    status_text: cxx_qt_lib::QString,
    screenshots: cxx_qt_lib::QStringList,
    urls: QVariantMap,
    permissions_known: bool,
    permissions: QVariantMap,
    ratings: QVariantMap,
    developer_apps: QVariantList,
    similar_apps: QVariantList,
    reviews: QVariantList,
    reviews_loading: bool,
    /// Load request generation: completions of superseded loads are dropped.
    generation: u64,
    /// The job this object started (for `operationFinished` and early cancel).
    job: Option<Arc<JobControl>>,
    /// Stops the registry subscription when the QObject is destroyed.
    watch: Option<tokio_util::sync::CancellationToken>,
}

impl Default for AppDetailsRust {
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
            screenshots: Default::default(),
            urls: Default::default(),
            permissions_known: false,
            permissions: Default::default(),
            ratings: Default::default(),
            developer_apps: Default::default(),
            similar_apps: Default::default(),
            reviews: Default::default(),
            reviews_loading: false,
            generation: 0,
            job: None,
            watch: None,
        }
    }
}

impl Drop for AppDetailsRust {
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
            $obj.as_mut().rust_mut().$field = value;
            $obj.as_mut().$signal();
        }
    }};
}

/// Assign a field and always emit its change signal (for container types).
macro_rules! replace {
    ($obj:expr, $field:ident, $signal:ident, $value:expr) => {{
        $obj.as_mut().rust_mut().$field = $value;
        $obj.as_mut().$signal();
    }};
}

impl qobject::AppDetails {
    pub fn set_app_id(mut self: Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        if self.app_id == app_id {
            return;
        }
        self.as_mut().rust_mut().app_id = app_id;
        self.as_mut().app_id_changed();
        // A job started for the previous app keeps running, but its
        // completion no longer belongs to this page.
        self.as_mut().rust_mut().job = None;
        self.as_mut().ensure_watch();
        self.as_mut().sync_registry();
        self.as_mut().start_load();
    }

    pub fn reload(self: Pin<&mut Self>) {
        self.start_load();
    }

    /// Subscribe to registry events once per object.
    fn ensure_watch(mut self: Pin<&mut Self>) {
        if self.watch.is_some() {
            return;
        }
        let cancel = tokio_util::sync::CancellationToken::new();
        self.as_mut().rust_mut().watch = Some(cancel.clone());
        let services = crate::services::services();
        let mut events = services.registry.subscribe();
        let thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    event = events.recv() => {
                        if matches!(event, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            break;
                        }
                        if thread.queue(|object| object.sync_registry()).is_err() {
                            break;
                        }
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
    fn sync_registry(mut self: Pin<&mut Self>) {
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
        update!(
            self,
            status_text,
            status_text_changed,
            cxx_qt_lib::QString::from(status)
        );
    }

    fn clear_details(mut self: Pin<&mut Self>) {
        let empty = cxx_qt_lib::QString::default;
        update!(self, error_message, error_message_changed, empty());
        update!(self, name, name_changed, empty());
        update!(self, summary, summary_changed, empty());
        update!(self, description, description_changed, empty());
        update!(self, icon_url, icon_url_changed, empty());
        update!(self, developer, developer_changed, empty());
        update!(self, license, license_changed, empty());
        replace!(self, screenshots, screenshots_changed, Default::default());
        replace!(self, urls, urls_changed, Default::default());
        update!(self, permissions_known, permissions_known_changed, false);
        replace!(self, permissions, permissions_changed, Default::default());
        replace!(self, ratings, ratings_changed, Default::default());
        replace!(
            self,
            developer_apps,
            developer_apps_changed,
            Default::default()
        );
        replace!(self, similar_apps, similar_apps_changed, Default::default());
        replace!(self, reviews, reviews_changed, Default::default());
    }

    fn start_load(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        self.as_mut().clear_details();
        let app_id = self.app_id.to_string();
        if app_id.is_empty() {
            update!(self, loading, loading_changed, false);
            update!(self, reviews_loading, reviews_loading_changed, false);
            return;
        }
        update!(self, loading, loading_changed, true);
        update!(self, reviews_loading, reviews_loading_changed, true);
        let qt_thread = self.qt_thread();
        klog!("AppDetails: loading \"{}\"", app_id);
        crate::runtime::runtime().spawn(async move {
            let result = detail_data::load(&app_id).await;
            let reviews_wanted = result.is_ok();
            let details_thread = qt_thread.clone();
            let _ = details_thread.queue(move |mut object| {
                if object.generation != generation {
                    return;
                }
                match result {
                    Ok(details) => object.as_mut().apply_details(details),
                    Err(e) => {
                        kerr!("AppDetails: loading details failed: {}", e);
                        update!(
                            object,
                            error_message,
                            error_message_changed,
                            cxx_qt_lib::QString::from(&e)
                        );
                    }
                }
                update!(object, loading, loading_changed, false);
                object.as_mut().loaded();
            });

            if !reviews_wanted {
                let _ = qt_thread.queue(move |mut object| {
                    if object.generation != generation {
                        return;
                    }
                    update!(object, reviews_loading, reviews_loading_changed, false);
                    object.as_mut().reviews_loaded();
                });
                return;
            }
            let (ratings, reviews) = detail_data::load_reviews(&app_id).await;
            let _ = qt_thread.queue(move |mut object| {
                if object.generation != generation {
                    return;
                }
                object.as_mut().apply_reviews(&ratings, &reviews);
                update!(object, reviews_loading, reviews_loading_changed, false);
                object.as_mut().reviews_loaded();
            });
        });
    }

    fn apply_details(mut self: Pin<&mut Self>, details: DetailData) {
        let q = |s: &str| cxx_qt_lib::QString::from(s);
        update!(self, name, name_changed, q(&details.name));
        update!(self, summary, summary_changed, q(&details.summary));
        update!(
            self,
            description,
            description_changed,
            q(&details.description)
        );
        update!(
            self,
            icon_url,
            icon_url_changed,
            q(&crate::image_cache::image_url(&details.icon))
        );
        update!(self, developer, developer_changed, q(&details.developer));
        update!(self, license, license_changed, q(&details.license));
        let screenshots: Vec<String> = details
            .screenshots
            .iter()
            .map(|url| crate::image_cache::image_url(url))
            .collect();
        replace!(
            self,
            screenshots,
            screenshots_changed,
            util::string_list(screenshots.iter().map(String::as_str))
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

    fn apply_reviews(mut self: Pin<&mut Self>, ratings: &OdrsRatings, reviews: &[OdrsReview]) {
        let ratings = serde_json::to_value(ratings).unwrap_or_default();
        replace!(
            self,
            ratings,
            ratings_changed,
            util::json_to_qvariant_map(&ratings)
        );
        replace!(self, reviews, reviews_changed, reviews_list(reviews));
    }

    fn start_job(mut self: Pin<&mut Self>, op: Operation, title: String) {
        let control = JobControl::new();
        self.as_mut().rust_mut().job = Some(control.clone());
        util::run_job(
            self.qt_thread(),
            op,
            title,
            control.clone(),
            // Progress is published from the registry subscription only.
            |_, _, _| {},
            move |mut object, outcome| {
                if !object
                    .job
                    .as_ref()
                    .is_some_and(|job| Arc::ptr_eq(job, &control))
                {
                    return;
                }
                object.as_mut().rust_mut().job = None;
                let (success, error) = match outcome {
                    JobOutcome::Done => (true, String::new()),
                    JobOutcome::Cancelled => (false, "operation cancelled".to_string()),
                    JobOutcome::Failed(e) => (false, e),
                };
                object.as_mut().sync_registry();
                object
                    .as_mut()
                    .operation_finished(success, cxx_qt_lib::QString::from(&error));
            },
        );
    }

    pub fn install(self: Pin<&mut Self>) {
        let app_id = self.app_id.to_string();
        if app_id.is_empty() {
            return;
        }
        klog!("AppDetails: installing \"{}\"", app_id);
        let (remote, scope) = crate::services::services().install_target(&app_id);
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

    pub fn uninstall(self: Pin<&mut Self>) {
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

    /// Cancel the active job for this app (whoever started it) through
    /// `Transactions::cancel(job_id)`.
    pub fn cancel(self: Pin<&mut Self>) {
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

    pub fn submit_review(
        self: Pin<&mut Self>,
        rating: i32,
        summary: cxx_qt_lib::QString,
        description: cxx_qt_lib::QString,
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
        self: Pin<&mut Self>,
        future: impl std::future::Future<Output = Result<(), String>> + Send + 'static,
    ) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let (ok, error) = match future.await {
                Ok(()) => (true, String::new()),
                Err(e) => (false, e),
            };
            let _ = qt_thread.queue(move |mut object| {
                object
                    .as_mut()
                    .review_action_finished(ok, cxx_qt_lib::QString::from(&error));
            });
        });
    }

    pub fn upvote_review(self: Pin<&mut Self>, review_id: i64) {
        self.finish_review_action(detail_data::review_action(ReviewAction::Upvote, review_id));
    }

    pub fn downvote_review(self: Pin<&mut Self>, review_id: i64) {
        self.finish_review_action(detail_data::review_action(
            ReviewAction::Downvote,
            review_id,
        ));
    }

    pub fn dismiss_review(self: Pin<&mut Self>, review_id: i64) {
        self.finish_review_action(detail_data::review_action(ReviewAction::Dismiss, review_id));
    }

    pub fn remove_review(self: Pin<&mut Self>, review_id: i64) {
        self.finish_review_action(detail_data::review_action(ReviewAction::Remove, review_id));
    }
}

fn qvariant_str(s: &str) -> cxx_qt_lib::QVariant {
    cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(s))
}

/// `[{appId, name, summary, iconUrl}]` for the related-app rows.
fn apps_list(apps: &[FlathubApp]) -> QVariantList {
    apps.iter()
        .map(|app| {
            let mut map = QVariantMap::default();
            map.insert("appId".into(), qvariant_str(&app.app_id));
            map.insert("name".into(), qvariant_str(&app.name));
            map.insert(
                "summary".into(),
                qvariant_str(app.summary.as_deref().unwrap_or_default()),
            );
            map.insert(
                "iconUrl".into(),
                qvariant_str(&crate::image_cache::image_url(
                    app.icon.as_deref().unwrap_or_default(),
                )),
            );
            cxx_qt_lib::QVariant::from(&map)
        })
        .collect()
}

/// Reviews with the property names `ReviewCard.qml` takes. `dateStr` is an
/// ISO date; `dateCreated` keeps the raw Unix timestamp (seconds) for
/// locale-aware formatting in QML.
fn reviews_list(reviews: &[OdrsReview]) -> QVariantList {
    reviews
        .iter()
        .map(|review| {
            let date = jiff::Timestamp::from_second(review.date_created as i64)
                .map(|d| d.strftime("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let mut map = QVariantMap::default();
            map.insert(
                "userName".into(),
                qvariant_str(review.user_display.as_deref().unwrap_or_default()),
            );
            map.insert("dateStr".into(), qvariant_str(&date));
            map.insert(
                "dateCreated".into(),
                cxx_qt_lib::QVariant::from(&review.date_created),
            );
            map.insert("rating".into(), cxx_qt_lib::QVariant::from(&review.rating));
            map.insert("summary".into(), qvariant_str(&review.summary));
            map.insert("description".into(), qvariant_str(&review.description));
            map.insert(
                "version".into(),
                qvariant_str(review.version.as_deref().unwrap_or_default()),
            );
            map.insert(
                "reviewId".into(),
                cxx_qt_lib::QVariant::from(&review.review_id),
            );
            map.insert(
                "userHash".into(),
                qvariant_str(review.user_hash.as_deref().unwrap_or_default()),
            );
            map.insert(
                "karmaUp".into(),
                cxx_qt_lib::QVariant::from(&review.karma_up),
            );
            map.insert(
                "karmaDown".into(),
                cxx_qt_lib::QVariant::from(&review.karma_down),
            );
            cxx_qt_lib::QVariant::from(&map)
        })
        .collect()
}
