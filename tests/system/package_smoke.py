#!/usr/bin/env python3
"""Smoke-test an actual artifact; Flatpak installation is restricted to the VM."""
import os
import pwd
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from test import desktop_environment


def run(command, *, expected=0, **kwargs):
    result = subprocess.run([str(item) for item in command], stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, timeout=600, **kwargs)
    print(result.stdout)
    if result.returncode != expected:
        raise RuntimeError(f'command failed ({result.returncode}): {command}')
    return result.stdout


def main(artifact, companion):
    if not Path('/run/kiosque-test-vm').exists():
        raise RuntimeError('package startup checks require tests/system/vm.py --artifact')
    with tempfile.TemporaryDirectory(prefix='kiosque-package-') as temporary, desktop_environment() as env:
        root = Path(temporary)
        helper = None
        as_user = []
        if artifact.suffix == '.flatpak':
            if not Path('/run/kiosque-test-vm').exists():
                raise RuntimeError('Flatpak package installation requires tests/system/vm.py --artifact')
            # Exercise the same per-user bus, runtime directory and UID checks
            # used on a desktop. Only package provisioning needs guest root.
            user = 'kiosque-smoke'
            run(['useradd', '--create-home', user])
            account = pwd.getpwnam(user)
            home = Path(account.pw_dir)
            runtime = Path('/run/user') / str(account.pw_uid)
            runtime.mkdir(mode=0o700, exist_ok=True)
            os.chown(runtime, account.pw_uid, account.pw_gid)
            env.update(HOME=str(home), XDG_RUNTIME_DIR=str(runtime), XDG_DATA_HOME=str(home / '.local/share'),
                       XDG_CONFIG_HOME=str(home / '.config'), XDG_CACHE_HOME=str(home / '.cache'))
            for variable in ('FLATPAK_USER_DIR', 'FLATPAK_SYSTEM_DIR'):
                env.pop(variable, None)
            as_user = ['runuser', '--user', user, '--']
            run([*as_user, 'flatpak', 'remote-add', '--user', '--if-not-exists', 'flathub', 'https://dl.flathub.org/repo/flathub.flatpakrepo'], env=env)
            run([*as_user, 'flatpak', 'install', '--user', '-y', '--noninteractive', 'flathub', 'org.kde.Platform//6.10'], env=env)
            run([*as_user, 'flatpak', 'install', '--user', '-y', '--noninteractive', '--bundle', artifact], env=env)
            app = ['flatpak', 'run', '--user', 'org.kiosque.Kiosque', '--check-ui']
            helper = ['flatpak', 'run', '--user', '--command=kiosque-update', 'org.kiosque.Kiosque', '--help']
        elif artifact.suffix == '.rpm':
            with subprocess.Popen(['rpm2cpio', artifact], stdout=subprocess.PIPE) as archive:
                extraction = subprocess.run(['cpio', '-id', '--no-absolute-filenames'], stdin=archive.stdout,
                                            cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
                archive.stdout.close()
                if extraction.returncode or archive.wait():
                    raise RuntimeError('RPM extraction failed')
            app = [root / 'usr/bin/kiosque', '--check-ui']
            helper = [root / 'usr/bin/kiosque-update', '--help']
        elif artifact.suffix.lower() == '.appimage':
            executable = root / 'artifact.AppImage'
            shutil.copy2(artifact, executable); executable.chmod(0o755)
            offset = run([executable, '--appimage-offset']).strip()
            if not offset.isdecimal():
                raise RuntimeError('AppImage did not report a valid filesystem offset')
            run(['unsquashfs', '-no-progress', '-o', offset, '-d', root / 'squashfs-root', executable])
            root = root / 'squashfs-root'
            app = [root / 'AppRun', '--check-ui']
            helper = [root / 'usr/bin/kiosque-update', '--help']
            env['LD_LIBRARY_PATH'] = ':'.join(str(path) for path in [root / 'usr/lib', root / 'usr/lib64'])
        else:
            raise RuntimeError('supported artifacts: .rpm, .flatpak, .AppImage')
        launch = ['unshare', '--net', '--', *as_user, 'xvfb-run', '-a', 'dbus-run-session', '--']
        if artifact.suffix == '.flatpak':
            if companion is None or not companion.is_file():
                raise RuntimeError('Flatpak qualification requires the matching native RPM via --companion')
            probe = [*launch, *app[:-1], '--check-backend']
            output = run(probe, env=env, expected=1)
            if 'KIOSQUE_BACKEND_ERROR host-companion:' not in output:
                raise RuntimeError('missing companion did not produce a broker error')
            run(['dnf', 'install', '-y', companion], env=env)
            if 'KIOSQUE_BACKEND_READY host-companion ' not in run(probe, env=env):
                raise RuntimeError('packaged client could not read installed state through the companion')
        if artifact.suffix != '.flatpak' and not list(root.rglob('kiosque.mo')):
            raise RuntimeError('artifact contains no compiled translations')
        output = run([*launch, *app], env=env)
        if 'KIOSQUE_UI_READY' not in output:
            raise RuntimeError('application did not confirm QML/resource startup')
        run([*launch, *helper], env=env)


if __name__ == '__main__':
    try:
        main(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else None)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
