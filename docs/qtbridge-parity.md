# Qtbridge feature parity

The migration incorporates the storefront baseline at `5037257` into the qtbridge workspace. The application and updater remain Rust binaries; qtbridge is pinned to 0.4.1 and still uses generated C++ glue and cxx-qt-lib internally.

| Area | Preserved behavior |
| --- | --- |
| Navigation | Single-column pages, shared desktop sidebar and compact tabs, Search, Categories, Installed, Updates, Settings and About |
| Settings | KDE FormCards, wrapping text, symbolic actions, canonical schedules, permission-denial feedback and preference rollback |
| Storefront | Branded carousel, separate daily pick, mobile and sorted Games collections, interactive screenshot collage without image stretching |
| Details | Independent state, permissions, release notes, safe descriptions, links, related applications, reviews and mutation refresh |
| Transactions | Shared progress, background Tasks dialog, cancellation, completion notifications, persistent update selection and visible backend errors |
| Repositories | Descriptor inspection and explicit origin/signing-key confirmation before addition or installation |
| Backends | Current native/broker implementations, companion sender authorization, scoped operations and hardened HTTP/OAuth/settings behavior |
| Localization | UTF-8 MO catalogs, context and plural forms, per-message language fallback; KDE Addons retain their own KI18n domain lookups |

Public QML names and Qt.UserRole numbers are retained. Callbacks which mutate their emitting qtbridge object defer through `Qt.callLater`. The test-only Rust harness supplies a mock Flatpak backend and private Background portal. Each Meson desktop suite runs serially so portal ownership cannot race between harness processes.

The featured cache uses schema 3 because daily picks now retain all screenshot URLs. Older layouts are ignored. Remote images are sanitized to bounded PNGs using the current hardened HTTP transport; core's existing decoded-pixel cache and its contracts remain available.

## Validation

Executed through `python3 scripts/test.py` in a disposable Fedora desktop container:

- `guidance`: nine runner tests and translation source-inventory validation passed.
- `core`: 68 domain tests, documentation tests and seven updater tests passed without Qt or native-backend features.
- `services`: 18 isolated service contracts passed.
- `lint`: formatting and core/updater Clippy passed with warnings denied.
- `qt --backend broker` and `qt --backend native`: Clippy, 11 application unit tests and the real model contract suite passed for each configuration.
- `qml --backend broker` and `qml --backend native`: all five suites passed, 46 checks per configuration.
- `security`: RustSec audit, lockfile inventory and redacted source/history scans passed. The old gettext/encoding dependency was removed rather than exempted.

Seven additional visual captures cover desktop and compact Settings, storefront, Search, Updates, Installed and details. Their services and images are synthetic fixtures; these are rendering/navigation evidence, not live service qualification. The collage regression independently checks three images and pointer motion.

A separate native build with `test_support=false` passed `--check-ui` in a network-isolated container, printed `KIOSQUE_UI_READY`, and contained no test bootstrap symbols. Its updater's `--help` passed. A staged install included translations, scalable/symbolic icons, native companion activation and KRunner metadata without modifying the host installation.

RPM, Flatpak and AppImage artifact VM checks, signed native lifecycle and packaged companion identity qualification were not rerun locally. Those remain CI/release qualification layers; source builds and staged installation do not replace them. Live OAuth providers were not exercised.
