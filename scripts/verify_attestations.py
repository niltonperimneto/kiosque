#!/usr/bin/env python3
"""Verify trusted package identity and source before executing smoke tests."""
import os
from pathlib import Path
import subprocess
import sys


def verify(directory):
    repository = os.environ['GITHUB_REPOSITORY']
    source = os.environ['KIOSQUE_SOURCE_SHA']
    packages = [p for p in Path(directory).rglob('*') if p.is_file() and p.suffix.lower() in {'.rpm', '.flatpak', '.appimage'}]
    if not packages:
        raise RuntimeError('No packages to verify')
    for package in packages:
        subprocess.run(['gh', 'attestation', 'verify', str(package), '--repo', repository,
                        '--signer-workflow', f'{repository}/.github/workflows/ci.yml',
                        '--source-digest', source, '--deny-self-hosted-runners'], check=True, timeout=120)


if __name__ == '__main__':
    verify(sys.argv[1])
