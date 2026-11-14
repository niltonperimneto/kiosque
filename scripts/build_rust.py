#!/usr/bin/env python3
"""Build the application library (with its cxx-qt initializers) and the kiosque-update binary for Meson."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--manifest', type=Path, required=True)
parser.add_argument('--target-dir', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--initializers', type=Path, required=True)
parser.add_argument('--updater', type=Path, required=True)
parser.add_argument('--localedir', required=True)
parser.add_argument('--profile', choices=['debug', 'release'], required=True)
parser.add_argument('--backend', choices=['native', 'cli'], required=True)
parser.add_argument('--test-support', choices=['true', 'false'], required=True)
args = parser.parse_args()
export = args.target_dir / 'cxxqt'
env = os.environ.copy()
env.update(CARGO_TARGET_DIR=str(args.target_dir), CXX_QT_EXPORT_DIR=str(export), CXX_QT_EXPORT_CRATE_kiosque='1')
env.setdefault('CARGO_BUILD_JOBS', '2')
# Compiled into kiosque_core::l10n as the last catalog search directory.
env['KIOSQUE_LOCALEDIR'] = args.localedir
command = ['cargo', 'build', '--locked', '--manifest-path', str(args.manifest), '-p', 'kiosque', '-p', 'kiosque-update']
if args.profile == 'release':
    command.append('--release')
if args.backend == 'cli':
    command.append('--no-default-features')
if args.test_support == 'true':
    command.extend(['--features', 'kiosque/test-util'])
subprocess.run(command, env=env, check=True)
shutil.copy2(args.target_dir / args.profile / 'libkiosque.a', args.output)
shutil.copy2(export / 'crates/kiosque/initializers.o', args.initializers)
shutil.copy2(args.target_dir / args.profile / 'kiosque-update', args.updater)
