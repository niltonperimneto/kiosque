//! Shared limits and validation at process, network and installation boundaries.
use std::net::IpAddr;

pub const MAX_REF_BYTES: usize = 1024 * 1024;
pub const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_OPERATIONS: usize = 32;
pub const MAX_UPDATE_IDS: usize = 256;
pub const MAX_TITLE_BYTES: usize = 512;

pub fn app_ref(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with(['-', '.', '/'])
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-/".contains(&c))
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Invalid application identifier".into());
    }
    Ok(())
}

pub fn remote_name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 255
        || value.starts_with(['-', '.'])
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("Invalid repository name".into());
    }
    Ok(())
}

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            segments[0] & 0xe000 == 0x2000
                && segments[0] != 0x2002
                && segments[0] != 0x3fff
                && !(segments[0] == 0x2001 && (segments[1] <= 0x1ff || segments[1] == 0xdb8))
        }
    }
}

pub fn https_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid HTTPS URL")?;
    if value.len() > 8192
        || value.chars().any(char::is_control)
        || url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some_and(|p| p != 443)
    {
        return Err("Only public HTTPS URLs without credentials are allowed".into());
    }
    let host = url.host_str().ok_or("Missing HTTPS host")?;
    let literal = host.trim_matches(['[', ']']).parse::<IpAddr>();
    if let Ok(ip) = literal {
        if !public_ip(ip) {
            return Err("Private or local network URLs are not allowed".into());
        }
    } else if !host.contains('.')
        || host.ends_with('.')
        || ["localhost", "local", "internal", "test", "invalid"]
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
    {
        return Err("Private or local network URLs are not allowed".into());
    }
    Ok(url)
}

/// Resolve once to public addresses; reject mixed public/private DNS responses.
/// IP literals are checked separately by `https_url` before a request is sent.
#[derive(Debug)]
pub struct PublicResolver;
impl reqwest::dns::Resolve for PublicResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addresses: Vec<_> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if addresses.is_empty() || addresses.iter().any(|address| !public_ip(address.ip())) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "DNS destination is not public",
                )
                .into());
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

#[derive(Clone, Debug)]
pub struct RefSummary {
    pub app_id: String,
    pub title: String,
    pub origin: String,
    pub branch: String,
    pub key_sha256: String,
}

pub fn ref_file(data: &[u8]) -> Result<RefSummary, String> {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    if data.len() > MAX_REF_BYTES {
        return Err("Reference file is too large".into());
    }
    let text = std::str::from_utf8(data).map_err(|_| "Reference file must be UTF-8")?;
    if text.contains('\0') {
        return Err("Reference file contains a NUL byte".into());
    }
    let mut fields = std::collections::HashMap::new();
    let mut section = false;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with(';'))
    {
        if line.starts_with('[') {
            if section || line != "[Flatpak Ref]" {
                return Err("Unexpected reference-file section".into());
            }
            section = true;
            continue;
        }
        if !section {
            return Err("Missing Flatpak Ref section".into());
        }
        let (key, value) = line.split_once('=').ok_or("Invalid reference-file entry")?;
        let key = key.trim();
        let value = value.trim();
        if ![
            "Name",
            "Title",
            "Url",
            "GPGKey",
            "Branch",
            "IsRuntime",
            "RuntimeRepo",
            "SuggestRemoteName",
            "Version",
            "Homepage",
            "Icon",
            "Comment",
            "Description",
            "DefaultBranch",
            "DeployCollectionID",
            "CollectionID",
        ]
        .contains(&key)
            || value.contains('\\')
            || value.chars().any(char::is_control)
            || fields.insert(key, value).is_some()
        {
            return Err("Invalid or duplicate reference-file entry".into());
        }
        if ["Url", "RuntimeRepo", "Homepage", "Icon"].contains(&key) && !value.is_empty() {
            https_url(value)?;
        }
    }
    let required = |key| {
        fields
            .get(key)
            .copied()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("Missing {key} in reference file"))
    };
    let app_id = required("Name")?;
    app_ref(app_id)?;
    if app_id.contains('/') {
        return Err("Reference Name must be an application id".into());
    }
    let origin = required("Url")?;
    https_url(origin)?;
    let key = base64::engine::general_purpose::STANDARD
        .decode(required("GPGKey")?.split_whitespace().collect::<String>())
        .map_err(|_| "Invalid signing key")?;
    if key.is_empty() || key.len() > 64 * 1024 {
        return Err("Invalid signing key size".into());
    }
    let branch = fields.get("Branch").copied().unwrap_or("stable");
    app_ref(branch)?;
    if branch.contains('/') {
        return Err("Invalid branch".into());
    }
    let title = fields.get("Title").copied().unwrap_or(app_id);
    if title.len() > MAX_TITLE_BYTES {
        return Err("Reference title is too long".into());
    }
    Ok(RefSummary {
        app_id: app_id.into(),
        title: title.into(),
        origin: https_url(origin)?.into(),
        branch: branch.into(),
        key_sha256: Sha256::digest(key)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    })
}

