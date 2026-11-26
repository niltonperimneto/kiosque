# Testing Kiosque

Run suites from the repository root with `python3 scripts/test.py <suite>`. Commands, combined output, tool/platform metadata with a lockfile hash, and JUnit summaries are saved under unique run directories in `.test-results/`. Missing prerequisites fail explicitly. Cargo build concurrency defaults to two jobs to bound memory use; override `CARGO_BUILD_JOBS` if appropriate.

## Suite matrix

| Suite | Prerequisites | Contract |
| --- | --- | --- |
| `guidance` | Python 3 | Skill frontmatter, local Markdown links, instruction inventory, change-to-suite routing |
| `security` | Python 3, Cargo, tar, Git and public network | Locked workspace sync, crates.io SHA-256 provenance check, checksummed RustSec/yanked audit, redacted source/history secret scan, and SPDX inventory |
| `lint` | Stable Rust with rustfmt and Clippy | Workspace formatting; core Clippy with warnings denied |
| `core` | Stable Rust, C/C++ build tools | Domain, registry, queue, parsers, scheduling, images, cache codecs; no Qt/KDE/libflatpak headers |
| `services` | Core tools, Python, loopback sockets | Local HTTP responses, cache behavior, temporary persistence, private D-Bus authorization |
| `qt` | Desktop dependencies | Real qtbridge QObject/model types, roles, loading signals and model reset invariants |
| `qml` | Desktop dependencies | Real bridge search/details/settings, shared transaction state, navigation and destruction during pending requests |
| `system` | QEMU, qemu-img, cloud-localds, SSH tools, network for provisioning | Signed local Flatpak fixture lifecycle and user/system identity inside a disposable VM |
| `packages --artifact PATH` | Disposable VM with desktop dependencies | Actual artifact startup, imports, image resources, translations and helper loading |
| `pr` | Core and desktop prerequisites | Guidance, lint, core, services, Qt and QML |

Use `--backend native` (default) or `--backend broker` for desktop suites. Each desktop configuration has its own Meson build directory. Meson tests can also be selected with `meson test -C build/tests-native --suite qt --print-errorlogs` after starting the fixture server through the shared runner.

The QML suite imports `org.kde.ki18n` at runtime. Use KI18n 6.21 or newer and install its QML module if your distribution packages it separately. The runtime also needs Qt’s SVG image plugin to load the packaged logo.

The desktop CI base is pinned by OCI digest. `tests/Containerfile` provides the corresponding developer environment. Install desktop dependencies in a disposable Fedora environment:

```sh
dnf install -y desktop-file-utils git cargo rust clippy gcc-c++ meson ninja-build cmake mold qt6-qtbase-private-devel qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtsvg flatpak-devel kf6-kirigami-devel kf6-ki18n-devel kf6-kirigami-addons kf6-qqc2-desktop-style dbus-daemon dbus-devel xorg-x11-server-Xvfb xauth gettext python3
python3 scripts/test.py qt --backend native
python3 scripts/test.py qml --backend native
python3 scripts/test.py qt --backend broker
python3 scripts/test.py qml --backend broker
```

## Isolation and fixtures

Unit/service tests construct independent instances. HTTP tests bind only loopback and never proxy live services. Fixtures under `tests/fixtures/http/` are synthetic, contain no credentials and tolerate extra API fields. Request timeouts use short configured limits; expiry tests use zero TTL instead of waiting for production expiry.

The desktop runner starts a local fixture API, supplies private XDG paths and a private D-Bus session, and uses Xvfb/software rendering. A dedicated test build installs a mock backend before QML type creation. Its fake Background portal records autostart requests and can deny them, so the production settings integration is exercised without changing desktop permissions or real autostart entries. Test bootstrap functions are feature-gated and absent from ordinary release builds. Delayed fixture responses exercise request ordering and QObject destruction.

`tests/qml/tst_models.qml` replaces the old C++ model harness and verifies roles and change notifications. The test-only Rust harness supplies mock backend controls and a private zbus Background portal.

