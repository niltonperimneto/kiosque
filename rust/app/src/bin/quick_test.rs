//! `kiosque-quick-test`: Qt Quick Test over `tests/qml` with isolated
//! services (mock Flatpak backend, loopback fixture API, private storage).
//! Built only with the `test-util` feature and never installed.
//!
//! Arguments are Qt Quick Test's (`-input tests/qml/tst_models.qml`, …).

use std::ffi::{CString, c_char};

use qtbridge::{QmlElement, qobject};

#[qobject(NoQmlElement)]
mod support {
    /// `TestSupport` singleton: harness facts for test files.
    pub struct TestSupport {
        pub(super) artifact_dir: String,
        pub(super) endpoint: String,
    }

    impl Default for TestSupport {
        fn default() -> Self {
            Self {
                artifact_dir: std::env::var("KIOSQUE_TEST_ARTIFACTS").unwrap_or_default(),
                endpoint: std::env::var("KIOSQUE_TEST_HTTP").unwrap_or_default(),
            }
        }
    }

    impl TestSupport {
        qproperty!("artifactDir", Member = artifact_dir, Constant);
        qproperty!("endpoint", Member = endpoint, Constant);
    }
}

impl QmlElement for support::TestSupport {
    const URI: &str = kiosque::bridge::URI;
    const ELEMENT_NAME: &str = "TestSupport";
    const MAJOR_VERSION: u8 = 1;
    const MINOR_VERSION: u8 = 0;
    const IS_SINGLETON: bool = true;
}

fn main() {
    let endpoint = std::env::var("KIOSQUE_TEST_HTTP").unwrap_or_default();
    let root = std::env::temp_dir().join(format!("kiosque-quick-test-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&root)
        .map_err(|e| e.to_string())
        .and_then(|_| kiosque::services::install_test_services(&endpoint, &root))
    {
        eprintln!(
            "kiosque-quick-test: an isolated fixture server and writable storage are required: {e}"
        );
        std::process::exit(2);
    }

    kiosque::register_resources();
    kiosque::configure_style();
    kiosque_core::l10n::init();
    kiosque::bridge::register_types();
    support::TestSupport::register();

    let source_dir = std::env::var("KIOSQUE_QML_TEST_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/qml").into());
    let args: Vec<CString> = std::env::args()
        .map(|a| CString::new(a).expect("argument without NUL"))
        .collect();
    let mut argv: Vec<*mut c_char> = args.iter().map(|a| a.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());
    let name = CString::new("kiosque").unwrap();
    let source_dir = CString::new(source_dir).expect("path without NUL");
    // SAFETY: argv holds args.len() valid C strings (plus a terminating null),
    // and every CString outlives the call.
    let failures = unsafe {
        kiosque::quicktest_ffi::ffi::quick_test_main(
            args.len() as i32,
            argv.as_mut_ptr(),
            name.as_ptr(),
            source_dir.as_ptr(),
        )
    };

    kiosque::services::shutdown_test_services();
    let _ = std::fs::remove_dir_all(&root);
    std::process::exit(failures.min(255));
}
