# Agent guidance and testing implementation

## Recovery and implementation

Recovered the existing working tree after the reported Linux freeze. Preserved the pre-existing backend refactor, staged deletions and other user changes; nothing was reset, staged or committed.

Implemented the approved architecture:

- Root and scoped `AGENTS.md`, four role playbooks and six repository skills.
- Qt-free `kiosque-core`, injectable HTTP/storage/settings/backend services and Qt compatibility adapters.
- Deterministic unit/service tests, real Qt model and QML interaction harnesses, signed Flatpak fixtures and disposable-VM lifecycle/package runners.
- Shared local/CI runner, native/CLI desktop matrix, system qualification and artifact smoke workflows.
- Architecture and testing documentation. Start with [testing](testing.md) and [architecture](architecture.md).

## Validation recorded on 2026-11-14

Against Cargo.lock SHA-256 `91da4b0dfa66664513306a99875113354965daf39fd5139b19ad74b4926ea3c9`:

| Check | Result |
| --- | --- |
| `python3 scripts/test.py lint` | Rustfmt and core Clippy with warnings denied passed |
| `python3 scripts/test.py core` | 33 unit tests passed; no doctests |
| `python3 scripts/test.py services` | 8 service and 4 process tests passed, including an unusable inherited-proxy environment |
| `python3 scripts/test.py guidance` | Guidance validation and 4 routing tests passed |
| `python3 scripts/test.py qt --backend native/cli` (each separately) | Both model suites passed |
| `python3 scripts/test.py qml --backend native/cli` (each separately) | Both interaction suites passed, 9 Qt Quick Test results each |
| Actual native application `--check-ui`, network disabled | Loaded QML/resources and emitted `KIOSQUE_UI_READY` |
| Six skills, bundled skill-creator validator | All passed |
| Signed fixture export in a temporary container | Runtime and both application revisions exported successfully |

An ordinary native application `cargo check` with test support disabled also passed against the initial dependency baseline, before the current lockfile refresh. Desktop builds and tests above were repeated against the current lockfile.

## Remaining qualification

The real CLI Flatpak VM booted, provisioned and exported its initial signed fixture, but exceeded the 3,000-second execution limit while compiling under QEMU software emulation. No KVM device was available. This is an incomplete check, not a passing lifecycle test. That guest used its launch snapshot from before the final lockfile refresh. The guest/container were destroyed. The runner now retrieves diagnostics on timeout as well as ordinary test failures.

The real native lifecycle and actual RPM/Flatpak/AppImage smoke suites have not passed locally; CI workflows are provided to execute them. Do not treat their implementation as release qualification. Daemon disconnect/reconnect and true sandbox host integration remain follow-up scenarios documented in [testing](testing.md).

No packages were published and no Flatpak application was installed on the developer host. The Flatpak manifest now permits the host-spawn D-Bus interface required by its CLI backend.

## Local environment and retained evidence

The host rustup shim cannot load libssl.so.3; direct stable toolchain binaries work. Host Qt/Meson dependencies were unavailable, so desktop validation used disposable Podman build containers. Command output, JUnit and environment metadata are under ignored `.test-results/`; desktop logs also reside in `build/tests-{native,cli}/meson-logs/`.

Generated Rust outputs were moved to ignored `.cache/verification/` when `/tmp` approached its capacity. `.cache/verification/before-core/` preserves the original Rust sources and manifests from before extraction. The VM image cache is ignored too. These local recovery/build files are not source deliverables.
