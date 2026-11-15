use cxx_qt::Threading;
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
    pub is_authenticated: bool,
    pub oauth_provider: cxx_qt_lib::QString,
    pub oauth_username: cxx_qt_lib::QString,
    pub oauth_avatar_url: cxx_qt_lib::QString,
    pub odrs_user_hash: cxx_qt_lib::QString,
}

impl qobject::SettingsController {
    pub fn load_settings(mut self: Pin<&mut Self>) {
        let settings = crate::services::services().settings.load();
        self.as_mut().set_auto_update(settings.auto_update);
        self.as_mut()
            .set_update_frequency(cxx_qt_lib::QString::from(&settings.update_frequency));
        self.as_mut()
            .set_update_time(cxx_qt_lib::QString::from(&settings.update_time));

        let is_auth = settings.oauth_token.is_some();
        self.as_mut().set_is_authenticated(is_auth);

        let provider = settings.oauth_provider.unwrap_or_default();
        self.as_mut()
            .set_oauth_provider(cxx_qt_lib::QString::from(&provider));

        let username = settings.oauth_username.unwrap_or_default();
        self.as_mut()
            .set_oauth_username(cxx_qt_lib::QString::from(&username));

        let avatar_url = settings.oauth_avatar_url.unwrap_or_default();
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
        if chrono::NaiveTime::parse_from_str(&time, "%H:%M").is_err() {
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
            if previous_auto_update == auto_update {
                self.settings_saved(true, cxx_qt_lib::QString::default());
                return;
            }
            self.as_mut().set_saving_settings(true);
            let thread = self.qt_thread();
            crate::runtime::runtime().spawn(async move {
                let result = crate::settings::apply_autostart(&settings).await;
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
                let _ = thread.queue(move |mut object| {
                    object.as_mut().load_settings();
                    object.as_mut().set_saving_settings(false);
                    object.settings_saved(error.is_empty(), cxx_qt_lib::QString::from(&error));
                });
            });
        }
    }

    pub fn login(self: Pin<&mut Self>, provider: cxx_qt_lib::QString) {
        let provider_str = provider.to_string();
        let provider_enum = match crate::flathub::oauth::OAuthProvider::from_str(&provider_str) {
            Some(p) => p,
            None => {
                eprintln!("[kiosque] Unknown OAuth provider: {}", provider_str);
                return;
            }
        };

        let qt_thread = self.qt_thread();
        crate::runtime::runtime().spawn(async move {
            match crate::flathub::oauth::perform_login(provider_enum).await {
                Ok(()) => {
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject.as_mut().load_settings();
                    });
                }
                Err(e) => {
                    eprintln!("[kiosque] OAuth Login failed: {}", e);
                    let err_str = e.to_string();
                    let _ = qt_thread.queue(move |mut qobject| {
                        qobject
                            .as_mut()
                            .login_failed(cxx_qt_lib::QString::from(&err_str));
                    });
                }
            }
        });
    }

    pub fn logout(mut self: Pin<&mut Self>) {
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
