import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('runner', Path(__file__).resolve().parents[2] / 'scripts/test.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RoutingTests(unittest.TestCase):
    def test_backend_changes_include_real_contracts(self):
        self.assertTrue({'core', 'services', 'system'} <= set(runner.selected_suites(['rust/core/src/flatpak/cli.rs'])))

    def test_qml_changes_require_real_bridge(self):
        self.assertTrue({'qt', 'qml'} <= set(runner.selected_suites(['qml/pages/InstalledPage.qml'])))

    def test_guidance_does_not_trigger_installation(self):
        self.assertEqual(runner.selected_suites(['skills/kiosque-rust-core/SKILL.md']), ['guidance'])

    def test_app_crate_changes_require_desktop_suites(self):
        self.assertTrue({'qt', 'qml'} <= set(runner.selected_suites(['rust/app/src/bridge/app_list_model.rs'])))

    def test_updater_changes_run_core(self):
        self.assertIn('core', runner.selected_suites(['rust/updater/src/main.rs']))

    def test_no_cpp_guard_rejects_sources(self):
        runner.no_cpp()  # the tree itself must pass

    def test_package_changes_check_artifacts(self):
        self.assertIn('packages', runner.selected_suites(['kiosque.spec']))
