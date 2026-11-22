from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch


class PackageSelectionTests(unittest.TestCase):
    def test_only_application_packages_launch_vms(self):
        script = Path(__file__).resolve().parents[2] / 'scripts/test_artifacts.py'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('kiosque-0.8-1.x86_64.rpm', 'kiosque-companion-0.8-1.x86_64.rpm',
                         'kiosque-debuginfo-0.8-1.x86_64.rpm', 'kiosque-debugsource-0.8-1.x86_64.rpm',
                         'kiosque.flatpak', 'Kiosque.AppImage'):
                (root / name).touch()
            companion = root / 'companion'
            companion.mkdir()
            rpm = companion / 'kiosque-companion-0.8-1.x86_64.rpm'
            rpm.touch()
            with patch.object(sys, 'argv', [str(script), str(root)]), patch('subprocess.run') as execute:
                runpy.run_path(str(script), run_name='__main__')
            commands = [call.args[0] for call in execute.call_args_list]
            self.assertEqual(len(commands), 3)
            flatpak = next(command for command in commands if str(root / 'kiosque.flatpak') in command)
            self.assertEqual(flatpak[-2:], ['--companion', str(rpm)])
            # The storefront RPM is qualified with the companion built beside it;
            # the companion alone never launches a VM.
            storefront = next(command for command in commands if str(root / 'kiosque-0.8-1.x86_64.rpm') in command)
            self.assertEqual(storefront[-2:], ['--companion', str(root / 'kiosque-companion-0.8-1.x86_64.rpm')])
            self.assertFalse(any(command[command.index('--artifact') + 1].endswith('kiosque-companion-0.8-1.x86_64.rpm')
                                 for command in commands))
            self.assertFalse(any('debug' in argument for command in commands for argument in command))
