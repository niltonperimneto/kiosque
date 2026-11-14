---
name: kiosque-rust-core
description: Implement and test Kiosque domain, service, cache, configuration and scheduling changes without introducing Qt dependencies.
---

# kiosque-rust-core

Read [architecture](../../docs/architecture.md) and scoped Rust guidance. Put domain data and independently constructible services in `rust/core/`; keep Qt model conversion and FFI in the adapter. Inject paths, HTTP client/endpoints, backend and time where the behavior needs them. Production constructors retain production defaults. Use temporary paths and local fixtures. Verify with `python3 scripts/test.py core` and `services`. Retain both application feature configurations when shared types change.
