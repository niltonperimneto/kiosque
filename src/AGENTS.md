# C++ shells and FFI

- C++ owns Qt application startup, engine resources, image ownership, and desktop integration. Domain logic belongs in the Rust core.
- Match Rust/C++ FFI signatures exactly. Free transferred buffers exactly once; marshal callbacks to the owning Qt thread.
- Do not add test invokables to release QML types. Test support is linked into separate harness binaries.
- Validate with `python3 scripts/test.py qt` and `python3 scripts/test.py qml`.
