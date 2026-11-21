//! Flatpak backend abstraction.
//!
//! Every Flatpak operation in Kiosque goes through the [`FlatpakBackend`] trait.
//! Two implementations exist:
//!
//! * [`native::LibflatpakBackend`] (feature `libflatpak`, the default) uses
//!   libflatpak directly: real `FlatpakTransaction` progress, `GCancellable`
//!   based cancellation and `FlatpakInstallation` file monitors.
//! * [`cli::CliBackend`] shells out to the `flatpak` binary. It is used when
//!   libflatpak is unavailable (feature disabled or initialisation failed).
//!
//! A [`mock::MockBackend`] is available to unit tests.

pub mod broker;
pub mod cli;
#[cfg(any(test, feature = "test-util"))]
pub mod mock;
#[cfg(feature = "libflatpak")]
pub mod native;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

// ── Shared data types ───────────────────────────────────────────────────────

/// Which Flatpak installation a ref lives in (or should be installed into).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Scope {
    #[default]
    User,
    System,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<Scope> {
        match s.trim().to_ascii_lowercase().as_str() {
            "user" => Some(Scope::User),
            "system" | "default" => Some(Scope::System),
            _ => None,
        }
    }

    /// The `flatpak` CLI flag selecting this installation.
    pub fn cli_flag(self) -> &'static str {
        match self {
            Scope::User => "--user",
            Scope::System => "--system",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RefKind {
    #[default]
    App,
    Runtime,
}

/// An installed application or runtime.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub branch: String,
    pub arch: String,
    pub origin: String,
    pub scope: Scope,
    pub kind: RefKind,
    /// Installed size in bytes (0 when unknown).
    pub installed_size: u64,
}

impl InstalledApp {
    pub fn is_runtime(&self) -> bool {
        self.kind == RefKind::Runtime
    }

    pub fn ref_string(&self) -> String {
        format_ref(self.kind, &self.id, &self.arch, &self.branch)
    }
}

/// An installed ref for which a newer commit is available.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub origin: String,
    pub scope: Scope,
    pub kind: RefKind,
    pub ref_string: String,
}

/// A configured remote repository.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub scope: Scope,
    pub disabled: bool,
    pub noenumerate: bool,
}

/// Where a new remote comes from.
#[derive(Debug, Clone)]
pub enum RemoteSource {
    /// Only dedicated test binaries can access signed local fixture repositories.
    #[cfg(feature = "test-util")]
    LocalFixture { data: Vec<u8>, root: PathBuf },
    /// Either a plain repo URL or a URL to a `.flatpakrepo` file.
    Url(String),
    /// The contents of a `.flatpakrepo` file.
    FlatpakRepo(Vec<u8>),
}

/// A unit of work for [`FlatpakBackend::run`].
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    /// Install `id` (an app id or a full ref) from `remote`.
    Install {
        remote: String,
        id: String,
        scope: Scope,
    },
    /// Install from the contents of a `.flatpakref` file.
    InstallRefFile { data: Vec<u8>, scope: Scope },
    /// Uninstall `id`. When `scope` is `None` the installation is looked up.
    Uninstall { id: String, scope: Option<Scope> },
    /// Update the given ids; an empty list means "update everything".
    Update { ids: Vec<String> },
}

impl Operation {
    /// The app ids this operation is about (empty for "update all"/ref files).
    pub fn app_ids(&self) -> Vec<String> {
        match self {
            Operation::Install { id, .. } | Operation::Uninstall { id, .. } => vec![id.clone()],
            Operation::Update { ids } => ids.clone(),
            Operation::InstallRefFile { .. } => vec![],
        }
    }
}

/// A progress report emitted while an [`Operation`] runs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    /// Overall completion in `0.0..=1.0`, or a negative value when unknown.
    pub fraction: f64,
    /// Human readable status line from flatpak.
    pub status: String,
    /// The ref currently being processed, if known.
    pub current_ref: String,
}

