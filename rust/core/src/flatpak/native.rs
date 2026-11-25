//! libflatpak backend.
//!
//! libflatpak objects are not `Send`, so every call creates its own
//! `FlatpakInstallation` on a blocking worker thread. Transactions report
//! real progress through `FlatpakTransactionProgress::changed` and are
//! cancelled through a `GCancellable` bridged from the caller's
//! [`CancellationToken`].
//!
//! Note: depending on the flatpak version, progress `changed` signals may be
//! dispatched on whichever thread iterates the global default GLib context
//! (with Qt's GLib event dispatcher that is the GUI thread). All signal
//! closures therefore only capture thread-safe state.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use gio::glib;
use gio::prelude::*;
use libflatpak::prelude::*;
use libflatpak::{Installation, InstalledRef, Remote, Transaction};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::{
    FlatpakBackend, FlatpakError, InstalledApp, InstalledRuntime, Operation, OperationPlan,
    PlannedStep, Progress, ProgressSink, RefKind, RemoteInfo, RemoteSource, Result, Scope,
    StepAction, UpdateInfo,
};

pub struct LibflatpakBackend;

fn none() -> Option<&'static gio::Cancellable> {
    gio::Cancellable::NONE
}

fn gerr(e: glib::Error) -> FlatpakError {
    if e.matches(gio::IOErrorEnum::Cancelled) {
        FlatpakError::Cancelled
    } else {
        FlatpakError::Failed(e.message().to_string())
    }
}

fn installation(scope: Scope) -> Result<Installation> {
    let variable = match scope {
        Scope::User => "FLATPAK_USER_DIR",
        Scope::System => "FLATPAK_SYSTEM_DIR",
    };
    if let Some(path) = std::env::var_os(variable) {
        // libflatpak can construct a lazy installation even for an invalid path.
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(FlatpakError::Failed(format!(
                    "{variable} is not an accessible installation directory"
                )));
            }
        }
        return Installation::for_path(&gio::File::for_path(path), scope == Scope::User, none())
            .map_err(gerr);
    }
    match scope {
        Scope::System => Installation::new_system(none()),
        Scope::User => Installation::new_user(none()),
    }
    .map_err(gerr)
}

/// All installations that could be opened, user first.
fn installations() -> Result<Vec<(Scope, Installation)>> {
    let available: Vec<_> = [Scope::User, Scope::System]
        .into_iter()
        .filter_map(|s| match installation(s) {
            Ok(i) => Some((s, i)),
            Err(e) => {
                kerr!("libflatpak: cannot open {} installation: {}", s.as_str(), e);
                None
            }
        })
        .collect();
    if available.is_empty() {
        return Err(FlatpakError::Failed(
            "No Flatpak installation could be opened".into(),
        ));
    }
    Ok(available)
}

async fn blocking<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| FlatpakError::Failed(format!("worker thread failed: {}", e)))?
}

fn kind_of(k: libflatpak::RefKind) -> RefKind {
    match k {
        libflatpak::RefKind::Runtime => RefKind::Runtime,
        _ => RefKind::App,
    }
}

fn s(v: Option<glib::GString>) -> String {
    v.map(|g| g.to_string()).unwrap_or_default()
}

