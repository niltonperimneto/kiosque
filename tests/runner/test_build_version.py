import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/build_version.py'
spec = importlib.util.spec_from_file_location('build_version', SCRIPT)
versioning = importlib.util.module_from_spec(spec)
spec.loader.exec_module(versioning)


class BuildVersionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'rust').mkdir()
        (self.root / 'rust/Cargo.toml').write_text('[workspace.package]\nversion = "1.0.0"\n')
        self.git('init', '-q')
        self.git('config', 'user.name', 'Synthetic Fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.root), *args], text=True).strip()

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')

    def test_untagged_build_identifies_revision_and_dirty_work(self):
        version = versioning.resolve(self.root)
        self.assertRegex(version, r'^1\.0\.0-dev\.1\+g[0-9a-f]{12}$')
        (self.root / 'source').write_text('changed')
        self.assertEqual(versioning.resolve(self.root), version + '.dirty')

    def test_tag_and_subsequent_build_sort_before_next_patch(self):
        self.git('tag', 'v1.0.0')
        self.assertEqual(versioning.resolve(self.root), '1.0.0')
        (self.root / 'source').write_text('new commit')
        self.commit()
        version = versioning.resolve(self.root)
        self.assertRegex(version, r'^1\.0\.1-dev\.1\+g[0-9a-f]{12}$')
        self.assertRegex(versioning.rpm_version(version), r'^1\.0\.1~dev\.1\.g[0-9a-f]{12}$')

    def test_source_archive_preserves_the_frozen_version(self):
        version = versioning.resolve(self.root)
        (self.root / 'build-version.json').write_text(json.dumps({'schema': 1, 'version': version}))
        shutil.rmtree(self.root / '.git')
        self.assertEqual(versioning.resolve(self.root), version)

    def test_missing_archive_snapshot_is_identifiable(self):
        shutil.rmtree(self.root / '.git')
        self.assertEqual(versioning.resolve(self.root), '1.0.0-dev.0+unknown')

    def test_shallow_checkout_is_rejected(self):
        (self.root / '.git/shallow').write_text(self.git('rev-parse', 'HEAD') + '\n')
        with self.assertRaisesRegex(ValueError, 'full Git history'):
            versioning.resolve(self.root)

    def test_conflicting_release_tags_are_rejected(self):
        self.git('tag', 'v1.0.0')
        self.git('tag', 'v2.0.0')
        with self.assertRaisesRegex(ValueError, 'conflicting release tags'):
            versioning.resolve(self.root)

    def test_container_checkout_owner_can_differ(self):
        expected = versioning.resolve(self.root)
        with patch.dict(os.environ, {'GIT_TEST_ASSUME_DIFFERENT_OWNER': '1'}):
            self.assertEqual(versioning.resolve(self.root), expected)

    def test_exact_release_tag_takes_precedence_over_older_tags(self):
        self.git('tag', 'v2.0.0')
        (self.root / 'source').write_text('backport release')
        self.commit()
        self.git('tag', 'v1.5.0')
        self.assertEqual(versioning.resolve(self.root), '1.5.0')

    def test_checkout_ignores_stale_source_archive_version(self):
        expected = versioning.resolve(self.root)
        (self.root / '.git/info/exclude').write_text('build-version.json\n')
        (self.root / 'build-version.json').write_text(json.dumps({'schema': 1, 'version': '0.1.0'}))
        self.assertEqual(versioning.resolve(self.root), expected)

    def test_package_metadata_uses_version_and_preserves_history(self):
        source = self.root / 'app.metainfo.xml'
        output = self.root / 'built.metainfo.xml'
        source.write_text('<component><releases><release version="0.9.0" date="2026-01-01"><description><p>Authored notes</p></description></release></releases></component>')
        versioning.write_metainfo(source, output, '1.0.0-dev.2+g123456789abc', 1767225600)
        releases = versioning.ET.parse(output).getroot().find('releases')
        self.assertEqual(releases[0].attrib, {'version': '1.0.0-dev.2+g123456789abc', 'type': 'development', 'date': '2026-01-01'})
        self.assertEqual(releases[1].find('description/p').text, 'Authored notes')
        self.assertEqual(releases[1].get('date'), '2026-01-01')
        self.assertNotIn('1.0.0-dev', source.read_text())

    def test_tagged_release_keeps_authored_notes_and_date(self):
        source = self.root / 'app.metainfo.xml'
        output = self.root / 'built.metainfo.xml'
        source.write_text('<component><releases><release version="1.0.0" date="2026-01-01"><description><p>Release notes</p></description></release></releases></component>')
        versioning.write_metainfo(source, output, '1.0.0', 1795400000)
        releases = versioning.ET.parse(output).getroot().find('releases')
        self.assertEqual(len(releases), 1)
        self.assertEqual(releases[0].get('date'), '2026-01-01')
        self.assertEqual(releases[0].find('description/p').text, 'Release notes')
