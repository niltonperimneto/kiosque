#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

artifacts = sorted(path for path in Path(sys.argv[1]).rglob('*') if path.parent.name != 'companion' and path.suffix.lower() in ('.rpm', '.flatpak', '.appimage'))
if not artifacts:
    sys.exit('no packaged artifacts found')
for artifact in artifacts:
    command = [sys.executable, 'tests/system/vm.py', '--artifact', str(artifact.resolve())]
    if artifact.suffix == '.flatpak':
        companion, = (Path(sys.argv[1]) / 'companion').glob('kiosque-[0-9]*.rpm')
        command.extend(['--companion', str(companion)])
    subprocess.run(command, check=True)
