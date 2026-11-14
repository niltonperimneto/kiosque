//! `flatpak` command-line backend.
//!
//! Used when libflatpak is unavailable. Output is parsed from the
//! tab-separated `--columns` format, which is stable across flatpak versions
//! (unlike `--json`, which only some subcommands support).
//!
//! Every child process is spawned with `kill_on_drop(true)`, and cancellation
//! first sends `SIGTERM` (which `flatpak-spawn --host` forwards to the host
//! process) before falling back to `SIGKILL`.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::{
    FlatpakBackend, FlatpakError, InstalledApp, Operation, Progress, ProgressSink, RefKind,
    RemoteInfo, RemoteSource, Result, Scope, UpdateInfo,
};

/// Command routing belongs to the instance, so tests never replace PATH.
#[derive(Clone)]
pub struct CliBackend {
    program: std::path::PathBuf,
    prefix: Vec<String>,
}

impl Default for CliBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CliBackend {
    pub fn new() -> Self {
        Self::with_routing(super::in_sandbox())
    }

    pub fn with_routing(sandbox: bool) -> Self {
        if sandbox {
            Self::with_command("flatpak-spawn", vec!["--host".into(), "flatpak".into()])
        } else {
            Self::with_command("flatpak", vec![])
        }
    }

    pub fn with_command(program: impl Into<std::path::PathBuf>, prefix: Vec<String>) -> Self {
        Self {
            program: program.into(),
            prefix,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.prefix)
            .env("LC_ALL", "C")
            .kill_on_drop(true);
        command
    }
}

pub fn flatpak_command() -> Command {
    CliBackend::new().command()
}

