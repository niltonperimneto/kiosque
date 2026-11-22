//! Authenticate the bus sender using credentials supplied by the bus daemon.
//! This constrains accidental/cross-application access; the Unix account itself
//! remains trusted (a hostile process with ptrace access can control that account).
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
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
    if other_mount.is_none() || own_mount.is_none() || other_mount == own_mount {
        return Err(denied());
    }
    let path = format!("/proc/{pid}/root/.flatpak-info");
    let mut info = String::new();
    use std::io::Read;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| denied())?;
    if !file.metadata().map_err(|_| denied())?.is_file() {
        return Err(denied());
    }
    file.take(65537)
        .read_to_string(&mut info)
        .map_err(|_| denied())?;
    if info.len() > 65536 || application_id(&info) != Some("org.kiosque.Kiosque") {
        return Err(denied());
    }
    if packaged_client(pid) {
        return Ok(());
    }
    // With Flatpak's filtered bus the kernel identifies xdg-dbus-proxy as
    // the peer. Its private mount namespace contains the instance metadata.
    #[cfg(feature = "libflatpak")]
    if same_executable(pid, std::path::Path::new("/usr/bin/xdg-dbus-proxy")) {
        use libflatpak::prelude::InstanceExt;
        let id = keyfile_value(&info, "[Instance]", "instance-id").ok_or_else(denied)?;
        if libflatpak::Instance::all().iter().any(|instance| {
            instance.app().as_deref() == Some("org.kiosque.Kiosque")
                && instance.id().as_deref() == Some(id)
                && u32::try_from(instance.child_pid()).is_ok_and(packaged_client)
        }) {
            return Ok(());
        }
    }
    Err(denied())
}

fn packaged_client(pid: u32) -> bool {
    same_executable(
        pid,
        std::path::Path::new(&format!("/proc/{pid}/root/app/bin/kiosque")),
    )
}

fn same_executable(pid: u32, expected: &std::path::Path) -> bool {
    let expected = std::fs::metadata(expected).ok();
    let running = std::fs::metadata(format!("/proc/{pid}/exe")).ok();
    expected
        .zip(running)
        .is_some_and(|(a, b)| a.ino() == b.ino() && a.dev() == b.dev())
}

fn denied() -> fdo::Error {
    fdo::Error::AccessDenied("Only the Kiosque application may use this interface".into())
}
fn application_id(info: &str) -> Option<&str> {
    keyfile_value(info, "[Application]", "name")
}
fn keyfile_value<'a>(info: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut in_section = false;
    let mut name = None;
    for line in info.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line == section;
        } else if in_section
            && let Some((found, value)) = line.split_once('=')
            && found == key
        {
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
    fn instance_identity_is_section_scoped_and_unique() {
        let field = |info| keyfile_value(info, "[Instance]", "instance-id");
        assert_eq!(field("[Application]\ninstance-id=42"), None);
        assert_eq!(field("[Instance]\ninstance-id=42"), Some("42"));
        assert_eq!(field("[Instance]\ninstance-id=42\ninstance-id=43"), None);
    }

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
