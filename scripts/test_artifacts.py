#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])


def companion_rpm(directory):
    """The kiosque-companion RPM built alongside, which other packages need."""
    found, = directory.glob('kiosque-companion-[0-9]*.rpm')
    return found


# Storefront RPMs are qualified together with their companion; the companion
# alone has no storefront to start, and debug packages are not applications.
artifacts = sorted(path for path in root.rglob('*') if path.parent.name != 'companion' and (path.suffix.lower() in ('.flatpak', '.appimage') or path.match('kiosque-[0-9]*.rpm')))
if not artifacts:
    sys.exit('no packaged artifacts found')
for artifact in artifacts:
    command = [sys.executable, 'tests/system/vm.py', '--artifact', str(artifact.resolve())]
    if artifact.suffix == '.flatpak':
        command.extend(['--companion', str(companion_rpm(root / 'companion').resolve())])
    elif artifact.suffix == '.rpm':
        command.extend(['--companion', str(companion_rpm(artifact.parent).resolve())])
    subprocess.run(command, check=True)
