//! `AppDetails`: one app's detail page state. Each `AppDetailPage` creates
//! its own instance, so several pages never share or clobber state.
//!
//! Install state and progress come from the shared registry (filtered by
//! `appId`), so they match the Installed page and the daemon. Responses that
//! arrive after `appId` changed, after `dispose()` or after the page was
//! destroyed are dropped.

use std::cell::RefCell;
use std::rc::Rc;

use qtbridge::qobject;
use tokio_util::sync::CancellationToken;

use super::app_list_model::AppListModel;
use crate::view::details::{self, AppDetailsData, ReviewsData};
use crate::view::ops;

pub use object::AppDetails;

#[qobject(Base = QParserStatus, NoQmlElement)]
mod object {
    use qtbridge::{QParserStatus, QmlObject};

    use super::*;

    pub struct AppDetails {
        pub(super) app_id: String,
        pub(super) complete: bool,
        pub(super) loading: bool,
        pub(super) error_message: String,
        pub(super) name: String,
        pub(super) summary: String,
        pub(super) description: String,
        pub(super) icon_url: String,
        pub(super) developer: String,
        pub(super) license: String,
        pub(super) is_installed: bool,
        pub(super) busy: bool,
        pub(super) install_progress: f64,
        pub(super) screenshots: Vec<String>,
        pub(super) permissions: serde_json::Value,
        pub(super) links: serde_json::Value,
        pub(super) ratings: serde_json::Value,
        pub(super) reviews: serde_json::Value,
        pub(super) reviews_loading: bool,
        pub(super) developer_apps: Rc<RefCell<AppListModel>>,
        pub(super) similar_apps: Rc<RefCell<AppListModel>>,
        pub(super) generation: u64,
        pub(super) watch: Option<CancellationToken>,
    }

    impl Default for AppDetails {
        fn default() -> Self {
            Self {
                app_id: String::new(),
                complete: false,
                loading: false,
                error_message: String::new(),
                name: String::new(),
                summary: String::new(),
                description: String::new(),
                icon_url: String::new(),
                developer: String::new(),
                license: String::new(),
                is_installed: false,
                busy: false,
                install_progress: 0.0,
                screenshots: vec![],
                permissions: serde_json::json!({}),
                links: serde_json::json!({}),
                ratings: ReviewsData::default().ratings_value(),
                reviews: serde_json::json!([]),
                reviews_loading: false,
                developer_apps: Default::default(),
                similar_apps: Default::default(),
                generation: 0,
                watch: None,
            }
        }
    }

    impl Drop for AppDetails {
        fn drop(&mut self) {
            if let Some(w) = &self.watch {
                w.cancel();
            }
        }
    }

    impl QParserStatus for AppDetails {
        fn class_begin(&mut self) {}
        fn component_complete(&mut self) {
            self.complete = true;
            self.reload();
        }
    }

    impl AppDetails {
        qproperty!("appId", Read = get_app_id, Write = set_app_id, Notify = app_id_changed);
        qproperty!("loading", Read = get_loading, Notify = loading_changed);
        qproperty!("errorMessage", Read = get_error_message, Notify = error_message_changed);
        qproperty!("name", Read = get_name, Notify = details_changed);
        qproperty!("summary", Read = get_summary, Notify = details_changed);
        qproperty!("description", Read = get_description, Notify = details_changed);
        qproperty!("iconUrl", Read = get_icon_url, Notify = details_changed);
        qproperty!("developer", Read = get_developer, Notify = details_changed);
        qproperty!("license", Read = get_license, Notify = details_changed);
        qproperty!("screenshots", Read = get_screenshots, Notify = details_changed);
        qproperty!("permissions", Read = get_permissions, Notify = details_changed);
        qproperty!("links", Read = get_links, Notify = details_changed);
        qproperty!("isInstalled", Read = get_is_installed, Notify = is_installed_changed);
        qproperty!("busy", Read = get_busy, Notify = busy_changed);
        qproperty!("installProgress", Read = get_install_progress, Notify = install_progress_changed);
        qproperty!("ratings", Read = get_ratings, Notify = reviews_changed);
        qproperty!("reviews", Read = get_reviews, Notify = reviews_changed);
        qproperty!("reviewsLoading", Read = get_reviews_loading, Notify = reviews_loading_changed);
        qproperty!("developerApps", Member = developer_apps, Constant);
        qproperty!("similarApps", Member = similar_apps, Constant);