fn installed_from(r: &InstalledRef, scope: Scope) -> InstalledApp {
    let id = s(r.name());
    let name = r
        .appdata_name()
        .map(|g| g.to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    InstalledApp {
        id,
        name,
        version: s(r.appdata_version()),
        branch: s(r.branch()),
        arch: s(r.arch()),
        origin: s(r.origin()),
        scope,
        kind: kind_of(r.kind()),
        installed_size: r.installed_size(),
    }
}

/// Find the installation (and full ref) an id is installed in.
fn find_installed(id: &str, scope: Option<Scope>) -> Result<(Installation, String)> {
    for (sc, inst) in installations()? {
        if scope.is_some_and(|want| want != sc) {
            continue;
        }
        let refs = inst.list_installed_refs(none()).map_err(gerr)?;
        if let Some(r) = refs
            .iter()
            .find(|r| s(r.name()) == id || s(r.format_ref()) == id)
        {
            return Ok((inst, s(r.format_ref())));
        }
    }
    Err(FlatpakError::Failed(format!("{} is not installed", id)))
}

/// Resolve an app id to a full ref available on `remote`, preferring the
/// `stable` branch.
fn resolve_remote_ref(
    inst: &Installation,
    remote: &str,
    id: &str,
    cancellable: &gio::Cancellable,
) -> Result<String> {
    if id.contains('/') {
        return Ok(id.to_string());
    }
    let arch = super::default_arch();
    let refs = inst
        .list_remote_refs_sync(remote, Some(cancellable))
        .map_err(gerr)?;
    let mut candidates: Vec<String> = refs
        .iter()
        .filter(|r| s(r.name()) == id && s(r.arch()) == arch)
        .map(|r| s(r.format_ref()))
        .collect();
    candidates.sort_by_key(|r| (!r.starts_with("app/"), !r.ends_with("/stable")));
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| FlatpakError::Failed(format!("{} was not found in remote {}", id, remote)))
}

/// Tracks transaction-wide progress across operations. Thread-safe because
/// progress signals can arrive on another thread (see module docs).
struct Tracker {
    sink: ProgressSink,
    /// Index of the current operation, 0-based.
    index: AtomicUsize,
    total: AtomicUsize,
    /// Fraction of the whole job taken by earlier transactions (update-all
    /// runs one transaction per installation).
    base: Mutex<(f64, f64)>,
}

impl Tracker {
    fn report(&self, op_fraction: f64, status: String, current_ref: String) {
        let total = self.total.load(Ordering::SeqCst).max(1) as f64;
        let index = self.index.load(Ordering::SeqCst) as f64;
        let (base, span) = *self.base.lock().unwrap();
        let fraction = base + span * ((index + op_fraction.clamp(0.0, 1.0)) / total).min(1.0);
        (self.sink)(Progress {
            fraction,
            status,
            current_ref,
        });
    }
}

fn run_transaction(
    txn: &Transaction,
    tracker: Arc<Tracker>,
    cancellable: &gio::Cancellable,
    keep_going: bool,
    allowed_origin: Option<String>,
) -> Result<()> {
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

    {
        let tracker = tracker.clone();
        txn.connect_ready(move |t| {
            tracker.total.store(t.operations().len(), Ordering::SeqCst);
            tracker.index.store(0, Ordering::SeqCst);
            true
        });
    }
    {
        let tracker = tracker.clone();
        txn.connect_new_operation(move |_, op, progress| {
            let op_ref = s(op.get_ref());
            let action = step_action(op.operation_type());
            let (index, total) = (
                tracker.index.load(Ordering::SeqCst),
                tracker.total.load(Ordering::SeqCst),
            );
            tracker.report(
                0.0,
                super::step_status(index, total, action, &op_ref, None),
                op_ref.clone(),
            );
            let tracker = tracker.clone();
            progress.connect_changed(move |p| {
                let percent = p.progress().clamp(0, 100) as u32;
                let status = super::step_status(index, total, action, &op_ref, Some(percent));
                tracker.report(percent as f64 / 100.0, status, op_ref.clone());
            });
        });
    }
    {
        let tracker = tracker.clone();
        // A removal has no resulting commit. The libflatpak 0.7 typed
        // trampoline unconditionally borrows that nullable C string.
        txn.connect("operation-done", false, move |args| {
            let op = args[1]
                .get::<libflatpak::TransactionOperation>()
                .expect("operation-done operation");
            tracker.report(1.0, String::new(), s(op.get_ref()));
            tracker.index.fetch_add(1, Ordering::SeqCst);
            None
        });
    }
    {
        let errors = errors.clone();
        txn.connect_operation_error(move |_, op, err, _| {
            if err.matches(libflatpak::Error::AlreadyInstalled)
                || err.matches(libflatpak::Error::Skipped)
            {
                return true;
            }
            errors
                .lock()
                .unwrap()
                .push(format!("{}: {}", s(op.get_ref()), err.message()));
            keep_going
        });
    }
    // Never accept an unreviewed dependency repository.
    txn.connect_add_new_remote(move |_, _, _, _, url| {
        crate::security::https_url(url)
            .ok()
            .is_some_and(|url| allowed_origin.as_deref() == Some(url.as_str()))
    });

    let res = txn.run(Some(cancellable));
    if cancellable.is_cancelled() {
        return Err(FlatpakError::Cancelled);
    }
    let errs = errors.lock().unwrap().clone();
    match res {
        Err(e) => {
            let e = gerr(e);
            if errs.is_empty() {
                Err(e)
            } else {
                Err(FlatpakError::Failed(errs.join("\n")))
            }
        }
        Ok(()) if !errs.is_empty() => Err(FlatpakError::Failed(errs.join("\n"))),
        Ok(()) => Ok(()),
    }
}

