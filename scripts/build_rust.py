#!/usr/bin/env python3
"""Build Kiosque's binaries with Cargo and copy them where Meson expects them."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--manifest', type=Path, required=True)
parser.add_argument('--target-dir', type=Path, required=True)
parser.add_argument('--profile', choices=['debug', 'release'], required=True)
parser.add_argument('--backend', choices=['native', 'broker'], required=True)
parser.add_argument('--test-support', choices=['true', 'false'], required=True)
parser.add_argument('--version', required=True)
parser.add_argument('--localedir', required=True)
parser.add_argument('--out-dir', type=Path, required=True)
parser.add_argument('--bins', nargs='+', required=True)
args = parser.parse_args()

env = os.environ.copy()
env['CARGO_TARGET_DIR'] = str(args.target_dir)
env.setdefault('CARGO_BUILD_JOBS', '2')
# Compiled in: the version shown by `kiosque --version` and Qt.application,
# and the last directory searched for .mo catalogs.
env['KIOSQUE_VERSION'] = args.version
env['KIOSQUE_LOCALEDIR'] = args.localedir

command = ['cargo', 'build', '--locked', '--manifest-path', str(args.manifest),
           '-p', 'kiosque', '-p', 'kiosque-update']
if env.get('CARGO_NET_OFFLINE', '').lower() in ('1', 'true'):
    command.append('--offline')
if args.profile == 'release':
    command.append('--release')
if args.backend == 'broker':
    command.append('--no-default-features')
if args.test_support == 'true':
    command.extend(['--features', 'kiosque/test-util'])
subprocess.run(command, env=env, check=True)
for name in args.bins:
    shutil.copy2(args.target_dir / args.profile / name, args.out_dir / name)