        fn get_app_id(&self) -> String {
            self.app_id.clone()
        }
        fn set_app_id(&mut self, app_id: String) {
            if app_id != self.app_id {
                self.app_id = app_id;
                if self.complete {
                    self.reload();
                }
                self.app_id_changed();
            }
        }
        fn get_loading(&self) -> bool {
            self.loading
        }
        fn get_error_message(&self) -> String {
            self.error_message.clone()
        }
        fn get_name(&self) -> String {
            self.name.clone()
        }
        fn get_summary(&self) -> String {
            self.summary.clone()
        }
        fn get_description(&self) -> String {
            self.description.clone()
        }
        fn get_icon_url(&self) -> String {
            self.icon_url.clone()
        }
        fn get_developer(&self) -> String {
            self.developer.clone()
        }
        fn get_license(&self) -> String {
            self.license.clone()
        }
        fn get_screenshots(&self) -> Vec<String> {
            self.screenshots.clone()
        }
        fn get_permissions(&self) -> serde_json::Value {
            self.permissions.clone()
        }
        fn get_links(&self) -> serde_json::Value {
            self.links.clone()
        }
        fn get_is_installed(&self) -> bool {
            self.is_installed
        }
        fn get_busy(&self) -> bool {
            self.busy
        }
        fn get_install_progress(&self) -> f64 {
            self.install_progress
        }
        fn get_ratings(&self) -> serde_json::Value {
            self.ratings.clone()
        }
        fn get_reviews(&self) -> serde_json::Value {
            self.reviews.clone()
        }
        fn get_reviews_loading(&self) -> bool {
            self.reviews_loading
        }

        #[qsignal]
        pub fn app_id_changed(&mut self);
        #[qsignal]
        pub fn loading_changed(&mut self);
        #[qsignal]
        pub fn error_message_changed(&mut self);
        #[qsignal]
        pub fn details_changed(&mut self);
        #[qsignal]
        pub fn is_installed_changed(&mut self);
        #[qsignal]
        pub fn busy_changed(&mut self);
        #[qsignal]
        pub fn install_progress_changed(&mut self);
        #[qsignal]
        pub fn reviews_changed(&mut self);
        #[qsignal]
        pub fn reviews_loading_changed(&mut self);
        #[qsignal(qml_name = "detailsLoaded")]
        pub fn details_loaded(&mut self);
        #[qsignal(qml_name = "reviewsLoaded")]
        pub fn reviews_loaded(&mut self);
        #[qsignal(qml_name = "installFinished")]
        pub fn install_finished(&mut self, success: bool);
        #[qsignal(qml_name = "reviewSubmitted")]
        pub fn review_submitted(&mut self, success: bool, error: String);
        #[qsignal(qml_name = "reviewActionFinished")]
        pub fn review_action_finished(&mut self, success: bool, error: String);

