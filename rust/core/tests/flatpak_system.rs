#![cfg(feature = "test-util")]
//! Explicitly selected real-backend contracts; never run by ordinary cargo test.
use kiosque_core::flatpak::{self, FlatpakBackend, FlatpakError, Operation, RemoteSource, Scope};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

fn root() -> PathBuf {
    assert!(
        std::path::Path::new("/run/kiosque-test-vm").exists(),
        "run through tests/system/vm.py"
    );
    let root =
        PathBuf::from(std::env::var("KIOSQUE_SYSTEM_ROOT").expect("private system root required"));
    for (_, path) in flatpak::installation_paths() {
        assert!(
            path.starts_with(&root),
            "installation escaped VM fixture root: {}",
            path.display()
        );
    }
    root
}

#[tokio::test]
#[ignore = "requires disposable VM and local signed fixture repository"]
async fn real_backend_lifecycle_and_scopes() {
    let root = root();
    let backend: Arc<dyn FlatpakBackend> =
        match std::env::var("KIOSQUE_TEST_BACKEND").unwrap().as_str() {
            "cli" => Arc::new(flatpak::cli::CliBackend::new()),
            #[cfg(feature = "libflatpak")]
            "native" => Arc::new(flatpak::native::LibflatpakBackend::new().unwrap()),
            other => panic!("backend unavailable: {other}"),
        };
    let repo = std::fs::read(root.join("fixture.flatpakrepo")).unwrap();
    for scope in [Scope::User, Scope::System] {
        backend
            .add_remote(
                "fixture",
                RemoteSource::LocalFixture {
                    data: repo.clone(),
                    root: root.clone(),
                },
                scope,
            )
            .await
            .unwrap();
        let progress: flatpak::ProgressSink = Arc::new(|p| assert!(p.fraction.is_finite()));
        backend
            .run(
                Operation::Install {
                    remote: "fixture".into(),
                    id: "org.kiosque.Fixture".into(),
                    scope,
                },
                progress,
                CancellationToken::new(),
            )
            .await
            .unwrap();
    }
    let installed = backend.list_installed().await.unwrap();
    for scope in [Scope::User, Scope::System] {
        assert!(
            installed
                .iter()
                .any(|a| a.id == "org.kiosque.Fixture" && a.scope == scope)
        );
    }
    let before = std::process::Command::new("flatpak")
        .args(["info", "--user", "--show-commit", "org.kiosque.Fixture"])
        .output()
        .unwrap();
    assert!(before.status.success());
    let publisher =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/system/publish_fixture.py");
    assert!(
        std::process::Command::new("python3")
            .arg(publisher)
            .arg(&root)
            .arg("2")
            .status()
            .unwrap()
            .success()
    );
    backend
        .run(
            Operation::Update {
                ids: vec!["org.kiosque.Fixture".into()],
            },
            Arc::new(|_| {}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let after = std::process::Command::new("flatpak")
        .args(["info", "--user", "--show-commit", "org.kiosque.Fixture"])
        .output()
        .unwrap();
    assert!(after.status.success());
    assert_ne!(before.stdout, after.stdout);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(
        backend
            .run(
                Operation::Update { ids: vec![] },
                Arc::new(|_| {}),
                cancelled
            )
            .await,
        Err(FlatpakError::Cancelled)
    );
    for scope in [Scope::User, Scope::System] {
        tokio::time::timeout(
            Duration::from_secs(60),
            backend.run(
                Operation::Uninstall {
                    id: "org.kiosque.Fixture".into(),
                    scope: Some(scope),
                },
                Arc::new(|_| {}),
                CancellationToken::new(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        if backend
            .list_installed()
            .await
            .unwrap()
            .iter()
            .any(|app| app.id == "org.kiosque.TestPlatform" && app.scope == scope)
        {
            backend
                .run(
                    Operation::Uninstall {
                        id: "org.kiosque.TestPlatform".into(),
                        scope: Some(scope),
                    },
                    Arc::new(|_| {}),
                    CancellationToken::new(),
                )
                .await
                .unwrap();
        }
        backend.remove_remote("fixture", Some(scope)).await.unwrap();
    }
    assert!(
        backend
            .list_installed()
            .await
            .unwrap()
            .iter()
            .all(|a| a.id != "org.kiosque.Fixture")
    );
}
