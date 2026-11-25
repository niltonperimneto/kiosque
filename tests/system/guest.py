#!/usr/bin/env python3
"""Provision and execute contracts inside a disposable QEMU guest only."""
import argparse
import os
from pathlib import Path
import subprocess
import sys


def run(command, **kwargs):
    subprocess.run(command, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--backend', choices=['native'], required=True)
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--companion', type=Path, help='matching kiosque-companion RPM (Flatpak and RPM package checks)')
    args = parser.parse_args()
    if subprocess.check_output(['systemd-detect-virt', '--vm'], text=True).strip() not in ('qemu', 'kvm'):
        raise RuntimeError('system contracts require a disposable QEMU VM')
    Path('/run/kiosque-test-vm').touch()
    Path('.test-results').mkdir(exist_ok=True)
    packages = ['cargo', 'rust', 'gcc', 'gcc-c++', 'glibc-static', 'flatpak', 'flatpak-devel', 'gnupg2', 'dbus-daemon', 'python3', 'pkgconf-pkg-config']
    if args.artifact:
        packages += ['meson', 'ninja-build', 'cmake', 'qt6-qtbase-devel', 'qt6-qtbase-private-devel', 'qt6-qtdeclarative-devel', 'qt6-qtsvg', 'kf6-kirigami-devel', 'kf6-ki18n-devel', 'kf6-kirigami-addons', 'kf6-qqc2-desktop-style', 'xorg-x11-server-Xvfb', 'xauth', 'cpio', 'squashfs-tools']
    # The release repositories ship Rust 1.94; the workspace needs 1.98
    # (rust/Cargo.toml), which Fedora 44 provides through its updates.
    run(['dnf', '--setopt=install_weak_deps=False', 'install', '-y', *packages])

    root = Path('/var/tmp/kiosque-contract')
    root.mkdir(parents=True)
    env = os.environ.copy()
    env.update(KIOSQUE_SYSTEM_ROOT=str(root), FLATPAK_USER_DIR=str(root / 'user'),
               FLATPAK_SYSTEM_DIR=str(root / 'system'), XDG_CONFIG_HOME=str(root / 'config'),
               XDG_CACHE_HOME=str(root / 'cache'), XDG_DATA_HOME=str(root / 'data'),
               CARGO_BUILD_JOBS='2')
    if args.artifact:
        command = [sys.executable, 'scripts/test.py', 'packages', '--artifact', str(args.artifact)]
        if args.companion:
            command.extend(['--companion', str(args.companion)])
        run(command, env=env)
        return
    run([sys.executable, 'tests/system/publish_fixture.py', str(root), '1'], env=env)
    command = ['dbus-run-session', '--', 'cargo', 'test', '--locked', '--manifest-path', 'rust/Cargo.toml', '-p', 'kiosque-core', '--no-default-features', '--features', 'test-util']
    command.extend(['--features', 'libflatpak'])
    guard = root / 'guard'
    guard.mkdir()
    recorder = guard / 'flatpak'
    recorder.write_text('#!/bin/sh\ntouch "$KIOSQUE_SYSTEM_ROOT/cli-invoked"\nexit 99\n')
    recorder.chmod(0o755)
    env['KIOSQUE_FIXTURE_PATH'] = env['PATH']
    env['PATH'] = str(guard) + os.pathsep + env['PATH']
    command.extend(['--test', 'flatpak_system', '--', '--ignored', '--nocapture'])
    results = Path('.test-results'); results.mkdir(exist_ok=True)
    with (results / f'system-{args.backend}.log').open('w') as log:
        result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT)
    print((results / f'system-{args.backend}.log').read_text())
    if (root / 'cli-invoked').exists():
        raise RuntimeError('Kiosque invoked the Flatpak CLI')
    if result.returncode:
        raise RuntimeError(f'Flatpak contracts exited {result.returncode}')


if __name__ == '__main__':
    main()
