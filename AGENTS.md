# Working on Kiosque

Kiosque is a Linux Flatpak storefront: Rust domain/services → qtbridge models → Qt6/Kirigami QML, built with Meson and Cargo. Read [architecture](docs/architecture.md) before changing boundaries and [testing](docs/testing.md) before selecting checks.

## Local rules

- Preserve existing user changes. Inspect the current diff before editing; never reset or replace an unrelated refactor.
- Keep Qt out of `kiosque-core`. Core defaults must build without Qt, KDE, or libflatpak headers. The application enables the native backend by default on the host and uses the native companion inside Flatpak.
- Test externally observable behavior. Keep network, filesystem, backend, and clock dependencies isolated; never install/remove host Flatpaks to validate an ordinary change.
- Use `python3 scripts/test.py <suite>` for shared local/CI checks. Missing prerequisites are failures, not successful skips. Report commands and unrun layers honestly.
- Read the scoped AGENTS.md for each directory touched. Consult only the skills needed below; repository `skills/` is explicitly routed here, not automatically discovered.

## Skill routing

| Task | Read |
| --- | --- |
| Select checks for a change | [verify-change](skills/kiosque-verify-change/SKILL.md) |
| Domain, service, storage, scheduling | [rust-core](skills/kiosque-rust-core/SKILL.md) |
| Models, controllers, QML | [qt-qml](skills/kiosque-qt-qml/SKILL.md) |
| Native/broker operations and cancellation | [flatpak-backends](skills/kiosque-flatpak-backends/SKILL.md) |
| Package and runtime smoke checks | [package-smoke](skills/kiosque-package-smoke/SKILL.md) |
| Reproduce and diagnose test failures | [diagnose-test-failure](skills/kiosque-diagnose-test-failure/SKILL.md) |

## Role playbooks

Read [planner](docs/agents/planner.md), [implementer](docs/agents/implementer.md), [test author](docs/agents/test-author.md), or [reviewer](docs/agents/reviewer.md) when performing that role. These documents do not create agents or grant external-action permissions. Delegate only when explicitly authorized.
