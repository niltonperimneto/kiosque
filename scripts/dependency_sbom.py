#!/usr/bin/env python3
"""Emit an SPDX inventory of exact locked Cargo sources and checksums."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import sys
import tomllib


def generate(lock):
    raw = Path(lock).read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    packages = tomllib.loads(raw.decode())['package']
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