fn new_transaction(inst: &Installation, cancellable: &gio::Cancellable) -> Result<Transaction> {
    let txn = Transaction::for_installation(inst, Some(cancellable)).map_err(gerr)?;
    // Let user installs reuse runtimes from the system installation.
    txn.add_default_dependency_sources();
    Ok(txn)
}

fn step_action(kind: libflatpak::TransactionOperationType) -> StepAction {
    match kind {
        libflatpak::TransactionOperationType::Update => StepAction::Update,
        libflatpak::TransactionOperationType::Uninstall => StepAction::Uninstall,
        _ => StepAction::Install,
    }
}

/// The installations holding each of `refs`, with the refs to remove there.
fn group_installed(refs: &[String]) -> Result<Vec<(Installation, Vec<String>)>> {
    let mut groups: Vec<(Scope, Installation, Vec<String>)> = vec![];
    for reference in refs {
        let (inst, full_ref) = find_installed(reference, None)?;
        let scope = if inst.is_user() {
            Scope::User
        } else {
            Scope::System
        };
        match groups.iter_mut().find(|g| g.0 == scope) {
            Some(group) => group.2.push(full_ref),
            None => groups.push((scope, inst, vec![full_ref])),
        }
    }
    Ok(groups
        .into_iter()
        .map(|(_, inst, refs)| (inst, refs))
        .collect())
}

/// A transaction for `op` (single installation operations only), plus the
/// repository origin it may add (for `.flatpakref` files).
fn transaction_for(
    op: &Operation,
    cancellable: &gio::Cancellable,
    plan_unused: bool,
) -> Result<Vec<(Transaction, Option<String>)>> {
    match op {
        Operation::Install { remote, id, scope } => {
            let inst = installation(*scope)?;
            let full_ref = resolve_remote_ref(&inst, remote, id, cancellable)?;
            let txn = new_transaction(&inst, cancellable)?;
            txn.add_install(remote, &full_ref, &[]).map_err(gerr)?;
            Ok(vec![(txn, None)])
        }
        Operation::InstallRefFile { data, scope } => {
            let summary = crate::security::ref_file(data).map_err(FlatpakError::Failed)?;
            let inst = installation(*scope)?;
            let txn = new_transaction(&inst, cancellable)?;
            // Runtimes and related refs come from repositories that are
            // already configured; only the confirmed origin may be added.
            txn.add_install_flatpakref(&glib::Bytes::from_owned(data.clone()))
                .map_err(gerr)?;
            Ok(vec![(txn, Some(summary.origin))])
        }
        Operation::Uninstall { id, scope } => {
            let (inst, full_ref) = find_installed(id, *scope)?;
            let txn = new_transaction(&inst, cancellable)?;
            if plan_unused {
                // Lists the runtimes this removal would leave unused.
                txn.set_include_unused_uninstall_ops(true);
            }
            txn.add_uninstall(&full_ref).map_err(gerr)?;
            Ok(vec![(txn, None)])
        }
        Operation::RemoveRuntimes { refs } => group_installed(refs)?
            .into_iter()
            .map(|(inst, refs)| {
                let txn = new_transaction(&inst, cancellable)?;
                for r in &refs {
                    txn.add_uninstall(r).map_err(gerr)?;
                }
                Ok((txn, None))
            })
            .collect(),
        Operation::Update { .. } => Err(FlatpakError::Failed(
            "updates are planned per installation".into(),
        )),
    }
}

