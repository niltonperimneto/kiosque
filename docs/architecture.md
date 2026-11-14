# Kiosque architecture

Kiosque uses a Cargo workspace under `rust/`. `kiosque-core` owns the Qt-independent domain and services. The `kiosque` package (`rust/app`) remains the static library linked by Meson and exports the existing `com.kiosque` QML types. `kiosque-update` (`rust/updater`) is the Qt-free updater.

```mermaid
flowchart LR
    QML[Qt Quick / Kirigami views] --> Qt[cxx-qt models and controllers]
    Qt --> Services[Application services]
    Services --> Core[Registry / transaction queue / catalog]
    Services --> HTTP[Flathub / ODRS clients]
    Services --> Storage[Settings and disk caches]
    Core --> Backend[FlatpakBackend]
    Backend --> Native[Optional libflatpak]
    Backend --> CLI[CLI fallback]
```

## Ownership and composition

- Core default features contain no Qt, KDE or libflatpak dependency. The application's default `libflatpak` feature forwards to the optional core backend. Disabling it retains CLI operation. The Flatpak manifest explicitly selects CLI mode to route operations through the host command instead of requiring libflatpak inside the KDE runtime. It grants `org.freedesktop.Flatpak` D-Bus access, which the host-spawn interface requires.
- The application composition root is `rust/app/src/services.rs`. Production constructors use production endpoints and paths. Dedicated `test-util` builds can install isolated services before any QObject is created; release builds do not export that bootstrap.
- Registry owns installed/update/job state. TransactionManager serializes backend operations, deduplicates complete operations including scope and remote, and publishes terminal state after reloading installed refs. `shutdown()` cancels pending operations and joins the worker. Worker ownership uses a weak reference to avoid an idle Arc cycle.
- Qt models project registry snapshots into serializable core entries. The adapter's compatibility helpers retain existing UI method names while sending mutations through the shared transaction client. Model/controller event subscriptions stop when their Rust QObject state is destroyed.
- Each HTTP client owns its transport, API base and cache handle. Production constructors share the production cache; explicit constructors allow independent caches/endpoints. Settings persistence and disk caches accept explicit paths; saving settings does not invoke a portal or systemctl. Background/autostart integration remains an explicit operation.
- Image decoding returns owned pixels from core. Only the adapter exposes raw C buffers, and the shared C++ image provider frees each allocation. Provider URLs encode the underlying URL, so changing metadata cannot reuse an old app-ID image mapping.

## Execution and compatibility

Tokio performs network, filesystem and process work. Qt changes are posted through cxx-qt thread handles. App-list request generations reject responses for superseded requests. The native backend runs libflatpak work on blocking workers; CLI children are owned and reaped during cancellation.

Meson builds the explicit `kiosque` package, links its static library and exported cxx-qt initializer object, and embeds QML resources. Keep the initializer export integration aligned with cxx-qt upgrades; its export layout is not a stable public interface.

The new core cache format is versioned. Old incompatible cache files are ignored. No settings schema migration is required; older records without an ODRS salt receive a generated salt when loaded.

`kiosque-update` (`rust/updater`) is pure Rust: it owns the daemon bus name (single instance, no replacement), serves a StatusNotifierItem with a dbusmenu, sends `org.freedesktop.Notifications` with `update`/`open` actions and reacts to `kiosque_core::daemon::DaemonEvent`s. Its behaviour is a pure state machine (`shell.rs`) tested without a bus. It never enables autostart; `kiosque_core::autostart::apply` runs only when the user saves settings.

## Sandbox and trust boundaries

The Flatpak build uses the CLI backend and runs `flatpak-spawn --host flatpak …`, which needs `--talk-name=org.freedesktop.Flatpak`. That name lets the app run host commands, so every value that becomes a `flatpak` argument (app ids, refs, remote names, repository URLs) is validated in `flatpak/cli.rs` before spawning. Option-like values (leading `-`) and characters outside flatpak's own id and name syntax are rejected. Keep this check in front of any new CLI call.

Every other permission is read-only and scoped to what the code reads:

| Permission | Reason |
| --- | --- |
| `/var/lib/flatpak:ro`, `xdg-data/flatpak:ro` | AppStream catalog and installed-state monitoring (`flatpak::installation_paths`) |
| `host-etc:ro`, `host-os:ro` | Distributor configuration under `/run/host/etc/kiosque` and `/run/host/usr/share/kiosque` |
| `org.freedesktop.Notifications`, `org.kde.StatusNotifierWatcher` | Update notifier notifications and tray icon |

The app writes only to its own `~/.var/app` directories. Temporary `.flatpakref`/`.flatpakrepo` files go to its cache directory, which has the same path on the host. Do not reintroduce `--filesystem=host`. Never include OAuth response bodies in errors or logs, since they can contain tokens.
