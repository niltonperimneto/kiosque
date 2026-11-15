#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

artifacts = sorted(path for path in Path(sys.argv[1]).rglob('*') if path.suffix.lower() in ('.rpm', '.flatpak', '.appimage'))
if not artifacts:
    sys.exit('no packaged artifacts found')
for artifact in artifacts:
    subprocess.run([sys.executable, 'tests/system/vm.py', '--artifact', str(artifact.resolve())], check=True)