        /// Load (or reload) everything for `appId`.
        #[qslot]
        fn reload(&mut self) {
            self.generation += 1;
            let generation = self.generation;
            let app_id = self.app_id.clone();
            self.apply_data(AppDetailsData::default());
            self.set_error_message(String::new());
            if app_id.is_empty() {
                self.set_loading(false);
                return;
            }
            self.ensure_watch();
            self.sync_install_state();
            self.set_loading(true);
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let s = crate::services::services();
                let (result, _installed) =
                    tokio::join!(details::load(&s.flathub, &app_id), ops::is_installed(&app_id));
                let ok = result.is_ok();
                handle.invoke_mut(move |d| d.finish_details(generation, result));
                if ok {
                    let reviews = details::load_reviews(&s.odrs, &app_id).await;
                    handle.invoke_mut(move |d| d.finish_reviews(generation, reviews));
                } else {
                    handle.invoke_mut(move |d| d.finish_reviews(generation, ReviewsData::default()));
                }
            });
        }

        /// Reload ratings and reviews only.
        #[qslot(qml_name = "reloadReviews")]
        fn reload_reviews(&mut self) {
            let generation = self.generation;
            let app_id = self.app_id.clone();
            if app_id.is_empty() {
                return;
            }
            self.set_reviews_loading(true);
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let s = crate::services::services();
                s.odrs.invalidate(&app_id).await;
                let reviews = details::load_reviews(&s.odrs, &app_id).await;
                handle.invoke_mut(move |d| d.finish_reviews(generation, reviews));
            });
        }

        #[qslot]
        fn install(&mut self) {
            self.run_operation(true);
        }

        #[qslot]
        fn uninstall(&mut self) {
            self.run_operation(false);
        }

        /// Cancel this app's queued or running transaction.
        #[qslot]
        fn cancel(&mut self) {
            if let Some(job) = ops::active_job(&self.app_id) {
                crate::bridge::spawn(async move { ops::cancel(job.id).await });
            }
        }

        #[qslot]
        fn launch(&mut self) {
            let app_id = self.app_id.clone();
            crate::bridge::spawn(async move {
                if let Err(e) = ops::launch(&app_id).await {
                    kerr!("AppDetails::launch: {}", e);
                }
            });
        }

        /// Submit an ODRS review; version, distribution and locale are
        /// filled in from the installed app and the environment.
        #[qslot(qml_name = "submitReview")]
        fn submit_review(&mut self, rating: i32, summary: String, description: String, anonymous: bool) {
            let app_id = self.app_id.clone();
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let version = ops::installed_version(&app_id)
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "1.0".into());
                let distro = match kiosque_core::config::distro_id() {
                    id if id == "generic" => "Linux".to_string(),
                    id => id,
                };
                let locale = std::env::var("LANG")
                    .ok()
                    .and_then(|l| l.split('.').next().map(str::to_string))
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| "en_US".into());
                let result = crate::services::services()
                    .odrs
                    .submit_review(
                        &app_id,
                        rating,
                        &summary,
                        &description,
                        &version,
                        &distro,
                        &locale,
                        anonymous,
                    )
                    .await
                    .map_err(|e| e.to_string());
                handle.invoke_mut(move |d| {
                    let ok = result.is_ok();
                    if ok {
                        d.reload_reviews();
                    }
                    d.review_submitted(ok, result.err().unwrap_or_default());
                });
            });
        }

        #[qslot(qml_name = "upvoteReview")]
        fn upvote_review(&mut self, review_id: i64) {
            self.review_action(review_id, ReviewAction::Upvote);
        }
        #[qslot(qml_name = "downvoteReview")]
        fn downvote_review(&mut self, review_id: i64) {
            self.review_action(review_id, ReviewAction::Downvote);
        }
        #[qslot(qml_name = "dismissReview")]
        fn dismiss_review(&mut self, review_id: i64) {
            self.review_action(review_id, ReviewAction::Dismiss);
        }
        #[qslot(qml_name = "removeReview")]
        fn remove_review(&mut self, review_id: i64) {
            self.review_action(review_id, ReviewAction::Remove);
        }

        /// Stop following the registry and drop pending responses. Called
        /// from `Component.onDestruction`; `Drop` does the same.
        #[qslot]
        fn dispose(&mut self) {
            self.generation += 1;
            if let Some(w) = self.watch.take() {
                w.cancel();
            }
        }
    }

    #[derive(Clone, Copy)]
    enum ReviewAction {
        Upvote,
        Downvote,
        Dismiss,
        Remove,
    }

    impl AppDetails {
        fn review_action(&mut self, review_id: i64, action: ReviewAction) {
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let odrs = &crate::services::services().odrs;
                let result = match action {
                    ReviewAction::Upvote => odrs.upvote_review(review_id).await,
                    ReviewAction::Downvote => odrs.downvote_review(review_id).await,
                    ReviewAction::Dismiss => odrs.dismiss_review(review_id).await,
                    ReviewAction::Remove => odrs.remove_review(review_id).await,
                }
                .map_err(|e| e.to_string());
                handle.invoke_mut(move |d| {
                    let ok = result.is_ok();
                    if ok {
                        d.reload_reviews();
                    }
                    d.review_action_finished(ok, result.err().unwrap_or_default());
                });
            });
        }

        fn run_operation(&mut self, install: bool) {
            if self.busy || self.app_id.is_empty() {
                return;
            }
            let app_id = self.app_id.clone();
            // Progress itself arrives through the registry watch.
            self.set_busy(true);
            self.set_install_progress(0.01);
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let result = if install {
                    ops::install(&app_id, |_| {}).await
                } else {
                    ops::uninstall(&app_id, |_| {}).await
                };
                handle.invoke_mut(move |d| {
                    if d.app_id != app_id {
                        return;
                    }
                    d.sync_install_state();
                    if let Err(e) = &result {
                        kerr!("AppDetails: {}", e);
                        d.set_error_message(e.clone());
                    }
                    d.install_finished(result.is_ok());
                });
            });
        }

        fn apply_data(&mut self, data: AppDetailsData) {
            self.name = data.name;
            self.summary = data.summary;
            self.description = data.description;
            self.icon_url = data.icon_url;
            self.developer = data.developer;
            self.license = data.license;
            self.screenshots = data.screenshots;
            self.permissions = data.permissions;
            self.links = if data.links.is_null() {
                serde_json::json!({})
            } else {
                data.links
            };
            self.developer_apps.borrow_mut().set_entries(data.developer_apps);
            self.similar_apps.borrow_mut().set_entries(data.similar_apps);
            self.details_changed();
        }

        fn finish_details(&mut self, generation: u64, result: Result<AppDetailsData, String>) {
            if generation != self.generation {
                return;
            }
            match result {
                Ok(data) => {
                    self.apply_data(data);
                    self.set_error_message(String::new());
                }
                Err(e) => {
                    kerr!("AppDetails {}: {}", self.app_id, e);
                    self.set_error_message(e);
                }
            }
            self.sync_install_state();
            self.set_loading(false);
            self.details_loaded();
        }

        fn finish_reviews(&mut self, generation: u64, data: ReviewsData) {
            if generation != self.generation {
                return;
            }
            self.ratings = data.ratings_value();
            self.reviews = data.reviews_value();
            self.reviews_changed();
            self.set_reviews_loading(false);
            self.reviews_loaded();
        }

        /// Installed flag and job progress from the shared registry.
        fn sync_install_state(&mut self) {
            let registry = &crate::services::services().registry;
            let installed = registry.is_installed(&self.app_id);
            let job = ops::active_job(&self.app_id);
            self.set_is_installed(installed);
            self.set_install_progress(job.as_ref().map_or(0.0, |j| j.fraction.clamp(0.01, 0.99)));
            self.set_busy(job.is_some());
        }

        fn ensure_watch(&mut self) {
            if self.watch.is_some() {
                return;
            }
            let cancel = CancellationToken::new();
            self.watch = Some(cancel.clone());
            let mut events = crate::services::services().registry.subscribe();
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                use kiosque_core::registry::Event;
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        event = events.recv() => match event {
                            Ok(Event::Job(_) | Event::Installed)
                            | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                handle.invoke_mut(|d| d.sync_install_state());
                            }
                            Ok(Event::Updates) => {}
                            Err(_) => break,
                        }
                    }
                }
            });
        }
    }
}

notify_setters!(AppDetails {
    set_loading: loading: bool => loading_changed,
    set_error_message: error_message: String => error_message_changed,
    set_is_installed: is_installed: bool => is_installed_changed,
    set_busy: busy: bool => busy_changed,
    set_install_progress: install_progress: f64 => install_progress_changed,
    set_reviews_loading: reviews_loading: bool => reviews_loading_changed,
});

qml_element!(AppDetails, "AppDetails");
