use cxx_qt::{CxxQtType, Threading};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bridge::util;
use crate::flathub::client::FlathubClient;
use crate::flathub::types::{AppDetails, FlathubApp};

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
    pub install_progress: f64,
    pub detail_screenshots_json: cxx_qt_lib::QString,
    pub detail_permissions_json: cxx_qt_lib::QString,
    pub detail_urls_json: cxx_qt_lib::QString,
    pub detail_developer_apps_json: cxx_qt_lib::QString,
    pub detail_similar_apps_json: cxx_qt_lib::QString,
    pub detail_ratings_json: cxx_qt_lib::QString,
    pub detail_reviews_json: cxx_qt_lib::QString,
    /// Cancellation token for the in-flight install/uninstall operation. A fresh
    /// token is created per operation, so `cancelOperation` only affects the most
    /// recently started one rather than a process-wide flag.
    pub cancel_flag: Arc<AtomicBool>,
}

impl qobject::StoreController {
    /// Reset every detail-view property to its empty state before a fresh load,
    /// so the UI never shows leftovers from the previously viewed app.
    fn clear_detail_state(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().set_detail_name(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_summary(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_description(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_icon_url(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_developer(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_license(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_is_installed(false);
        self.as_mut().set_error_message(cxx_qt_lib::QString::from(""));
        self.as_mut().set_detail_screenshots_json(cxx_qt_lib::QString::from("[]"));
        self.as_mut().set_detail_permissions_json(cxx_qt_lib::QString::from("{}"));
        self.as_mut().set_detail_urls_json(cxx_qt_lib::QString::from("{}"));
        self.as_mut().set_detail_developer_apps_json(cxx_qt_lib::QString::from("[]"));
        self.as_mut().set_detail_similar_apps_json(cxx_qt_lib::QString::from("[]"));
        self.as_mut().set_detail_ratings_json(cxx_qt_lib::QString::from("{}"));
        self.as_mut().set_detail_reviews_json(cxx_qt_lib::QString::from("[]"));
    }

    pub fn load_app_details(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        self.as_mut().clear_detail_state();

        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();

        klog!("StoreController::load_app_details: fetching \"{}\"", app_id_str);
        crate::runtime::runtime().spawn(async move {
            let client = FlathubClient::new();

            // Fetch details and installation status concurrently
            let (details_res, installed) = tokio::join!(
                client.fetch_details(&app_id_str),
                crate::flatpak::cli::is_installed_cached(&app_id_str),
            );

            // Fetch the supplementary lists (permissions, developer & similar apps)
            // only when the core details came back; otherwise leave them empty.
            let (permissions_val, developer_apps, similar_apps) = match details_res {
                Ok(ref details) => fetch_supplementary(&client, details, &app_id_str).await,
                Err(_) => (serde_json::json!({}), vec![], vec![]),
            };

            // ODRS ratings & reviews run in a separate task so slow ODRS responses
            // never hold up the main detail view.
            if details_res.is_ok() {
                let odrs_app_id = app_id_str.clone();
                let odrs_qt_thread = qt_thread.clone();
                crate::runtime::runtime().spawn(async move {
                    let odrs_client = crate::flathub::odrs::OdrsClient::new();
                    let (ratings_res, reviews_res) = tokio::join!(
                        odrs_client.fetch_ratings(&odrs_app_id),
                        odrs_client.fetch_reviews(&odrs_app_id),
                    );

                    let _ = odrs_qt_thread.queue(move |mut qobject| {
                        let ratings_qs = match ratings_res {
                            Ok(r) => util::json_qstring(&r, "{}"),
                            Err(_) => cxx_qt_lib::QString::from("{}"),
                        };
                        qobject.as_mut().set_detail_ratings_json(ratings_qs);

                        let reviews_qs = match reviews_res {
                            Ok(r) => util::json_qstring(&r, "[]"),
                            Err(_) => cxx_qt_lib::QString::from("[]"),
                        };
                        qobject.as_mut().set_detail_reviews_json(reviews_qs);

                        qobject.as_mut().reviews_loaded();
                    });
                });
            }

            let _ = qt_thread.queue(move |mut qobject| {
                match details_res {
                    Ok(details) => {
                        apply_details(qobject.as_mut(), details, installed, permissions_val, developer_apps, similar_apps);
                    }
                    Err(e) => {
                        kerr!("StoreController::load_app_details: {}", e);
                        qobject.as_mut().set_error_message(cxx_qt_lib::QString::from(&e.to_string()));
                        qobject.as_mut().reviews_loaded();
                    }
                }
                qobject.as_mut().set_loading(false);
                qobject.as_mut().details_loaded();
            });
        });
    }

    pub fn cancel_operation(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().rust().cancel_flag.store(true, Ordering::SeqCst);
        self.as_mut().set_install_progress(0.0);
    }

    pub fn install_app(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_install_progress(0.01);
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().cancel_flag = cancel.clone();
        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();

        klog!("StoreController::install_app: installing \"{}\"", app_id_str);
        util::run_with_progress(
            qt_thread,
            async move { crate::flatpak::cli::install_app(&app_id_str).await },
            Some(cancel),
            |q, p| q.set_install_progress(p),
            |mut q| {
                q.as_mut().set_install_progress(0.0);
                q.as_mut().install_finished(false);
            },
            |mut q, res| {
                q.as_mut().set_install_progress(1.0);
                match res {
                    Ok(()) => {
                        klog!("StoreController::install_app: successfully installed");
                        q.as_mut().set_detail_is_installed(true);
                        q.as_mut().install_finished(true);
                    }
                    Err(e) => {
                        kerr!("StoreController::install_app: {}", e);
                        q.as_mut().set_error_message(cxx_qt_lib::QString::from(&e));
                        q.as_mut().install_finished(false);
                    }
                }
            },
        );
    }

    pub fn uninstall_app(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_install_progress(0.01);
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().cancel_flag = cancel.clone();
        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();

        klog!("StoreController::uninstall_app: uninstalling \"{}\"", app_id_str);
        util::run_with_progress(
            qt_thread,
            async move { crate::flatpak::cli::uninstall_app(&app_id_str).await },
            Some(cancel),
            |q, p| q.set_install_progress(p),
            |mut q| {
                q.as_mut().set_install_progress(0.0);
                q.as_mut().install_finished(false);
            },
            |mut q, res| {
                q.as_mut().set_install_progress(1.0);
                match res {
                    Ok(()) => {
                        klog!("StoreController::uninstall_app: successfully uninstalled");
                        q.as_mut().set_detail_is_installed(false);
                        q.as_mut().install_finished(true);
                    }
                    Err(e) => {
                        kerr!("StoreController::uninstall_app: {}", e);
                        q.as_mut().set_error_message(cxx_qt_lib::QString::from(&e));
                        q.as_mut().install_finished(false);
                    }
                }
            },
        );
    }

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
        let app_id_str = app_id.to_string();
        let summary_str = summary.to_string();
        let description_str = description.to_string();
        let mut version_str = version.to_string();
        let mut distro_str = distro.to_string();
        let mut locale_str = locale.to_string();

        crate::runtime::runtime().spawn(async move {
            // Distro auto-detection if empty
            if distro_str.is_empty() {
                distro_str = if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
                    let mut found = None;
                    for line in content.lines() {
                        if line.starts_with("ID=") {
                            found = Some(line.trim_start_matches("ID=")
                                .trim_matches('"')
                                .to_string());
                            break;
                        }
                    }
                    found.unwrap_or_else(|| "Linux".to_string())
                } else {
                    "Linux".to_string()
                };
            }

            // Locale auto-detection if empty
            if locale_str.is_empty() {
                locale_str = std::env::var("LANG")
                    .unwrap_or_else(|_| "en_US".to_string())
                    .split('.')
                    .next()
                    .unwrap_or("en_US")
                    .to_string();
            }

            // Version auto-detection if empty
            if version_str.is_empty() {
                if let Ok(installed_apps) = crate::flatpak::cli::list_installed_cached().await {
                    if let Some(app) = installed_apps.iter().find(|a| a.application_id == app_id_str) {
                        if let Some(ref v) = app.version {
                            version_str = v.clone();
                        }
                    }
                }
            }
            if version_str.is_empty() {
                version_str = "1.0".to_string();
            }

            let client = crate::flathub::odrs::OdrsClient::new();
            match client.submit_review(
                &app_id_str,
                rating,
                &summary_str,
                &description_str,
                &version_str,
                &distro_str,
                &locale_str,
                is_anonymous,
            ).await {
                Ok(()) => {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_submitted(true, cxx_qt_lib::QString::from(""));
                    });
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_submitted(false, cxx_qt_lib::QString::from(&err_msg));
                    });
                }
            }
        });
    }

