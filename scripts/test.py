#!/usr/bin/env python3
"""Shared local/CI entry point. Missing prerequisites and failed suites exit nonzero."""
import argparse
import contextlib
import json
import hashlib
import platform
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import signal
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / '.test-results'
RECORDS = []


def run(command, *, env=None, timeout=900):
    command = [str(part) for part in command]
    print('+ ' + ' '.join(command), flush=True)
    start = time.monotonic()
    RESULTS.mkdir(parents=True, exist_ok=True)
    index = len(RECORDS) + 1
    output = []
    with (RESULTS / f'{index:02d}-{Path(command[0]).name}.log').open('w') as log:
        process = subprocess.Popen(command, cwd=ROOT, env=env, text=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        def consume():
            for line in process.stdout:
                output.append(line)
                log.write(line); log.flush()
                print(line, end='', flush=True)
        reader = threading.Thread(target=consume, daemon=True)
        reader.start()
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            code = 124
        reader.join(timeout=10)
    text = ''.join(output)
    RECORDS.append((' '.join(command), time.monotonic() - start, code, text))
    if code:
        raise RuntimeError(f'command exited {code}: {command[0]}')
    return text


def require(*names):
    missing = [name for name in names if not shutil.which(name)]
    if missing:
        raise RuntimeError('missing prerequisites: ' + ', '.join(missing))


def cargo(*args):
    require('cargo')
    return run(['cargo', *args[:1], '--manifest-path', ROOT / 'rust/Cargo.toml', *args[1:]])


def guidance():
    skills = sorted((ROOT / 'skills').glob('kiosque-*/SKILL.md'))
    if len(skills) != 6:
        raise RuntimeError('expected six project skills')
    documents = [ROOT / 'AGENTS.md', *ROOT.glob('docs/**/*.md'), *skills]
    documents += [ROOT / directory / 'AGENTS.md' for directory in ('rust', 'qml', 'tests', '.github')]
    for document in documents:
        content = document.read_text()
        if document.name == 'SKILL.md':
            match = re.match(r'---\nname: ([a-z0-9-]{1,63})\ndescription: (.+)\n---\n', content)
            if not match or match[1] != document.parent.name:
                raise RuntimeError(f'invalid skill frontmatter: {document}')
        for link in re.findall(r'\]\(([^)]+)\)', content):
            if '://' not in link and not link.startswith('#'):
                target = document.parent / link.split('#', 1)[0]
                if not target.exists():
                    raise RuntimeError(f'broken link in {document}: {link}')
    no_cpp()
    run([sys.executable, ROOT / "scripts/update_pot.py", "--check"])
    run([sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/runner', '-v'])
    print(f'Validated {len(documents)} guidance documents and six project skills.')


CPP_PATTERNS = ('*.cpp', '*.cc', '*.cxx', '*.h', '*.hh', '*.hpp', '*.hxx', '*.qrc')


def no_cpp():
    """The project is Rust + QML only; generated cxx shims live in target/."""
    if (ROOT / '.git').exists() and shutil.which('git'):
        listed = subprocess.run(['git', 'ls-files', *CPP_PATTERNS], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    else:
        skip = {'target', 'build', '.git', '.flatpak-builder', '.test-results'}
        listed = [str(p.relative_to(ROOT)) for pattern in CPP_PATTERNS for p in ROOT.rglob(pattern)
                  if not skip.intersection(p.relative_to(ROOT).parts)]
    if listed:
        raise RuntimeError('C/C++ sources are not allowed: ' + ', '.join(sorted(listed)))


def security():
    require('python3', 'tar', 'git')
    tools = RESULTS / 'security-tools'
    tools.mkdir(parents=True, exist_ok=True)
    archive = tools / 'cargo-audit.tgz'
    run(['python3', 'scripts/download_build_tool.py', 'cargo-audit', archive])
    run(['tar', '-xzf', archive, '-C', tools, '--strip-components=1'])
    run(['python3', 'scripts/dependency_sbom.py', ROOT / 'rust/Cargo.lock', RESULTS / 'rust-dependencies.spdx.json'])
    run([tools / 'cargo-audit', 'audit', '--file', ROOT / 'rust/Cargo.lock', '--json', '--deny', 'warnings'], timeout=600)
    secrets = tools / 'gitleaks.tgz'
    run(['python3', 'scripts/download_build_tool.py', 'gitleaks', secrets])
    run(['tar', '-xzf', secrets, '-C', tools])
    # Scan only repository sources, including uncommitted work; never tool homes,
    # ignored build outputs or symlinks into private locations.
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT).split(b'\0')
    with tempfile.TemporaryDirectory(prefix='kiosque-secret-scan-') as directory:
        snapshot = Path(directory)
        for raw in paths:
            if not raw:
                continue
            relative = Path(os.fsdecode(raw))
            source = ROOT / relative
            excluded = {'.git', '.agents', '.codex', '.aws', '.cache', '.test-results'}
            if not excluded.intersection(relative.parts) and source.is_file() and not source.is_symlink() and source.resolve().is_relative_to(ROOT):
                destination = snapshot / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, destination)
        run([tools / 'gitleaks', 'dir', snapshot, '--redact=100', '--no-banner', '--report-format', 'json', '--report-path', RESULTS / 'secrets-worktree.json'])
    run([tools / 'gitleaks', 'git', ROOT, '--log-opts=--all', '--redact=100', '--no-banner', '--report-format', 'json', '--report-path', RESULTS / 'secrets-history.json'])


def core():
    cargo('test', '--locked', '-p', 'kiosque-core', '--no-default-features', '--lib')
    cargo('test', '--locked', '-p', 'kiosque-core', '--no-default-features', '--doc')
    cargo('test', '--locked', '-p', 'kiosque-update', '--no-default-features')


def services():
    cargo('test', '--locked', '-p', 'kiosque-core', '--no-default-features', '--test', 'services')


def lint():
    cargo('fmt', '--all', '--check')
    cargo('clippy', '--locked', '-p', 'kiosque-core', '-p', 'kiosque-update', '--no-default-features', '--all-targets', '--', '-D', 'warnings')


def require_qmake():
    """Qt Bridge and cxx-qt locate Qt through qmake; honour QMAKE like their build scripts do."""
    qmake = os.environ.get('QMAKE')
    if qmake:
        if not (shutil.which(qmake) or Path(qmake).is_file()):
            raise RuntimeError(f'missing prerequisites: QMAKE={qmake} is not executable')
        return
    if not (shutil.which('qmake6') or shutil.which('qmake')):
        raise RuntimeError('missing prerequisites: qmake6 (or set QMAKE)')


@contextlib.contextmanager
def desktop_environment():
    from fixture_server import fixture_server
    with tempfile.TemporaryDirectory(prefix='kiosque-tests-') as root, fixture_server() as endpoint:
        env = os.environ.copy()
        for key, name in [('XDG_CACHE_HOME', 'cache'), ('XDG_CONFIG_HOME', 'config'), ('XDG_DATA_HOME', 'data'), ('XDG_RUNTIME_DIR', 'runtime')]:
            path = Path(root) / name
            path.mkdir(mode=0o700)
            env[key] = str(path)
        env.update(KIOSQUE_TEST_HTTP=endpoint, NO_PROXY='127.0.0.1,localhost', no_proxy='127.0.0.1,localhost', QT_QUICK_BACKEND='software',
                   QSG_RHI_BACKEND='software', KIOSQUE_TEST_ARTIFACTS=str(RESULTS),
                   QT_LOGGING_RULES='qt.qml.connections.warning=true')
        yield env


def desktop(suite, backend):
    require('meson', 'ninja', 'dbus-run-session', 'xvfb-run')
    require_qmake()
    if suite == 'qt':
        features = ['--no-default-features'] if backend == 'broker' else []
        cargo('clippy', '--locked', '-p', 'kiosque', *features, '--features', 'test-util', '--all-targets', '--', '-D', 'warnings')
        cargo('test', '--locked', '-p', 'kiosque', *features, '--lib')
    directory = Path(os.environ.get('KIOSQUE_TEST_BUILD_DIR', ROOT / 'build' / f'tests-{backend}'))
    setup = ['meson', 'setup', directory]
    if (directory / 'meson-private/coredata.dat').exists():
        setup.append('--reconfigure')
    if os.environ.get('KIOSQUE_CARGO_TARGET_DIR'):
        setup.append('-Dcargo_target_dir=' + os.environ['KIOSQUE_CARGO_TARGET_DIR'])
    run([*setup, '--buildtype=debug', '-Dtests=true', '-Dtest_support=true', f'-Dflatpak_backend={backend}'])
    run(['meson', 'compile', '-C', directory, '-j', os.environ.get('CARGO_BUILD_JOBS', '2')])
    with desktop_environment() as env:
        run(['xvfb-run', '-a', 'dbus-run-session', '--', 'meson', 'test', '-C', directory,
             '--suite', suite, '--no-rebuild', '--print-errorlogs', '--timeout-multiplier', '2'], env=env)


def selected_suites(paths):
    """Conservative change-to-contract mapping used by guidance and tooling."""
    selected = {'guidance'}
    for path in paths:
        if path.startswith('rust/core/'):
            selected.update(('core', 'services'))
        if path.startswith('rust/updater/'):
            selected.add('core')
        if path.startswith(('rust/app/', 'qml/', 'meson', 'tests/qml/', 'po/', 'scripts/build_rust.py', 'scripts/fixture_server.py')):
            selected.update(('core', 'services', 'qt', 'qml'))
        if '/flatpak/' in path or '/transactions' in path or '/daemon/' in path:
            selected.add('system')
        if path in ('rust/Cargo.toml', 'rust/Cargo.lock') or path.startswith('.github/'):
            selected.update(('core', 'services', 'qt', 'qml'))
        if path.endswith(('.spec', '.desktop', '.metainfo.xml')) or path == 'org.kiosque.Kiosque.json' or path.startswith('flatpak/'):
            selected.add('packages')
    return sorted(selected)


def environment_record(suite, backend):
    lock = ROOT / 'rust/Cargo.lock'
    record = {'suite': suite, 'backend': backend if suite in ('qt', 'qml', 'system', 'pr') else None, 'python': sys.version,
              'platform': platform.platform(), 'cargo_build_jobs': os.environ.get('CARGO_BUILD_JOBS'),
              'lock_sha256': hashlib.sha256(lock.read_bytes()).hexdigest() if lock.exists() else None,
              'tools': {}}
    names = ['cargo', 'rustc'] if suite in ('lint', 'core', 'services', 'qt', 'qml', 'pr') else []
    if suite in ('qt', 'qml', 'pr'):
        names += ['meson', 'ninja']
    for name in names:
        if shutil.which(name):
            try:
                result = subprocess.run([name, '--version'], capture_output=True, text=True, timeout=5)
                record['tools'][name] = (result.stdout or result.stderr).strip().splitlines()[:1]
            except (OSError, subprocess.TimeoutExpired):
                record['tools'][name] = ['version probe unavailable']
    (RESULTS / 'environment.json').write_text(json.dumps(record, indent=2) + '\n')


def main():
    global RESULTS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('suite', choices=['guidance', 'security', 'lint', 'core', 'services', 'qt', 'qml', 'pr', 'system', 'packages'])
    parser.add_argument('--backend', choices=['native', 'broker'], default='native')
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--companion', type=Path, help='matching kiosque-companion RPM (Flatpak and RPM package checks)')
    args = parser.parse_args()
    RESULTS = RESULTS / f'{args.suite}-{args.backend}' / f'{time.time_ns()}-{os.getpid()}'
    os.environ.setdefault('CARGO_BUILD_JOBS', '2')
    os.environ.setdefault('CARGO_TARGET_DIR', str(ROOT / 'rust/target'))
    RESULTS.mkdir(parents=True, exist_ok=True)
    os.environ['KIOSQUE_RESULT_DIR'] = str(RESULTS)
    environment_record(args.suite, args.backend)
    try:
        if args.suite == 'pr':
            guidance(); lint(); core(); services(); desktop('qt', args.backend); desktop('qml', args.backend)
        elif args.suite in ('qt', 'qml'):
            desktop(args.suite, args.backend)
        elif args.suite == 'system':
            run([sys.executable, ROOT / 'tests/system/vm.py', '--backend', args.backend], timeout=3600)
        elif args.suite == 'packages':
            if not args.artifact or not args.artifact.is_file():
                raise RuntimeError('packages requires --artifact pointing to a built artifact')
            command = [sys.executable, ROOT / 'tests/system/package_smoke.py', args.artifact.resolve()]
            if args.companion:
                command.append(args.companion.resolve())
            run(command, timeout=900)
        else:
            globals()[args.suite]()
    except (RuntimeError, OSError, subprocess.TimeoutExpired) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        RECORDS.append(('suite prerequisites/execution', 0, 1, str(error)))
        return 1
    finally:
        suite = ET.Element('testsuite', name=args.suite, tests=str(len(RECORDS)), failures=str(sum(code != 0 for _, _, code, _ in RECORDS)))
        for command, duration, code, output in RECORDS:
            case = ET.SubElement(suite, 'testcase', name=command, time=f'{duration:.3f}')
            ET.SubElement(case, 'system-out').text = output
            if code:
                ET.SubElement(case, 'failure', message=f'exit {code}').text = output
        ET.ElementTree(suite).write(RESULTS / f'{args.suite}-{args.backend}.xml', encoding='utf-8', xml_declaration=True)
    return 0


if __name__ == '__main__':
    sys.exit(main())
