# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Release failures must precede writes; only tested, current artifacts may ship."""

import copy
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import ci  # noqa: E402
import release  # noqa: E402
from ci_support import digest, read, write  # noqa: E402
from test_ci_contract import CHECKS, DEFINITIONS, IDENTITY, records  # noqa: E402


class Preflight(unittest.TestCase):
    def test_version_and_notes_match_the_first_release(self):
        self.assertEqual(release.version_for('v0.1.0'), '0.1.0')
        for tag in ('v0.8.0', 'v0.7.1', '0.1.0', 'v0.1.0-rc1', 'v00.1.0', '../main'):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.version_for(tag)

    def test_dispatch_requires_official_main_workflow_and_commit(self):
        env = {'GITHUB_EVENT_NAME': 'workflow_dispatch', 'GITHUB_REF': 'refs/heads/main',
               'GITHUB_WORKFLOW_REF': release.SIGNER + '@refs/heads/main',
               'GITHUB_WORKFLOW_SHA': IDENTITY['commit']}
        with mock.patch.dict(os.environ, env, clear=True), \
                mock.patch.object(release, 'identity', return_value=IDENTITY):
            self.assertEqual(release.hosted_identity(), IDENTITY)
            for name in env:
                with mock.patch.dict(os.environ, {name: 'other'}), self.assertRaises(ValueError):
                    release.hosted_identity()
        for key, value in (('dirty', True), ('hosted', False), ('repository', 'fork/repo')):
            with mock.patch.dict(os.environ, env, clear=True), \
                    mock.patch.object(release, 'identity', return_value={**IDENTITY, key: value}), \
                    self.assertRaises(ValueError):
                release.hosted_identity()

    def test_main_identity_and_latest_failed_attempt(self):
        good = {'head_sha': IDENTITY['commit'], 'event': 'push', 'head_branch': 'main',
                'path': '.github/workflows/rust-main.yml', 'status': 'completed',
                'repository': {'full_name': release.REPOSITORY},
                'head_repository': {'full_name': release.REPOSITORY},
                'conclusion': 'success', 'id': 100, 'run_attempt': 2, 'html_url': 'main-url'}
        release.check_main(good, IDENTITY['commit'])
        for key in ('head_sha', 'event', 'head_branch', 'path', 'status', 'conclusion'):
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.check_main({**good, key: 'wrong'}, IDENTITY['commit'])
        with mock.patch.object(release, 'pages', return_value=[
                good, {**good, 'id': 101, 'conclusion': 'failure'}]), self.assertRaises(ValueError):
            release.main_evidence(IDENTITY['commit'])

    def test_unused_tag_rejects_existing_tag_draft_or_changed_main(self):
        main = {'object': {'sha': IDENTITY['commit']}}
        with mock.patch.object(release, 'api', side_effect=[main, []]), \
                mock.patch.object(release, 'pages', return_value=[]):
            release.unused_tag('v0.1.0', IDENTITY['commit'])
        for refs, releases in (([{'ref': 'refs/tags/v0.1.0'}], []),
                               ([], [{'tag_name': 'v0.1.0', 'draft': True}])):
            with mock.patch.object(release, 'api', side_effect=[main, refs]), \
                    mock.patch.object(release, 'pages', return_value=releases), \
                    self.assertRaises(ValueError):
                release.unused_tag('v0.1.0', IDENTITY['commit'])
        with mock.patch.object(release, 'api', return_value={'object': {'sha': 'b' * 40}}), \
                self.assertRaisesRegex(ValueError, 'Main moved'):
            release.unused_tag('v0.1.0', IDENTITY['commit'])

    def test_gate_is_attempt_scoped_and_unique(self):
        good = {'name': 'full / checks / Rust quality gate',
                'status': 'completed', 'conclusion': 'success'}
        with mock.patch.object(release, 'fetch_jobs', return_value=[good]) as fetch:
            release.check_gate('123', '2')
            fetch.assert_called_once_with(release.REPOSITORY, '123', '2')
        for jobs in ([], [good, good], [{**good, 'conclusion': 'skipped'}],
                     [{**good, 'status': 'in_progress'}]):
            with mock.patch.object(release, 'fetch_jobs', return_value=jobs), self.assertRaises(ValueError):
                release.check_gate('123', '2')


