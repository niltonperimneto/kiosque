---
name: kiosque-package-smoke
description: Validate built Kiosque RPM, Flatpak and AppImage artifacts for launch and runtime resources.
---

# kiosque-package-smoke

Use [package testing](../../docs/testing.md). Consume the artifact produced by the packaging job; do not silently replace it with a source-tree binary. Run `python3 scripts/test.py packages --artifact <path>` in the prepared isolated environment. Check startup, QML imports, image decoding, translations and helper availability. Preserve loader/QML logs on failure. Artifact creation or validation does not authorize publishing a release.
