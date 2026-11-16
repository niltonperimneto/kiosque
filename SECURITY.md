# Security reports

Report vulnerabilities privately through [GitHub's private vulnerability reporting](https://github.com/niltonperimneto/kiosque/security/advisories/new).
Include the affected revision, reproduction steps using synthetic data, and expected impact. Do not include real OAuth credentials or personal settings files.

The application trusts the operating system, installed Flatpak tooling and the current Unix account. The native companion constrains other applications' D-Bus access; it does not defend an account already controlled through ptrace, injected libraries or modified executables. System installations continue to use Flatpak's normal host authorization.

See [the hardening record](docs/security-hardening.md) for implemented boundaries, validation evidence and outstanding release qualification.
