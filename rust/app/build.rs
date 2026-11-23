//! Embeds `qml/` as Qt resources and, for test builds, generates the cxx
//! shim for Qt Quick Test's `quick_test_main`. No hand-written C++.

include!("../build_version.rs");

use std::path::{Path, PathBuf};

/// QML-loadable files; guidance documents and editor files stay out of qrc.
fn wanted(path: &Path) -> bool {
    path.file_name().is_some_and(|x| x == "qmldir")
        || path
            .extension()
            .is_some_and(|x| x == "qml" || x == "js" || x == "svg" || x == "png")
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create resource dir");
    let mut entries: Vec<_> = std::fs::read_dir(src)
        .expect("read qml dir")
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            copy_tree(&path, &dst.join(entry.file_name()));
        } else if wanted(&path) {
            std::fs::copy(&path, dst.join(entry.file_name())).expect("copy qml file");
        }
    }
}

fn main() {
    configure_build_version();
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let qml = manifest.join("../../qml");

    // `include_bytes_qml!` names resources after its literal path argument, so
    // the tree is staged in OUT_DIR and included as "qml" → qrc:/qml/….
    let staged = out.join("qml");
    let _ = std::fs::remove_dir_all(&staged);
    copy_tree(&qml, &staged);
    std::fs::write(
        out.join("qml_resources.rs"),
        "{ qtbridge::include_bytes_qml!(\"qml\", \"\"); }\n",
    )
    .unwrap();
    println!("cargo:rerun-if-changed={}", qml.display());

    if std::env::var_os("CARGO_FEATURE_TEST_UTIL").is_some() {
        let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".into());
        let headers = std::process::Command::new(&qmake)
            .args(["-query", "QT_INSTALL_HEADERS"])
            .output()
            .unwrap_or_else(|e| panic!("{qmake}: {e}"));
        let headers = String::from_utf8(headers.stdout).unwrap();
        let headers = Path::new(headers.trim());
        cxx_build::bridge("src/quicktest_ffi.rs")
            .include(headers)
            .include(headers.join("QtCore"))
            .include(headers.join("QtQuickTest"))
            .flag_if_supported("-std=c++17")
            // Qt headers trip GCC 16 diagnostics that are not ours to fix.
            .flag_if_supported("-Wno-sfinae-incomplete")
            .compile("kiosque-quicktest-ffi");
        println!("cargo:rerun-if-changed=src/quicktest_ffi.rs");
        println!("cargo:rerun-if-env-changed=QMAKE");
    }
}
