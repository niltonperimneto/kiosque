use cxx_qt::Threading;

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
}

static CANCEL_FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl qobject::StoreController {
    pub fn load_app_details(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_loading(true);
        // Clear stale state from previous detail view immediately
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

        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();
        
        eprintln!("[kiosque] StoreController::load_app_details: fetching \"{}\"", app_id_str);
        crate::runtime::runtime().spawn(async move {
            let client = crate::flathub::client::FlathubClient::new();

            // Fetch details and installation status concurrently
            let (details_res, installed) = tokio::join!(
                client.fetch_details(&app_id_str),
                crate::flatpak::cli::is_installed_cached(&app_id_str),
            );

            // If detail fetch succeeded, launch the additional details fetches concurrently
            let mut permissions_val = serde_json::json!({});
            let mut developer_apps = vec![];
            let mut similar_apps = vec![];
            
            let success = details_res.is_ok();

            if let Ok(ref details) = details_res {
                let dev_name = details.developer_name.clone();
                let category = details.categories.first().cloned();
                let app_id_clone = app_id_str.clone();
                let client_ref = &client;

                let perm_fut = async move {
                    client_ref.fetch_summary(&app_id_clone).await.unwrap_or_else(|e| {
                        eprintln!("[kiosque] ERROR load_app_details: fetch_summary failed: {}", e);
                        serde_json::json!({})
                    })
                };

                let dev_fut = async move {
                    if let Some(ref dev) = dev_name {
                        client_ref.fetch_developer_apps(dev).await.unwrap_or_else(|e| {
                            eprintln!("[kiosque] ERROR load_app_details: fetch_developer_apps failed: {}", e);
                            vec![]
                        })
                    } else {
                        vec![]
                    }
                };

                let cat_fut = async move {
                    if let Some(ref cat) = category {
                        client_ref.fetch_category(cat).await.unwrap_or_else(|e| {
                            eprintln!("[kiosque] ERROR load_app_details: fetch_category failed: {}", e);
                            vec![]
                        })
                    } else {
                        vec![]
                    }
                };

                let (perm_res, dev_res, cat_res) = tokio::join!(perm_fut, dev_fut, cat_fut);
                permissions_val = perm_res;
                developer_apps = dev_res;
                similar_apps = cat_res;

                // Filter out current app and limit to 8 items
                similar_apps.retain(|app| app.app_id != app_id_str);
                developer_apps.truncate(8);
                similar_apps.truncate(8);
            }

            // Spawn separate task for ODRS ratings & reviews to prevent slow ODRS from hanging detail load
            if success {
                let odrs_app_id = app_id_str.clone();
                let odrs_qt_thread = qt_thread.clone();
                crate::runtime::runtime().spawn(async move {
                    let odrs_client = crate::flathub::odrs::OdrsClient::new();
                    let (ratings_res, reviews_res) = tokio::join!(
                        odrs_client.fetch_ratings(&odrs_app_id),
                        odrs_client.fetch_reviews(&odrs_app_id),
                    );

                    let _ = odrs_qt_thread.queue(move |mut qobject| {
                        let ratings_json = ratings_res.map(|r| serde_json::to_string(&r).unwrap_or_else(|_| "{}".to_string())).unwrap_or_else(|_| "{}".to_string());
                        qobject.as_mut().set_detail_ratings_json(cxx_qt_lib::QString::from(&ratings_json));

                        let reviews_json = reviews_res.map(|r| serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())).unwrap_or_else(|_| "[]".to_string());
                        qobject.as_mut().set_detail_reviews_json(cxx_qt_lib::QString::from(&reviews_json));

                        qobject.as_mut().reviews_loaded();
                    });
                });
            }

            let _ = qt_thread.queue(move |mut qobject| {
                match details_res {
                    Ok(details) => {
                        eprintln!("[kiosque] StoreController::load_app_details: OK — \"{}\" (installed={})", details.name, installed);
                        qobject.as_mut().set_detail_name(cxx_qt_lib::QString::from(&details.name));
                        qobject.as_mut().set_detail_summary(cxx_qt_lib::QString::from(&details.summary));
                        let desc = details.description.unwrap_or_default();
                        qobject.as_mut().set_detail_description(cxx_qt_lib::QString::from(&desc));
                        let icon = details.icon.unwrap_or_default();
                        qobject.as_mut().set_detail_icon_url(cxx_qt_lib::QString::from(&icon));
                        let dev = details.developer_name.unwrap_or_default();
                        qobject.as_mut().set_detail_developer(cxx_qt_lib::QString::from(&dev));
                        let license = details.project_license.unwrap_or_default();
                        qobject.as_mut().set_detail_license(cxx_qt_lib::QString::from(&license));
                        qobject.as_mut().set_detail_is_installed(installed);
                        qobject.as_mut().set_error_message(cxx_qt_lib::QString::from(""));

                        // ── Process screenshots ──
                        let screenshot_urls: Vec<String> = details.screenshots.iter()
                            .filter_map(|s| {
                                s.sizes.iter()
                                    .max_by_key(|size| {
                                        size.width.as_ref()
                                            .and_then(|w| w.as_str().and_then(|s| s.parse::<i32>().ok())
                                                        .or_else(|| w.as_i64().map(|i| i as i32)))
                                            .unwrap_or(0)
                                    })
                                    .map(|size| size.src.clone())
                                    .or_else(|| s.sizes.first().map(|size| size.src.clone()))
                            })
                            .collect();
                        let screenshots_json = serde_json::to_string(&screenshot_urls).unwrap_or_else(|_| "[]".to_string());
                        qobject.as_mut().set_detail_screenshots_json(cxx_qt_lib::QString::from(&screenshots_json));

                        // ── Process permissions ──
                        let permissions_json = serde_json::to_string(&permissions_val.get("metadata").and_then(|m| m.get("permissions")).unwrap_or(&permissions_val)).unwrap_or_else(|_| "{}".to_string());
                        qobject.as_mut().set_detail_permissions_json(cxx_qt_lib::QString::from(&permissions_json));

                        // ── Process URLs ──
                        let mut urls_map = serde_json::Map::new();
                        if let Some(ref u) = details.urls {
                            if let Some(ref h) = u.homepage { urls_map.insert("homepage".to_string(), serde_json::Value::String(h.clone())); }
                            if let Some(ref b) = u.bugtracker { urls_map.insert("bugtracker".to_string(), serde_json::Value::String(b.clone())); }
                            if let Some(ref d) = u.donation { urls_map.insert("donation".to_string(), serde_json::Value::String(d.clone())); }
                            if let Some(ref hp) = u.help { urls_map.insert("help".to_string(), serde_json::Value::String(hp.clone())); }
                            if let Some(ref v) = u.vcs_browser { urls_map.insert("vcs_browser".to_string(), serde_json::Value::String(v.clone())); }
                        }
                        if let Some(ref m) = details.metadata
                            && let Some(ref manifest) = m.manifest { urls_map.insert("manifest".to_string(), serde_json::Value::String(manifest.clone())); }
                        let urls_json = serde_json::Value::Object(urls_map);
                        let urls_json_str = serde_json::to_string(&urls_json).unwrap_or_else(|_| "{}".to_string());
                        qobject.as_mut().set_detail_urls_json(cxx_qt_lib::QString::from(&urls_json_str));

                        // ── Process Developer Apps ──
                        let dev_apps_json: Vec<serde_json::Value> = developer_apps.into_iter().map(|app| {
                            serde_json::json!({
                                "app_id": app.app_id,
                                "name": app.name,
                                "summary": app.summary.unwrap_or_default(),
                                "icon": app.icon.unwrap_or_default()
                            })
                        }).collect();
                        let dev_apps_json_str = serde_json::to_string(&dev_apps_json).unwrap_or_else(|_| "[]".to_string());
                        qobject.as_mut().set_detail_developer_apps_json(cxx_qt_lib::QString::from(&dev_apps_json_str));

                        // ── Process Similar Apps ──
                        let sim_apps_json: Vec<serde_json::Value> = similar_apps.into_iter().map(|app| {
                            serde_json::json!({
                                "app_id": app.app_id,
                                "name": app.name,
                                "summary": app.summary.unwrap_or_default(),
                                "icon": app.icon.unwrap_or_default()
                            })
                        }).collect();
                        let sim_apps_json_str = serde_json::to_string(&sim_apps_json).unwrap_or_else(|_| "[]".to_string());
                        qobject.as_mut().set_detail_similar_apps_json(cxx_qt_lib::QString::from(&sim_apps_json_str));
                    }
                    Err(e) => {
                        eprintln!("[kiosque] ERROR StoreController::load_app_details: {}", e);
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
        CANCEL_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
        self.as_mut().set_install_progress(0.0);
    }

    pub fn install_app(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_install_progress(0.01);
        CANCEL_FLAG.store(false, std::sync::atomic::Ordering::SeqCst);
        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();
        
        eprintln!("[kiosque] StoreController::install_app: installing \"{}\"", app_id_str);
        crate::runtime::runtime().spawn(async move {
            let qt_thread_prog = qt_thread.clone();
            let mut progress = 0.01;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
            
            let (tx, mut rx) = tokio::sync::oneshot::channel();
            
            tokio::spawn(async move {
                let res = crate::flatpak::cli::install_app(&app_id_str).await;
                let _ = tx.send(res);
            });

            loop {
                if CANCEL_FLAG.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().set_install_progress(0.0);
                        qobject.as_mut().install_finished(false);
                    });
                    break;
                }
                tokio::select! {
                    res_opt = &mut rx => {
                        let res = res_opt.unwrap_or_else(|_| Err("Install task panicked".into()));
                        let _ = qt_thread.queue(move |mut qobject| {
                            qobject.as_mut().set_install_progress(1.0);
                            match res {
                                Ok(()) => {
                                    eprintln!("[kiosque] StoreController::install_app: successfully installed");
                                    qobject.as_mut().set_detail_is_installed(true);
                                    qobject.as_mut().install_finished(true);
                                }
                                Err(e) => {
                                    eprintln!("[kiosque] ERROR StoreController::install_app: {}", e);
                                    qobject.as_mut().set_error_message(cxx_qt_lib::QString::from(&e.to_string()));
                                    qobject.as_mut().install_finished(false);
                                }
                            }
                        });
                        break;
                    }
                    _ = interval.tick() => {
                        progress += (0.95 - progress) * 0.05;
                        let p = progress;
                        let _ = qt_thread_prog.queue(move |mut qobject| {
                            qobject.as_mut().set_install_progress(p);
                        });
                    }
                }
            }
        });
    }

    pub fn uninstall_app(mut self: std::pin::Pin<&mut Self>, app_id: cxx_qt_lib::QString) {
        self.as_mut().set_install_progress(0.01);
        CANCEL_FLAG.store(false, std::sync::atomic::Ordering::SeqCst);
        let qt_thread = self.qt_thread();
        let app_id_str = app_id.to_string();
        
        eprintln!("[kiosque] StoreController::uninstall_app: uninstalling \"{}\"", app_id_str);
        crate::runtime::runtime().spawn(async move {
            let qt_thread_prog = qt_thread.clone();
            let mut progress = 0.01;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
            
            let (tx, mut rx) = tokio::sync::oneshot::channel();
            
            tokio::spawn(async move {
                let res = crate::flatpak::cli::uninstall_app(&app_id_str).await;
                let _ = tx.send(res);
            });

            loop {
                if CANCEL_FLAG.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().set_install_progress(0.0);
                        qobject.as_mut().install_finished(false);
                    });
                    break;
                }
                tokio::select! {
                    res_opt = &mut rx => {
                        let res = res_opt.unwrap_or_else(|_| Err("Uninstall task panicked".into()));
                        let _ = qt_thread.queue(move |mut qobject| {
                            qobject.as_mut().set_install_progress(1.0);
                            match res {
                                Ok(()) => {
                                    eprintln!("[kiosque] StoreController::uninstall_app: successfully uninstalled");
                                    qobject.as_mut().set_detail_is_installed(false);
                                    qobject.as_mut().install_finished(true);
                                }
                                Err(e) => {
                                    eprintln!("[kiosque] ERROR StoreController::uninstall_app: {}", e);
                                    qobject.as_mut().set_error_message(cxx_qt_lib::QString::from(&e.to_string()));
                                    qobject.as_mut().install_finished(false);
                                }
                            }
                        });
                        break;
                    }
                    _ = interval.tick() => {
                        progress += (0.95 - progress) * 0.05;
                        let p = progress;
                        let _ = qt_thread_prog.queue(move |mut qobject| {
                            qobject.as_mut().set_install_progress(p);
                        });
                    }
                }
            }
        });
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
