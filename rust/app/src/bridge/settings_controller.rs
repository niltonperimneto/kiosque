//! Feature-parity adapter for the current storefront.
pub use object::SettingsController;
use qtbridge::qobject;
#[qobject(NoQmlElement)]
mod object {
    use qtbridge::QmlObject;

    #[derive(Default)]
    pub struct SettingsController {
        pub(super) auto_update: bool,
        pub(super) update_frequency: String,
        pub(super) update_time: String,
        pub(super) saving_settings: bool,
        pub(super) login_in_progress: bool,
        pub(super) login_user_code: String,
        pub(super) login_verification_url: String,
        pub(super) is_authenticated: bool,
        pub(super) login_cancel: Option<tokio_util::sync::CancellationToken>,
        pub(super) oauth_provider: String,
        pub(super) oauth_username: String,
        pub(super) oauth_avatar_url: String,
        pub(super) odrs_user_hash: String,
    }

    impl Drop for SettingsController {
        fn drop(&mut self) {
            if let Some(cancel) = &self.login_cancel {
                cancel.cancel();
            }
        }
    }

    impl SettingsController {
        #[qslot(qml_name = "loadSettings")]
        pub fn load_settings(&mut self) {
            let settings = crate::services::services().settings.load();
            self.set_auto_update(settings.auto_update);
            self.set_update_frequency(String::from(&settings.update_frequency));
            self.set_update_time(String::from(&settings.update_time));

            let is_auth = settings
                .oauth_user_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
                && settings.oauth_provider.is_some();
            self.set_is_authenticated(is_auth);

            let provider = settings.oauth_provider.unwrap_or_default();
            self.set_oauth_provider(String::from(&provider));

            let username = settings.oauth_username.unwrap_or_default();
            self.set_oauth_username(String::from(&username));

            self.set_oauth_avatar_url(settings.oauth_avatar_url.unwrap_or_default());

            let user_hash = crate::flathub::odrs::get_user_hash();
            self.set_odrs_user_hash(String::from(&user_hash));
        }

        #[qslot(qml_name = "saveSettings")]
        pub fn save_settings(&mut self, auto_update: bool, frequency: String, time: String) {
            if self.saving_settings {
                return;
            }
            let frequency = frequency.to_string();
            let frequency = if frequency.eq_ignore_ascii_case("weekly") {
                "Weekly"
            } else if frequency.eq_ignore_ascii_case("daily") {
                "Daily"
            } else {
                self.settings_saved(false, "Choose a daily or weekly update schedule".into());
                return;
            };
            let time = time.to_string();
            if jiff::civil::Time::strptime("%H:%M", &time).is_err() {
                self.settings_saved(false, "Enter a valid update time (HH:MM)".into());
                return;
            }
            let mut settings = crate::services::services().settings.load();
            let previous_auto_update = settings.auto_update;
            settings.auto_update = auto_update;
            settings.update_frequency = frequency.into();
            settings.update_time = time;

            if let Err(e) = crate::services::services().settings.save(&settings) {
                self.settings_saved(false, String::from(&e));
            } else {
                self.set_auto_update(settings.auto_update);
                self.set_update_frequency(String::from(&settings.update_frequency));
                self.set_update_time(String::from(&settings.update_time));
                let toggled = previous_auto_update != auto_update;
                if !toggled {
                    // A schedule edit needs no permission: it is saved now, and
                    // the daemon is told in the background.
                    self.settings_saved(true, String::new());
                    let thread = std::sync::Arc::new(self.get_thread_handle());
                    crate::runtime::runtime().spawn(async move {
                        if let Err(error) = super::apply_automatic_updates(&settings, false).await {
                            thread.invoke_mut(move |object| {
                                object.settings_saved(false, error);
                            });
                        }
                    });
                    return;
                }
                self.set_saving_settings(true);
                let thread = std::sync::Arc::new(self.get_thread_handle());
                crate::runtime::runtime().spawn(async move {
                    let result = super::apply_automatic_updates(&settings, true).await;
                    let error = match result {
                        Ok(()) => String::new(),
                        Err(error) => {
                            let store = &crate::services::services().settings;
                            let mut current = store.load();
                            current.auto_update = previous_auto_update;
                            match store.save(&current) {
                                Ok(()) => error,
                                Err(save_error) => {
                                    format!("{error}; could not restore the setting: {save_error}")
                                }
                            }
                        }
                    };
                    thread.invoke_mut(move |object| {
                        object.load_settings();
                        object.set_saving_settings(false);
                        object.settings_saved(error.is_empty(), String::from(&error));
                    });
                });
            }
        }

