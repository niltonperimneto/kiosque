use cxx_qt::{CxxQtType, Threading};
use std::pin::Pin;

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
        #[qproperty(bool, auto_update)]
        #[qproperty(QString, update_frequency)]
        #[qproperty(QString, update_time)]
        #[qproperty(bool, saving_settings)]
        #[qproperty(bool, login_in_progress)]
        #[qproperty(QString, login_user_code)]
        #[qproperty(QString, login_verification_url)]
        #[qproperty(bool, is_authenticated)]
        #[qproperty(QString, oauth_provider)]
        #[qproperty(QString, oauth_username)]
        #[qproperty(QString, oauth_avatar_url)]
        #[qproperty(QString, odrs_user_hash)]
        type SettingsController = super::SettingsControllerRust;

        #[qinvokable]
        #[cxx_name = "loadSettings"]
        fn load_settings(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "saveSettings"]
        fn save_settings(
            self: Pin<&mut SettingsController>,
            auto_update: bool,
            frequency: QString,
            time: QString,
        );

        #[qinvokable]
        #[cxx_name = "login"]
        fn login(self: Pin<&mut SettingsController>, provider: QString);

        #[qinvokable]
        #[cxx_name = "cancelLogin"]
        fn cancel_login(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "logout"]
        fn logout(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "regenerateSalt"]
        fn regenerate_salt(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "clearCache"]
        fn clear_cache(self: Pin<&mut SettingsController>);

        #[qsignal]
        #[cxx_name = "loginFailed"]
        fn login_failed(self: Pin<&mut SettingsController>, error: QString);

        #[qsignal]
        #[cxx_name = "settingsSaved"]
        fn settings_saved(self: Pin<&mut SettingsController>, success: bool, error: QString);
    }

    impl cxx_qt::Threading for SettingsController {}
}

#[derive(Default)]
pub struct SettingsControllerRust {
    pub auto_update: bool,
    pub update_frequency: cxx_qt_lib::QString,
    pub update_time: cxx_qt_lib::QString,
    pub saving_settings: bool,
    pub login_in_progress: bool,
    pub login_user_code: cxx_qt_lib::QString,
    pub login_verification_url: cxx_qt_lib::QString,
    pub is_authenticated: bool,
    login_cancel: Option<tokio_util::sync::CancellationToken>,
    pub oauth_provider: cxx_qt_lib::QString,
    pub oauth_username: cxx_qt_lib::QString,
    pub oauth_avatar_url: cxx_qt_lib::QString,
    pub odrs_user_hash: cxx_qt_lib::QString,
}

impl Drop for SettingsControllerRust {
    fn drop(&mut self) {
        if let Some(cancel) = &self.login_cancel {
            cancel.cancel();
        }
    }
}

impl qobject::SettingsController {
    pub fn load_settings(mut self: Pin<&mut Self>) {
        let settings = crate::services::services().settings.load();
        self.as_mut().set_auto_update(settings.auto_update);
        self.as_mut()
            .set_update_frequency(cxx_qt_lib::QString::from(&settings.update_frequency));
        self.as_mut()
            .set_update_time(cxx_qt_lib::QString::from(&settings.update_time));

        let is_auth = settings
            .oauth_user_id
            .as_ref()
            .is_some_and(|id| !id.is_empty())
            && settings.oauth_provider.is_some();
        self.as_mut().set_is_authenticated(is_auth);

        let provider = settings.oauth_provider.unwrap_or_default();
        self.as_mut()
            .set_oauth_provider(cxx_qt_lib::QString::from(&provider));

        let username = settings.oauth_username.unwrap_or_default();
        self.as_mut()
            .set_oauth_username(cxx_qt_lib::QString::from(&username));

        let avatar_url =
            crate::image_cache::image_url(&settings.oauth_avatar_url.unwrap_or_default());
        self.as_mut()
            .set_oauth_avatar_url(cxx_qt_lib::QString::from(&avatar_url));

        let user_hash = crate::flathub::odrs::get_user_hash();
        self.as_mut()
            .set_odrs_user_hash(cxx_qt_lib::QString::from(&user_hash));
    }