/// Resolve `op` without downloading or changing anything.
fn plan_blocking(op: Operation) -> Result<OperationPlan> {
    crate::security::operation(&op, "").map_err(FlatpakError::Failed)?;
    if matches!(op, Operation::InstallRefFile { .. }) {
        // Resolving a reference file adds its origin repository before the
        // transaction is ready, so it cannot be planned without consent.
        return Err(FlatpakError::Failed(
            "reference files cannot be planned before they are confirmed".into(),
        ));
    }
    let cancellable = gio::Cancellable::new();
    let mut steps = vec![];
    for (txn, _origin) in transaction_for(&op, &cancellable, true)? {
        let collected: Arc<Mutex<Option<Vec<PlannedStep>>>> = Arc::new(Mutex::new(None));
        let eol: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(vec![]));
        {
            let eol = eol.clone();
            txn.connect_end_of_lifed(move |_, reference, reason, rebase| {
                let note = if rebase.is_empty() {
                    reason.to_string()
                } else {
                    format!("{reason} (replaced by {rebase})")
                };
                eol.lock().unwrap().push((reference.to_string(), note));
            });
        }
        {
            let collected = collected.clone();
            txn.connect_ready(move |t| {
                let steps = t
                    .operations()
                    .iter()
                    .filter(|op| !op.is_skipped())
                    .map(|op| {
                        let reference = s(op.get_ref());
                        PlannedStep {
                            kind: if reference.starts_with("runtime/") {
                                RefKind::Runtime
                            } else {
                                RefKind::App
                            },
                            action: step_action(op.operation_type()),
                            remote: s(op.remote()),
                            download_size: op.download_size(),
                            installed_size: op.installed_size(),
                            reference,
                            eol: String::new(),
                        }
                    })
                    .collect();
                *collected.lock().unwrap() = Some(steps);
                // Stop before anything is downloaded or changed.
                false
            });
        }
        // Planning never adds repositories.
        txn.connect_add_new_remote(|_, _, _, _, _| false);
        let result = txn.run(Some(&cancellable));
        let Some(mut planned) = collected.lock().unwrap().take() else {
            return Err(result.err().map(gerr).unwrap_or_else(|| {
                FlatpakError::Failed("the transaction could not be planned".into())
            }));
        };
        for (reference, note) in eol.lock().unwrap().iter() {
            if let Some(step) = planned.iter_mut().find(|st| &st.reference == reference) {
                step.eol = note.clone();
            }
        }
        steps.extend(planned);
    }
    Ok(OperationPlan { steps })
}

fn runtimes_blocking() -> Result<Vec<InstalledRuntime>> {
    let installs = installations()?;
    let mut uses: Vec<(String, String)> = vec![];
    let mut runtimes = vec![];
    for (scope, inst) in &installs {
        let unused: Vec<String> = inst
            .list_unused_refs(None, none())
            .map(|refs| refs.iter().map(|r| s(r.format_ref())).collect())
            .unwrap_or_default();
        for r in inst.list_installed_refs(none()).map_err(gerr)? {
            let reference = s(r.format_ref());
            match kind_of(r.kind()) {
                RefKind::App => {
                    if let Ok(bytes) = r.load_metadata(none())
                        && let Some(runtime) =
                            super::metadata_runtime(&String::from_utf8_lossy(&bytes))
                    {
                        uses.push((runtime, s(r.name())));
                    }
                }
                RefKind::Runtime => runtimes.push(InstalledRuntime {
                    info: installed_from(&r, *scope),
                    unused: unused.contains(&reference),
                    eol: s(r.eol()),
                    used_by: vec![],
                }),
            }
        }
    }
    // Apps may use a runtime from either installation.
    for runtime in &mut runtimes {
        let reference = runtime.info.ref_string();
        runtime.used_by = uses
            .iter()
            .filter(|(r, _)| *r == reference)
            .map(|(_, app)| app.clone())
            .collect();
        runtime.used_by.sort();
        runtime.used_by.dedup();
    }
    Ok(runtimes)
}

