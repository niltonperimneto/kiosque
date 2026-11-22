# Storefront security hardening

This work addresses the review of `78fac71` and is based on merged main `0587a81`, on `security/harden-storefront-boundaries`. Concurrent storefront and file-import changes in the shared checkout were preserved. The pull request includes this implementation, its validation record and the retained supporting UI changes. Follow [the maintainer setup guide](security-setup.md) after reviewing the PR.

## Implemented boundaries

- Removed bearer tokens from settings and migrated legacy records. Settings use private directory/file modes, bounded regular-file reads and exclusive atomic writes; symlinks and hard-link reads are rejected.
- GitLab sign-in uses S256 PKCE, a listener bound before browser launch, random loopback ports/state, bounded callbacks and an overall deadline. GitHub uses device authorization without embedding a client secret. Login can be cancelled; token responses are bounded, errors redacted, and tokens remain in memory only.
- External references/repositories require bounded, unambiguous HTTPS descriptors with signing keys. Confirmation retains the inspected bytes and shows origin, signing-key SHA-256 and user scope. Native transactions reject unreviewed dependency repositories; external-ref dependencies/related refs are not installed automatically.
- Flatpak host spawning and its broad D-Bus permission were removed. Sandboxed operations use a typed native companion API and fail closed when it is absent. The Flatpak package does not install daemon activation/KRunner files that could shadow the host companion.
- Companion methods check bus-provided UID/PID and native/Flatpak executable identity. Identifiers, scopes and payloads are validated. Transactions cap active operations at 32, descriptors at 1 MiB and update batches at 256 IDs. Disconnect monitoring fails unfinished jobs rather than waiting forever.
- Production HTTP requests require public HTTPS destinations, including DNS answers, with no proxies or redirects. JSON/image streams have decoded byte limits. Images also have dimension/allocation/concurrency budgets and local icon-directory restrictions. Description/catalog text uses plain text; external links require public HTTPS.
- Unknown permission summaries are distinct from verified empty summaries. Unknown permissions display a warning and block a new installation until details can be verified.
- CI actions, base images and executable downloads are pinned. AppImage PR builds no longer have privileged/FUSE access. Security checks audit dependencies with warnings denied and scan both source/history for secrets with redacted reports. SPDX inventories cover exact locked Rust dependencies. Trusted package builds produce provenance attestations, which artifact smoke tests verify against their exact source commit before execution. CodeQL is configured for trusted/scheduled Rust, C++, Python and Actions scans.
- Replaced the unmaintained unsafe YAML dependency with a safe Rust parser with explicit resource budgets and YAML 1.2 boolean semantics. Disabled unused postcard features that pulled in an unmaintained atomic dependency.

## Repository controls applied

Verified through GitHub's API on 2026-11-15:

- Main requires pull requests, up-to-date core/native/CLI checks and resolved conversations, including administrators. Force pushes and deletion are disabled.
- The repository has one collaborator, so no impossible second-person approval requirement was introduced. Independent review remains a process limitation.
- Dependency alerts and automatic Dependabot security fixes are enabled.
- Secret scanning and push protection are enabled; private vulnerability reporting is enabled.
- The API left generic-secret scanning and validity-check options disabled after the attempted update. The added Gitleaks CI gate supplies additional generic-secret detection; those GitHub options are not claimed enabled.

Add `Security audit` as a required branch check after its first published workflow run. New workflow definitions, attestations and CodeQL configuration need successful GitHub runs for the submitted source revision.

## Validation record

Completed on the stable implementation:

- `python3 scripts/test.py guidance`: six runner tests and guidance checks pass.
- `python3 scripts/test.py lint`: formatting and warnings-as-errors core Clippy pass.
- `python3 scripts/test.py core`: 52 tests pass; Qt-independent core and doctests build.
- `python3 scripts/test.py services`: four real fake-process tests and 18 service tests pass, including private-bus caller rejection and chunked/oversized download limits.
- `python3 scripts/test.py security`: 342 locked packages audited with no advisories or warnings. Redacted source scan and all 62 Git commits contain no detected secrets. An SPDX inventory is emitted with the report.
- `python3 scripts/test.py qt --backend native` and `--backend cli` pass. `python3 scripts/test.py qml --backend native` and `--backend cli` each pass all 31 Qt Quick Test results, including the final avatar-loader change. Desktop checks used the isolated development container and `build/security-tests-{native,cli}` with `build/security-target`.
- All three pinned AppImage build tools were downloaded and their upstream SHA-256 digests verified. Workflow YAML parses and every action reference is a full commit hash. `git diff --check` passes.

The first failures were retained in `.test-results/`: native API-name corrections, YAML boolean compatibility, and the UI regression's old raw-error-body expectation. These were corrected rather than suppressed. Overlapping backend builds also exposed a Cargo-to-Meson export-copy race; a filesystem lock now covers compilation and collection of both exported objects. The generic secret scanner initially flagged two synthetic public-key stubs; the stubs were shortened without excluding files or rules.

## Remaining release qualification

The native companion narrows cross-application access, but the current Unix account, host Flatpak/Polkit implementation and operating system remain trusted. Flatpak repository transport is performed by host Flatpak and does not inherit the API client's custom DNS/redirect policy.

Live OAuth requires provider-side public-client registration; GitHub device flow must be enabled, and GitLab client IDs must be configured. No live sign-in or credential-bearing requests were made during deterministic tests.

Signed native/CLI lifecycle, packaged Flatpak-to-host companion identity/activation and exact RPM/Flatpak/AppImage smoke tests have not been run for this change. This host has no `/dev/kvm`; prior software-emulated VM compilation exceeded its qualification deadline. No developer-host Flatpaks were installed or removed. The VM fixture path is explicitly feature-gated and unavailable in release constructors.

The SPDX inventory covers Rust source dependencies, including optional/build/test crates, rather than all bundled system libraries. Package provenance is configured for future trusted CI runs; no release was published or artifact attestation fabricated.

Final successful report directories under `.test-results/`:

- `lint-native/1794789458353807458-2`
- `core-native/1794789460182126332-35`
- `services-native/1794789476029311808-641`
- `security-native/1794789580388134456-58988`
- `qt-native/1794789946282853859-2`
- `qml-native/1794790038023430369-674`
- `qt-cli/1794789683946268935-239`
- `qml-cli/1794789794171453778-1550`

These artifacts are ignored local execution evidence, not published release attestations.