async fn output(backend: &CliBackend, args: &[&str]) -> Result<String> {
    let out = backend
        .command()
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| {
            FlatpakError::Failed(format!(
                "failed to execute flatpak {}: {}",
                args.join(" "),
                e
            ))
        })?;
    if !out.status.success() {
        return Err(FlatpakError::Failed(format!(
            "flatpak {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[async_trait]
impl FlatpakBackend for CliBackend {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn list_installed(&self) -> Result<Vec<InstalledApp>> {
        let cols = "--columns=application,name,version,branch,arch,origin,installation,size";
        let app_args = ["list", "--app", cols];
        let runtime_args = ["list", "--runtime", cols];
        let (apps, runtimes) = tokio::join!(output(self, &app_args), output(self, &runtime_args));
        let mut all = parse_list(&apps?, RefKind::App);
        all.extend(parse_list(&runtimes.unwrap_or_default(), RefKind::Runtime));
        Ok(all)
    }

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        let cols = "--columns=application,name,version,origin,ref";
        let user_args = ["remote-ls", "--user", "--updates", cols];
        let system_args = ["remote-ls", "--system", "--updates", cols];
        let (user, system) = tokio::join!(output(self, &user_args), output(self, &system_args));
        if let (Err(e), Err(_)) = (&user, &system) {
            return Err(e.clone());
        }
        let mut all = parse_updates(&user.unwrap_or_default(), Scope::User);
        all.extend(parse_updates(&system.unwrap_or_default(), Scope::System));
        Ok(all)
    }

    async fn list_remotes(&self) -> Result<Vec<RemoteInfo>> {
        let out = output(
            self,
            &[
                "remotes",
                "--show-disabled",
                "--columns=name,title,url,description,options",
            ],
        )
        .await?;
        Ok(parse_remotes(&out))
    }

    async fn add_remote(&self, name: &str, source: RemoteSource, scope: Scope) -> Result<()> {
        match source {
            RemoteSource::Url(url) => {
                output(
                    self,
                    &[
                        "remote-add",
                        "--if-not-exists",
                        scope.cli_flag(),
                        name,
                        &url,
                    ],
                )
                .await?;
            }
            RemoteSource::FlatpakRepo(data) => {
                let path = write_temp("remote.flatpakrepo", &data).await?;
                let p = path.to_string_lossy().to_string();
                let res = output(
                    self,
                    &["remote-add", "--if-not-exists", scope.cli_flag(), name, &p],
                )
                .await;
                let _ = tokio::fs::remove_file(&path).await;
                res?;
            }
        }
        Ok(())
    }

    async fn remove_remote(&self, name: &str, scope: Option<Scope>) -> Result<()> {
        let scopes = match scope {
            Some(s) => vec![s],
            None => vec![Scope::User, Scope::System],
        };
        let mut last_err = None;
        for s in scopes {
            match output(self, &["remote-delete", s.cli_flag(), "--force", name]).await {
                Ok(_) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| FlatpakError::Failed("no installation".into())))
    }

    async fn run(
        &self,
        op: Operation,
        progress: ProgressSink,
        cancel: CancellationToken,
    ) -> Result<()> {
        if cancel.is_cancelled() {
            return Err(FlatpakError::Cancelled);
        }
        let mut temp = None;
        let mut args: Vec<String> = vec![];
        match &op {
            Operation::Install { remote, id, scope } => {
                args.extend([
                    "install".into(),
                    scope.cli_flag().into(),
                    remote.clone(),
                    id.clone(),
                ]);
            }
            Operation::InstallRefFile { data, scope } => {
                let path = write_temp("install.flatpakref", data).await?;
                args.extend([
                    "install".into(),
                    scope.cli_flag().into(),
                    "--from".into(),
                    path.to_string_lossy().into(),
                ]);
                temp = Some(path);
            }
            Operation::Uninstall { id, scope } => {
                args.push("uninstall".into());
                if let Some(s) = scope {
                    args.push(s.cli_flag().into());
                }
                args.push(id.clone());
            }
            Operation::Update { ids } => {
                args.push("update".into());
                args.extend(ids.iter().cloned());
            }
        }
        args.extend(["--assumeyes".into(), "--noninteractive".into()]);

        let res = run_streaming(self, &args, progress, cancel).await;
        if let Some(p) = temp {
            let _ = tokio::fs::remove_file(p).await;
        }
        res
    }

    async fn launch(&self, id: &str) -> Result<()> {
        self.command()
            .args(["run", id])
            .kill_on_drop(false)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| FlatpakError::Failed(format!("failed to launch {}: {}", id, e)))?;
        Ok(())
    }

    async fn refresh_appstream(&self, remote: &RemoteInfo) -> Result<()> {
        output(
            self,
            &[
                "update",
                "--appstream",
                remote.scope.cli_flag(),
                &remote.name,
            ],
        )
        .await?;
        Ok(())
    }
}

async fn write_temp(name: &str, data: &[u8]) -> Result<std::path::PathBuf> {
    // The cache dir lives under ~/.var/app/<id>/cache inside the sandbox,
    // which is the same path on the host, so `flatpak-spawn --host` sees it.
    let path = crate::disk_cache::cache_dir().join(format!("{}-{}", std::process::id(), name));
    tokio::fs::write(&path, data)
        .await
        .map_err(|e| FlatpakError::Failed(format!("failed to write {}: {}", path.display(), e)))?;
    Ok(path)
}

/// Spawn `flatpak <args>`, turn its output into [`Progress`] reports and
/// honour `cancel`.
async fn run_streaming(
    backend: &CliBackend,
    args: &[String],
    progress: ProgressSink,
    cancel: CancellationToken,
) -> Result<()> {
    let mut child = backend
        .command()
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            FlatpakError::Failed(format!("failed to execute flatpak {}: {}", args[0], e))
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let reader = tokio::spawn(read_progress(stdout, progress));
    let err_reader = tokio::spawn(async move {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut s).await;
        }
        s
    });

    let status = tokio::select! {
        status = child.wait() => status,
        _ = cancel.cancelled() => {
            terminate(&mut child).await;
            reader.abort();
            err_reader.abort();
            let _ = reader.await;
            let _ = err_reader.await;
            return Err(FlatpakError::Cancelled);
        }
    };
    let _ = reader.await;
    let stderr = err_reader.await.unwrap_or_default();
    let status =
        status.map_err(|e| FlatpakError::Failed(format!("failed to wait for flatpak: {}", e)))?;
    if status.success() {
        Ok(())
    } else {
        Err(FlatpakError::Failed(format!(
            "flatpak {} failed: {}",
            args[0],
            stderr.trim()
        )))
    }
}