fn run_blocking(op: Operation, sink: ProgressSink, cancellable: gio::Cancellable) -> Result<()> {
    crate::security::operation(&op, "").map_err(FlatpakError::Failed)?;
    let tracker = Arc::new(Tracker {
        sink,
        index: AtomicUsize::new(0),
        total: AtomicUsize::new(1),
        base: Mutex::new((0.0, 1.0)),
    });
    tracker.report(0.0, String::new(), String::new());

    match op {
        Operation::Install { .. } | Operation::Uninstall { .. } => {
            let (txn, _) = transaction_for(&op, &cancellable, false)?
                .into_iter()
                .next()
                .expect("one transaction");
            run_transaction(&txn, tracker, &cancellable, false, None)
        }
        Operation::InstallRefFile { .. } => {
            let (txn, origin) = transaction_for(&op, &cancellable, false)?
                .into_iter()
                .next()
                .expect("one transaction");
            run_transaction(&txn, tracker, &cancellable, false, origin)
        }
        Operation::RemoveRuntimes { .. } => {
            let groups = transaction_for(&op, &cancellable, false)?;
            let n = groups.len().max(1) as f64;
            for (i, (txn, _)) in groups.into_iter().enumerate() {
                *tracker.base.lock().unwrap() = (i as f64 / n, 1.0 / n);
                tracker.index.store(0, Ordering::SeqCst);
                run_transaction(&txn, tracker.clone(), &cancellable, false, None)?;
            }
            Ok(())
        }
        Operation::Update { ids } => {
            // One transaction per installation that has something to update.
            let mut plans = vec![];
            for (_, inst) in installations()? {
                let refs = inst
                    .list_installed_refs_for_update(Some(&cancellable))
                    .map_err(gerr)?;
                let wanted: Vec<String> = refs
                    .iter()
                    .filter(|r| {
                        ids.is_empty()
                            || ids
                                .iter()
                                .any(|i| *i == s(r.name()) || *i == s(r.format_ref()))
                    })
                    .map(|r| s(r.format_ref()))
                    .collect();
                if !wanted.is_empty() {
                    plans.push((inst, wanted));
                }
            }
            let n = plans.len().max(1) as f64;
            let mut errors = vec![];
            for (i, (inst, refs)) in plans.into_iter().enumerate() {
                *tracker.base.lock().unwrap() = (i as f64 / n, 1.0 / n);
                tracker.index.store(0, Ordering::SeqCst);
                let txn = new_transaction(&inst, &cancellable)?;
                for r in &refs {
                    txn.add_update(r, &[], None).map_err(gerr)?;
                }
                match run_transaction(&txn, tracker.clone(), &cancellable, true, None) {
                    Err(FlatpakError::Cancelled) => return Err(FlatpakError::Cancelled),
                    Err(FlatpakError::Failed(e)) => errors.push(e),
                    Ok(()) => {}
                }
            }
            if errors.is_empty() {
                Ok(())
            } else {
                Err(FlatpakError::Failed(errors.join("\n")))
            }
        }
    }
}

