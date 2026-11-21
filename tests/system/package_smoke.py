#!/usr/bin/env python3
"""Smoke-test an actual artifact; Flatpak installation is restricted to the VM."""
import os
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
        if artifact.suffix == '.flatpak':
            if not Path('/run/kiosque-test-vm').exists():
                raise RuntimeError('Flatpak package installation requires tests/system/vm.py --artifact')
            run(['flatpak', 'remote-add', '--user', '--if-not-exists', 'flathub', 'https://dl.flathub.org/repo/flathub.flatpakrepo'], env=env)
            run(['flatpak', 'install', '--user', '-y', '--noninteractive', 'flathub', 'org.kde.Platform//6.10'], env=env)
            run(['flatpak', 'install', '--user', '-y', '--noninteractive', '--no-gpg-verify', artifact], env=env)
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
            run([executable, '--appimage-extract'], cwd=root)
            root = root / 'squashfs-root'
            app = [root / 'AppRun', '--check-ui']
            helper = [root / 'usr/bin/kiosque-update', '--help']
            env['LD_LIBRARY_PATH'] = ':'.join(str(path) for path in [root / 'usr/lib', root / 'usr/lib64'])
        else:
            raise RuntimeError('supported artifacts: .rpm, .flatpak, .AppImage')
        if artifact.suffix == '.flatpak':
            if companion is None or not companion.is_file():
                raise RuntimeError('Flatpak qualification requires the matching native RPM via --companion')
            probe = ['unshare', '--net', '--', 'xvfb-run', '-a', 'dbus-run-session', '--',
                     *app[:-1], '--check-backend']
            output = run(probe, env=env, expected=1)
            if 'KIOSQUE_BACKEND_ERROR host-companion:' not in output:
                raise RuntimeError('missing companion did not produce a broker error')
            run(['dnf', 'install', '-y', companion], env=env)
            if 'KIOSQUE_BACKEND_READY host-companion ' not in run(probe, env=env):
                raise RuntimeError('packaged client could not read installed state through the companion')
        if artifact.suffix != '.flatpak' and not list(root.rglob('kiosque.mo')):
            raise RuntimeError('artifact contains no compiled translations')
        output = run(['unshare', '--net', '--', 'xvfb-run', '-a', 'dbus-run-session', '--', *app], env=env)
        if 'KIOSQUE_UI_READY' not in output:
            raise RuntimeError('application did not confirm QML/resource startup')
        run(['unshare', '--net', '--', 'xvfb-run', '-a', 'dbus-run-session', '--', *helper], env=env)


if __name__ == '__main__':
    try:
        main(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else None)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