class Payload(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.evidence = self.root / 'evidence'
        self.output = self.root / 'release'
        self.data = records('full')
        self.request = {'schema': 1, 'tag': 'v0.1.0', 'version': '0.1.0',
                        'identity': copy.deepcopy(IDENTITY), 'definitions': DEFINITIONS,
                        'notes_sha256': digest(release.NOTES), 'core': release.lock(True)}
        packages = self.evidence / 'packages'
        packages.mkdir(parents=True)
        for record in self.data:
            record['require_release'] = True
            if record['configuration']['id'] == 'linux-stable':
                for crate in release.CRATES:
                    path = packages / f'{crate}-0.1.0.crate'
                    path.write_bytes((crate + ' tested bytes').encode())
                    record['package'][crate] = digest(path)
                for consumer in record['consumer'].values():
                    consumer['packages'] = record['package']
            write(self.evidence / 'builds' / record['configuration']['id'] / 'record.json', record)
        self.quality = release.validate_records(self.data, 'full', IDENTITY, DEFINITIONS, CHECKS, True)
        write(self.evidence / 'quality.json', self.quality)
        write(self.evidence / 'request.json', self.request)
        for name in ('validate_request', 'check_gate'):
            patcher = mock.patch.object(release, name)
            patcher.start()
            self.addCleanup(patcher.stop)

    def assemble(self):
        release.assemble(self.evidence, self.output)

    def test_exact_tested_packages_and_complete_evidence(self):
        self.assemble()
        self.assertEqual(release.verify_payload(self.output), self.request)
        for path in (self.evidence / 'packages').iterdir():
            self.assertEqual(path.read_bytes(), (self.output / path.name).read_bytes())
        self.assertEqual(len(list(self.output.iterdir())), 5)

    def test_incomplete_stale_unverified_or_nonrelease_full_is_rejected(self):
        for kind in ('missing', 'attempt', 'definitions', 'unverified', 'release', 'package', 'gate'):
            data, quality = copy.deepcopy(self.data), copy.deepcopy(self.quality)
            if kind == 'missing':
                data.pop()
            elif kind == 'attempt':
                data[0]['identity']['run_attempt'] = '1'
            elif kind == 'definitions':
                data[0]['definitions'] = {}
            elif kind == 'unverified':
                data[0]['core'][0]['verified'] = False
            elif kind == 'release':
                data[0]['require_release'] = False
            elif kind == 'package':
                data[0]['package'].pop('opentlv')
            else:
                quality['checks']['codeql']['result'] = 'failure'
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                release.check_full(quality, data, self.request)

    def test_missing_or_changed_package_cannot_assemble(self):
        package = next((self.evidence / 'packages').iterdir())
        package.write_bytes(b'tampered')
        with self.assertRaisesRegex(ValueError, 'tested package'):
            self.assemble()
        self.assertFalse(self.output.exists())
        package.unlink()
        with self.assertRaisesRegex(ValueError, 'package set'):
            self.assemble()

    def test_recomputed_checksums_cannot_hide_replaced_crate(self):
        self.assemble()
        next(self.output.glob('*.crate')).write_bytes(b'tampered')
        (self.output / 'SHA256SUMS').write_text(''.join(
            f'{digest(p)}  {p.name}\n' for p in sorted(self.output.iterdir()) if p.name != 'SHA256SUMS'),
            encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'tested package'):
            release.verify_payload(self.output)

    def test_changed_notes_extra_assets_and_checksums_are_rejected(self):
        self.assemble()
        for filename in ('release-notes.md', 'SHA256SUMS', 'extra.txt'):
            path = self.output / filename
            original = path.read_bytes() if path.exists() else None
            path.write_bytes(b'changed')
            with self.subTest(filename=filename), self.assertRaises(ValueError):
                release.verify_payload(self.output)
            if original is None:
                path.unlink()
            else:
                path.write_bytes(original)

    def test_attestation_failure_precedes_any_publication_write(self):
        self.assemble()
        with mock.patch.object(release, 'run', side_effect=subprocess.CalledProcessError(1, 'gh')) as run, \
                mock.patch.object(release, 'unused_tag') as unused, \
                self.assertRaises(subprocess.CalledProcessError):
            release.publish(self.output)
        self.assertEqual(run.call_args.args[1:3], ('attestation', 'verify'))
        unused.assert_not_called()

    def test_remote_asset_metadata_is_checked(self):
        self.assemble()
        published = {'tag_name': 'v0.1.0', 'draft': True, 'prerelease': False,
                     'body': release.NOTES.read_text(encoding='utf-8'),
                     'assets': [{'name': p.name, 'size': p.stat().st_size,
                                 'state': 'uploaded', 'digest': 'sha256:' + digest(p)}
                                for p in self.output.iterdir()]}
        release.verify_published_assets(published, self.output, self.request, True)
        for key, value in (('digest', 'sha256:' + '0' * 64), ('size', 0), ('state', 'new')):
            changed = copy.deepcopy(published)
            changed['assets'][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.verify_published_assets(changed, self.output, self.request, True)
        with self.assertRaises(ValueError):
            release.verify_published_assets({**published, 'assets': published['assets'][:-1]},
                                           self.output, self.request, True)

    def test_publication_verifies_draft_before_making_it_public(self):
        self.assemble()
        draft = {'id': 42, 'html_url': 'https://github.com/OpenTLV/opentlv-rust/releases/tag/v0.1.0',
                 'tag_name': 'v0.1.0', 'draft': True, 'prerelease': False,
                 'body': release.NOTES.read_text(encoding='utf-8'),
                 'assets': [{'name': p.name, 'size': p.stat().st_size,
                             'state': 'uploaded', 'digest': 'sha256:' + digest(p)}
                            for p in self.output.iterdir()]}
        for corrupt in (False, True):
            remote = copy.deepcopy(draft)
            if corrupt:
                remote['assets'][0]['digest'] = 'sha256:' + '0' * 64
            replies = [remote, {'sha': IDENTITY['commit']},
                       {**draft, 'draft': False}, {'sha': IDENTITY['commit']}]
            with self.subTest(corrupt=corrupt), mock.patch.object(release, 'run') as run, \
                    mock.patch.object(release, 'unused_tag'), \
                    mock.patch.object(release, 'pages', return_value=[draft]), \
                    mock.patch.object(release, 'api', side_effect=replies), \
                    mock.patch.object(release, 'ROOT', self.root), mock.patch('builtins.print'):
                if corrupt:
                    with self.assertRaisesRegex(ValueError, 'digest'):
                        release.publish(self.output)
                else:
                    release.publish(self.output)
                    self.assertEqual(read(self.root / 'build/published.json')['url'], draft['html_url'])
                calls = [call.args for call in run.call_args_list]
                self.assertTrue(all(args[1:3] == ('attestation', 'verify') for args in calls[:5]))
                self.assertEqual(calls[5][1:4], ('api', '--method', 'POST'))
                self.assertIn('--draft', calls[6])
                edits = [args for args in calls if args[1:3] == ('release', 'edit')]
                self.assertEqual(len(edits), 0 if corrupt else 1)


class Workflow(unittest.TestCase):
    def test_release_runs_verified_full_before_attesting_and_publishing(self):
        workflow = yaml.load((release.ROOT / release.WORKFLOW).read_text(encoding='utf-8'),
                             Loader=yaml.BaseLoader)
        self.assertEqual(set(workflow['on']), {'workflow_dispatch'})
        jobs = workflow['jobs']
        self.assertEqual(jobs['full']['needs'], 'prepare')
        self.assertEqual(jobs['full']['with'], {'require_verified_core': 'true'})
        self.assertEqual(jobs['full']['uses'], './.github/workflows/rust-full.yml')
        self.assertEqual(set(jobs['publish']['needs']), {'full', 'assemble'})
        for name, job in jobs.items():
            permissions = job.get('permissions', workflow['permissions'])
            self.assertEqual(permissions['contents'], 'write' if name == 'publish' else 'read')
        self.assertTrue(any(s.get('uses', '').startswith('actions/attest-build-provenance@')
                            for s in jobs['assemble']['steps']))

    def test_compiler_evidence_when_only_a_verified_binary_is_used(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'out/core-probe/CMakeFiles/4.0/CMakeCCompiler.cmake'
            path.parent.mkdir(parents=True)
            path.write_text('set(CMAKE_C_COMPILER_ID "MSVC")\n'
                            'set(CMAKE_C_COMPILER_VERSION "19.44")\n', encoding='utf-8')
            self.assertEqual(ci.compilers(root), [{'id': 'MSVC', 'version': '19.44'}])


if __name__ == '__main__':
    unittest.main()