`tests/qml/tst_regressions.qml` opens the real Settings and detail pages. It checks that page initialization preserves saved preferences, schedules use canonical identifiers despite translated labels, background permission failures are visible and restore the saved preference, failed details can be retried, and review mutations refresh both reviews and ratings. The synthetic HTTP server keeps review state per fixture-server instance; it never forwards requests to live services.

System tests are ignored by ordinary Cargo runs and selected explicitly by the VM guest. The host launcher verifies the pinned Fedora cloud image checksum, creates a disposable overlay, forwards SSH on loopback, copies the current source while pruning private directories and build caches, retrieves diagnostics and terminates the guest in a `finally` block. KVM is used when available; TCG is the slower fallback.

```sh
python3 scripts/test.py system --backend native
python3 tests/system/vm.py --artifact /absolute/path/kiosque.flatpak --companion /absolute/path/kiosque-companion.rpm
python3 tests/system/vm.py --artifact /absolute/path/kiosque.rpm --companion /absolute/path/kiosque-companion.rpm
```

Guest provisioning installs packages from the pinned Fedora release and updates repositories with weak dependencies disabled; it may download packages. The updates repository is required: the workspace needs Rust 1.98 (`rust/Cargo.toml`), newer than the release's Rust 1.94. Artifact guests match the libraries used by packaging jobs. Package checks extract artifacts under `/var/tmp`, because an extracted AppImage does not fit the guest's tmpfs `/tmp`. Fixture export itself can also be validated in a temporary container without installing applications. The actual lifecycle test uses only a local, signed repository containing a tiny synthetic runtime/application and two app revisions. User and system installations are explicitly redirected below the fixture root and checked before operations. No test runs a real install/uninstall against developer host installations. Package qualification may download the declared KDE runtime inside the guest. All artifact launches run in a private network namespace after dependency setup, so production API requests cannot reach live services.

## CI and release qualification

Pull requests run security/dependency review, core/service checks and a native/broker desktop matrix before packaging. Dependabot groups minor and patch Cargo updates with a publication cooldown (bypassed automatically for security updates) and updates manifests alongside `Cargo.lock`; a daily scheduled `Dependency audit` workflow re-runs `security` against fresh RustSec and crates.io metadata so newly disclosed advisories or yanks in locked crates fail promptly even without code pushes. The existing RPM, Flatpak and AppImage builds remain. Scheduled/manual system qualification runs the native signed fixture lifecycle in disposable VMs. Trusted successful push builds trigger artifact smoke tests for the exact source commit and packaging run; untrusted pull-request artifacts are never promoted to the VM workflow.

Release tags also trigger builds and system qualification. Before publishing, require green CI, the native system check, and all three package smoke checks for the same commit. Release publication remains a separate action. Configure main to require pull requests and the core/native/broker checks, including administrators, with resolved conversations and force-push/deletion disabled. Dependency alerts, automatic security fixes, secret push protection and private vulnerability reporting are enabled. Keep the Security audit context required.

The `kiosque --check-ui` diagnostic loads the actual packaged QML and checks its image resource before exiting with `KIOSQUE_UI_READY`. `kiosque-update --help` exits without checking or applying updates.

## Failure diagnosis and limits

Preserve the first failure, source revision, backend selection and environment versions. Inspect `.test-results/` and the Meson logs; a compile failure, unavailable socket and behavior assertion are different failures. Do not retry away a failure. Reproduce its smallest suite, add a behavioral regression, then rerun affected boundaries.

No coverage percentage or screenshot-equality gate is imposed. Sanitizers, performance baselines, broader distribution/desktop matrices, and dedicated accessibility audits are follow-up qualification. The Flatpak package smoke test probes installed state before and after installing the matching attested `kiosque-companion` RPM. The storefront RPM is qualified together with its companion, and must be the only package that installs a launcher. Daemon disconnect/reconnect during an active transaction needs separate qualification.

## Security qualification

