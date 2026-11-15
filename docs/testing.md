# Testing Kiosque

Run suites from the repository root with `python3 scripts/test.py <suite>`. Commands, combined output, tool/platform metadata with a lockfile hash, and JUnit summaries are saved under unique run directories in `.test-results/`. Missing prerequisites fail explicitly. Cargo build concurrency defaults to two jobs to bound memory use; override `CARGO_BUILD_JOBS` if appropriate.

## Suite matrix

| Suite | Prerequisites | Contract |
| --- | --- | --- |
| `guidance` | Python 3 | Skill frontmatter, local Markdown links, instruction inventory, change-to-suite routing |
| `lint` | Stable Rust with rustfmt and Clippy | Workspace formatting; core Clippy with warnings denied |
| `core` | Stable Rust, C/C++ build tools | Domain, registry, queue, parsers, scheduling, images, cache codecs; no Qt/KDE/libflatpak headers |
| `services` | Core tools, Python, loopback sockets | Local HTTP responses, cache behavior, temporary persistence, real fake-command processes, cancellation/reaping |
| `qt` | Desktop dependencies | Real QObject/model types, roles, loading signals and model reset invariants |
| `qml` | Desktop dependencies | Real bridge search/details/settings, shared transaction state, navigation and destruction during pending requests |
| `system` | QEMU, qemu-img, cloud-localds, SSH tools, network for provisioning | Signed local Flatpak fixture lifecycle and user/system identity inside a disposable VM |
| `packages --artifact PATH` | Disposable VM with desktop dependencies | Actual artifact startup, imports, image resources, translations and helper loading |
| `pr` | Core and desktop prerequisites | Guidance, lint, core, services, Qt and QML |

Use `--backend native` (default) or `--backend cli` for desktop/system suites. Each desktop configuration has its own Meson build directory. Meson tests can also be selected with `meson test -C build/tests-native --suite qt --print-errorlogs` after starting the fixture server through the shared runner.

The desktop CI base is pinned by OCI digest. `tests/Containerfile` provides the corresponding developer environment. Install desktop dependencies in a disposable Fedora environment:

```sh
dnf install -y desktop-file-utils cargo rust gcc-c++ meson ninja-build cmake mold qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtsvg flatpak-devel kf6-kirigami-devel kf6-ki18n-devel kf6-kirigami-addons kf6-qqc2-desktop-style dbus-daemon dbus-devel xorg-x11-server-Xvfb xauth gettext python3
python3 scripts/test.py qt --backend native
python3 scripts/test.py qml --backend native
python3 scripts/test.py qt --backend cli
python3 scripts/test.py qml --backend cli
```

## Isolation and fixtures

Unit/service tests construct independent instances. HTTP tests bind only loopback and never proxy live services. Fixtures under `tests/fixtures/http/` are synthetic, contain no credentials and tolerate extra API fields. Request timeouts use short configured limits; expiry tests use zero TTL instead of waiting for production expiry.

The desktop runner starts a local fixture API, supplies private XDG paths and a private D-Bus session, and uses Xvfb/software rendering. A dedicated test build installs a mock backend before QML type creation. Test bootstrap functions are feature-gated and absent from ordinary release builds. Delayed fixture responses exercise request ordering and QObject destruction.

System tests are ignored by ordinary Cargo runs and selected explicitly by the VM guest. The host launcher verifies the pinned Fedora cloud image checksum, creates a disposable overlay, forwards SSH on loopback, copies the current source while pruning private directories and build caches, retrieves diagnostics and terminates the guest in a `finally` block. KVM is used when available; TCG is the slower fallback.

```sh
python3 scripts/test.py system --backend native
python3 scripts/test.py system --backend cli
python3 tests/system/vm.py --artifact /absolute/path/kiosque.flatpak
```

Lifecycle guest provisioning uses the pinned Fedora release repositories with weak dependencies disabled; it may download packages. Artifact guests use the updates repository too, matching the libraries used by packaging jobs. Fixture export itself can also be validated in a temporary container without installing applications. The actual lifecycle test uses only a local, signed repository containing a tiny synthetic runtime/application and two app revisions. User and system installations are explicitly redirected below the fixture root and checked before operations. No test runs a real install/uninstall against developer host installations. Package qualification may download the declared KDE runtime inside the guest. All artifact launches run in a private network namespace after dependency setup, so production API requests cannot reach live services.

## CI and release qualification

Pull requests run core/service checks and a native/CLI desktop matrix before packaging. The existing RPM, Flatpak and AppImage builds remain. Scheduled/manual system qualification runs both real backends in disposable VMs. Trusted successful push builds trigger artifact smoke tests for the exact source commit and packaging run; untrusted pull-request artifacts are never promoted to the VM workflow.

Release tags also trigger builds and system qualification. Before publishing, require green CI, both system backend checks, and all three package smoke checks for the same commit. This repository does not publish releases or configure remote branch-protection rules automatically. Configure required pull-request checks in repository settings if not already enforced.

The `kiosque --check-ui` diagnostic loads the actual packaged QML and checks its image resource before exiting with `KIOSQUE_UI_READY`. `kiosque-update --help` exits without checking or applying updates.

## Failure diagnosis and limits

Preserve the first failure, source revision, backend selection and environment versions. Inspect `.test-results/` and the Meson logs; a compile failure, unavailable socket and behavior assertion are different failures. Do not retry away a failure. Reproduce its smallest suite, add a behavioral regression, then rerun affected boundaries.

No coverage percentage or screenshot-equality gate is imposed. Sanitizers, performance baselines, broader distribution/desktop matrices, and dedicated accessibility audits are follow-up qualification. Current system contracts focus on backend lifecycle/scope; daemon disconnect/reconnect and sandbox host integration need dedicated guest scenarios as those application paths are integrated.