    pub fn upvote_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::odrs::OdrsClient::new();
            match client.upvote_review(review_id).await {
                Ok(()) => {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(true, cxx_qt_lib::QString::from(""));
                    });
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(false, cxx_qt_lib::QString::from(&err_msg));
                    });
                }
            }
        });
    }

    pub fn downvote_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::odrs::OdrsClient::new();
            match client.downvote_review(review_id).await {
                Ok(()) => {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(true, cxx_qt_lib::QString::from(""));
                    });
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(false, cxx_qt_lib::QString::from(&err_msg));
                    });
                }
            }
        });
    }

    pub fn dismiss_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::odrs::OdrsClient::new();
            match client.dismiss_review(review_id).await {
                Ok(()) => {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(true, cxx_qt_lib::QString::from(""));
                    });
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(false, cxx_qt_lib::QString::from(&err_msg));
                    });
                }
            }
        });
    }

    pub fn remove_review(self: std::pin::Pin<&mut Self>, review_id: i64) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::odrs::OdrsClient::new();
            match client.remove_review(review_id).await {
                Ok(()) => {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(true, cxx_qt_lib::QString::from(""));
                    });
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().review_action_finished(false, cxx_qt_lib::QString::from(&err_msg));
                    });
                }
            }
        });
    }
}