        #[qslot(qml_name = "login")]
        pub fn login(&mut self, provider: String) {
            if self.login_in_progress {
                return;
            }
            let Some(provider) =
                crate::flathub::oauth::OAuthProvider::from_str(&provider.to_string())
            else {
                self.login_failed("Unknown sign-in provider".into());
                return;
            };
            let cancel = tokio_util::sync::CancellationToken::new();
            self.login_cancel = Some(cancel.clone());
            self.set_login_in_progress(true);
            self.set_login_user_code(Default::default());
            self.set_login_verification_url(Default::default());
            let thread = std::sync::Arc::new(self.get_thread_handle());
            let prompt_thread = thread.clone();
            crate::runtime::runtime().spawn(async move {
            let result = tokio::select! {
                _ = cancel.cancelled() => Err("Sign-in cancelled".to_string()),
                result = crate::flathub::oauth::perform_login_with_prompt(provider, move |prompt| {
                    prompt_thread.invoke_mut(move |object| {
                        object.set_login_user_code(prompt.user_code.as_str().into());
                        object.set_login_verification_url(prompt.verification_uri.as_str().into());
                    });
                }) => result,
            };
            thread.invoke_mut(move |object| {
                object.login_cancel = None;
                object.set_login_in_progress(false);
                object.set_login_user_code(Default::default());
                object
                    .set_login_verification_url(Default::default());
                match result {
                    Ok(()) => object.load_settings(),
                    Err(error) => object.login_failed(error.as_str().into()),
                }
            });
        });
        }

        #[qslot(qml_name = "cancelLogin")]
        pub fn cancel_login(&mut self) {
            if let Some(cancel) = &self.login_cancel {
                cancel.cancel();
            }
        }

        #[qslot(qml_name = "logout")]
        pub fn logout(&mut self) {
            self.cancel_login();
            if let Err(e) = crate::flathub::oauth::perform_logout() {
                eprintln!("[kiosque] Logout failed: {}", e);
            } else {
                self.load_settings();
            }
        }