The service suite runs a private D-Bus daemon and confirms that unrelated clients cannot invoke even valid-looking mutation requests. Streaming HTTP fixtures exercise length headers and chunked bodies; core tests cover private settings migration, invalid origins, PKCE/callback handling, queue bounds and image dimension budgets. Dedicated desktop fixtures supply an explicit local transport; release constructors retain public HTTPS restrictions.

Use `KIOSQUE_TEST_BUILD_DIR` to select a separate Meson directory when another session is building the checkout. The security audit verifies `--locked` Cargo workspace resolution, rejects non-crates.io sources, missing SHA-256 checksums, yanked crates and `[patch]`/`[replace]` overrides, and emits an SPDX lockfile inventory under its results directory; this covers Rust source dependencies, including optional/test/build crates, and is not a complete inventory of bundled system libraries. Pull requests also run GitHub dependency review across Cargo and Actions diffs. Trusted package pushes produce GitHub provenance attestations. Scheduled/trusted CodeQL scans cover Rust, Python and Actions; no successful scan is claimed before those workflows execute.

Live OAuth provider registration, signed native lifecycle, Flatpak-to-native companion identity/activation and exact package smoke checks require separate release qualification. See [the hardening record](security-hardening.md) for the current execution evidence.

## Automatic build versions

`scripts/build_version.py` supplies the version for Cargo, Meson and CI packages. Python 3.11 or newer and full Git history are build prerequisites. CI checks out history and tags explicitly. A clean `vMAJOR.MINOR.PATCH` tag produces that release version. Other builds include a development counter and the Git revision, for example `1.0.1-dev.4+g123456789abc`. Local changes append `.dirty`. With no release tag, the development baseline comes from `rust/Cargo.toml` under `[workspace.package]`.

Cargo build scripts recompute the version when tracked inputs, the index, HEAD or tags change. Both `kiosque --version` and `kiosque-update --version` report it. Building again after a commit updates the version without requiring Meson reconfiguration. RPM uses the equivalent `1.0.1~dev.4.g123456789abc`, which sorts before the corresponding stable release. Flatpak and AppImage filenames contain the source version.

Before exporting a source archive or invoking Flatpak Builder with a directory source, freeze the version into the source tree. The generated file is ignored by Git and included in the source export. A live checkout always derives its version from Git, even if an old snapshot exists.

```sh
python3 scripts/build_version.py --write build-version.json
flatpak-builder --user --install-deps-from=flathub build org.kiosque.Kiosque.json
```

For RPM source packaging, use `python3 scripts/build_version.py --write build-version.json --format rpm` as the RPM version, preserve `build-version.json` in the source tarball, and pass `--define "kiosque_version VERSION"` to rpmbuild. CI performs these steps automatically. An archive lacking a snapshot reports the workspace baseline with `-dev.0+unknown` so its provenance remains visible. Generated package AppStream metadata carries the same version and source commit date, preserving authored release history. The frozen snapshot carries that date into Git-free package builds.

## Publishing a release

On GitHub, select **Actions → Release → Run workflow**, keep **main** selected,
and enter a stable version such as **1.0.0** without a v prefix. The workflow
tags the current main commit as v1.0.0 and explicitly dispatches CI for that tag.
It uses the repository token, so no personal access token is needed.

Tagged CI runs the existing security, core, service and desktop checks, builds
RPM, Flatpak and AppImage packages, and attests them. The packaged artifact smoke
workflow verifies the source commit and tag, checks attestations, and tests the
packages inside disposable VMs. Once every package passes, it uploads the exact
qualified artifacts to a GitHub Release and publishes generated release notes.
Both storefront and companion RPMs are included.

A failed package build or smoke test prevents publication. A failed asset upload
leaves a draft release that a rerun can complete. Published releases are preserved.
To retry after main has advanced, rerun CI with the existing tag selected, then
rerun its package smoke workflow if needed. An existing tag is never moved.
A normal successful main build continues to run package smoke tests without
publishing a release. Pushing a stable vMAJOR.MINOR.PATCH tag also uses this
release path.
