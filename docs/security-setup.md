# Maintainer setup after security hardening

Start with the pull request checks. OAuth registration and installation of a native host package require your account or administrator session; CI does not need an OAuth client secret.

## 1. Review and merge the pull request

Require green `Security audit`, `Core and service contracts`, `Desktop (native)` and `Desktop (cli)` checks. The packaging jobs also need to succeed. The PR includes the retained consent-dialog, localization, image and installed-page changes exercised by the security regression suite.

Main already requires pull requests, up-to-date core/desktop checks and resolved conversations, including administrators. Force pushes and deletion are disabled. Dependency alerts, Dependabot security fixes, secret scanning/push protection and private vulnerability reporting are enabled.

After the first `Security audit` run appears, add it to the existing required checks under repository **Settings → Branches → main protection** if it is not already listed. Preserve the existing requirements. No additional Actions secrets are needed for auditing, scans or provenance; workflows use their scoped GitHub token and OIDC permissions.

The new CodeQL workflow runs on main after merge and on its schedule. Review **Security → Code scanning** once it finishes. Generic-secret scanning and validity checking remained disabled through the repository API; Gitleaks supplies a separate source/history scan. If GitHub offers those options for this repository under **Settings → Advanced Security**, they can be enabled there too.

## 2. Configure GitHub login

1. Open [Developer settings → OAuth Apps](https://github.com/settings/developers). Edit the Kiosque app you own, or register a new OAuth app with a public homepage such as this repository.
2. Enable **Device Flow**. Kiosque uses device authorization; its GitHub flow does not receive a browser callback or require a client secret. If registration requires a callback field, `http://127.0.0.1/oauth/callback` is unused by the GitHub device flow.
3. Copy the public **Client ID**. The source currently has a default GitHub client ID; use an explicit override if that registration is not yours.
4. Launch the native application with your ID:

   ```sh
   KIOSQUE_GITHUB_CLIENT_ID=YOUR_PUBLIC_CLIENT_ID kiosque
   ```

   For the Flatpak application:

   ```sh
   flatpak run --env=KIOSQUE_GITHUB_CLIENT_ID=YOUR_PUBLIC_CLIENT_ID org.kiosque.Kiosque
   ```

   To retain that public ID for subsequent Flatpak launches:

   ```sh
   flatpak override --user --env=KIOSQUE_GITHUB_CLIENT_ID=YOUR_PUBLIC_CLIENT_ID org.kiosque.Kiosque
   ```

5. In Kiosque Settings, choose GitHub, confirm the displayed code on GitHub, and authorize the profile request. Confirm that your profile appears. Also check cancellation and logout.

Keep token expiry enabled. Do not generate or embed a client secret for this desktop flow. GitHub documents the registration and Device Flow option in [Creating an OAuth app](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/creating-an-oauth-app).

## 3. Configure optional GitLab providers

These providers currently have placeholder client IDs. Skip a provider you do not intend to support.

On each intended instance, go to **Edit profile → Access → Applications** and register a public/non-confidential application with only `read_user`. Use `http://127.0.0.1/oauth/callback` as the redirect URI. Kiosque binds an available loopback port for each login and sends that exact URI in both authorization and token requests.

| Instance | Runtime environment variable |
| --- | --- |
| `gitlab.com` | `KIOSQUE_GITLAB_CLIENT_ID` |
| `gitlab.gnome.org` | `KIOSQUE_GNOME_CLIENT_ID` |
| `invent.kde.org` | `KIOSQUE_KDE_CLIENT_ID` |

Use the same native or Flatpak launch pattern shown for GitHub, substituting the variable and that instance's Application ID. Do not supply a client secret. Test sign-in separately on each instance; older deployments may differ in loopback redirect handling. If the instance rejects the changing port, retain PKCE and public-client settings and report the instance/version and redacted error before changing the integration.

GitLab documents [application registration](https://docs.gitlab.com/integration/oauth_provider/) and [PKCE without a client secret](https://docs.gitlab.com/api/oauth2/#authorization-code-with-proof-key-for-code-exchange-pkce). Its OAuth library [permits loopback ports to vary](https://github.com/doorkeeper-gem/doorkeeper/blob/main/lib/doorkeeper/oauth/helpers/uri_checker.rb); the deployed instance still needs a live compatibility check.

## 4. Install the native companion on Fedora

The Flatpak application now needs the native Kiosque host package. That package provides `kiosque-update`, its session-bus activation file and the sibling native executable used by the caller policy. Installing only the Flatpak or extracting an AppImage does not install the host activation service.

After merge, use the **CI** run for the exact merged commit. Download its `Kiosque-RPM` artifact and verify provenance before installing it:

```sh
gh attestation verify /path/to/kiosque.rpm \
  --repo niltonperimneto/kiosque \
  --signer-workflow niltonperimneto/kiosque/.github/workflows/ci.yml \
  --source-digest MERGED_COMMIT_SHA \
  --deny-self-hosted-runners
sudo dnf install /path/to/kiosque.rpm
```

Replace the file and commit placeholders with the actual artifact and its source commit. The trusted main build creates these attestations; an unmerged PR artifact does not have them.

Run Kiosque as your normal desktop user. The host companion uses the **session** bus and should activate on demand. Do not run it with sudo, grant `org.freedesktop.Flatpak` back to the sandbox, or weaken caller checks to make activation work. After the GUI requests installed applications, check its owner with:

```sh
busctl --user --no-pager status org.kiosque.Kiosque.Daemon
```

A direct `busctl` call to the restricted application interface is expected to be denied, so it is not an authorization smoke test. First verify read-only behavior in the GUI. Exercise installation, removal, cancellation and user/system scope in a disposable VM before treating the companion integration as release-qualified.

## 5. Complete release qualification

After merging, let the trusted main CI run finish. Its package smoke workflow verifies provenance before executing each RPM, Flatpak and AppImage in a disposable VM.

In **Actions → System and package qualification → Run workflow**, select main and run the signed native/CLI fixture contracts. A scheduled run performs the same qualification. Before a release, require both backend jobs, all three exact-package smoke checks and CodeQL to be green for the intended source revision. Do not create a release tag as a way to bypass qualification.

The development host lacked `/dev/kvm`; its earlier software-emulated VM build exceeded the deadline. An administrator can make hardware virtualization/KVM available on a suitable machine if local qualification is needed. The shared commands are:

```sh
python3 scripts/test.py system --backend native
python3 scripts/test.py system --backend cli
python3 scripts/test.py packages --artifact /path/to/the/exact/package
```

These commands launch disposable environments; they must not be replaced with install/remove tests on your developer Flatpak installations.

See [the hardening record](security-hardening.md) for completed checks and the remaining trust boundaries. The SPDX report currently inventories Rust dependencies, not every system library bundled into a release artifact.