pub fn operation(op: &crate::flatpak::Operation, title: &str) -> Result<(), String> {
    use crate::flatpak::Operation;
    if title.len() > MAX_TITLE_BYTES || title.chars().any(char::is_control) {
        return Err("Invalid transaction title".into());
    }
    match op {
        Operation::Install { remote, id, .. } => {
            remote_name(remote)?;
            app_ref(id)?;
        }
        Operation::InstallRefFile { data, .. } => {
            ref_file(data)?;
        }
        Operation::Uninstall { id, .. } => app_ref(id)?,
        Operation::RemoveRuntimes { refs } => {
            if refs.is_empty() || refs.len() > MAX_UPDATE_IDS {
                return Err("Invalid number of runtimes to remove".into());
            }
            for reference in refs {
                app_ref(reference)?;
                match crate::flatpak::parse_ref(reference) {
                    Some((crate::flatpak::RefKind::Runtime, ..)) => {}
                    _ => return Err("Only runtimes can be removed this way".into()),
                }
            }
        }
        Operation::Update { ids } => {
            if ids.len() > MAX_UPDATE_IDS {
                return Err("Too many applications in one update".into());
            }
            for id in ids {
                app_ref(id)?;
            }
        }
    }
    Ok(())
}

/// Repository descriptors use the same strict URL/key grammar as references.
pub fn repo_file(data: &[u8]) -> Result<RefSummary, String> {
    if data.len() > MAX_REF_BYTES {
        return Err("Repository file is too large".into());
    }
    let text = std::str::from_utf8(data).map_err(|_| "Repository file must be UTF-8")?;
    if text
        .lines()
        .filter(|line| line.trim() == "[Flatpak Repo]")
        .count()
        != 1
        || text.contains("[Flatpak Ref]")
    {
        return Err("Expected a Flatpak Repo descriptor".into());
    }
    // Supply only the two fields that repository descriptors do not require.
    let converted = text.replacen(
        "[Flatpak Repo]",
        "[Flatpak Ref]\nName=org.kiosque.Repository",
        1,
    );
    let summary = ref_file(converted.as_bytes())?;
    if let Some(name) = crate::flatpak::keyfile_value(data, "SuggestRemoteName") {
        remote_name(&name)?;
    }
    Ok(summary)
}

#[cfg(feature = "test-util")]
pub fn repo_fixture(data: &[u8], root: &std::path::Path) -> Result<(), String> {
    if data.len() > MAX_REF_BYTES {
        return Err("Fixture descriptor too large".into());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "Fixture root unavailable")?;
    if !std::path::Path::new("/run/kiosque-test-vm").exists()
        || !root.starts_with("/var/tmp/kiosque-contract")
    {
        return Err("Local repositories require the isolated VM harness".into());
    }
    let url = crate::flatpak::keyfile_value(data, "Url").ok_or("Missing fixture URL")?;
    let path = reqwest::Url::parse(&url)
        .map_err(|_| "Invalid fixture URL")?
        .to_file_path()
        .map_err(|_| "Expected local fixture URL")?
        .canonicalize()
        .map_err(|_| "Fixture repository unavailable")?;
    if !path.starts_with(root) {
        return Err("Fixture repository escaped its private root".into());
    }
    let text = std::str::from_utf8(data).map_err(|_| "Invalid fixture descriptor")?;
    repo_file(
        text.replace(&format!("Url={url}"), "Url=https://example.org/fixture")
            .as_bytes(),
    )?;
    Ok(())
}

/// Bound reads through an opened regular-file descriptor; reject FIFOs and
/// symlinks without blocking on an attacker-controlled special file.
pub fn read_local_file(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Could not open the selected file")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Could not inspect the selected file")?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("Expected a regular file within the size limit".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the selected file")?;
    if bytes.len() > limit {
        return Err("Selected file exceeds its size limit".into());
    }
    Ok(bytes)
}

