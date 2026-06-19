use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("com.kiosque"))
        .files([
            "src/bridge/app_list_model.rs",
            "src/bridge/featured_model.rs",
            "src/bridge/installed_model.rs",
            "src/bridge/store_controller.rs",
            "src/bridge/settings_controller.rs",
            "src/bridge/repo_model.rs",
        ])
        .build();
}

