import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/bundle_kde_dependencies.sh'
SONAME = 'libKirigamiAddonsStatefulApp.so.6'
TRANSITIVE = 'libKF6Example.so.6'


class AppImageDependencyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='kiosque bundle ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.appdir = self.root / 'AppDir'
        self.destination = self.appdir / 'usr/lib'
        self.destination.mkdir(parents=True)
        binary = self.appdir / 'kiosque'
        binary.write_bytes(b'application fixture')
        binary.chmod(0o755)
        self.source = self.root / 'libraries'
        self.source.mkdir()
        tools = self.root / 'bin'
        tools.mkdir()
        ldd = tools / 'ldd'
        ldd.write_text(f'''#!{sys.executable}
import sys
from pathlib import Path
for name in sys.argv[1:]:
    if Path(name).name == 'kiosque':
        print('{SONAME} => /fixture/{SONAME} (0x0)')
        print('libc.so.6 => /fixture/libc.so.6 (0x0)')
    elif Path(name).name == '{SONAME}':
        print('{TRANSITIVE} => /fixture/{TRANSITIVE} (0x0)')
''')
        ldd.chmod(0o755)
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'])

    def run_bundle(self):
        return subprocess.run(
            ['bash', str(SCRIPT), str(self.appdir), str(self.source)],
            env=self.env, capture_output=True, text=True, timeout=5,
        )

    def test_copies_symlink_target_and_transitive_dependency_then_converges(self):
        target = self.source / 'libKirigamiAddonsStatefulApp.so.1.10.0'
        target.write_bytes(b'Kirigami fixture')
        (self.source / SONAME).symlink_to(target.name)
        (self.source / TRANSITIVE).write_bytes(b'KF6 fixture')
        (self.destination / SONAME).symlink_to(target.name)

        result = self.run_bundle()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.destination / SONAME).read_bytes(), target.read_bytes())
        self.assertFalse((self.destination / SONAME).is_symlink())
        self.assertEqual((self.destination / TRANSITIVE).read_bytes(), b'KF6 fixture')
        self.assertFalse((self.destination / 'libc.so.6').exists())
        self.assertEqual(result.stdout.count('Bundling missing dependency:'), 2)

        repeated = self.run_bundle()
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertEqual(repeated.stdout, '')

    def test_missing_library_fails_instead_of_retrying(self):
        result = self.run_bundle()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(SONAME, result.stderr)
        self.assertEqual(result.stdout.count('Bundling missing dependency:'), 1)
