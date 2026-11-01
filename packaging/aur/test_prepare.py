import unittest
import hashlib
import tempfile
from pathlib import Path
from prepare import prepare

class PrepareTests(unittest.TestCase):
    def test_git_publications_remain_ordered_after_force_push(self):
        recipe = 'pkgver=old\npkgrel=4\nsource=(git+x#branch=main)\npkgver() {\n old\n}\n'
        output = prepare(recipe, 'git', '123')
        self.assertIn('epoch=1', output)
        self.assertIn('pkgver=0.r123', output)
        self.assertIn('#branch=dev', output)
        self.assertNotIn('pkgver()', output)

    def test_binary_pins_exact_release_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'release.tar.gz'
            archive.write_bytes(b'actual release archive')
            recipe = "pkgver=old\npkgrel=8\n_version=old\nsha256sums=('bad')\n"
            output = prepare(recipe, 'bin', '1', '0.1.0-alpha.3', archive)
            self.assertIn('pkgver=0.1.0prealpha.3', output)
            self.assertIn('_version=0.1.0-alpha.3', output)
            self.assertIn(hashlib.sha256(archive.read_bytes()).hexdigest(), output)

    def test_semver_identifiers_and_metadata_remain_in_release_url_version(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'release'
            archive.write_bytes(b'release')
            recipe = "pkgver=old\npkgrel=1\n_version=old\nsha256sums=('old')\n"
            for version, pkgver in (
                ('1.2.3-alpha-1+build.001', '1.2.3prealpha_1'),
                ('1.2.3+metadata', '1.2.3'),
                ('1.2.3-0.a-b.01x', '1.2.3pre0.a_b.01x'),
            ):
                with self.subTest(version=version):
                    output = prepare(recipe, 'bin', '1', version, archive)
                    self.assertIn('pkgver=' + pkgver + '\n', output)
                    self.assertIn('_version=' + version + '\n', output)

    def test_rejects_invalid_semver(self):
        for version in ('01.2.3', '1.02.3', '1.2.03', '1.2.3-01', '1.2.3-a..b', '1.2.3+', '1.2.3-'):
            with self.subTest(version=version), self.assertRaises(ValueError):
                prepare('', 'bin', '1', version, '/unused')

    def test_rejects_shell_in_release_version(self):
        with self.assertRaises(ValueError):
            prepare('', 'bin', '1', '1.0.0;echo unsafe', '/unused')

class VersionTests(unittest.TestCase):
    def test_arch_upgrade_order_and_stale_releases(self):
        from versions import newer
        def info(version, epoch=0):
            return f'pkgver = {version}\npkgrel = 1\nepoch = {epoch}\n'
        self.assertTrue(newer(info('0.1.0prealpha.3'), info('0.1.0alpha.2')))
        self.assertTrue(newer(info('1.2.3'), info('1.2.3pre0')))
        self.assertTrue(newer(info('0.1.0'), info('0.1.0prealpha.3')))
        self.assertFalse(newer(info('0.1.0alpha.2'), info('0.1.0alpha.3')))
        self.assertFalse(newer(info('0.1.0alpha.3'), info('0.1.0alpha.3')))
        self.assertTrue(newer(info('0.r100', 1), info('0.1.0alpha.r55.g3cf6e1c')))
        self.assertTrue(newer(info('0.r101', 1), info('0.r100', 1)))

    def test_unknown_or_conflicting_srcinfo_fails(self):
        from versions import package_version
        for metadata in ('pkgver = 1.0\n', 'pkgver = 1\npkgver = 2\npkgrel = 1\n', 'epoch = bad\npkgver = 1\npkgrel = 1\n'):
            with self.assertRaises(ValueError):
                package_version(metadata)