/// SIGTERM, give it a moment to clean up, then SIGKILL.
async fn terminate(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        // SAFETY: plain kill(2) on a pid we own.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
        if tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .is_ok()
        {
            return;
        }
    }
    let _ = child.kill().await;
}

async fn read_progress<R: AsyncRead + Unpin>(stream: Option<R>, progress: ProgressSink) {
    let Some(mut stream) = stream else { return };
    let mut tracker = ProgressParser::default();
    let mut buf = [0u8; 4096];
    let mut pending = String::new();
    loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.push_str(&String::from_utf8_lossy(&buf[..n]));
        // flatpak redraws its progress line with '\r'; treat it as a newline.
        while let Some(pos) = pending.find(['\n', '\r']) {
            let line: String = pending.drain(..=pos).collect();
            if let Some(p) = tracker.feed(line.trim()) {
                progress(p);
            }
        }
    }
}

/// Turns `flatpak install/update` output lines into overall progress.
///
/// Recognises step headers such as `Installing 2/3…` and percentages such as
/// `… 45%`, combining them into a fraction of the whole transaction.
#[derive(Default)]
pub struct ProgressParser {
    step: usize,
    steps: usize,
    last: f64,
}

impl ProgressParser {
    pub fn feed(&mut self, line: &str) -> Option<Progress> {
        if line.is_empty() {
            return None;
        }
        if let Some((i, n)) = parse_step(line) {
            self.step = i;
            self.steps = n;
        }
        let pct = parse_percent(line);
        let fraction = match (pct, self.steps) {
            (Some(p), n) if n > 0 => ((self.step.saturating_sub(1)) as f64 + p / 100.0) / n as f64,
            (Some(p), _) => p / 100.0,
            (None, n) if n > 0 => (self.step.saturating_sub(1)) as f64 / n as f64,
            (None, _) => -1.0,
        };
        // Never move backwards; flatpak re-prints lines.
        let fraction = if fraction >= 0.0 {
            self.last = self.last.max(fraction.min(1.0));
            self.last
        } else if self.last > 0.0 {
            self.last
        } else {
            -1.0
        };
        Some(Progress {
            fraction,
            status: line.to_string(),
            current_ref: String::new(),
        })
    }
}

fn parse_step(line: &str) -> Option<(usize, usize)> {
    // e.g. "Installing 2/3…", "Updating 1/1… ████ 45%"
    line.split_whitespace().find_map(|tok| {
        let tok = tok.trim_end_matches(['…', '.', ':']);
        let (a, b) = tok.split_once('/')?;
        let (a, b) = (a.parse::<usize>().ok()?, b.parse::<usize>().ok()?);
        (a >= 1 && b >= a && b < 10_000).then_some((a, b))
    })
}

fn parse_percent(line: &str) -> Option<f64> {
    line.split_whitespace().rev().find_map(|tok| {
        let n = tok.strip_suffix('%')?.parse::<f64>().ok()?;
        (0.0..=100.0).contains(&n).then_some(n)
    })
}

/// Parse a human size such as `565.4 MB` or `1,2 GB` into bytes.
pub fn parse_size(s: &str) -> u64 {
    let s = s.trim().replace(',', ".").replace('\u{a0}', " ");
    let mut parts = s.split_whitespace();
    let num: f64 = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
    let mult = match parts.next().map(|u| u.to_ascii_lowercase()) {
        Some(u) if u.starts_with('t') => 1e12,
        Some(u) if u.starts_with('g') => 1e9,
        Some(u) if u.starts_with('m') => 1e6,
        Some(u) if u.starts_with('k') => 1e3,
        _ => 1.0,
    };
    (num * mult) as u64
}

fn cols(line: &str) -> Vec<&str> {
    line.split('\t').map(str::trim).collect()
}

