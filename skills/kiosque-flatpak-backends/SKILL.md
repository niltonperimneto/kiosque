---
name: kiosque-flatpak-backends
description: Change Kiosque native and broker Flatpak backends, transaction state, progress and cancellation with behavior parity.
---

# kiosque-flatpak-backends

Read [system testing](../../docs/testing.md). Preserve remote, ref, branch, architecture and installation scope when comparing operations. Exercise native transactions against the signed VM fixture and the packaged broker against the matching host companion. Cancellation must stop active transactions and leave terminal registry state. Test sandbox host routing independently. Never validate install/remove behavior against a developer installation. Run `python3 scripts/test.py system` only through its disposable-VM launcher.
