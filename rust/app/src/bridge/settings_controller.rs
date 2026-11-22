//! `SettingsController` singleton: preferences, Flathub login, ODRS identity.

use qtbridge::qobject;

pub use object::SettingsController;

#[qobject(NoQmlElement)]
mod object {
    use qtbridge::QmlObject;

    #[derive(Default)]
    pub struct SettingsController {
        pub(super) auto_update: bool,
        pub(super) update_frequency: String,
        pub(super) update_time: String,
        pub(super) is_authenticated: bool,
        pub(super) oauth_provider: String,
        pub(super) oauth_username: String,
        pub(super) oauth_avatar_url: String,
        pub(super) odrs_user_hash: String,
    }

    impl SettingsController {
        qproperty!(
            "auto_update",
            Read = get_auto_update,
            Notify = auto_update_changed
        );
        qproperty!(
            "update_frequency",
            Read = get_update_frequency,
            Notify = update_frequency_changed
        );
        qproperty!(
            "update_time",
            Read = get_update_time,
            Notify = update_time_changed
        );
        qproperty!(
            "is_authenticated",
            Read = get_is_authenticated,
            Notify = is_authenticated_changed
        );
        qproperty!(
            "oauth_provider",
            Read = get_oauth_provider,
            Notify = oauth_provider_changed
        );
        qproperty!(
            "oauth_username",
            Read = get_oauth_username,
            Notify = oauth_username_changed
        );
        qproperty!(
            "oauth_avatar_url",
            Read = get_oauth_avatar_url,
            Notify = oauth_avatar_url_changed
        );
        qproperty!(
            "odrs_user_hash",
            Read = get_odrs_user_hash,
            Notify = odrs_user_hash_changed
        );

        fn get_auto_update(&self) -> bool {
            self.auto_update
        }
        fn get_update_frequency(&self) -> String {
            self.update_frequency.clone()
        }
        fn get_update_time(&self) -> String {
            self.update_time.clone()
        }
        fn get_is_authenticated(&self) -> bool {
            self.is_authenticated
        }
        fn get_oauth_provider(&self) -> String {
            self.oauth_provider.clone()
        }
        fn get_oauth_username(&self) -> String {
            self.oauth_username.clone()
        }
        fn get_oauth_avatar_url(&self) -> String {
            self.oauth_avatar_url.clone()
        }
        fn get_odrs_user_hash(&self) -> String {
            self.odrs_user_hash.clone()
        }

        #[qsignal]
        pub fn auto_update_changed(&mut self);
        #[qsignal]
        pub fn update_frequency_changed(&mut self);
        #[qsignal]
        pub fn update_time_changed(&mut self);
        #[qsignal]
        pub fn is_authenticated_changed(&mut self);
        #[qsignal]
        pub fn oauth_provider_changed(&mut self);
        #[qsignal]
        pub fn oauth_username_changed(&mut self);
        #[qsignal]
        pub fn oauth_avatar_url_changed(&mut self);
        #[qsignal]
        pub fn odrs_user_hash_changed(&mut self);
        #[qsignal(qml_name = "loginFailed")]
        pub fn login_failed(&mut self, error: String);

        #[qslot(qml_name = "loadSettings")]
        fn load_settings(&mut self) {
            let s = crate::services::services().settings.load();
            self.set_auto_update(s.auto_update);
            self.set_update_frequency(s.update_frequency);
            self.set_update_time(s.update_time);
            self.set_is_authenticated(s.oauth_token.is_some());
            self.set_oauth_provider(s.oauth_provider.unwrap_or_default());
            self.set_oauth_username(s.oauth_username.unwrap_or_default());
            self.set_oauth_avatar_url(s.oauth_avatar_url.unwrap_or_default());
            self.set_odrs_user_hash(kiosque_core::flathub::odrs::get_user_hash());
        }

        /// Persist the update preferences. This is the only caller of
        /// `autostart::apply`, and only when the toggle actually changed.
        #[qslot(qml_name = "saveSettings")]
        fn save_settings(&mut self, auto_update: bool, frequency: String, time: String) {
            let store = &crate::services::services().settings;
            let mut s = store.load();
            let autostart_changed = s.auto_update != auto_update;
            s.auto_update = auto_update;
            s.update_frequency = frequency;
            s.update_time = time;
            if let Err(e) = store.save(&s) {
                kerr!("settings: cannot save: {}", e);
                return;
            }
            if autostart_changed {
                crate::bridge::spawn(async move {
                    if let Err(e) = kiosque_core::autostart::apply(auto_update).await {
                        kerr!("autostart: {}", e);
                    }
                });
            }
            self.set_auto_update(s.auto_update);
            self.set_update_frequency(s.update_frequency);
            self.set_update_time(s.update_time);
        }

        #[qslot]
        fn login(&mut self, provider: String) {
            let Some(provider) = kiosque_core::flathub::oauth::OAuthProvider::from_str(&provider)
            else {
                self.login_failed(format!("unknown provider {provider:?}"));
                return;
            };
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let result = kiosque_core::flathub::oauth::perform_login(provider).await;
                handle.invoke_mut(move |c| match result {
                    Ok(()) => c.load_settings(),
                    Err(e) => {
                        kerr!("login failed: {}", e);
                        c.login_failed(e);
                    }
                });
            });
        }

        #[qslot]
        fn logout(&mut self) {
            match kiosque_core::flathub::oauth::perform_logout() {
                Ok(()) => self.load_settings(),
                Err(e) => kerr!("logout failed: {}", e),
            }
        }

        #[qslot(qml_name = "regenerateSalt")]
        fn regenerate_salt(&mut self) {
            let store = &crate::services::services().settings;
            let mut s = store.load();
            s.odrs_salt = kiosque_core::settings::AppSettings::default().odrs_salt;
            if let Err(e) = store.save(&s) {
                kerr!("settings: cannot save salt: {}", e);
            }
            self.load_settings();
        }

        #[qslot(qml_name = "clearCache")]
        fn clear_cache(&mut self) {
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                kiosque_core::cache::app_cache().clear().await;
                handle.invoke_mut(|c| c.load_settings());
            });
        }
    }
}

notify_setters!(SettingsController {
    set_auto_update: auto_update: bool => auto_update_changed,
    set_update_frequency: update_frequency: String => update_frequency_changed,
    set_update_time: update_time: String => update_time_changed,
    set_is_authenticated: is_authenticated: bool => is_authenticated_changed,
    set_oauth_provider: oauth_provider: String => oauth_provider_changed,
    set_oauth_username: oauth_username: String => oauth_username_changed,
    set_oauth_avatar_url: oauth_avatar_url: String => oauth_avatar_url_changed,
    set_odrs_user_hash: odrs_user_hash: String => odrs_user_hash_changed,
});

qml_element!(SettingsController, "SettingsController", singleton);
