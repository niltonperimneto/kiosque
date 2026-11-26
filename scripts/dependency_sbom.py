#!/usr/bin/env python3
"""Validate locked Cargo provenance and emit an SPDX inventory of exact sources and checksums."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib

CRATES_IO_SOURCE = 'registry+https://github.com/rust-lang/crates.io-index'
SHA256_RE = re.compile(r'^[0-9a-f]{64}$')


def workspace_packages(manifest):
    root_manifest = Path(manifest)
    root = tomllib.loads(root_manifest.read_text())
    for section in ('patch', 'replace'):
        if section in root:
            raise ValueError(f'{root_manifest} contains disallowed [{section}] override')
    names = set()
    if 'package' in root and 'name' in root['package']:
        names.add(root['package']['name'])
    for member in root.get('workspace', {}).get('members', []):
        member_path = root_manifest.parent / member / 'Cargo.toml'
        member_toml = tomllib.loads(member_path.read_text())
        for section in ('patch', 'replace'):
            if section in member_toml:
                raise ValueError(f'{member_path} contains disallowed [{section}] override')
        names.add(member_toml['package']['name'])
    return names


def verify_lockfile(parsed, allowed_local=None):
    if 'patch' in parsed:
        raise ValueError('Cargo.lock contains disallowed [patch] entries')
    packages = parsed.get('package', [])
    if not packages:
        raise ValueError('Cargo.lock contains no packages')
    for pkg in packages:
        name = pkg['name']
        version = pkg['version']
        source = pkg.get('source')
        if source is None:
            if allowed_local is not None and name not in allowed_local:
                raise ValueError(f'untrusted local/path package in Cargo.lock: {name} {version}')
            if 'checksum' in pkg:
                raise ValueError(f'unexpected checksum on workspace package: {name} {version}')
            continue
        if source != CRATES_IO_SOURCE:
            raise ValueError(f'disallowed dependency source for {name} {version}: {source}')
        checksum = pkg.get('checksum', '')
        if not SHA256_RE.fullmatch(checksum):
            raise ValueError(f'missing or invalid SHA-256 checksum for {name} {version}')


def generate(lock, manifest=None):
    lock_path = Path(lock)
    raw = lock_path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    parsed = tomllib.loads(raw.decode())
    manifest_path = Path(manifest) if manifest is not None else lock_path.with_name('Cargo.toml')
    allowed_local = workspace_packages(manifest_path) if manifest_path.is_file() else None
    verify_lockfile(parsed, allowed_local)
    packages = parsed['package']
    ids = {(p['name'], p['version']): f'SPDXRef-package-{i}' for i, p in enumerate(packages)}
    by_name = {}
    for name, version in ids:
        by_name.setdefault(name, []).append(version)
    output = []
    relationships = []
    for p in packages:
        identifier = ids[p['name'], p['version']]
        package = {'name': p['name'], 'versionInfo': p['version'], 'SPDXID': identifier,
                   'downloadLocation': p.get('source', 'NOASSERTION'), 'filesAnalyzed': False,
                   'licenseConcluded': 'NOASSERTION', 'licenseDeclared': 'NOASSERTION',
                   'copyrightText': 'NOASSERTION'}
        if 'checksum' in p:
            package['checksums'] = [{'algorithm': 'SHA256', 'checksumValue': p['checksum']}]
        output.append(package)
        relationships.append({'spdxElementId': 'SPDXRef-DOCUMENT', 'relationshipType': 'DESCRIBES', 'relatedSpdxElement': identifier})
        for dependency in p.get('dependencies', []):
            parts = dependency.split()
            versions = by_name[parts[0]]
            version = parts[1] if len(parts) > 1 else versions[0] if len(versions) == 1 else None
            if (parts[0], version) in ids:
                relationships.append({'spdxElementId': identifier, 'relationshipType': 'DEPENDS_ON', 'relatedSpdxElement': ids[parts[0], version]})
    return {'spdxVersion': 'SPDX-2.3', 'dataLicense': 'CC0-1.0', 'SPDXID': 'SPDXRef-DOCUMENT',
            'name': 'Kiosque locked Rust dependencies', 'documentNamespace': f'https://kiosque.app/spdx/{digest}',
            'creationInfo': {'created': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'), 'creators': ['Tool: Kiosque dependency_sbom.py']},
            'documentComment': 'Lockfile inventory, including optional/build/test crates; system and packaged runtime libraries require artifact-level inventory.',
            'packages': output, 'relationships': relationships}


if __name__ == '__main__':
    Path(sys.argv[2]).write_text(json.dumps(generate(sys.argv[1]), indent=2) + '\n')
