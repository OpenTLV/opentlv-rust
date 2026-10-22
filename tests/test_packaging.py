# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Protect the crate package identities, versioning and Core compatibility record."""

import copy
from pathlib import Path
import sys
import tomllib
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import ci  # noqa: E402
from ci_contract import CRATES, REPOSITORY  # noqa: E402
from ci_support import ROOT, WORKSPACE, read  # noqa: E402

INHERITED = ('version', 'edition', 'rust-version', 'license', 'repository', 'keywords')


def manifest(path):
    return tomllib.loads(Path(path).read_text(encoding='utf-8'))


class Packaging(unittest.TestCase):
    def setUp(self):
        self.workspace = manifest(WORKSPACE / 'Cargo.toml')['workspace']['package']
        self.crates = {crate: manifest(WORKSPACE / crate / 'Cargo.toml') for crate in CRATES}

    def test_shared_identity_metadata(self):
        self.assertEqual(self.workspace['license'], 'MIT')
        self.assertEqual(self.workspace['repository'], f'https://github.com/{REPOSITORY}')
        for crate, data in self.crates.items():
            package = data['package']
            self.assertEqual(package['name'], crate)
            self.assertTrue(package['description'])
            self.assertTrue(package['categories'])
            self.assertNotIn('publish', package)
            for key in INHERITED:
                self.assertEqual(package[key], {'workspace': True}, (crate, key))

    def test_crates_are_versioned_together(self):
        version = self.workspace['version']
        locked = {item['name']: item['version']
                  for item in manifest(WORKSPACE / 'Cargo.lock')['package']}
        self.assertEqual(locked, {crate: version for crate in CRATES})
        self.assertEqual(self.crates['opentlv']['dependencies'], {'opentlv-sys': {
            'version': '=' + version, 'path': '../opentlv-sys', 'default-features': False}})
        self.assertNotIn('dependencies', self.crates['opentlv-sys'])

    def test_features_and_native_metadata_match_the_migration_baseline(self):
        baseline = read(ROOT / 'docs/migration/source-inventory.json')['crates']
        for crate, data in self.crates.items():
            package = data['package']
            # Issue #2 added the `opentlv` build script; `build` is not compared.
            self.assertEqual((package['description'], package.get('links'), data.get('features', {})),
                             (baseline[crate]['description'], baseline[crate]['links'],
                              baseline[crate]['features']), crate)
            self.assertEqual(set(data.get('dependencies', {})),
                             set(baseline[crate]['dependencies']), crate)

    def test_packages_carry_license_and_readme(self):
        license_text = (ROOT / 'LICENSE').read_bytes()
        for crate in CRATES:
            self.assertEqual((WORKSPACE / crate / 'LICENSE').read_bytes(), license_text, crate)
            self.assertTrue((WORKSPACE / crate / 'README.md').is_file(), crate)

    def test_core_compatibility_is_recorded(self):
        lock = read(WORKSPACE / 'opentlv-sys/core/core-artifact.json')
        version = self.workspace['version']
        row = (f"| {version} | {lock['version']} | `{lock['commit']}` | "
               f"{lock['release_tag'] or 'none locked'} |")
        for path in (ROOT / 'docs/packaging.md', WORKSPACE / 'opentlv-sys/README.md'):
            self.assertIn(row, path.read_text(encoding='utf-8'), path)


class PackagedManifest(unittest.TestCase):
    """The normalized manifest checks of the CI `package` check."""

    def manifest(self, crate='opentlv'):
        package = {'name': crate, 'version': '0.7.1', 'license': 'MIT', 'readme': 'README.md',
                   'repository': f'https://github.com/{REPOSITORY}'}
        data = {'package': package, 'lib': {'path': 'src/lib.rs'}}
        if crate == 'opentlv':
            data['dependencies'] = {'opentlv-sys': {'version': '=0.7.1', 'default-features': False}}
        return data

    def test_accepts_registry_manifests(self):
        for crate in CRATES:
            ci.packaged_manifest(crate, '0.7.1', self.manifest(crate))

    def test_rejects_local_paths_ranges_and_wrong_metadata(self):
        cases = [
            (('dependencies', 'opentlv-sys', 'path'), '../opentlv-sys', 'local dependency path'),
            (('dependencies', 'opentlv-sys', 'version'), '0.7.1', 'dependencies'),
            (('dependencies', 'extra'), {'version': '1'}, 'dependencies'),
            (('target', 'cfg(unix)'), {'dev-dependencies': {'x': {'path': 'x'}}},
             'local dependency path'),
            (('package', 'repository'), 'https://github.com/marekcingel/OpenTLV', 'repository'),
            (('package', 'version'), '0.7.2', 'metadata'),
            (('package', 'readme'), None, 'metadata'),
        ]
        for keys, value, message in cases:
            with self.subTest(keys=keys):
                data = copy.deepcopy(self.manifest())
                *parents, last = keys
                table = data
                for key in parents:
                    table = table.setdefault(key, {})
                table[last] = value
                with self.assertRaisesRegex(ValueError, message):
                    ci.packaged_manifest('opentlv', '0.7.1', data)
        with self.assertRaisesRegex(ValueError, 'dependencies'):
            sys_manifest = self.manifest('opentlv-sys')
            sys_manifest['dependencies'] = {'opentlv': {'version': '=0.7.1'}}
            ci.packaged_manifest('opentlv-sys', '0.7.1', sys_manifest)


if __name__ == '__main__':
    unittest.main()