/// Format AppStream descriptions for a PlainText QML label. Text is never
/// interpreted as HTML by the renderer, including malformed/truncated markup.
/// Source indentation and line wrapping are dropped: paragraphs are separated
/// by a blank line and list items start on their own line with a bullet.
pub fn description_text(value: &str) -> String {
    use std::sync::OnceLock;
    static SPACE: OnceLock<regex::Regex> = OnceLock::new();
    static BLOCKS: OnceLock<regex::Regex> = OnceLock::new();
    static LINES: OnceLock<regex::Regex> = OnceLock::new();
    static ITEMS: OnceLock<regex::Regex> = OnceLock::new();
    static TAGS: OnceLock<regex::Regex> = OnceLock::new();
    static GAPS: OnceLock<regex::Regex> = OnceLock::new();
    let re = |cell: &'static OnceLock<regex::Regex>, pattern: &str| {
        cell.get_or_init(|| regex::Regex::new(pattern).unwrap())
    };
    let bounded: String = value.chars().take(32 * 1024).collect();
    let text = re(&SPACE, r"\s+").replace_all(&bounded, " ");
    let text = re(&BLOCKS, r"(?i)</(?:p|ul|ol|h[1-6])\s*>").replace_all(&text, "\n\n");
    let text = re(&LINES, r"(?i)<br\s*/?>").replace_all(&text, "\n");
    let text = re(&ITEMS, r"(?i)<li(?:\s[^>]*)?>").replace_all(&text, "\n• ");
    let plain = re(&TAGS, r"(?s)<[^>]*>")
        .replace_all(&text, "")
        .replace("&nbsp;", " ");
    let plain = quick_xml::escape::unescape(&plain)
        .map(|t| t.into_owned())
        .unwrap_or(plain);
    let lines: Vec<&str> = plain.lines().map(str::trim).collect();
    re(&GAPS, r"\n{3,}")
        .replace_all(&lines.join("\n"), "\n\n")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &[u8] =
        b"[Flatpak Ref]\nName=org.example.App\nUrl=https://example.org/repo\nGPGKey=dGVzdC1rZXk=\n";
    #[test]
    fn only_runtimes_are_removed_as_runtimes() {
        use crate::flatpak::Operation;
        let ok = Operation::RemoveRuntimes {
            refs: vec!["runtime/org.gnome.Platform/x86_64/48".into()],
        };
        assert!(operation(&ok, "Unused runtimes").is_ok());
        for refs in [
            vec![],
            vec!["app/org.example.App/x86_64/stable".to_string()],
            vec!["runtime/../x86_64/48".to_string()],
            vec!["org.gnome.Platform".to_string()],
        ] {
            assert!(operation(&Operation::RemoveRuntimes { refs }, "x").is_err());
        }
    }
    #[test]
    fn urls_reject_local_destinations_and_credentials() {
        for url in [
            "http://example.org",
            "https://127.0.0.1",
            "https://[::1]",
            "https://10.1.2.3",
            "https://100.64.0.1",
            "https://192.0.2.1",
            "https://localhost",
            "https://a.local",
            "https://a.internal",
            "https://user:pass@example.org",
            "https://example.org:444",
            "https://example.org/#fragment",
        ] {
            assert!(https_url(url).is_err(), "{url}");
        }
        assert!(https_url("https://dl.flathub.org/repo").is_ok());
    }
    #[test]
    fn references_require_a_single_unambiguous_signed_origin() {
        let summary = ref_file(VALID).unwrap();
        assert_eq!(summary.app_id, "org.example.App");
        assert_eq!(summary.key_sha256.len(), 64);
        for extra in [
            "Url=https://other.org\n",
            "[Other]\n",
            "GPGVerify=false\n",
            "RuntimeRepo=http://example.org\n",
            "Title=bad\u{1b}title\n",
        ] {
            let mut data = VALID.to_vec();
            data.extend(extra.as_bytes());
            assert!(ref_file(&data).is_err(), "{extra}");
        }
        assert!(
            ref_file(b"[Flatpak Ref]\nName=org.example.App\nUrl=https://example.org\n").is_err()
        );
        assert!(ref_file(&vec![b'a'; MAX_REF_BYTES + 1]).is_err());
        assert!(repo_file(b"[Flatpak Repo]\nUrl=https://example.org\nGPGKey=dGVzdA==\n").is_ok());
    }
    #[test]
    fn option_and_path_payloads_are_rejected() {
        for value in [
            "--command=sh",
            "../foo",
            "app//stable",
            "app/../stable",
            "a\nb",
            "",
        ] {
            assert!(app_ref(value).is_err());
        }
        assert!(app_ref("app/org.example.App/x86_64/stable").is_ok());
    }
    #[test]
    fn descriptions_are_bounded_text_even_when_markup_is_encoded() {
        assert_eq!(
            description_text("<p>First &amp; second</p><p>Next</p>"),
            "First & second\n\nNext"
        );
        // Source wrapping and indentation never reach the label.
        assert_eq!(
            description_text(
                "<p>\n    Kiosque is an\n      app   store.\n  </p>\n  <p>Features:</p>\n  <ul>\n    <li>Search</li>\n    <li>Updates</li>\n  </ul>\n  <p>End</p>"
            ),
            "Kiosque is an app store.\n\nFeatures:\n\n• Search\n• Updates\n\nEnd"
        );
        assert_eq!(
            description_text("&lt;img src='file:///private'&gt;"),
            "<img src='file:///private'>"
        );
        assert_eq!(description_text(&"x".repeat(100_000)).len(), 32 * 1024);
    }
}
