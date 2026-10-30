# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Protect the local registry that isolated consumers resolve the crate archives from."""

import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import registry_consumer as consumer  # noqa: E402
from ci_support import digest  # noqa: E402

SYS_MANIFEST = '''[package]
name = "opentlv-sys"
version = "0.1.0"
links = "tlv"

[features]
lldp = []
'''
SAFE_MANIFEST = '''[package]
name = "opentlv"
version = "0.1.0"

[features]
lldp = ["opentlv-sys/lldp"]

[dependencies.opentlv-sys]
version = "=0.1.0"
default-features = false
'''


def write_archive(directory, crate, manifest, version='0.1.0'):
    path = Path(directory) / f'{crate}-{version}.crate'
    with tarfile.open(path, 'w:gz') as archive:
        for name, text in (('Cargo.toml', manifest), ('src/lib.rs', '')):
            data = text.encode()
            member = tarfile.TarInfo(f'{crate}-{version}/{name}')
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
    return path


class Fixture(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.packages = self.root / 'packages'
        self.packages.mkdir()
        self.sys = write_archive(self.packages, 'opentlv-sys', SYS_MANIFEST)
        self.safe = write_archive(self.packages, 'opentlv', SAFE_MANIFEST)


class Registry(Fixture):
    def test_finds_exactly_one_archive_per_crate(self):
        self.assertEqual(consumer.find_archives(self.packages), {
            'opentlv-sys': ('0.1.0', self.sys), 'opentlv': ('0.1.0', self.safe)})
        other = write_archive(self.packages, 'opentlv', SAFE_MANIFEST, '0.1.1')
        with self.assertRaisesRegex(ValueError, 'one opentlv archive'):
            consumer.find_archives(self.packages)
        self.safe.unlink()
        with self.assertRaisesRegex(ValueError, 'versions differ'):
            consumer.find_archives(self.packages)
        other.unlink()
        with self.assertRaisesRegex(ValueError, 'one opentlv archive'):
            consumer.find_archives(self.packages)

    def test_index_entry_follows_the_normalized_manifest(self):
        self.assertEqual(consumer.index_entry('opentlv', '0.1.0', self.safe), {
            'name': 'opentlv', 'vers': '0.1.0', 'cksum': digest(self.safe),
            'deps': [{'name': 'opentlv-sys', 'req': '=0.1.0', 'features': [], 'optional': False,
                      'default_features': False, 'target': None, 'kind': 'normal'}],
            'features': {'lldp': ['opentlv-sys/lldp']}, 'yanked': False, 'links': None})
        self.assertEqual(consumer.index_entry('opentlv-sys', '0.1.0', self.sys)['links'], 'tlv')

    def test_rejects_dependency_kinds_the_index_does_not_model(self):
        for table in ('dev-dependencies', 'build-dependencies', 'target'):
            with self.subTest(table=table):
                path = write_archive(self.root, 'opentlv', f'{SAFE_MANIFEST}\n[{table}]\n')
                with self.assertRaisesRegex(ValueError, 'does not model'):
                    consumer.index_entry('opentlv', '0.1.0', path)

    def test_registry_serves_unchanged_archives(self):
        registry = self.root / 'registry'
        registry.mkdir()
        consumer.local_registry(consumer.find_archives(self.packages), registry)
        for path in (self.sys, self.safe):
            self.assertEqual(digest(registry / path.name), digest(path))
        entry = json.loads((registry / 'index/op/en/opentlv-sys').read_text(encoding='utf-8'))
        self.assertEqual(entry['cksum'], digest(self.sys))
        self.assertTrue((registry / 'index/op/en/opentlv').is_file())

    @unittest.skipUnless(shutil.which('cargo'), 'requires Cargo')
    def test_cargo_resolves_the_archives_as_crates_io_packages(self):
        registry = self.root / 'registry'
        registry.mkdir()
        consumer.local_registry(consumer.find_archives(self.packages), registry)
        home = self.root / 'home'
        home.mkdir()
        (home / 'config.toml').write_text(
            '[source.crates-io]\nreplace-with = "local"\n\n[source.local]\n'
            f'local-registry = {json.dumps(registry.as_posix())}\n', encoding='utf-8')
        project = self.root / 'consumer'
        (project / 'src').mkdir(parents=True)
        (project / 'src/main.rs').write_text('fn main() {}\n', encoding='utf-8')
        (project / 'Cargo.toml').write_text(
            consumer.consumer_manifest('0.1.0', ['lldp']), encoding='utf-8')
        subprocess.run(['cargo', 'generate-lockfile', '--offline'], cwd=project, check=True,
                       env={**os.environ, 'CARGO_HOME': str(home)})
        digests = {'opentlv-sys': digest(self.sys), 'opentlv': digest(self.safe)}
        consumer.check_lock(project / 'Cargo.lock', digests)
        # The dependency is exact, so the archive digest of a different build cannot resolve.
        with self.assertRaisesRegex(ValueError, 'archive digest'):
            consumer.check_lock(project / 'Cargo.lock', {**digests, 'opentlv': '0' * 64})


class Consumer(unittest.TestCase):
    def test_manifest_requires_exact_version_and_selected_features(self):
        data = tomllib.loads(consumer.consumer_manifest('1.2.3', None))
        self.assertEqual(data['dependencies'], {'opentlv': '=1.2.3'})
        self.assertIn('workspace', data)
        self.assertEqual(tomllib.loads(consumer.consumer_manifest('1.2.3', []))['dependencies'], {
            'opentlv': {'version': '=1.2.3', 'default-features': False, 'features': []}})
        self.assertEqual(
            tomllib.loads(consumer.consumer_manifest('1.2.3', ['nfc']))['dependencies']['opentlv']['features'],
            ['nfc'])

    def test_program_uses_exactly_the_selected_features(self):
        for features, formats, present in ((None, 6, ('Lldp', 'NfcType2', 'Document')),
                                           ([], 4, ()), (['lldp'], 5, ('Lldp',)),
                                           (['nfc'], 5, ('NfcType2',)), (['document'], 4, ('Document',))):
            with self.subTest(features=features):
                program = consumer.consumer_program(features)
                self.assertIn(f'Format::ALL.len(), {formats})', program)
                for name in ('Lldp', 'NfcType2', 'Document'):
                    self.assertEqual(name in program, name in present, name)

    def test_lock_must_name_the_registry_and_the_archive_digests(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'Cargo.lock'

            def lock(source=consumer.CRATES_IO, checksum='a' * 64, extra=''):
                path.write_text(
                    f'[[package]]\nname = "{consumer.CONSUMER}"\nversion = "0.0.0"\n'
                    f'[[package]]\nname = "opentlv"\nversion = "0.1.0"\nsource = "{source}"\n'
                    f'checksum = "{checksum}"\n{extra}', encoding='utf-8')

            lock()
            consumer.check_lock(path, {'opentlv': 'a' * 64})
            lock(source='path+file:///checkout')
            with self.assertRaisesRegex(ValueError, 'from the registry'):
                consumer.check_lock(path, {'opentlv': 'a' * 64})
            lock(checksum='b' * 64)
            with self.assertRaisesRegex(ValueError, 'archive digest'):
                consumer.check_lock(path, {'opentlv': 'a' * 64})
            lock(extra='[[package]]\nname = "other"\nversion = "1.0.0"\n')
            with self.assertRaisesRegex(ValueError, 'unexpected packages'):
                consumer.check_lock(path, {'opentlv': 'a' * 64})


if __name__ == '__main__':
    unittest.main()