// ── Detail-loading helpers ────────────────────────────────────────────────────

/// Concurrently fetch the supplementary detail lists for an app: its sandbox
/// permissions (from the summary endpoint), other apps by the same developer,
/// and similar apps from its primary category. Failures degrade to empty values
/// rather than aborting the whole detail load. The current app is removed from
/// the similar list and both app lists are capped at 8 entries.
async fn fetch_supplementary(
    client: &FlathubClient,
    details: &AppDetails,
    app_id: &str,
) -> (serde_json::Value, Vec<FlathubApp>, Vec<FlathubApp>) {
    let dev_name = details.developer_name.clone();
    let category = details.categories.first().cloned();

    let perm_fut = async {
        client.fetch_summary(app_id).await.unwrap_or_else(|e| {
            kerr!("load_app_details: fetch_summary failed: {}", e);
            serde_json::json!({})
        })
    };
    let dev_fut = async {
        match dev_name {
            Some(ref dev) => client.fetch_developer_apps(dev).await.unwrap_or_else(|e| {
                kerr!("load_app_details: fetch_developer_apps failed: {}", e);
                vec![]
            }),
            None => vec![],
        }
    };
    let cat_fut = async {
        match category {
            Some(ref cat) => client.fetch_category(cat).await.unwrap_or_else(|e| {
                kerr!("load_app_details: fetch_category failed: {}", e);
                vec![]
            }),
            None => vec![],
        }
    };

    let (permissions_val, mut developer_apps, mut similar_apps) =
        tokio::join!(perm_fut, dev_fut, cat_fut);

    similar_apps.retain(|app| app.app_id != app_id);
    developer_apps.truncate(8);
    similar_apps.truncate(8);

    (permissions_val, developer_apps, similar_apps)
}

/// Pick the highest-resolution screenshot URL for each screenshot entry,
/// falling back to the first available size.
fn screenshot_urls(details: &AppDetails) -> Vec<String> {
    details
        .screenshots
        .iter()
        .filter_map(|s| {
            s.sizes
                .iter()
                .max_by_key(|size| {
                    size.width
                        .as_ref()
                        .and_then(|w| {
                            w.as_str()
                                .and_then(|s| s.parse::<i32>().ok())
                                .or_else(|| w.as_i64().map(|i| i as i32))
                        })
                        .unwrap_or(0)
                })
                .map(|size| size.src.clone())
                .or_else(|| s.sizes.first().map(|size| size.src.clone()))
        })
        .collect()
}