pub type ProgressSink = Arc<dyn Fn(Progress) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FlatpakError {
    #[error("operation cancelled")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}

impl From<String> for FlatpakError {
    fn from(s: String) -> Self {
        FlatpakError::Failed(s)
    }
}

pub type Result<T> = std::result::Result<T, FlatpakError>;

// ── Backend trait ───────────────────────────────────────────────────────────

// async-trait emits must_use on futures that are already must_use.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait FlatpakBackend: Send + Sync {
    /// Short identifier used in logs ("libflatpak", "cli", "mock").
    fn name(&self) -> &'static str;

    async fn list_installed(&self) -> Result<Vec<InstalledApp>>;

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>>;

    async fn list_remotes(&self) -> Result<Vec<RemoteInfo>>;

    async fn add_remote(&self, name: &str, source: RemoteSource, scope: Scope) -> Result<()>;

    async fn remove_remote(&self, name: &str, scope: Option<Scope>) -> Result<()>;

    /// Run a transaction, reporting progress through `progress`. Must return
    /// [`FlatpakError::Cancelled`] promptly once `cancel` fires, and must not
    /// leave any helper process running afterwards.
    async fn run(
        &self,
        op: Operation,
        progress: ProgressSink,
        cancel: CancellationToken,
    ) -> Result<()>;

    async fn launch(&self, id: &str) -> Result<()>;

    /// Refresh the AppStream metadata of a remote on disk.
    async fn refresh_appstream(&self, remote: &RemoteInfo) -> Result<()>;

    /// Start watching the installations for changes, sending `()` on `tx`
    /// whenever something was installed, removed or updated (by Kiosque or by
    /// any other tool). The default implementation polls the `.changed`
    /// marker file flatpak touches after every modification.
    fn spawn_monitor(&self, tx: broadcast::Sender<()>) {
        spawn_polling_monitor(tx);
    }
}

// ── Global backend ──────────────────────────────────────────────────────────

static BACKEND: OnceLock<Arc<dyn FlatpakBackend>> = OnceLock::new();

/// The process-wide backend: libflatpak when available, CLI otherwise.
pub fn backend() -> Arc<dyn FlatpakBackend> {
    BACKEND
        .get_or_init(|| {
            let b = select_backend();
            klog!("flatpak backend: {}", b.name());
            b
        })
        .clone()
}

fn select_backend() -> Arc<dyn FlatpakBackend> {
    if in_sandbox() {
        return Arc::new(broker::BrokerBackend);
    }
    if std::env::var_os("KIOSQUE_FLATPAK_BACKEND").is_some_and(|v| v == "cli") {
        return Arc::new(cli::CliBackend::new());
    }
    #[cfg(feature = "libflatpak")]
    {
        match native::LibflatpakBackend::new() {
            Ok(b) => return Arc::new(b),
            Err(e) => kerr!(
                "libflatpak unavailable ({}), falling back to the flatpak CLI",
                e
            ),
        }
    }
    Arc::new(cli::CliBackend::new())
}

// ── Installation-change notifications ───────────────────────────────────────

/// Broadcast channel fired whenever any Flatpak installation changes.
pub fn installation_changes() -> broadcast::Receiver<()> {
    change_sender().subscribe()
}

fn change_sender() -> &'static broadcast::Sender<()> {
    static TX: OnceLock<broadcast::Sender<()>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        backend().spawn_monitor(tx.clone());
        tx
    })
}

/// Start the installation monitor (idempotent).
pub fn start_monitor() {
    let _ = change_sender();
}

/// Default monitor: poll the mtime of `<installation>/.changed` every 2 s.
pub fn spawn_polling_monitor(tx: broadcast::Sender<()>) {
    crate::runtime::runtime().spawn(async move {
        let markers: Vec<PathBuf> = installation_paths()
            .into_iter()
            .map(|(_, p)| p.join(".changed"))
            .collect();
        let stamp = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let mut last: Vec<_> = markers.iter().map(stamp).collect();
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            let now: Vec<_> = markers.iter().map(stamp).collect();
            if now != last {
                last = now;
                let _ = tx.send(());
            }
        }
    });
}

// ── Environment helpers ─────────────────────────────────────────────────────

/// True when running inside a Flatpak sandbox.
pub fn in_sandbox() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Paths of the user and system installations *on the host*. Inside the
/// sandbox `$XDG_DATA_HOME` points at the app's private data dir, so the
/// host's location is used instead.
pub fn installation_paths() -> Vec<(Scope, PathBuf)> {
    let user_data = std::env::var_os("HOST_XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|_| in_sandbox())
        .or_else(|| {
            if in_sandbox() {
                dirs::home_dir().map(|h| h.join(".local/share"))
            } else {
                dirs::data_dir()
            }
        })
        .unwrap_or_else(|| PathBuf::from("~/.local/share"));
    let system = std::env::var_os("FLATPAK_SYSTEM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/flatpak"));
    let user = std::env::var_os("FLATPAK_USER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| user_data.join("flatpak"));
    vec![(Scope::User, user), (Scope::System, system)]
}

/// The default architecture string flatpak uses (`x86_64`, `aarch64`, …).
pub fn default_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "i386",
        "arm" => "arm",
        other => other,
    }
}

