//! Private desktop fixtures; absent from production builds.
use qtbridge::{QmlElement, qobject};
use std::sync::{Arc, Mutex, OnceLock};
#[derive(Default)]
struct PortalState {
    count: i32,
    autostart: bool,
    command: Vec<String>,
    deny: bool,
}
fn portal() -> &'static Arc<Mutex<PortalState>> {
    static STATE: OnceLock<Arc<Mutex<PortalState>>> = OnceLock::new();
    STATE.get_or_init(|| Arc::new(Mutex::new(PortalState::default())))
}
struct BackgroundPortal;
#[zbus::interface(name = "org.freedesktop.portal.Background")]
impl BackgroundPortal {
    async fn request_background(
        &self,
        _parent: String,
        options: std::collections::HashMap<String, zbus::OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<zbus::OwnedObjectPath> {
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".into()))?
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = options
            .get("handle_token")
            .and_then(|v| <&str>::try_from(v).ok())
            .unwrap_or("fixture");
        let path = zbus::OwnedObjectPath::try_from(format!(
            "/org/freedesktop/portal/desktop/request/{sender}/{token}"
        ))
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let autostart = options
            .get("autostart")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false);
        let denied = {
            let mut state = portal().lock().unwrap();
            state.count += 1;
            state.autostart = autostart;
            state.command = options
                .get("commandline")
                .and_then(|v| v.try_clone().ok())
                .and_then(|v| Vec::<String>::try_from(v).ok())
                .unwrap_or_default();
            std::mem::take(&mut state.deny)
        };
        let results = std::collections::HashMap::from([
            ("background", zbus::zvariant::Variant::from(!denied)),
            (
                "autostart",
                zbus::zvariant::Variant::from(autostart && !denied),
            ),
        ]);
        conn.emit_signal(
            None::<&str>,
            path.as_str(),
            "org.freedesktop.portal.Request",
            "Response",
            &(if denied { 2u32 } else { 0u32 }, results),
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(path)
    }
}
#[qobject(NoQmlElement)]
mod fixtures {
    #[derive(Default)]
    pub struct BackgroundPortalFixture {}
    impl BackgroundPortalFixture {
        qproperty!("requestCount", Read = count);
        qproperty!("lastAutostart", Read = autostart);
        qproperty!("lastCommand", Read = command);
        qproperty!("denyNext", Read = deny, Write = set_deny);
        fn count(&self) -> i32 {
            super::portal().lock().unwrap().count
        }
        fn autostart(&self) -> bool {
            super::portal().lock().unwrap().autostart
        }
        fn command(&self) -> serde_json::Value {
            super::portal().lock().unwrap().command.clone().into()
        }
        fn deny(&self) -> bool {
            super::portal().lock().unwrap().deny
        }
        fn set_deny(&mut self, deny: bool) {
            super::portal().lock().unwrap().deny = deny;
        }
    }
}
#[qobject(NoQmlElement)]
mod backend_fixture {
    #[derive(Default)]
    pub struct BackendFixture {}
    impl BackendFixture {
        #[qslot(qml_name = "setUpdates")]
        fn set_updates(&self, ids: Vec<String>) -> bool {
            crate::services::set_test_updates(&ids)
        }
        #[qslot(qml_name = "holdOperations")]
        fn hold_operations(&self, hold: bool) {
            crate::services::hold_test_operations(hold);
        }
        #[qslot(qml_name = "operationCount")]
        fn operation_count(&self) -> i64 {
            crate::services::test_operation_count() as i64
        }
        /// The remote of the most recent mock install ("" if none).
        #[qslot(qml_name = "lastInstallRemote")]
        fn last_install_remote(&self) -> String {
            crate::services::last_test_install_remote()
        }
    }
}
qml_element!(
    fixtures::BackgroundPortalFixture,
    "BackgroundPortalFixture",
    singleton
);
qml_element!(backend_fixture::BackendFixture, "BackendFixture", singleton);
pub fn register() -> Result<zbus::Connection, String> {
    fixtures::BackgroundPortalFixture::register();
    backend_fixture::BackendFixture::register();
    crate::runtime::runtime().block_on(async {
        zbus::connection::Builder::session()
            .name("org.freedesktop.portal.Desktop")
            .serve_at("/org/freedesktop/portal/desktop", BackgroundPortal)
            .build()
            .await
            .map_err(|e| e.to_string())
    })
}
