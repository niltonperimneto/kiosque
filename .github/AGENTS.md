# CI

- Invoke shared `scripts/test.py` suites; do not maintain separate test implementations in YAML.
- Keep pull-request jobs unprivileged and free of secrets. Never execute untrusted pull requests on a privileged system-test runner.
- Run real Flatpak operations only in disposable QEMU guests on schedules/manual dispatch or trusted release commits.
- Upload reports/logs even after failure. Do not hide missing prerequisites or flakes with unconditional success or retries.
- Keep RPM, Flatpak and AppImage builds and validate the produced artifacts.
