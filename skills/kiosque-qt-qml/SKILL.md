---
name: kiosque-qt-qml
description: Implement and validate Kiosque Qt models, controllers, QML interactions and Rust/C++ ownership boundaries.
---

# kiosque-qt-qml

Read [architecture](../../docs/architecture.md) and [desktop tests](../../docs/testing.md). Test real registered `com.kiosque` types with isolated services. Assert role data and model notifications using Qt Test, and signals/interactions using Qt Quick Test. Post worker results to the Qt thread and tolerate destruction during pending work. Preserve localization and keyboard interaction. Use `python3 scripts/test.py qt` and `qml` for both backend configurations when bridge contracts change. Keep test support out of release invokables.
