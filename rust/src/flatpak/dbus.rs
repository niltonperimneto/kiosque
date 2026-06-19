use zbus::Connection;

pub async fn check_flatpak_available() -> bool {
    Connection::session().await.is_ok()
}
