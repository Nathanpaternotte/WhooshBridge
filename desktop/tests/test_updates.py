"""Offline release safety checks using synthetic bundles only."""
import importlib.util
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


configure = load('configure_updates', 'desktop/scripts/configure-updates.py')
release = load('prepare_update', 'scripts/prepare-update.py')


class UpdateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.app = self.root / 'Whoosh Bridge.app'
        (self.app / 'Contents').mkdir(parents=True)
        self.plist = self.app / 'Contents/Info.plist'
        self.plist.write_bytes(plistlib.dumps({'CFBundleIdentifier': 'test.synthetic'}))
        self.config = json.loads((ROOT / 'desktop/update-config.json').read_text())

    def archive(self):
        archive = self.root / 'synthetic.zip'
        with zipfile.ZipFile(archive, 'w') as output:
            output.write(self.plist, 'Whoosh Bridge.app/Contents/Info.plist')
        return archive

    def test_architectures_have_separate_feeds_and_secure_defaults(self):
        feeds = []
        for arch in ('arm64', 'x86_64'):
            configure.configure(self.app, arch)
            info, found_arch = release.validate_archive(self.archive(), self.config)
            self.assertEqual(found_arch, arch)
            self.assertTrue(info['SURequireSignedFeed'])
            self.assertTrue(info['SUVerifyUpdateBeforeExtraction'])
            self.assertFalse(info['SUAllowsAutomaticUpdates'])
            self.assertFalse(info['SUEnableSystemProfiling'])
            feeds.append(info['SUFeedURL'])
        self.assertNotEqual(*feeds)

    def test_rejects_unknown_architecture(self):
        with self.assertRaises(ValueError):
            configure.configure(self.app, 'universal')

    def test_rejects_stale_build_wrong_key_and_unsigned_policy(self):
        for key, value in [('CFBundleVersion', '0'), ('SUPublicEDKey', 'wrong'),
                           ('SUFeedURL', 'https://example.invalid/feed.xml'),
                           ('SURequireSignedFeed', False), ('SUVerifyUpdateBeforeExtraction', False)]:
            with self.subTest(key=key):
                configure.configure(self.app, 'arm64')
                info = plistlib.loads(self.plist.read_bytes())
                info[key] = value
                self.plist.write_bytes(plistlib.dumps(info))
                with self.assertRaises(ValueError):
                    release.validate_archive(self.archive(), self.config)


if __name__ == '__main__':
    unittest.main()