/// Collect the app's external URLs (homepage, bugtracker, donation, …) plus the
/// build manifest into a JSON object keyed by link type.
fn urls_object(details: &AppDetails) -> serde_json::Value {
    let mut urls_map = serde_json::Map::new();
    if let Some(ref u) = details.urls {
        if let Some(ref h) = u.homepage { urls_map.insert("homepage".to_string(), serde_json::Value::String(h.clone())); }
        if let Some(ref b) = u.bugtracker { urls_map.insert("bugtracker".to_string(), serde_json::Value::String(b.clone())); }
        if let Some(ref d) = u.donation { urls_map.insert("donation".to_string(), serde_json::Value::String(d.clone())); }
        if let Some(ref hp) = u.help { urls_map.insert("help".to_string(), serde_json::Value::String(hp.clone())); }
        if let Some(ref v) = u.vcs_browser { urls_map.insert("vcs_browser".to_string(), serde_json::Value::String(v.clone())); }
    }
    if let Some(ref m) = details.metadata
        && let Some(ref manifest) = m.manifest
    {
        urls_map.insert("manifest".to_string(), serde_json::Value::String(manifest.clone()));
    }
    serde_json::Value::Object(urls_map)
}

/// Convert a list of apps into the compact `{app_id, name, summary, icon}` JSON
/// shape consumed by the developer-apps and similar-apps QML rows.
fn apps_to_json(apps: Vec<FlathubApp>) -> Vec<serde_json::Value> {
    apps.into_iter()
        .map(|app| {
            serde_json::json!({
                "app_id": app.app_id,
                "name": app.name,
                "summary": app.summary.unwrap_or_default(),
                "icon": app.icon.unwrap_or_default()
            })
        })
        .collect()
}

/// Marshal a fully-loaded [`AppDetails`] into the StoreController's detail
/// properties on the Qt thread.
fn apply_details(
    mut qobject: std::pin::Pin<&mut qobject::StoreController>,
    details: AppDetails,
    installed: bool,
    permissions_val: serde_json::Value,
    developer_apps: Vec<FlathubApp>,
    similar_apps: Vec<FlathubApp>,
) {
    klog!("StoreController::load_app_details: OK — \"{}\" (installed={})", details.name, installed);
    qobject.as_mut().set_detail_name(cxx_qt_lib::QString::from(&details.name));
    qobject.as_mut().set_detail_summary(cxx_qt_lib::QString::from(&details.summary));
    qobject.as_mut().set_detail_description(cxx_qt_lib::QString::from(&details.description.clone().unwrap_or_default()));
    qobject.as_mut().set_detail_icon_url(cxx_qt_lib::QString::from(&details.icon.clone().unwrap_or_default()));
    qobject.as_mut().set_detail_developer(cxx_qt_lib::QString::from(&details.developer_name.clone().unwrap_or_default()));
    qobject.as_mut().set_detail_license(cxx_qt_lib::QString::from(&details.project_license.clone().unwrap_or_default()));
    qobject.as_mut().set_detail_is_installed(installed);
    qobject.as_mut().set_error_message(cxx_qt_lib::QString::from(""));

    qobject.as_mut().set_detail_screenshots_json(util::json_qstring(&screenshot_urls(&details), "[]"));

    let permissions = permissions_val
        .get("metadata")
        .and_then(|m| m.get("permissions"))
        .unwrap_or(&permissions_val);
    qobject.as_mut().set_detail_permissions_json(util::json_qstring(permissions, "{}"));

    qobject.as_mut().set_detail_urls_json(util::json_qstring(&urls_object(&details), "{}"));
    qobject.as_mut().set_detail_developer_apps_json(util::json_qstring(&apps_to_json(developer_apps), "[]"));
    qobject.as_mut().set_detail_similar_apps_json(util::json_qstring(&apps_to_json(similar_apps), "[]"));
}