pub fn format_ref(kind: RefKind, id: &str, arch: &str, branch: &str) -> String {
    let k = match kind {
        RefKind::App => "app",
        RefKind::Runtime => "runtime",
    };
    format!("{}/{}/{}/{}", k, id, arch, branch)
}

/// Parse `app/org.foo.Bar/x86_64/stable` into `(kind, id, arch, branch)`.
pub fn parse_ref(r: &str) -> Option<(RefKind, String, String, String)> {
    let mut parts = r.splitn(4, '/');
    let kind = match parts.next()? {
        "app" => RefKind::App,
        "runtime" => RefKind::Runtime,
        _ => return None,
    };
    let id = parts.next()?.to_string();
    let arch = parts.next()?.to_string();
    let branch = parts.next()?.to_string();
    Some((kind, id, arch, branch))
}

/// Read the `Name=` key of a `.flatpakref`/`.flatpakrepo` keyfile.
pub fn keyfile_value(data: &[u8], key: &str) -> Option<String> {
    let text = std::str::from_utf8(data).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().to_string())
    })
}

/// What a `.flatpakref` or `.flatpakrepo` file asks for, shown to the user
/// before anything is installed or added.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyfileSummary {
    /// `Title=`, falling back to `Name=`.
    pub title: String,
    /// `Name=` (application id; empty for `.flatpakrepo`).
    pub name: String,
    /// `Url=`: the repository the file points at.
    pub url: String,
    /// `Branch=` (empty when absent).
    pub branch: String,
    /// `IsRuntime=true`.
    pub is_runtime: bool,
    /// `SuggestRemoteName=` (empty when absent).
    pub suggested_remote_name: String,
}

/// Summarize a `.flatpakref`/`.flatpakrepo` keyfile. Control characters are
/// stripped so a crafted file cannot spoof the confirmation text.
pub fn summarize_keyfile(data: &[u8]) -> KeyfileSummary {
    let get = |key: &str| {
        keyfile_value(data, key)
            .map(|v| v.chars().filter(|c| !c.is_control()).collect::<String>())
            .unwrap_or_default()
    };
    let name = get("Name");
    let title = Some(get("Title"))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| name.clone());
    KeyfileSummary {
        title,
        name,
        url: get("Url"),
        branch: get("Branch"),
        is_runtime: get("IsRuntime").eq_ignore_ascii_case("true"),
        suggested_remote_name: get("SuggestRemoteName"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_roundtrip() {
        let r = format_ref(RefKind::App, "org.foo.Bar", "x86_64", "stable");
        assert_eq!(r, "app/org.foo.Bar/x86_64/stable");
        let (k, id, a, b) = parse_ref(&r).unwrap();
        assert_eq!(
            (k, id.as_str(), a.as_str(), b.as_str()),
            (RefKind::App, "org.foo.Bar", "x86_64", "stable")
        );
        assert!(parse_ref("org.foo.Bar").is_none());
    }

    #[test]
    fn scope_parse() {
        assert_eq!(Scope::parse("User"), Some(Scope::User));
        assert_eq!(Scope::parse("system"), Some(Scope::System));
        assert_eq!(Scope::parse("bogus"), None);
    }

    #[test]
    fn keyfile_lookup() {
        let data = b"[Flatpak Ref]\nName=org.foo.Bar\nBranch=stable\n";
        assert_eq!(keyfile_value(data, "Name").as_deref(), Some("org.foo.Bar"));
        assert_eq!(keyfile_value(data, "Url"), None);
    }

    #[test]
    fn keyfile_summary_prefers_title_and_strips_control_characters() {
        let data = b"[Flatpak Ref]\nName=org.foo.Bar\nTitle=Foo\x1b[2J Bar\nUrl=https://example.org/repo/\nBranch=stable\nIsRuntime=False\n";
        let summary = summarize_keyfile(data);
        assert_eq!(summary.title, "Foo[2J Bar");
        assert_eq!(summary.name, "org.foo.Bar");
        assert_eq!(summary.url, "https://example.org/repo/");
        assert_eq!(summary.branch, "stable");
        assert!(!summary.is_runtime);

        let repo = b"[Flatpak Repo]\nUrl=https://example.org/repo/\nSuggestRemoteName=example\n";
        let summary = summarize_keyfile(repo);
        assert_eq!(summary.title, "");
        assert_eq!(summary.suggested_remote_name, "example");
        assert!(summarize_keyfile(b"[Flatpak Ref]\nName=a.b.C\nIsRuntime=true\n").is_runtime);
    }
}
