---
name: kiosque-flatpak-backends
description: Change Kiosque native and CLI Flatpak backends, transaction state, progress and cancellation with behavior parity.
---

# kiosque-flatpak-backends

Read [system testing](../../docs/testing.md). Preserve remote, ref, branch, architecture and installation scope when comparing operations. Exercise parsers and fake-process execution locally, then the real native and CLI backends against the VM fixture repository. Cancellation must stop owned helpers and leave terminal registry state. Test sandbox host routing independently. Never validate install/remove behavior against a developer installation. Run `python3 scripts/test.py system` only through its disposable-VM launcher.
