#!/usr/bin/env python3
"""Boot a checksum-pinned disposable guest. Never mount host installation paths."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def run(command, **kwargs):
    return subprocess.run([str(x) for x in command], check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['native', 'cli'], default='native')
    parser.add_argument('--artifact', type=Path)
    args = parser.parse_args()
    for tool in ['qemu-system-x86_64', 'qemu-img', 'cloud-localds', 'ssh-keygen', 'ssh', 'scp']:
        if not shutil.which(tool):
            raise RuntimeError(f'missing prerequisite: {tool}')
    pin = json.loads((ROOT / 'tests/system/image.json').read_text())
    cache = ROOT / '.cache/vm'
    cache.mkdir(parents=True, exist_ok=True)
    image = cache / Path(pin['url']).name
    if not image.exists():
        download = image.with_suffix('.download')
        urllib.request.urlretrieve(pin['url'], download)
        download.replace(image)
    with image.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    if digest != pin['sha256']:
        raise RuntimeError('VM image checksum mismatch; remove the cached image and retry')
    results = Path(os.environ.get('KIOSQUE_RESULT_DIR', ROOT / '.test-results'))
    results.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='kiosque-vm-') as temporary:
        work = Path(temporary)
        key = work / 'id_ed25519'
        run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-f', key])
        cloud = work / 'user-data'
        cloud.write_text('''#cloud-config
users:
  - name: root
    lock_passwd: true
    ssh_authorized_keys:
      - ''' + key.with_suffix('.pub').read_text().strip() + '''
disable_root: false
ssh_pwauth: false
''')
        (work / 'meta-data').write_text('instance-id: kiosque-disposable\nlocal-hostname: kiosque-test\n')
        run(['cloud-localds', work / 'seed.iso', cloud, work / 'meta-data'])
        run(['qemu-img', 'create', '-f', 'qcow2', '-F', 'qcow2', '-b', image.resolve(), work / 'guest.qcow2', '24G'])
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            port = reservation.getsockname()[1]
        archive = work / 'source.tar.gz'
        excluded = {'.git', '.agents', '.aws', '.codex', '.cache', '.test-results', '.flatpak-builder', '__pycache__', 'target', 'build', 'builddir', 'AppDir'}
        with tarfile.open(archive, 'w:gz') as tar:
            for directory, children, names in os.walk(ROOT):
                children[:] = [name for name in children if name not in excluded and not (Path(directory) == ROOT and name == 'repo')]
                for name in names:
                    path = Path(directory) / name
                    if path.is_file() and not path.is_symlink() and path.suffix.lower() not in {'.rpm', '.flatpak', '.appimage'}:
                        tar.add(path, arcname=path.relative_to(ROOT), recursive=False)
        common = ['-i', str(key), '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=5', '-o', 'StrictHostKeyChecking=accept-new', '-o', f'UserKnownHostsFile={work / "known_hosts"}']
        ssh = ['ssh', *common, '-p', str(port), 'root@127.0.0.1']
        accelerator = 'kvm' if os.access('/dev/kvm', os.R_OK | os.W_OK) else 'tcg'
        with (results / 'vm-console.log').open('w') as console:
            guest = subprocess.Popen(['qemu-system-x86_64', '-m', '2048', '-smp', '2', '-accel', accelerator,
                '-cpu', 'host' if accelerator == 'kvm' else 'max', '-nographic', '-drive', f'file={work / "guest.qcow2"},if=virtio', '-drive', f'file={work / "seed.iso"},format=raw,media=cdrom',
                '-netdev', f'user,id=network,hostfwd=tcp:127.0.0.1:{port}-:22', '-device', 'virtio-net-pci,netdev=network'], stdout=console, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 600
                while time.monotonic() < deadline:
                    if guest.poll() is not None:
                        raise RuntimeError('QEMU exited before SSH became available; see vm-console.log')
                    probe = subprocess.run([*ssh, 'true'], capture_output=True)
                    if probe.returncode == 0:
                        break
                    time.sleep(2)
                else:
                    raise RuntimeError('guest SSH readiness timed out')
                run(['scp', *common, '-P', str(port), archive, 'root@127.0.0.1:/tmp/source.tar.gz'])
                if args.artifact:
                    run(['scp', *common, '-P', str(port), args.artifact.resolve(), 'root@127.0.0.1:/tmp/'])
                command = f'mkdir -p /work; tar -xzf /tmp/source.tar.gz -C /work; cd /work; python3 tests/system/guest.py --backend {args.backend}'
                if args.artifact:
                    import shlex
                    command += ' --artifact ' + shlex.quote('/tmp/' + args.artifact.name)
                try:
                    result = subprocess.run([*ssh, command], timeout=3000)
                finally:
                    # Timeouts must retain diagnostics before the guest is destroyed too.
                    try:
                        subprocess.run(['scp', *common, '-P', str(port), '-r', 'root@127.0.0.1:/work/.test-results/.', str(results)], check=False, timeout=30)
                    except (OSError, subprocess.SubprocessError) as error:
                        print(f'Could not retrieve guest diagnostics: {error}', file=sys.stderr)
                if result.returncode:
                    raise RuntimeError(f'guest tests exited {result.returncode}')
            finally:
                guest.terminate()
                try:
                    guest.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    guest.kill(); guest.wait()


if __name__ == '__main__':
    try:
        main()
    except (RuntimeError, OSError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
