//! Authenticate the bus sender using credentials supplied by the bus daemon.
//! This constrains accidental/cross-application access; the Unix account itself
//! remains trusted (a hostile process with ptrace access can control that account).
use std::os::unix::fs::MetadataExt;
use zbus::{Connection, fdo, message::Header};

pub async fn authorize(header: &Header<'_>, connection: &Connection) -> fdo::Result<()> {
    let sender = header.sender().ok_or_else(denied)?;
    let bus = fdo::DBusProxy::new(connection)
        .await
        .map_err(|_| denied())?;
    let credentials = bus
        .get_connection_credentials(sender.clone().into())
        .await
        .map_err(|_| denied())?;
    if credentials.unix_user_id() != Some(unsafe { libc::geteuid() }) {
        return Err(denied());
    }
    let pid = credentials.process_id().ok_or_else(denied)?;
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).map_err(|_| denied())?;
    let current = std::env::current_exe().map_err(|_| denied())?;
    let native = current.parent().is_some_and(|parent| {
        exe == parent.join("kiosque") || exe == parent.join("kiosque-update")
    });
    if native {
        return Ok(());
    }
    // A sandbox client must run the packaged binary in its own mount namespace.
    let other_mount = std::fs::read_link(format!("/proc/{pid}/ns/mnt")).ok();
    let own_mount = std::fs::read_link("/proc/self/ns/mnt").ok();
    let packaged = std::fs::metadata(format!("/proc/{pid}/root/app/bin/kiosque")).ok();
    let running = std::fs::metadata(format!("/proc/{pid}/exe")).ok();
    let packaged_binary = packaged.zip(running).is_some_and(|(packaged, running)| {
        packaged.ino() == running.ino() && packaged.dev() == running.dev()
    });
    if packaged_binary && other_mount.is_some() && other_mount != own_mount {
        let path = format!("/proc/{pid}/root/.flatpak-info");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() <= 64 * 1024) {
            let info = std::fs::read_to_string(path).map_err(|_| denied())?;
            if application_id(&info) == Some("org.kiosque.Kiosque") {
                return Ok(());
            }
        }
    }
    Err(denied())
}

fn denied() -> fdo::Error {
    fdo::Error::AccessDenied("Only the Kiosque application may use this interface".into())
}
fn application_id(info: &str) -> Option<&str> {
    let mut application = false;
    let mut name = None;
    for line in info.lines().map(str::trim) {
        if line.starts_with('[') {
            application = line == "[Application]";
        } else if application && let Some(value) = line.strip_prefix("name=") {
            if name.is_some() {
                return None;
            }
            name = Some(value);
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_is_section_scoped_and_unambiguous() {
        assert_eq!(application_id("[Instance]\nname=org.kiosque.Kiosque"), None);
        assert_eq!(
            application_id("[Application]\nname=other\nname=org.kiosque.Kiosque"),
            None
        );
        assert_eq!(
            application_id("[Application]\nname=org.kiosque.Kiosque\n[Instance]\nname=other"),
            Some("org.kiosque.Kiosque")
        );
    }
}
