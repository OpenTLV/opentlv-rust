# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Verified Core release failures must stop before build.rs builds or links Core."""

import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

CORE = Path(__file__).resolve().parents[1] / 'opentlv-sys/core'
sys.path.insert(0, str(CORE))
import verified_core as verified  # noqa: E402

PIN = json.loads((CORE / 'core-artifact.json').read_text(encoding='utf-8'))
COMMIT = PIN['commit']
VERSION = PIN['version']
TARGET = 'x86_64-pc-windows-msvc'
COMPONENTS = {'OPENTLV_READER': 'ON', 'OPENTLV_DOCUMENT': 'ON', 'OPENTLV_LLDP': 'OFF'}


def source_archive(directory, commit=COMMIT):
    archive = directory / 'source.tar.gz'
    with tarfile.open(archive, 'w:gz') as stream:
        files = {'opentlv-source.txt': f'version={VERSION}\ncommit={commit}\n'.encode()}
        for path in list(verified.CONFIGURATION_SOURCES) + [
                '.github/workflows/' + name for name in verified.WORKFLOWS]:
            files[path] = b'fixture workflow\n'
        for path, data in files.items():
            member = tarfile.TarInfo(f'OpenTLV-{VERSION}/{path}')
            member.size = len(data)
            stream.addfile(member, io.BytesIO(data))
    return archive


