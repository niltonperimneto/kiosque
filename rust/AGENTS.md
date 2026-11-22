# Rust

- `core/` owns domain/services; `app/src/bridge/` and application FFI adapt them to Qt. Avoid core dependencies on Qt adapter types.
- Default core tests: `python3 scripts/test.py core`. Service contracts: `python3 scripts/test.py services`.
- Test native and broker application configurations with `python3 scripts/test.py qt --backend native` and `--backend broker` in the desktop environment.
- Construct independent backends, caches, paths, and clients in tests. Do not overwrite process globals or mutate environment variables inside parallel Rust tests.
- Keep cancellation cooperative, join owned workers, and publish Qt changes on the GUI thread. Preserve the public QML type names and standalone Rust binary packaging.