    pub fn save_settings(
        mut self: Pin<&mut Self>,
        auto_update: bool,
        frequency: cxx_qt_lib::QString,
        time: cxx_qt_lib::QString,
    ) {
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
            self.settings_saved(false, cxx_qt_lib::QString::from(&e));
        } else {
            self.as_mut().set_auto_update(settings.auto_update);
            self.as_mut()
                .set_update_frequency(cxx_qt_lib::QString::from(&settings.update_frequency));
            self.as_mut()
                .set_update_time(cxx_qt_lib::QString::from(&settings.update_time));
            let toggled = previous_auto_update != auto_update;
            if !toggled {
                // A schedule edit needs no permission: it is saved now, and
                // the daemon is told in the background.
                self.as_mut()
                    .settings_saved(true, cxx_qt_lib::QString::default());
                let thread = self.qt_thread();
                crate::runtime::runtime().spawn(async move {
                    if let Err(error) = apply_automatic_updates(&settings, false).await {
                        let _ = thread.queue(move |object| {
                            object.settings_saved(false, cxx_qt_lib::QString::from(&error));
                        });
                    }
                });
                return;
            }
            self.as_mut().set_saving_settings(true);
            let thread = self.qt_thread();
            crate::runtime::runtime().spawn(async move {
                let error = match apply_automatic_updates(&settings, toggled).await {
                    Ok(()) => String::new(),
                    Err(error) if toggled => {
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
                    Err(error) => error,
                };
                let _ = thread.queue(move |mut object| {
                    object.as_mut().load_settings();
                    object.as_mut().set_saving_settings(false);
                    object.settings_saved(error.is_empty(), cxx_qt_lib::QString::from(&error));
                });
            });
        }
    }

    pub fn login(mut self: Pin<&mut Self>, provider: cxx_qt_lib::QString) {
        if self.login_in_progress {
            return;
        }
        let Some(provider) = crate::flathub::oauth::OAuthProvider::from_str(&provider.to_string())
        else {
            self.login_failed("Unknown sign-in provider".into());
            return;
        };
        let cancel = tokio_util::sync::CancellationToken::new();
        self.as_mut().rust_mut().login_cancel = Some(cancel.clone());
        self.as_mut().set_login_in_progress(true);
        self.as_mut().set_login_user_code(Default::default());
        self.as_mut().set_login_verification_url(Default::default());
        let thread = self.qt_thread();
        let prompt_thread = thread.clone();
        crate::runtime::runtime().spawn(async move {
            let result = tokio::select! {
                _ = cancel.cancelled() => Err("Sign-in cancelled".to_string()),
                result = crate::flathub::oauth::perform_login_with_prompt(provider, move |prompt| {
                    let _ = prompt_thread.queue(move |mut object| {
                        object.as_mut().set_login_user_code(prompt.user_code.as_str().into());
                        object.set_login_verification_url(prompt.verification_uri.as_str().into());
                    });
                }) => result,
            };
            let _ = thread.queue(move |mut object| {
                object.as_mut().rust_mut().login_cancel = None;
                object.as_mut().set_login_in_progress(false);
                object.as_mut().set_login_user_code(Default::default());
                object
                    .as_mut()
                    .set_login_verification_url(Default::default());
                match result {
                    Ok(()) => object.load_settings(),
                    Err(error) => object.login_failed(error.as_str().into()),
                }
            });
        });
    }

    pub fn cancel_login(self: Pin<&mut Self>) {
        if let Some(cancel) = &self.login_cancel {
            cancel.cancel();
        }
    }

    pub fn logout(mut self: Pin<&mut Self>) {
        self.as_mut().cancel_login();
        if let Err(e) = crate::flathub::oauth::perform_logout() {
            eprintln!("[kiosque] Logout failed: {}", e);
        } else {
            self.as_mut().load_settings();
        }
    }

    pub fn regenerate_salt(self: Pin<&mut Self>) {
        let mut settings = crate::services::services().settings.load();
        settings.odrs_salt = crate::settings::AppSettings::default().odrs_salt;

        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            if let Err(e) = crate::services::services().settings.save(&settings) {
                eprintln!("[kiosque] Failed to save regenerated salt settings: {}", e);
            }
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().load_settings();
            });
        });
    }

    pub fn clear_cache(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            crate::cache::app_cache().clear().await;
            eprintln!("[kiosque] Cache cleared successfully.");
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().load_settings();
            });
        });
    }
}

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
            kerr!("SettingsController: daemon not reachable, applying at next login: {error}");
            Ok(())
        }
    }
}