        #[qslot(qml_name = "regenerateSalt")]
        pub fn regenerate_salt(&mut self) {
            let mut settings = crate::services::services().settings.load();
            settings.odrs_salt = crate::settings::AppSettings::default().odrs_salt;

            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                if let Err(e) = crate::services::services().settings.save(&settings) {
                    eprintln!("[kiosque] Failed to save regenerated salt settings: {}", e);
                }
                qt_thread.invoke_mut(move |qobject| {
                    qobject.load_settings();
                });
            });
        }

        #[qslot(qml_name = "clearCache")]
        pub fn clear_cache(&mut self) {
            let qt_thread = std::sync::Arc::new(self.get_thread_handle());
            crate::runtime::runtime().spawn(async move {
                crate::cache::app_cache().clear().await;
                eprintln!("[kiosque] Cache cleared successfully.");
                qt_thread.invoke_mut(move |qobject| {
                    qobject.load_settings();
                });
            });
        }
    }

    impl SettingsController {
        qproperty!(
            "auto_update",
            Read = get_auto_update,
            Notify = auto_update_changed
        );
        fn get_auto_update(&self) -> bool {
            self.auto_update
        }
        #[qsignal(qml_name = "auto_updateChanged")]
        pub fn auto_update_changed(&mut self);
        qproperty!(
            "update_frequency",
            Read = get_update_frequency,
            Notify = update_frequency_changed
        );
        fn get_update_frequency(&self) -> String {
            self.update_frequency.clone()
        }
        #[qsignal(qml_name = "update_frequencyChanged")]
        pub fn update_frequency_changed(&mut self);
        qproperty!(
            "update_time",
            Read = get_update_time,
            Notify = update_time_changed
        );
        fn get_update_time(&self) -> String {
            self.update_time.clone()
        }
        #[qsignal(qml_name = "update_timeChanged")]
        pub fn update_time_changed(&mut self);
        qproperty!(
            "saving_settings",
            Read = get_saving_settings,
            Notify = saving_settings_changed
        );
        fn get_saving_settings(&self) -> bool {
            self.saving_settings
        }
        #[qsignal(qml_name = "saving_settingsChanged")]
        pub fn saving_settings_changed(&mut self);
        qproperty!(
            "login_in_progress",
            Read = get_login_in_progress,
            Notify = login_in_progress_changed
        );
        fn get_login_in_progress(&self) -> bool {
            self.login_in_progress
        }
        #[qsignal(qml_name = "login_in_progressChanged")]
        pub fn login_in_progress_changed(&mut self);
        qproperty!(
            "login_user_code",
            Read = get_login_user_code,
            Notify = login_user_code_changed
        );
        fn get_login_user_code(&self) -> String {
            self.login_user_code.clone()
        }
        #[qsignal(qml_name = "login_user_codeChanged")]
        pub fn login_user_code_changed(&mut self);
        qproperty!(
            "login_verification_url",
            Read = get_login_verification_url,
            Notify = login_verification_url_changed
        );
        fn get_login_verification_url(&self) -> String {
            self.login_verification_url.clone()
        }
        #[qsignal(qml_name = "login_verification_urlChanged")]
        pub fn login_verification_url_changed(&mut self);
        qproperty!(
            "is_authenticated",
            Read = get_is_authenticated,
            Notify = is_authenticated_changed
        );
        fn get_is_authenticated(&self) -> bool {
            self.is_authenticated
        }
        #[qsignal(qml_name = "is_authenticatedChanged")]
        pub fn is_authenticated_changed(&mut self);
        qproperty!(
            "oauth_provider",
            Read = get_oauth_provider,
            Notify = oauth_provider_changed
        );
        fn get_oauth_provider(&self) -> String {
            self.oauth_provider.clone()
        }
        #[qsignal(qml_name = "oauth_providerChanged")]
        pub fn oauth_provider_changed(&mut self);
        qproperty!(
            "oauth_username",
            Read = get_oauth_username,
            Notify = oauth_username_changed
        );
        fn get_oauth_username(&self) -> String {
            self.oauth_username.clone()
        }
        #[qsignal(qml_name = "oauth_usernameChanged")]
        pub fn oauth_username_changed(&mut self);
        qproperty!(
            "oauth_avatar_url",
            Read = get_oauth_avatar_url,
            Notify = oauth_avatar_url_changed
        );
        fn get_oauth_avatar_url(&self) -> String {
            self.oauth_avatar_url.clone()
        }
        #[qsignal(qml_name = "oauth_avatar_urlChanged")]
        pub fn oauth_avatar_url_changed(&mut self);
        qproperty!(
            "odrs_user_hash",
            Read = get_odrs_user_hash,
            Notify = odrs_user_hash_changed
        );
        fn get_odrs_user_hash(&self) -> String {
            self.odrs_user_hash.clone()
        }
        #[qsignal(qml_name = "odrs_user_hashChanged")]
        pub fn odrs_user_hash_changed(&mut self);
        #[qsignal(qml_name = "loginFailed")]
        pub fn login_failed(&mut self, error: String);
        #[qsignal(qml_name = "settingsSaved")]
        pub fn settings_saved(&mut self, success: bool, error: String);
    }
}
notify_setters!(SettingsController {
set_auto_update: auto_update: bool => auto_update_changed,
set_update_frequency: update_frequency: String => update_frequency_changed,
set_update_time: update_time: String => update_time_changed,
set_saving_settings: saving_settings: bool => saving_settings_changed,
set_login_in_progress: login_in_progress: bool => login_in_progress_changed,
set_login_user_code: login_user_code: String => login_user_code_changed,
set_login_verification_url: login_verification_url: String => login_verification_url_changed,
set_is_authenticated: is_authenticated: bool => is_authenticated_changed,
set_oauth_provider: oauth_provider: String => oauth_provider_changed,
set_oauth_username: oauth_username: String => oauth_username_changed,
set_oauth_avatar_url: oauth_avatar_url: String => oauth_avatar_url_changed,
set_odrs_user_hash: odrs_user_hash: String => odrs_user_hash_changed
});
qml_element!(SettingsController, "SettingsController", singleton);

/// Register (or remove) the login autostart when automatic updates were
/// toggled, then hand the schedule to the running daemon so it applies now
/// instead of after the next login.
///
/// Inside Flatpak the host companion performs the updates, so enabling fails
/// when it cannot be reached. A native install still updates from the next
/// login if the daemon cannot be reached now.
async fn apply_automatic_updates(
    settings: &kiosque_core::settings::AppSettings,
    toggled: bool,
) -> Result<(), String> {
    // Hand schedules to the daemon one at a time, in the order they were saved.
    static ORDER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _turn = ORDER.lock().await;
    if toggled {
        kiosque_core::settings::apply_autostart(settings).await?;
    }
    match kiosque_core::daemon::client::configure_automatic_updates(settings).await {
        Ok(()) => Ok(()),
        Err(error) if kiosque_core::flatpak::in_sandbox() && settings.auto_update => {
            if toggled {
                // Undo the login autostart registered above.
                let mut off = settings.clone();
                off.auto_update = false;
                let _ = kiosque_core::settings::apply_autostart(&off).await;
            }
            Err(format!(
                "Automatic updates need the Kiosque companion. {error}"
            ))
        }
        Err(error) => {
            kiosque_core::kerr!(
                "SettingsController: daemon not reachable, applying at next login: {error}"
            );
            Ok(())
        }
    }
}
