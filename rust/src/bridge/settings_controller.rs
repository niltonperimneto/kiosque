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
        type SettingsController = super::SettingsControllerRust;

        #[qinvokable]
        #[cxx_name = "loadSettings"]
        fn load_settings(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "saveSettings"]
        fn save_settings(self: Pin<&mut SettingsController>, auto_update: bool, frequency: QString, time: QString);
    }
}

#[derive(Default)]
pub struct SettingsControllerRust {
    pub auto_update: bool,
    pub update_frequency: cxx_qt_lib::QString,
    pub update_time: cxx_qt_lib::QString,
}

impl qobject::SettingsController {
    pub fn load_settings(mut self: Pin<&mut Self>) {
        let settings = crate::settings::load_settings();
        self.as_mut().set_auto_update(settings.auto_update);
        self.as_mut().set_update_frequency(cxx_qt_lib::QString::from(&settings.update_frequency));
        self.as_mut().set_update_time(cxx_qt_lib::QString::from(&settings.update_time));
    }

    pub fn save_settings(
        mut self: Pin<&mut Self>,
        auto_update: bool,
        frequency: cxx_qt_lib::QString,
        time: cxx_qt_lib::QString,
    ) {
        let mut settings = crate::settings::load_settings();
        settings.auto_update = auto_update;
        settings.update_frequency = frequency.to_string();
        settings.update_time = time.to_string();

        if let Err(e) = crate::settings::save_settings(&settings) {
            eprintln!("[kiosque] Failed to save settings: {}", e);
        } else {
            self.as_mut().set_auto_update(settings.auto_update);
            self.as_mut().set_update_frequency(cxx_qt_lib::QString::from(&settings.update_frequency));
            self.as_mut().set_update_time(cxx_qt_lib::QString::from(&settings.update_time));
        }
    }
}
