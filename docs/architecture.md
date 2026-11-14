# Kiosque architecture

Kiosque uses a Cargo workspace under `rust/`. `kiosque-core` owns the Qt-independent domain and services. `libkiosque` remains the static library linked by Meson and exports the existing `com.kiosque` QML types.

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
- The application composition root is `rust/src/services.rs`. Production constructors use production endpoints and paths. Dedicated `test-util` builds can install isolated services before any QObject is created; release builds do not export that bootstrap.
- Registry owns installed/update/job state. TransactionManager serializes backend operations, deduplicates complete operations including scope and remote, and publishes terminal state after reloading installed refs. `shutdown()` cancels pending operations and joins the worker. Worker ownership uses a weak reference to avoid an idle Arc cycle.
- Qt models project registry snapshots into serializable core entries. The adapter's compatibility helpers retain existing UI method names while sending mutations through the shared transaction client. Model/controller event subscriptions stop when their Rust QObject state is destroyed.
- Each HTTP client owns its transport, API base and cache handle. Production constructors share the production cache; explicit constructors allow independent caches/endpoints. Settings persistence and disk caches accept explicit paths; saving settings does not invoke a portal or systemctl. Background/autostart integration remains an explicit operation.
- Image decoding returns owned pixels from core. Only the adapter exposes raw C buffers, and the shared C++ image provider frees each allocation. Provider URLs encode the underlying URL, so changing metadata cannot reuse an old app-ID image mapping.

## Execution and compatibility

Tokio performs network, filesystem and process work. Qt changes are posted through cxx-qt thread handles. App-list request generations reject responses for superseded requests. The native backend runs libflatpak work on blocking workers; CLI children are owned and reaped during cancellation.

Meson builds the explicit `libkiosque` package, links its static library and exported cxx-qt initializer object, and embeds QML resources. Keep the initializer export integration aligned with cxx-qt upgrades; its export layout is not a stable public interface.

The new core cache format is versioned. Old incompatible cache files are ignored. No settings schema migration is required; older records without an ODRS salt receive a generated salt when loaded.

The updater's existing C++ tray/one-shot shell is retained. The core daemon modules expose the ongoing Rust daemon refactor; a broader rewrite of the C++ shell is not part of the test architecture migration.