#[async_trait]
impl FlatpakBackend for LibflatpakBackend {
    fn name(&self) -> &'static str {
        "libflatpak"
    }

    async fn list_installed(&self) -> Result<Vec<InstalledApp>> {
        blocking(|| {
            let mut out = vec![];
            for (scope, inst) in installations()? {
                let refs = inst.list_installed_refs(none()).map_err(gerr)?;
                out.extend(refs.iter().map(|r| installed_from(r, scope)));
            }
            Ok(out)
        })
        .await
    }

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        blocking(|| {
            let mut out = vec![];
            for (scope, inst) in installations()? {
                let refs = inst.list_installed_refs_for_update(none()).map_err(gerr)?;
                out.extend(refs.iter().map(|r| {
                    let app = installed_from(r, scope);
                    UpdateInfo {
                        ref_string: s(r.format_ref()),
                        id: app.id,
                        name: app.name,
                        version: String::new(),
                        origin: app.origin,
                        scope,
                        kind: app.kind,
                    }
                }));
            }
            Ok(out)
        })
        .await
    }

    async fn list_remotes(&self) -> Result<Vec<RemoteInfo>> {
        blocking(|| {
            let mut out = vec![];
            for (scope, inst) in installations()? {
                let remotes = inst.list_remotes(none()).map_err(gerr)?;
                out.extend(remotes.iter().map(|r| RemoteInfo {
                    name: s(r.name()),
                    title: s(r.title()),
                    url: s(r.url()),
                    description: s(r.description()),
                    scope,
                    disabled: r.is_disabled(),
                    noenumerate: r.is_noenumerate(),
                }));
            }
            Ok(out)
        })
        .await
    }

    async fn add_remote(&self, name: &str, source: RemoteSource, scope: Scope) -> Result<()> {
        crate::security::remote_name(name).map_err(FlatpakError::Failed)?;
        let fixture = false;
        #[cfg(feature = "test-util")]
        let (source, fixture) = match source {
            RemoteSource::LocalFixture { data, root } => {
                crate::security::repo_fixture(&data, &root).map_err(FlatpakError::Failed)?;
                (RemoteSource::FlatpakRepo(data), true)
            }
            other => (other, fixture),
        };
        let data = match source {
            #[cfg(feature = "test-util")]
            RemoteSource::LocalFixture { .. } => unreachable!("normalized fixture source"),
            RemoteSource::FlatpakRepo(data) => data,
            RemoteSource::Url(url) => crate::flathub::client::HttpClient::default()
                .fetch_bytes_limited(&url, crate::security::MAX_REF_BYTES)
                .await
                .map_err(|e| FlatpakError::Failed(e.to_string()))?,
        };
        if !fixture {
            crate::security::repo_file(&data).map_err(FlatpakError::Failed)?;
        }
        let name = name.to_string();
        blocking(move || {
            let inst = installation(scope)?;
            let remote = Remote::from_file(&name, &glib::Bytes::from_owned(data)).map_err(gerr)?;
            remote.set_gpg_verify(true);
            inst.add_remote(&remote, true, none()).map_err(gerr)
        })
        .await
    }

    async fn remove_remote(&self, name: &str, scope: Option<Scope>) -> Result<()> {
        let name = name.to_string();
        blocking(move || {
            let mut last = None;
            for (sc, inst) in installations()? {
                if scope.is_some_and(|want| want != sc) {
                    continue;
                }
                if inst.remote_by_name(&name, none()).is_err() {
                    continue;
                }
                match inst.remove_remote(&name, none()) {
                    Ok(()) => return Ok(()),
                    Err(e) => last = Some(gerr(e)),
                }
            }
            Err(last.unwrap_or_else(|| FlatpakError::Failed(format!("remote {} not found", name))))
        })
        .await
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
        let cancellable = gio::Cancellable::new();
        let bridge = {
            let c = cancellable.clone();
            let token = cancel.clone();
            tokio::spawn(async move {
                token.cancelled().await;
                c.cancel();
            })
        };
        let res = blocking(move || run_blocking(op, progress, cancellable)).await;
        bridge.abort();
        let _ = bridge.await;
        if cancel.is_cancelled() {
            return Err(FlatpakError::Cancelled);
        }
        res
    }

    async fn launch(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        blocking(move || {
            let (inst, full_ref) = find_installed(&id, None)?;
            let (_, name, arch, branch) = super::parse_ref(&full_ref)
                .ok_or_else(|| FlatpakError::Failed(format!("bad ref {}", full_ref)))?;
            inst.launch(&name, Some(&arch), Some(&branch), None, none())
                .map_err(gerr)
        })
        .await
    }

    async fn plan(&self, op: &Operation) -> Result<OperationPlan> {
        let op = op.clone();
        blocking(move || plan_blocking(op)).await
    }

    async fn runtimes(&self) -> Result<Vec<InstalledRuntime>> {
        blocking(runtimes_blocking).await
    }

    async fn refresh_appstream(&self, remote: &RemoteInfo) -> Result<()> {
        let (name, scope) = (remote.name.clone(), remote.scope);
        blocking(move || {
            let inst = installation(scope)?;
            inst.update_appstream_sync(&name, None, none())
                .map_err(gerr)
        })
        .await
    }

    async fn remote_metadata(&self, remote: &str, scope: Scope, reference: &str) -> Result<String> {
        crate::security::remote_name(remote).map_err(FlatpakError::Failed)?;
        crate::security::app_ref(reference).map_err(FlatpakError::Failed)?;
        let (remote, reference) = (remote.to_string(), reference.to_string());
        blocking(move || {
            let inst = installation(scope)?;
            let cancellable = gio::Cancellable::new();
            let full_ref = resolve_remote_ref(&inst, &remote, &reference, &cancellable)?;
            let (kind, name, arch, branch) = super::parse_ref(&full_ref)
                .ok_or_else(|| FlatpakError::Failed(format!("bad ref {}", full_ref)))?;
            let kind = match kind {
                RefKind::App => libflatpak::RefKind::App,
                RefKind::Runtime => libflatpak::RefKind::Runtime,
            };
            // The summary already carries each ref's metadata; only fetch the
            // commit when it does not.
            let remote_ref = inst
                .fetch_remote_ref_sync(&remote, kind, &name, Some(&arch), Some(&branch), none())
                .map_err(gerr)?;
            let bytes = match remote_ref.metadata() {
                Some(bytes) => bytes,
                None => inst
                    .fetch_remote_metadata_sync(&remote, &remote_ref, none())
                    .map_err(gerr)?,
            };
            if bytes.len() > super::MAX_METADATA_BYTES {
                return Err(FlatpakError::Failed(
                    "repository metadata is too large".into(),
                ));
            }
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
        .await
    }

    fn spawn_monitor(&self, tx: broadcast::Sender<()>) {
        let spawned = std::thread::Builder::new()
            .name("kiosque-flatpak-monitor".into())
            .spawn(move || {
                let ctx = glib::MainContext::new();
                let res = ctx.with_thread_default(|| {
                    let mut monitors = vec![];
                    for (scope, inst) in installations().unwrap_or_default() {
                        match inst.create_monitor(none()) {
                            Ok(m) => {
                                let tx = tx.clone();
                                m.connect_changed(move |_, _, _, _| {
                                    let _ = tx.send(());
                                });
                                monitors.push(m);
                            }
                            Err(e) => kerr!(
                                "libflatpak: cannot monitor {} installation: {}",
                                scope.as_str(),
                                e.message()
                            ),
                        }
                    }
                    if monitors.is_empty() {
                        super::spawn_polling_monitor(tx.clone());
                        return;
                    }
                    glib::MainLoop::new(Some(&ctx), false).run();
                    drop(monitors);
                });
                if res.is_err() {
                    kerr!("libflatpak: could not acquire monitor main context; polling instead");
                    super::spawn_polling_monitor(tx);
                }
            });
        if let Err(e) = spawned {
            kerr!("libflatpak: could not start monitor thread: {}", e);
        }
    }
}