pub fn parse_list(out: &str, kind: RefKind) -> Vec<InstalledApp> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let c = cols(l);
            let id = c.first()?.to_string();
            if id.is_empty() {
                return None;
            }
            let get = |i: usize| c.get(i).map(|s| s.to_string()).unwrap_or_default();
            Some(InstalledApp {
                name: if get(1).is_empty() {
                    id.clone()
                } else {
                    get(1)
                },
                id,
                version: get(2),
                branch: get(3),
                arch: get(4),
                origin: get(5),
                scope: Scope::parse(&get(6)).unwrap_or(Scope::System),
                installed_size: parse_size(&get(7)),
                kind,
            })
        })
        .collect()
}

pub fn parse_updates(out: &str, scope: Scope) -> Vec<UpdateInfo> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let c = cols(l);
            let get = |i: usize| c.get(i).map(|s| s.to_string()).unwrap_or_default();
            let id = get(0);
            if id.is_empty() {
                return None;
            }
            let ref_string = get(4);
            let kind = if ref_string.starts_with("runtime/") {
                RefKind::Runtime
            } else {
                RefKind::App
            };
            Some(UpdateInfo {
                name: if get(1).is_empty() {
                    id.clone()
                } else {
                    get(1)
                },
                id,
                version: get(2),
                origin: get(3),
                scope,
                kind,
                ref_string,
            })
        })
        .collect()
}

pub fn parse_remotes(out: &str) -> Vec<RemoteInfo> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let c = cols(l);
            let get = |i: usize| c.get(i).map(|s| s.to_string()).unwrap_or_default();
            let name = get(0);
            if name.is_empty() {
                return None;
            }
            let options = get(4);
            let opts: Vec<&str> = options.split(',').map(str::trim).collect();
            Some(RemoteInfo {
                title: get(1),
                url: get(2),
                description: get(3),
                scope: if opts.contains(&"user") {
                    Scope::User
                } else {
                    Scope::System
                },
                disabled: opts.contains(&"disabled"),
                noenumerate: opts.contains(&"no-enumerate"),
                name,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_parsing() {
        let out = "org.mozilla.firefox\tFirefox\t133.0\tstable\tx86_64\tflathub\tsystem\t265.1 MB\n\
                   org.gnome.Calculator\tCalculator\t\tstable\tx86_64\tflathub\tuser\t3,4 MB\n";
        let apps = parse_list(out, RefKind::App);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].id, "org.mozilla.firefox");
        assert_eq!(apps[0].scope, Scope::System);
        assert_eq!(apps[0].installed_size, 265_100_000);
        assert_eq!(apps[1].scope, Scope::User);
        assert_eq!(apps[1].version, "");
        assert_eq!(apps[1].installed_size, 3_400_000);
    }

    #[test]
    fn update_parsing() {
        let out = "org.kde.Platform\tKDE Application Platform\t\tflathub\truntime/org.kde.Platform/x86_64/6.8\n";
        let ups = parse_updates(out, Scope::User);
        assert_eq!(ups.len(), 1);
        assert_eq!(ups[0].kind, RefKind::Runtime);
        assert_eq!(ups[0].scope, Scope::User);
    }

    #[test]
    fn remote_parsing() {
        let out = "flathub\tFlathub\thttps://dl.flathub.org/repo/\tCentral repository\tsystem\n\
                   beta\tFlathub beta\thttps://dl.flathub.org/beta-repo/\t\tuser,disabled\n";
        let r = parse_remotes(out);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].scope, Scope::System);
        assert!(!r[0].disabled);
        assert_eq!(r[1].scope, Scope::User);
        assert!(r[1].disabled);
    }

    #[test]
    fn progress_parsing() {
        let mut p = ProgressParser::default();
        assert_eq!(p.feed("Looking for matches…").unwrap().fraction, -1.0);
        assert_eq!(p.feed("Installing 1/2… 50%").unwrap().fraction, 0.25);
        assert_eq!(p.feed("Installing 2/2… 100%").unwrap().fraction, 1.0);
        // never goes backwards
        assert_eq!(p.feed("Installing 2/2… 10%").unwrap().fraction, 1.0);
        assert!(p.feed("").is_none());
    }

    #[test]
    fn size_parsing() {
        assert_eq!(parse_size("1.5 GB"), 1_500_000_000);
        assert_eq!(parse_size("12 kB"), 12_000);
        assert_eq!(parse_size(""), 0);
    }
}
