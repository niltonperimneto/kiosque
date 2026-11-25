#!/usr/bin/env python3
"""Resolve a reproducible source version for Cargo, Meson and binary packages."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess
import tomllib
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT = 'build-version.json'
RELEASE = re.compile(r'v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)')
VERSION = re.compile(r'\d+\.\d+\.\d+(?:-dev\.\d+)?(?:\+(?:g[0-9a-f]+|unknown)(?:\.dirty)?)?')


def git(root, *args):
    # Container builds can run as a different UID from the checkout owner.
    # Trust only the explicit source root for this metadata query.
    result = subprocess.run(['git', '-c', f'safe.directory={Path(root).resolve()}', '-C', str(root), *args],
                            capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError('Git version metadata unavailable: ' + result.stderr.strip())
    return result.stdout.strip()


def resolve(root=ROOT):
    root = Path(root)
    baseline = tomllib.loads((root / 'rust/Cargo.toml').read_text())['workspace']['package']['version']
    if not isinstance(baseline, str) or baseline.startswith('v') or not RELEASE.fullmatch(baseline):
        raise ValueError('workspace version must be major.minor.patch')
    # A snapshot belongs to an exported source tree. A live checkout always
    # derives its version from Git so stale packaging files cannot override it.
    if not (root / '.git').exists():
        snapshot = root / SNAPSHOT
        if snapshot.exists():
            data = json.loads(snapshot.read_text())
            if data.get('schema') != 1 or not isinstance(data.get('version'), str) or not VERSION.fullmatch(data['version']):
                raise ValueError('invalid source version snapshot')
            return data['version']
        return baseline + '-dev.0+unknown'
    if git(root, 'rev-parse', '--is-shallow-repository') == 'true':
        raise ValueError('versioning requires full Git history; fetch with --unshallow --tags')
    commit = git(root, 'rev-parse', '--short=12', 'HEAD')
    dirty = bool(git(root, 'status', '--porcelain', '--untracked-files=normal'))
    tags = git(root, 'tag', '--merged', 'HEAD').splitlines()
    releases = [tag for tag in tags if RELEASE.fullmatch(tag)]
    exact = set(git(root, 'tag', '--points-at', 'HEAD').splitlines())
    versions = {tag: tuple(map(int, RELEASE.fullmatch(tag).groups())) for tag in releases}
    if len({versions[tag] for tag in releases if tag in exact}) > 1:
        raise ValueError('conflicting release tags at HEAD')
    if releases:
        tag = max([tag for tag in releases if tag in exact] or releases, key=lambda candidate: versions[candidate])
        base = versions[tag]
        distance = int(git(root, 'rev-list', '--count', tag + '..HEAD'))
        if tag in exact and not dirty:
            return '.'.join(map(str, base))
        # Development builds following a release sort below the next release.
        base = max(tuple(map(int, baseline.split('.'))), (base[0], base[1], base[2] + 1))
    else:
        base = tuple(map(int, baseline.split('.')))
        distance = int(git(root, 'rev-list', '--count', 'HEAD'))
    return '.'.join(map(str, base)) + f'-dev.{distance}+g{commit}' + ('.dirty' if dirty else '')


def source_epoch(root=ROOT):
    root = Path(root)
    if (root / '.git').exists():
        return int(git(root, 'show', '-s', '--format=%ct', 'HEAD'))
    snapshot = root / SNAPSHOT
    value = json.loads(snapshot.read_text()).get('source_date_epoch', 0) if snapshot.exists() else 0
    if not isinstance(value, int) or value < 0:
        raise ValueError('invalid source date in version snapshot')
    return value


def write_metainfo(source, output, version, epoch):
    parser = ET.XMLParser(target=ET.TreeBuilder(insert_comments=True))
    tree = ET.parse(source, parser=parser)
    root = tree.getroot()
    releases = root.find('releases')
    if releases is None:
        releases = ET.SubElement(root, 'releases')
    existing = next((entry for entry in releases if entry.get('version') == version), None)
    if existing is None:
        attributes = {'version': version, 'type': 'development' if '-dev.' in version else 'stable'}
        if epoch:
            attributes['date'] = datetime.fromtimestamp(epoch, timezone.utc).date().isoformat()
        releases.insert(0, ET.Element('release', attributes))
    else:
        releases.remove(existing)
        releases.insert(0, existing)
    ET.indent(tree, space='  ')
    # Preserve the source declaration and license comments before the root.
    source_text = Path(source).read_text()
    prefix = source_text[:source_text.index('<component')]
    Path(output).write_text(prefix + ET.tostring(root, encoding='unicode') + '\n')


def rpm_version(version):
    return version.replace('-dev.', '~dev.').replace('+', '.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--format', choices=['version', 'rpm'], default='version')
    parser.add_argument('--write', type=Path, help='freeze version into an exported source tree')
    parser.add_argument('--metainfo', type=Path, help='generate package AppStream metadata')
    parser.add_argument('--output', type=Path, help='destination for generated metadata')
    parser.add_argument('--watch-files', action='store_true', help='Cargo rebuild dependencies')
    args = parser.parse_args()
    if args.watch_files:
        paths = git(ROOT, 'ls-files').splitlines() if (ROOT / '.git').exists() else [SNAPSHOT, 'rust/Cargo.toml']
        if (ROOT / '.git').exists():
            git_dir = Path(git(ROOT, 'rev-parse', '--absolute-git-dir'))
            paths += [str(git_dir / 'HEAD'), str(git_dir / 'index'), git(ROOT, 'rev-parse', '--git-path', 'refs'), git(ROOT, 'rev-parse', '--git-path', 'packed-refs')]
        print('\n'.join(str(ROOT / path) for path in paths))
        return
    version = resolve()
    if args.write:
        args.write.write_text(json.dumps({'schema': 1, 'version': version, 'source_date_epoch': source_epoch()}, indent=2) + '\n')
    if args.metainfo:
        if args.output is None:
            parser.error('--metainfo requires --output')
        write_metainfo(args.metainfo, args.output, version, source_epoch())
    print(rpm_version(version) if args.format == 'rpm' else version)


if __name__ == '__main__':
    main()