class Fixture(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__('shutil').rmtree(self.tmp, ignore_errors=True))
        self.lock_path = self.tmp / 'core-artifact.json'
        self.release_lock = {**PIN, 'release_tag': 'v' + VERSION}
        self.lock_path.write_text(json.dumps(self.release_lock), encoding='utf-8')
        self.probe = {'COMPILER_ID': 'MSVC', 'COMPILER_VERSION': '19.44', 'SIZEOF_DATA_PTR': '8',
                      'SYSTEM': 'Windows', 'COMPILER_ARCHITECTURE': 'x64', 'PROCESSOR': 'AMD64',
                      'CROSS_COMPILING': 'FALSE'}
        self.binary = {'kind': 'binary', 'id': 'windows-x64-static', 'architecture': 'x64',
                       'os': 'windows', 'build_type': 'Release', 'linkage': 'static',
                       'compiler': {'CMAKE_C_COMPILER_ID': 'MSVC', 'CMAKE_C_COMPILER_VERSION': '19.44'},
                       'msvc_runtime': 'MultiThreadedDLL', 'options': dict(COMPONENTS)}
        definition = 'fixture workflow\n'
        definition_hash = hashlib.sha256(definition.encode()).hexdigest()
        tag = self.release_lock['release_tag']
        self.manifest = {
            'schema': 2, 'version': VERSION, 'commit': COMMIT,
            'candidate': {'version': VERSION, 'tag': tag, 'commit': COMMIT,
                          'repository': verified.REPOSITORY, 'run_id': '123', 'run_attempt': '2'},
            'quality': {'schema': 2, 'tier': 'full', 'repository': verified.REPOSITORY,
                        'commit': COMMIT, 'run_id': '123', 'run_attempt': '2',
                        'configuration': {
                            'inputs': {'full': True, 'release_tag': tag},
                            'configuration_sources': dict.fromkeys(verified.CONFIGURATION_SOURCES,
                                                                   definition_hash),
                            'workflows': {name: {'sha256': definition_hash, 'definition': definition}
                                          for name in verified.WORKFLOWS}},
                        'checks': dict.fromkeys(verified.FULL_CHECKS, 'success')},
            'artifacts': [{'kind': 'source', 'filename': 'source.tar.gz', 'sha256': 'a' * 64,
                           'version': VERSION, 'commit': COMMIT}]}
        self.windows()

    def windows(self):
        for name, value in (('system', 'Windows'), ('machine', 'AMD64')):
            patcher = mock.patch.object(verified.platform, name, return_value=value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def patch(self, name, value):
        patcher = mock.patch.object(verified, name, value)
        patcher.start()
        self.addCleanup(patcher.stop)

    def release(self, files):
        """Serve `files` as authenticated release assets."""
        self.patch('authenticated_release', lambda *_: (
            self.manifest, {p.name: verified.digest(p) for p in files},
            lambda name: self.tmp / name, {'release_id': 1}))

    def resolve(self, probe=True, crt_static=False):
        return verified.resolve(self.lock_path, self.tmp / 'work', TARGET, TARGET, COMPONENTS,
                                self.probe if probe else None, crt_static)


class LockTest(Fixture):
    def test_committed_lock_selects_core_080(self):
        selected = verified.lock(CORE / 'core-artifact.json')
        self.assertEqual(selected['release_tag'], 'v0.8.0')
        self.assertEqual(selected['commit'], '345c77acb29266036df5f27d04ce315bb61853cc')

    def test_null_lock_cannot_verify_a_release(self):
        self.lock_path.write_text(json.dumps({**PIN, 'release_tag': None}), encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'published Core'):
            verified.lock(self.lock_path)

    def test_release_lock(self):
        self.assertEqual(verified.lock(self.lock_path)['release_tag'], 'v' + VERSION)

    def test_invalid_lock(self):
        for field, value in [('commit', 'main'), ('commit', 'abc1234'), ('version', None),
                             ('release_tag', 'v9.0.0'), ('allow_verified_source', 'yes'),
                             ('repository', 'other/repo'), ('schema', 2)]:
            with self.subTest(field=field):
                self.lock_path.write_text(json.dumps({**self.release_lock, field: value}))
                with self.assertRaises(ValueError):
                    verified.lock(self.lock_path)


class ManifestTest(Fixture):
    def test_manifest_identity(self):
        verified.verify_manifest(self.manifest, self.release_lock)

    def test_manifest_rejects_mismatch(self):
        for section, field, value in [
                (None, 'commit', 'b' * 40), (None, 'version', '9.0.0'),
                ('quality', 'repository', 'other/repo'), ('quality', 'tier', 'pr'),
                ('quality', 'checks', {'clang': 'skipped'}), ('quality', 'configuration', {}),
                ('candidate', 'run_id', '999'), ('candidate', 'run_attempt', '1'),
                ('candidate', 'commit', 'b' * 40)]:
            with self.subTest(section=section, field=field):
                manifest = copy.deepcopy(self.manifest)
                (manifest if section is None else manifest[section])[field] = value
                with self.assertRaises(ValueError):
                    verified.verify_manifest(manifest, self.release_lock)

    def test_duplicate_artifact(self):
        self.manifest['artifacts'] *= 2
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            verified.verify_manifest(self.manifest, self.release_lock)

    def test_partial_full_gate(self):
        self.manifest['quality']['checks'].pop('abi')
        with self.assertRaisesRegex(ValueError, 'Full CI'):
            verified.verify_manifest(self.manifest, self.release_lock)

    def test_workflow_hash(self):
        self.manifest['quality']['configuration']['workflows']['core-release.yml']['definition'] += 'x'
        with self.assertRaisesRegex(ValueError, 'definition/hash'):
            verified.verify_manifest(self.manifest, self.release_lock)

    def test_configuration_bound_to_source(self):
        verified.extract(source_archive(self.tmp), self.tmp / 'out')
        source = self.tmp / 'out' / f'OpenTLV-{VERSION}'
        verified.verify_configuration(self.manifest, source)
        (source / 'CMakeLists.txt').write_text('different\n')
        with self.assertRaisesRegex(ValueError, 'configuration source'):
            verified.verify_configuration(self.manifest, source)

    def test_checksum_file_rejects_duplicates(self):
        path = self.tmp / 'SHA256SUMS'
        path.write_text((('a' * 64) + '  source.tar.gz\n') * 2)
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            verified.checksums(path)


class CompatibilityTest(Fixture):
    def compatible(self, record=None, probe=None, target=TARGET, host=TARGET, crt_static=False):
        return verified.compatible(record or self.binary, probe or self.probe, target, host,
                                   crt_static, COMPONENTS)

    def test_windows_static_binary(self):
        self.assertTrue(self.compatible())

    def test_rejects_other_toolchains_and_targets(self):
        for field, value in [('COMPILER_VERSION', '19.43'), ('COMPILER_ID', 'Clang'),
                             ('SIZEOF_DATA_PTR', '4'), ('COMPILER_ARCHITECTURE', 'ARM64'),
                             ('CROSS_COMPILING', 'TRUE')]:
            with self.subTest(field=field):
                self.assertFalse(self.compatible(probe={**self.probe, field: value}))
        self.assertFalse(self.compatible(target='x86_64-pc-windows-gnu', host='x86_64-pc-windows-gnu'))
        self.assertFalse(self.compatible(host='x86_64-unknown-linux-gnu'))
        self.assertFalse(self.compatible(crt_static=True))

    def test_rejects_other_records(self):
        for field, value in [('linkage', 'shared'), ('id', 'windows-x64-shared'),
                             ('build_type', 'Debug'), ('msvc_runtime', 'MultiThreaded'),
                             ('architecture', 'arm64')]:
            with self.subTest(field=field):
                self.assertFalse(self.compatible({**self.binary, field: value}))

    def test_components_must_match_cargo_features(self):
        for name, value in COMPONENTS.items():
            with self.subTest(name=name):
                record = copy.deepcopy(self.binary)
                record['options'][name] = 'OFF' if value == 'ON' else 'ON'
                self.assertFalse(self.compatible(record))

    def test_linux_static_requires_declared_pic(self):
        target = 'x86_64-unknown-linux-gnu'
        with mock.patch.object(verified.platform, 'system', return_value='Linux'), \
                mock.patch.object(verified.platform, 'libc_ver', return_value=('glibc', '2.39')):
            record = {**self.binary, 'id': 'linux-x64-static', 'os': 'linux',
                      'libc': ['glibc', '2.39'], 'msvc_runtime': None}
            probe = {**self.probe, 'SYSTEM': 'Linux'}
            self.assertFalse(self.compatible(record, probe, target, target))
            record['position_independent_code'] = True
            self.assertTrue(self.compatible(record, probe, target, target))
            record['libc'] = ['glibc', '2.35']
            self.assertFalse(self.compatible(record, probe, target, target))


class ArchiveTest(Fixture):
    def test_archive_traversal(self):
        for index, name in enumerate(['../escape', '/absolute', 'C:/escape', 'a\\escape']):
            with self.subTest(name=name):
                archive = self.tmp / 'bad.zip'
                with zipfile.ZipFile(archive, 'w') as stream:
                    entry = zipfile.ZipInfo('placeholder')
                    entry.filename = name  # Do not let Windows ZipInfo normalize the attack.
                    stream.writestr(entry, 'bad')
                with self.assertRaises(ValueError):
                    verified.extract(archive, self.tmp / f'out{index}')


class ResolveTest(Fixture):
    def binary_archive(self, files=('lib/tlv.lib', 'include/tlv/version.h', 'include/tlv/config.h')):
        archive = self.tmp / 'binary.zip'
        with zipfile.ZipFile(archive, 'w') as stream:
            for name in files:
                stream.writestr(name, '# authenticated fixture\n')
        return archive

    def serve(self, source, binary=None):
        self.manifest['artifacts'][0].update(sha256=verified.digest(source), filename=source.name)
        files = [source]
        if binary is not None:
            self.binary.update(filename=binary.name, sha256=verified.digest(binary),
                               version=VERSION, commit=COMMIT)
            self.manifest['artifacts'].append(self.binary)
            files.append(binary)
        self.release(files)

    def test_verified_source(self):
        self.serve(source_archive(self.tmp))
        result = self.resolve()
        self.assertEqual(result['mode'], 'verified-source')
        self.assertTrue(result['verified'])
        self.assertTrue(Path(result['source_dir'], 'opentlv-source.txt').is_file())
        self.assertNotIn('lib_dir', result)

    def test_verified_source_failures(self):
        for failure in ['checksum', 'identity', 'fallback']:
            with self.subTest(failure=failure):
                self.setUp()
                source = source_archive(self.tmp, 'b' * 40 if failure == 'identity' else COMMIT)
                self.serve(source)
                if failure == 'checksum':
                    self.manifest['artifacts'][0]['sha256'] = 'b' * 64
                if failure == 'fallback':
                    self.lock_path.write_text(json.dumps({**self.release_lock,
                                                          'allow_verified_source': False}))
                with self.assertRaises(ValueError):
                    self.resolve()

    def test_verified_binary(self):
        self.serve(source_archive(self.tmp), self.binary_archive())
        result = self.resolve()
        self.assertEqual(result['mode'], 'verified-binary')
        self.assertTrue(Path(result['lib_dir'], 'tlv.lib').is_file())
        self.assertTrue(Path(result['include_dir'], 'tlv/version.h').is_file())
        self.assertTrue(Path(result['source_dir'], 'opentlv-source.txt').is_file())

    def test_incompatible_binary_falls_back_to_verified_source(self):
        self.serve(source_archive(self.tmp), self.binary_archive())
        self.assertEqual(self.resolve(crt_static=True)['mode'], 'verified-source')

    def test_source_only_skips_binaries(self):
        self.serve(source_archive(self.tmp), self.binary_archive())
        self.lock_path.write_text(json.dumps({**self.release_lock, 'allow_verified_source': False}))
        self.assertEqual(self.resolve(probe=False)['mode'], 'verified-source')

    def test_binary_package_layout(self):
        for files in [('include/tlv/version.h', 'include/tlv/config.h'),
                      ('lib/tlv.lib', 'lib/libtlv.a', 'include/tlv/version.h', 'include/tlv/config.h'),
                      ('lib/tlv.lib', 'include/tlv/version.h')]:
            with self.subTest(files=files):
                self.setUp()
                self.serve(source_archive(self.tmp), self.binary_archive(files))
                with self.assertRaises(ValueError):
                    self.resolve()

    def test_binary_checksum(self):
        self.serve(source_archive(self.tmp), self.binary_archive())
        self.binary['sha256'] = 'c' * 64
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.resolve()


class AuthenticationTest(Fixture):
    def releases(self, assets):
        self.patch('api', lambda path: {'sha': COMMIT} if '/commits/' in path else
                   {'id': 42, 'draft': False, 'tag_name': self.release_lock['release_tag'],
                    'assets': [{'name': name} for name in assets]})

    def test_attestation_failure_stops_before_manifest_parse(self):
        self.releases(['release-manifest.json'])
        calls = []

        def command(*args, capture=False):
            calls.append(args)
            if args[1] == 'attestation':
                raise subprocess.CalledProcessError(1, args)

        self.patch('run', command)
        with self.assertRaises(subprocess.CalledProcessError):
            verified.authenticated_release(self.tmp / 'assets', self.release_lock)
        self.assertEqual(calls[-1][1:3], ('attestation', 'verify'))
        self.assertEqual(calls[-1][-4:], ('--signer-workflow', verified.SIGNER,
                                          '--source-digest', COMMIT))

    def test_moved_tag_rejected(self):
        self.patch('api', lambda path: {'sha': 'b' * 40} if '/commits/' in path else
                   {'id': 42, 'draft': False, 'tag_name': self.release_lock['release_tag'],
                    'assets': []})
        with self.assertRaisesRegex(ValueError, 'moved'):
            verified.authenticated_release(self.tmp / 'assets', self.release_lock)

    def test_authenticated_manifest(self):
        files = self.tmp / 'download'
        files.mkdir()
        verified.write(files / 'release-manifest.json', self.manifest)
        (files / 'SHA256SUMS').write_text(
            verified.digest(files / 'release-manifest.json') + '  release-manifest.json\n')
        self.releases(['release-manifest.json', 'SHA256SUMS'])
        calls = []

        def command(*args, capture=False):
            calls.append(args)
            if args[1:3] == ('release', 'download'):
                name = args[args.index('--pattern') + 1]
                destination = Path(args[args.index('--dir') + 1])
                (destination / name).write_bytes((files / name).read_bytes())

        self.patch('run', command)
        actual, _, _, identity = verified.authenticated_release(self.tmp / 'assets', self.release_lock)
        self.assertEqual((actual, identity['release_id']), (self.manifest, 42))
        attestations = [args for args in calls if args[1:3] == ('attestation', 'verify')]
        self.assertEqual(len(attestations), 2)
        for args in attestations:
            self.assertEqual(args[-4:], ('--signer-workflow', verified.SIGNER,
                                         '--source-digest', COMMIT))


class CommandLineTest(Fixture):
    def main(self, *extra):
        output = self.tmp / 'result.txt'
        output.write_text('stale\n')
        argv = ['verified_core.py', '--lock', str(self.lock_path), '--work', str(self.tmp / 'w'),
                '--output', str(output), '--target', TARGET, '--host', TARGET,
                '--components', 'OPENTLV_READER=ON,OPENTLV_LLDP=OFF', *extra]
        with mock.patch.object(sys, 'argv', argv):
            try:
                verified.main()
            except SystemExit as error:
                return error.code, output
        return 0, output

    def test_result_for_build_script(self):
        self.patch('resolve', lambda *a: {'mode': 'verified-source', 'version': VERSION,
                                          'commit': COMMIT, 'source_dir': '/core'})
        code, output = self.main('--source-only')
        self.assertEqual(code, 0)
        self.assertEqual(output.read_text(),
                         f'mode=verified-source\nversion={VERSION}\ncommit={COMMIT}\nsource_dir=/core\n')

    def test_failure_removes_previous_result(self):
        self.patch('resolve', mock.Mock(side_effect=ValueError('attestation failed')))
        code, output = self.main('--source-only')
        self.assertEqual(code, 1)
        self.assertFalse(output.exists())

    def test_requires_probe_or_source_only(self):
        code, output = self.main()
        self.assertEqual(code, 1)
        self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
