#!/usr/bin/env python3
"""Export signed local Flatpak fixtures; this does not install or remove applications."""
import base64
import os
from pathlib import Path
import shutil
import subprocess
import sys


def run(command, **kwargs):
    return subprocess.run([str(item) for item in command], check=True, **kwargs)


def publish(root, revision):
    gpg = root / 'gpg'; gpg.mkdir(parents=True, exist_ok=True, mode=0o700)
    if not (root / 'key-id').exists():
        run(['gpg', '--homedir', gpg, '--batch', '--passphrase', '', '--quick-generate-key', 'Kiosque Synthetic Fixture', 'rsa2048', 'sign', '0'])
        keys = subprocess.check_output(['gpg', '--homedir', str(gpg), '--with-colons', '--list-secret-keys'], text=True)
        fingerprint = next(line.split(':')[9] for line in keys.splitlines() if line.startswith('fpr:'))
        (root / 'key-id').write_text(fingerprint)
    key = (root / 'key-id').read_text()
    signing = ['--gpg-sign=' + key, '--gpg-homedir=' + str(gpg)]
    repo = root / 'repo'
    architecture = subprocess.check_output(['flatpak', '--default-arch'], text=True).strip()
    runtime = root / 'runtime'
    if not (root / 'runtime-exported').exists():
        (runtime / 'usr').mkdir(parents=True, exist_ok=True)
        (runtime / 'metadata').write_text(f'[Runtime]\nname=org.kiosque.TestPlatform\nruntime=org.kiosque.TestPlatform/{architecture}/stable\nsdk=org.kiosque.TestPlatform/{architecture}/stable\n')
        run(['flatpak', 'build-export', '--runtime', '--files=usr', *signing, repo, runtime, 'stable'])
        (root / 'runtime-exported').touch()
    app = root / 'app'
    shutil.rmtree(app, ignore_errors=True)
    (app / 'files/bin').mkdir(parents=True)
    # The synthetic app has no desktop exports, but requires a finalized export tree.
    (app / 'export').mkdir()
    source = root / 'fixture.c'
    source.write_text(f'#include <stdio.h>\nint main(void) {{ puts("fixture revision {revision}"); return 0; }}\n')
    run(['gcc', '-static', source, '-o', app / 'files/bin/fixture'])
    (app / 'metadata').write_text(f'[Application]\nname=org.kiosque.Fixture\nruntime=org.kiosque.TestPlatform/{architecture}/stable\nsdk=org.kiosque.TestPlatform/{architecture}/stable\ncommand=fixture\n')
    run(['flatpak', 'build-export', *signing, repo, app, 'stable'])
    key_data = subprocess.check_output(['gpg', '--homedir', str(gpg), '--export', key])
    (root / 'fixture.flatpakrepo').write_text('[Flatpak Repo]\nTitle=Kiosque Synthetic Fixture\nUrl=' + repo.as_uri() + '\nGPGKey=' + base64.b64encode(key_data).decode() + '\n')


if __name__ == '__main__':
    publish(Path(sys.argv[1]).resolve(), int(sys.argv[2]))
