---
name: kiosque-verify-change
description: Select and run Kiosque verification suites for a code or guidance change, and report concrete evidence.
---

# kiosque-verify-change

Inspect the diff and consult [the test matrix](../../docs/testing.md). Documentation/skill edits require `guidance`; core/domain edits require `core` and relevant `services`; bridge/QML changes require `qt` and `qml`; backend changes add both native/broker checks and VM contracts; package changes require artifact smoke checks. Invoke `python3 scripts/test.py <suite>` from the repository root. Report the exact commands, results, affected contracts, and any unavailable prerequisites. Never report a skipped or unbuilt layer as passing.
