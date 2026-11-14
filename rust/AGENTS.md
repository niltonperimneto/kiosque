# Rust

- `core/` owns domain/services; `src/bridge/` and application FFI adapt them to Qt. Avoid core dependencies on model entry structs or cxx-qt.
- Default core tests: `python3 scripts/test.py core`. Service contracts: `python3 scripts/test.py services`.
- Test native and CLI application configurations with `python3 scripts/test.py qt --backend native` and `--backend cli` in the desktop environment.
- Construct independent backends, caches, paths, and clients in tests. Do not overwrite process globals or mutate environment variables inside parallel Rust tests.
- Keep cancellation cooperative, join owned workers, and publish Qt changes on the GUI thread. Preserve static library and QML type names.
