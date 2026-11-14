use kiosque_core::flatpak::{FlatpakBackend, FlatpakError, Operation, Scope, cli::CliBackend};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn executable_receives_columns_locale_and_scope() {
    let program = "import os,sys\nassert os.environ['LC_ALL']=='C'\nassert sys.argv[1]=='list'\nassert any(x.startswith('--columns=') for x in sys.argv)\nif '--app' in sys.argv: print('org.kiosque.Fixture\\tFixture\\t1\\tstable\\tx86_64\\tfixture\\tuser\\t10 B')";
    let backend = CliBackend::with_command("/usr/bin/python3", vec!["-c".into(), program.into()]);
    let apps = backend.list_installed().await.unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].scope, Scope::User);
}

#[test]
fn sandbox_routing_is_explicit() {
    let host = CliBackend::with_routing(false).command();
    assert_eq!(host.as_std().get_program(), "flatpak");
    let sandbox = CliBackend::with_routing(true).command();
    assert_eq!(sandbox.as_std().get_program(), "flatpak-spawn");
    assert_eq!(
        sandbox.as_std().get_args().collect::<Vec<_>>(),
        vec!["--host", "flatpak"]
    );
}

#[tokio::test]
async fn cancellation_reaps_owned_process_and_reports_terminal_result() {
    let root = tempfile::tempdir().unwrap();
    let pid_path = root.path().join("pid");
    let program = format!(
        "import os,time\nopen({:?},'w').write(str(os.getpid()))\nprint('working',flush=True)\ntime.sleep(60)",
        pid_path.to_str().unwrap()
    );
    let backend = CliBackend::with_command("/usr/bin/python3", vec!["-c".into(), program]);
    let started = Arc::new(tokio::sync::Notify::new());
    let notify = started.clone();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let task = tokio::spawn(async move {
        backend
            .run(
                Operation::Update { ids: vec![] },
                Arc::new(move |_| notify.notify_one()),
                token,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    let pid = std::fs::read_to_string(&pid_path).unwrap();
    cancel.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        Err(FlatpakError::Cancelled)
    );
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

#[tokio::test]
async fn process_failure_is_not_parsed_as_empty_success() {
    let backend = CliBackend::with_command(
        "/usr/bin/python3",
        vec![
            "-c".into(),
            "import sys; sys.stderr.write('fixture error'); sys.exit(17)".into(),
        ],
    );
    assert!(
        backend
            .list_installed()
            .await
            .unwrap_err()
            .to_string()
            .contains("fixture error")
    );
}
