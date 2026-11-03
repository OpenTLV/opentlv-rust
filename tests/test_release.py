# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Release failures must precede writes; only tested, current artifacts may ship."""

import copy
import os
from pathlib import Path
import shutil
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
        self.assertEqual(release.version_for('v0.2.2'), '0.2.2')
        for tag in ('v0.8.0', 'v0.7.1', '0.2.2', 'v0.2.2-rc1', 'v00.2.2', '../main'):
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

    def state(self, refs, releases, tag_commit=IDENTITY['commit']):
        main = {'object': {'sha': IDENTITY['commit']}}
        with mock.patch.object(release, 'api', side_effect=[main, refs, {'sha': tag_commit}]), \
                mock.patch.object(release, 'pages', return_value=releases):
            return release.release_state('v0.2.2', IDENTITY['commit'])

    def test_release_state_is_unused_or_an_interrupted_publication_of_this_commit(self):
        tag = [{'ref': 'refs/tags/v0.2.2'}]
        draft = {'id': 42, 'tag_name': 'v0.2.2', 'draft': True, 'prerelease': False}
        other = {'id': 7, 'tag_name': 'v0.2.0', 'draft': False, 'prerelease': False}
        self.assertIsNone(self.state([{'ref': 'refs/tags/v0.2.20'}], [other]))
        self.assertEqual(self.state(tag, [other]), {'draft': None})
        self.assertEqual(self.state(tag, [draft, other]), {'draft': 42})

    def test_release_state_never_resumes_another_commit_or_a_public_release(self):
        tag = [{'ref': 'refs/tags/v0.2.2'}]
        draft = {'id': 42, 'tag_name': 'v0.2.2', 'draft': True, 'prerelease': False}
        for name, refs, releases, commit, message in (
                ('other commit', tag, [draft], 'b' * 40, 'another commit'),
                ('published', tag, [{**draft, 'draft': False}], IDENTITY['commit'], 'already exists'),
                ('prerelease', tag, [{**draft, 'prerelease': True}], IDENTITY['commit'], 'already exists'),
                ('duplicate', tag, [draft, {**draft, 'id': 43}], IDENTITY['commit'], 'Duplicate'),
                ('untagged draft', [], [draft], IDENTITY['commit'], 'without its tag')):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, message):
                self.state(refs, releases, commit)
        with mock.patch.object(release, 'api', return_value={'object': {'sha': 'b' * 40}}), \
                self.assertRaisesRegex(ValueError, 'Main moved'):
            release.release_state('v0.2.2', IDENTITY['commit'])

    def test_a_new_draft_is_found_although_the_release_list_lags_behind(self):
        draft = {'id': 42, 'tag_name': 'v0.2.2', 'draft': True}
        with mock.patch.object(release, 'pages', side_effect=[[], [], [draft]]), \
                mock.patch.object(release, 'api', return_value={**draft, 'assets': []}) as api, \
                mock.patch.object(release.time, 'sleep') as sleep:
            self.assertEqual(release.find_draft('v0.2.2')['id'], 42)
        api.assert_called_once_with(f'repos/{release.REPOSITORY}/releases/42')
        self.assertEqual(sleep.call_count, 2)
        clock = iter([0, 1, release.DRAFT_WAIT_SECONDS + 1])
        with mock.patch.object(release, 'pages', return_value=[]), \
                mock.patch.object(release.time, 'monotonic', side_effect=lambda: next(clock)), \
                mock.patch.object(release.time, 'sleep'), \
                self.assertRaisesRegex(ValueError, 'did not appear'):
            release.find_draft('v0.2.2')
        for releases, message in (([draft, {**draft, 'id': 43}], 'Duplicate'),
                                  ([{**draft, 'draft': False}], 'not a draft')):
            with self.subTest(message=message), mock.patch.object(release, 'pages', return_value=releases), \
                    mock.patch.object(release.time, 'sleep') as sleep, \
                    self.assertRaisesRegex(ValueError, message):
                release.find_draft('v0.2.2')
            sleep.assert_not_called()

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
        self.request = {'schema': 1, 'tag': 'v0.2.2', 'version': '0.2.2',
                        'identity': copy.deepcopy(IDENTITY), 'definitions': DEFINITIONS,
                        'notes_sha256': digest(release.NOTES), 'core': release.lock(True),
                        'resume': None}
        packages = self.evidence / 'packages'
        packages.mkdir(parents=True)
        for record in self.data:
            record['require_release'] = True
            if record['configuration']['id'] == 'linux-stable':
                for crate in release.CRATES:
                    path = packages / f'{crate}-0.2.2.crate'
                    path.write_bytes((crate + ' tested bytes').encode())
                    record['package'][crate] = digest(path)
                for consumer in record['consumer'].values():
                    consumer['packages'] = {crate: record['package'][crate]
                                            for crate in consumer['packages']}
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
                mock.patch.object(release, 'release_state') as state, \
                self.assertRaises(subprocess.CalledProcessError):
            release.publish(self.output)
        self.assertEqual(run.call_args.args[1:3], ('attestation', 'verify'))
        state.assert_not_called()

    def test_remote_asset_metadata_is_checked(self):
        self.assemble()
        published = {'tag_name': 'v0.2.2', 'draft': True, 'prerelease': False,
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

    def draft(self, directory=None):
        directory = directory or self.output
        return {'id': 42, 'html_url': 'https://github.com/OpenTLV/opentlv-rust/releases/tag/v0.2.2',
                'tag_name': 'v0.2.2', 'draft': True, 'prerelease': False,
                'body': release.NOTES.read_text(encoding='utf-8'),
                'assets': [{'id': index, 'name': p.name, 'size': p.stat().st_size,
                            'state': 'uploaded', 'digest': 'sha256:' + digest(p)}
                           for index, p in enumerate(sorted(directory.iterdir()))]}

    def resume(self, state):
        """Assembles a run whose request found this release state."""
        self.request['resume'] = state
        write(self.evidence / 'request.json', self.request)
        self.assemble()

    def publish(self, state, replies, earlier=None):
        """Runs publish with GitHub mocked; earlier is the directory the existing draft serves."""
        def download(asset, path):
            path.write_bytes((earlier / asset['name']).read_bytes())

        with mock.patch.object(release, 'run') as run, \
                mock.patch.object(release, 'release_state', return_value=state), \
                mock.patch.object(release, 'find_draft', side_effect=lambda tag: replies.pop(0)), \
                mock.patch.object(release, 'api', side_effect=lambda path: replies.pop(0)), \
                mock.patch.object(release, 'download', side_effect=download) as downloaded, \
                mock.patch.object(release, 'ROOT', self.root), \
                mock.patch.object(release, 'RESUMED', self.root / 'build/resumed'), \
                mock.patch('builtins.print'):
            try:
                release.publish(self.output)
            finally:
                self.calls = [call.args for call in run.call_args_list]
                self.downloads = downloaded.call_count

    def written(self, kind):
        return [args for args in self.calls if kind in ' '.join(map(str, args))]

    def test_publication_verifies_draft_before_making_it_public(self):
        self.resume(None)
        for corrupt in (False, True):
            draft = self.draft()
            remote = copy.deepcopy(draft)
            if corrupt:
                remote['assets'][0]['digest'] = 'sha256:' + '0' * 64
            replies = [remote, {'sha': IDENTITY['commit']},
                       {**self.draft(), 'draft': False}, {'sha': IDENTITY['commit']}]
            with self.subTest(corrupt=corrupt):
                if corrupt:
                    with self.assertRaisesRegex(ValueError, 'digest'):
                        self.publish(None, replies)
                else:
                    self.publish(None, replies)
                    published = read(self.root / 'build/published.json')
                    self.assertEqual(published['url'], draft['html_url'])
                    self.assertFalse(published['resumed'])
                self.assertTrue(all(args[1:3] == ('attestation', 'verify') for args in self.calls[:5]))
                self.assertEqual(self.calls[5][1:4], ('api', '--method', 'POST'))
                self.assertIn('--draft', self.calls[6])
                # Published by the identity of the verified draft, never looked up by tag again.
                edits = self.written('--method PATCH')
                self.assertEqual(edits, [] if corrupt else [
                    ('gh', 'api', '--method', 'PATCH', f'repos/{release.REPOSITORY}/releases/42',
                     '-F', 'draft=false')])
                self.assertEqual(self.downloads, 0)

    def test_an_existing_tag_of_this_commit_gets_its_draft_without_a_new_tag(self):
        self.resume({'draft': None})
        self.publish({'draft': None}, [self.draft(), {'sha': IDENTITY['commit']},
                                       {**self.draft(), 'draft': False}, {'sha': IDENTITY['commit']}])
        self.assertEqual(self.written('git/refs'), [])
        self.assertEqual(len(self.written('release create')), 1)
        self.assertEqual(len(self.written('--method PATCH')), 1)
        self.assertTrue(read(self.root / 'build/published.json')['resumed'])

    def test_a_matching_interrupted_draft_is_published_unchanged(self):
        self.resume({'draft': 42})
        earlier = self.root / 'earlier'
        shutil.copytree(self.output, earlier)
        self.publish({'draft': 42}, [self.draft(earlier), {'sha': IDENTITY['commit']},
                                     {**self.draft(earlier), 'draft': False}, {'sha': IDENTITY['commit']}],
                     earlier)
        self.assertEqual(self.downloads, 5)
        # Its downloaded assets are attested again before anything is written.
        self.assertEqual(len(self.written('attestation verify')), 10)
        self.assertEqual(self.written('git/refs') + self.written('release create'), [])
        self.assertEqual(len(self.written('--method PATCH')), 1)
        self.assertTrue(read(self.root / 'build/published.json')['resumed'])

    def test_a_mismatched_or_partial_draft_is_never_changed(self):
        self.resume({'draft': 42})
        earlier = self.root / 'earlier'
        for kind, message in (('partial', 'asset set'), ('notes', 'Release notes changed'),
                              ('digest', 'digest'), ('crate', 'checksums differ'),
                              ('state', 'not a release of this verified state'),
                              ('rebuilt', 'other crate bytes than this run tested')):
            shutil.rmtree(earlier, ignore_errors=True)
            shutil.copytree(self.output, earlier)
            # Self-consistent evidence that verify_contents returns for the downloaded draft only.
            substitute = None
            if kind == 'partial':
                # An upload interrupted after three assets.
                draft = self.draft(earlier)
                draft['assets'] = draft['assets'][:3]
            elif kind == 'notes':
                (earlier / 'release-notes.md').write_bytes(b'# OpenTLV Rust v0.2.2\n\n- Other\n')
                (earlier / 'SHA256SUMS').write_text(''.join(
                    f'{digest(p)}  {p.name}\n' for p in sorted(earlier.iterdir())
                    if p.name != 'SHA256SUMS'), encoding='utf-8')
            elif kind == 'crate':
                next(earlier.glob('*.crate')).write_bytes(b'other crate bytes')
            elif kind == 'state':
                # Consistent evidence of another commit, as an older tag would have.
                substitute = {**self.request, 'identity': {**self.request['identity'], 'commit': 'b' * 40}}
            elif kind == 'rebuilt':
                manifest = read(earlier / 'release-manifest.json')
                write(earlier / 'release-manifest.json', {**manifest, 'packages': {
                    name: 'c' * 64 for name in manifest['packages']}})
                substitute = self.request
            if kind != 'partial':
                draft = self.draft(earlier)
            if kind == 'digest':
                draft['assets'][0]['digest'] = 'sha256:' + '0' * 64

            def contents(directory, real=release.verify_contents, substitute=substitute):
                if substitute is None or directory != self.root / 'build/resumed':
                    return real(directory)
                return substitute

            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, message), \
                    mock.patch.object(release, 'verify_contents', side_effect=contents):
                self.publish({'draft': 42}, [draft], earlier)
            self.assertEqual(self.written('--method') + self.written('release create'), [])

    def test_a_changed_release_state_stops_before_any_write(self):
        self.resume(None)
        with mock.patch.object(release, 'run') as run, \
                mock.patch.object(release, 'release_state', return_value={'draft': 42}), \
                self.assertRaisesRegex(ValueError, 'state changed'):
            release.publish(self.output)
        self.assertTrue(all(call.args[1:3] == ('attestation', 'verify') for call in run.call_args_list))


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
