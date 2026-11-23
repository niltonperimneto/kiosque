// Shared build-script hook for the storefront and companion.
fn configure_build_version() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let resolver = manifest.join("../../scripts/build_version.py");
    let run = |args: &[&str]| {
        let output = std::process::Command::new("python3")
            .arg(&resolver)
            .args(args)
            .output()
            .expect("Python 3 is required to resolve the source version");
        assert!(
            output.status.success(),
            "version resolution failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("version resolver returned invalid UTF-8")
    };
    println!("cargo:rerun-if-changed={}", resolver.display());
    println!("cargo:rustc-env=KIOSQUE_VERSION={}", run(&[]).trim());
    for path in run(&["--watch-files"]).lines() {
        println!("cargo:rerun-if-changed={path}");
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("../build_version.rs").display()
    );
}
