import importlib.util
from pathlib import Path
import re
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('dependency_sbom', ROOT / 'scripts/dependency_sbom.py')
sbom = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sbom)

VALID_CHECKSUM = 'a' * 64


class DependencySecurityTests(unittest.TestCase):
    def test_repository_lockfile_passes_provenance_validation(self):
        doc = sbom.generate(ROOT / 'rust/Cargo.lock', ROOT / 'rust/Cargo.toml')
        self.assertEqual(doc['spdxVersion'], 'SPDX-2.3')
        self.assertGreater(len(doc['packages']), 50)

    def test_rejects_non_crates_io_source_and_missing_checksum(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.toml').write_text('[package]\nname = "demo"\nversion = "0.1.0"\n')
            git_lock = root / 'Cargo.lock'
            git_lock.write_text(
                'version = 4\n'
                '[[package]]\nname = "demo"\nversion = "0.1.0"\n'
                '[[package]]\nname = "evil"\nversion = "0.1.0"\n'
                'source = "git+https://example.com/evil.git#deadbeef"\n'
            )
            with self.assertRaisesRegex(ValueError, 'disallowed dependency source'):
                sbom.generate(git_lock)

            unverified_lock = root / 'unverified.lock'
            unverified_lock.write_text(
                'version = 4\n'
                '[[package]]\nname = "demo"\nversion = "0.1.0"\n'
                '[[package]]\nname = "dep"\nversion = "1.0.0"\n'
                f'source = "{sbom.CRATES_IO_SOURCE}"\n'
            )
            with self.assertRaisesRegex(ValueError, 'SHA-256 checksum'):
                sbom.generate(unverified_lock, root / 'Cargo.toml')

    def test_rejects_undeclared_local_package_and_patch_overrides(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'Cargo.toml'
            manifest.write_text('[package]\nname = "demo"\nversion = "0.1.0"\n')
            rogue_local = root / 'Cargo.lock'
            rogue_local.write_text(
                'version = 4\n'
                '[[package]]\nname = "demo"\nversion = "0.1.0"\n'
                '[[package]]\nname = "rogue-local"\nversion = "0.1.0"\n'
            )
            with self.assertRaisesRegex(ValueError, 'untrusted local/path package'):
                sbom.generate(rogue_local, manifest)

            patched_lock = root / 'patched.lock'
            patched_lock.write_text(
                'version = 4\n'
                '[[package]]\nname = "demo"\nversion = "0.1.0"\n'
                '[[patch.unused]]\nname = "dep"\nversion = "1.0.0"\n'
                f'source = "{sbom.CRATES_IO_SOURCE}"\nchecksum = "{VALID_CHECKSUM}"\n'
            )
            with self.assertRaisesRegex(ValueError, r'\[patch\]'):
                sbom.generate(patched_lock, manifest)

            manifest.write_text(
                '[package]\nname = "demo"\nversion = "0.1.0"\n'
                '[patch.crates-io]\ndep = { path = "../dep" }\n'
            )
            valid_lock = root / 'valid.lock'
            valid_lock.write_text('version = 4\n[[package]]\nname = "demo"\nversion = "0.1.0"\n')
            with self.assertRaisesRegex(ValueError, r'\[patch\]'):
                sbom.generate(valid_lock, manifest)

    def test_workflows_pin_actions_by_commit_sha_and_configure_dependabot_policy(self):
        uses_re = re.compile(r'^\s*-?\s*uses:\s*([^\s#]+)')
        pinned_re = re.compile(r'^[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}$')
        workflows = sorted((ROOT / '.github/workflows').glob('*.yml'))
        self.assertGreaterEqual(len(workflows), 5)
        for workflow in workflows:
            for line in workflow.read_text().splitlines():
                match = uses_re.match(line)
                if match:
                    self.assertRegex(match.group(1), pinned_re, f'unpinned action in {workflow.name}: {line}')

        dependabot = (ROOT / '.github/dependabot.yml').read_text()
        for required in ('versioning-strategy: increase', 'cooldown:', 'cargo-minor-and-patch:', 'qtbridge*'):
            self.assertIn(required, dependabot)

        ci_workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('actions/dependency-review-action@', ci_workflow)
        audit_workflow = (ROOT / '.github/workflows/dependency-audit.yml').read_text()
        self.assertIn('python3 scripts/test.py security', audit_workflow)
        self.assertIn('schedule:', audit_workflow)
