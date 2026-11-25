//! Qt Bridge adapters: the only place that uses `qtbridge`. Logic lives in
//! `view/` and `kiosque_core`; these types translate it to QML.
//!
//! Rules (see `qml/AGENTS.md`):
//! * Emit signals last, after all state is updated.
//! * Results computed off-thread come back through `QThreadHandle::invoke_mut`,
//!   which is silently dropped if the QML object is already gone.
//! * QML handlers that call back into the emitting object use `Qt.callLater`;
//!   a fresh mutable borrow during an emission aborts the process.

/// Implement `QmlElement` under the `com.kiosque` URI. Qt Bridge would
/// otherwise derive the module name from the Cargo package.
macro_rules! qml_element {
    ($ty:ty, $name:literal) => {
        qml_element!(@impl $ty, $name, false);
    };
    ($ty:ty, $name:literal, singleton) => {
        qml_element!(@impl $ty, $name, true);
    };
    (@impl $ty:ty, $name:literal, $singleton:expr) => {
        impl qtbridge::QmlElement for $ty {
            const URI: &str = $crate::bridge::URI;
            const ELEMENT_NAME: &str = $name;
            const MAJOR_VERSION: u8 = 1;
            const MINOR_VERSION: u8 = 0;
            const IS_SINGLETON: bool = $singleton;
        }
    };
}

/// Generate change-notifying setters for member properties:
/// `notify_setters!(Type { set_x: x: T => x_changed, … })`.
macro_rules! notify_setters {
    ($ty:ty { $($setter:ident : $field:ident : $t:ty => $signal:ident),* $(,)? }) => {
        impl $ty {
            $(
                #[allow(dead_code)]
                pub fn $setter(&mut self, value: $t) {
                    if self.$field != value {
                        self.$field = value;
                        self.$signal();
                    }
                }
            )*
        }
    };
}

pub mod app_details;
pub mod app_list_model;
pub mod app_shell;
pub mod detail_data;
pub mod featured_model;
pub mod installed_model;
pub mod jobs_model;
pub mod l10n;
pub mod remote_image;
pub mod repo_model;
pub mod runtime_planner;
pub mod runtimes_model;
pub mod settings_controller;
pub mod store_controller;
pub mod util;

pub const URI: &str = "com.kiosque";

/// Register every `com.kiosque` type. Explicit, so nothing depends on link
/// order or `linkme`.
pub fn register_types() {
    use qtbridge::QmlElement;
    app_list_model::AppListModel::register();
    featured_model::FeaturedModel::register();
    installed_model::InstalledModel::register();
    repo_model::RepoModel::register();
    runtime_planner::RuntimePlanner::register();
    runtimes_model::RuntimesModel::register();
    settings_controller::SettingsController::register();
    app_details::AppDetails::register();
    store_controller::StoreController::register();
    jobs_model::JobsModel::register();
    remote_image::RemoteImage::register();
    l10n::L10n::register();
    app_shell::AppShell::register();
}

/// Same as [`register_types`], chained on an application.
pub fn register_all(app: &mut qtbridge::QApp) {
    let _ = app;
    register_types();
}

/// Spawn `future` on the shared runtime.
pub(crate) fn spawn<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    kiosque_core::runtime::runtime().spawn(future);
}
