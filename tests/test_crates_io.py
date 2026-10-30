# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""crates.io is immutable: every failure must precede the upload it would invalidate."""

import copy
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import urllib.error

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import crates_io  # noqa: E402
import release  # noqa: E402
from ci_support import ROOT, digest, read  # noqa: E402
from test_ci_contract import DEFINITIONS, IDENTITY  # noqa: E402
import test_release  # noqa: E402

VERSION = '0.1.0'
SYS, SAFE = 'opentlv-sys', 'opentlv'
PACKAGES = {SYS: 'a' * 64, SAFE: 'b' * 64}
TOKEN = 'cio-secret'


def registry(state):
    """Mocks owners and index_entry from {crate: (owners, {version: entry})}."""
    return (mock.patch.object(crates_io, 'owners', side_effect=lambda crate: state[crate][0]),
            mock.patch.object(crates_io, 'index_entry',
                              side_effect=lambda crate, version: state[crate][1].get(version)))


def entry(crate, **changes):
    return {'vers': VERSION, 'cksum': PACKAGES[crate], 'yanked': False, **changes}


class Plan(unittest.TestCase):
    def plan(self, state):
        owned, indexed = registry(state)
        with owned, indexed:
            return crates_io.upload_plan(VERSION, PACKAGES)

    def test_first_publication_uploads_the_dependency_first_with_the_bootstrap_token(self):
        self.assertEqual(self.plan({SYS: (None, {}), SAFE: (None, {})}), ([SYS, SAFE], 'bootstrap'))

    def test_later_versions_use_trusted_publishing_only(self):
        owner = {crates_io.OWNER, 'github:opentlv:maintainers'}
        self.assertEqual(self.plan({SYS: (owner, {}), SAFE: (owner, {})}), ([SYS, SAFE], 'trusted'))

    def test_retry_uploads_only_what_is_missing_and_never_replaces(self):
        owner = {crates_io.OWNER}
        # Interrupted first publication: the safe crate is still new and cannot use OIDC.
        self.assertEqual(self.plan({SYS: (owner, {VERSION: entry(SYS)}), SAFE: (None, {})}),
                         ([SAFE], 'bootstrap'))
        self.assertEqual(self.plan({SYS: (owner, {VERSION: entry(SYS)}), SAFE: (owner, {})}),
                         ([SAFE], 'trusted'))
        self.assertEqual(self.plan({SYS: (owner, {VERSION: entry(SYS)}),
                                    SAFE: (owner, {VERSION: entry(SAFE)})}), ([], 'none'))

    def test_other_bytes_or_a_yanked_version_require_a_new_version(self):
        owner = {crates_io.OWNER}
        for changes in ({'cksum': 'c' * 64}, {'yanked': True}):
            with self.subTest(changes=changes), self.assertRaisesRegex(ValueError, 'new version'):
                self.plan({SYS: (owner, {VERSION: entry(SYS, **changes)}), SAFE: (owner, {})})

    def test_a_name_owned_by_someone_else_stops_before_any_upload(self):
        # The safe crate is checked in the same pass, before the dependency can be uploaded.
        with self.assertRaisesRegex(ValueError, 'opentlv on crates.io is not owned'):
            self.plan({SYS: (None, {}), SAFE: ({'squatter'}, {})})


class Registry(unittest.TestCase):
    def test_requests_identify_the_client_and_missing_is_only_what_the_caller_names(self):
        response = mock.MagicMock()
        response.__enter__.return_value.read.return_value = b'data'
        with mock.patch('urllib.request.urlopen', return_value=response) as urlopen:
            self.assertEqual(crates_io.fetch('https://example.invalid/x'), b'data')
        self.assertIn('opentlv-rust', urlopen.call_args.args[0].get_header('User-agent'))

        def failing(code):
            return urllib.error.HTTPError('https://example.invalid/x', code, 'error', {}, io.BytesIO())

        with mock.patch('urllib.request.urlopen', side_effect=failing(404)):
            self.assertIsNone(crates_io.fetch('https://example.invalid/x'))
        # A blocked API request must not look like a free name.
        for code, missing in ((403, (404,)), (500, (403, 404))):
            with self.subTest(code=code), mock.patch('urllib.request.urlopen', side_effect=failing(code)), \
                    self.assertRaises(urllib.error.HTTPError):
                crates_io.fetch('https://example.invalid/x', missing=missing)

    def test_index_and_owner_formats(self):
        lines = b'{"vers":"0.0.9","cksum":"x","yanked":false}\n{"vers":"0.1.0","cksum":"y","yanked":false}\n'
        with mock.patch.object(crates_io, 'fetch', return_value=lines) as fetch:
            self.assertEqual(crates_io.index_entry(SYS, '0.1.0')['cksum'], 'y')
            self.assertIsNone(crates_io.index_entry(SYS, '0.2.0'))
            self.assertEqual(fetch.call_args.args[0], 'https://index.crates.io/op/en/opentlv-sys')
        with mock.patch.object(crates_io, 'fetch', return_value=None):
            self.assertIsNone(crates_io.index_entry(SYS, '0.1.0'))
            self.assertIsNone(crates_io.owners(SYS))
        users = json.dumps({'users': [{'login': 'someone', 'kind': 'user'}]}).encode()
        with mock.patch.object(crates_io, 'fetch', return_value=users):
            self.assertEqual(crates_io.owners(SYS), {'someone'})


class Protection(unittest.TestCase):
    # The shape GitHub returns for a protected environment, as in opentlv-lua.
    ENVIRONMENT = {
        'name': 'crates-io', 'can_admins_bypass': False,
        'deployment_branch_policy': {'protected_branches': False, 'custom_branch_policies': True},
        'protection_rules': [{'type': 'required_reviewers', 'prevent_self_review': False,
                              'reviewers': [{'type': 'User', 'reviewer': {'login': 'marekcingel'}}]},
                             {'type': 'branch_policy'}]}
    BRANCHES = {'branch_policies': [{'name': 'main', 'type': 'branch'}]}

    def check(self, environment, branches=None):
        with mock.patch.object(crates_io, 'api', side_effect=[environment, branches or self.BRANCHES]):
            crates_io.protected_environment()

    def test_accepts_required_reviewers_limited_to_main(self):
        self.check(self.ENVIRONMENT)

    def test_rejects_an_environment_that_would_not_stop_the_job(self):
        no_rules = {**self.ENVIRONMENT, 'protection_rules': [{'type': 'branch_policy'}]}
        no_reviewers = {**self.ENVIRONMENT, 'protection_rules': [
            {'type': 'required_reviewers', 'reviewers': []}]}
        bypass = {**self.ENVIRONMENT, 'can_admins_bypass': True}
        any_branch = {**self.ENVIRONMENT, 'deployment_branch_policy': None}
        for name, environment in (('rules', no_rules), ('reviewers', no_reviewers),
                                  ('bypass', bypass), ('branch', any_branch)):
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.check(environment)
        for branches in ([], [{'name': 'main', 'type': 'tag'}],
                         [{'name': 'main', 'type': 'branch'}, {'name': 'dev', 'type': 'branch'}]):
            with self.subTest(branches=branches), self.assertRaisesRegex(ValueError, 'limited to main'):
                self.check(self.ENVIRONMENT, {'branch_policies': branches})

    def test_a_missing_environment_fails_the_check(self):
        with mock.patch.object(crates_io, 'api', side_effect=subprocess.CalledProcessError(1, 'gh')), \
                self.assertRaises(subprocess.CalledProcessError):
            crates_io.protected_environment()

    def test_approval_must_be_recorded_for_this_environment(self):
        approved = {'state': 'approved', 'user': {'login': 'marekcingel'},
                    'environments': [{'name': 'crates-io'}]}
        with mock.patch.object(crates_io, 'api', return_value=[approved]) as api:
            self.assertEqual(crates_io.approved_by('5'), ['marekcingel'])
            self.assertIn('/actions/runs/5/approvals', api.call_args.args[0])
        for history in ([], [{**approved, 'state': 'rejected'}],
                        [{**approved, 'environments': [{'name': 'other'}]}]):
            with self.subTest(history=history), mock.patch.object(crates_io, 'api', return_value=history), \
                    self.assertRaisesRegex(ValueError, 'no recorded approval'):
                crates_io.approved_by('5')


class AttestedRelease(unittest.TestCase):
    """Uses a really assembled release payload; only the GitHub boundary is mocked."""

    def setUp(self):
        test_release.Payload.setUp(self)
        release.assemble(self.evidence, self.output)
        self.assets = self.root / 'assets'
        self.commit = IDENTITY['commit']
        self.remote = {'id': 42, 'html_url': 'release-url', 'tag_name': 'v0.1.0', 'draft': False,
                       'prerelease': False, 'body': release.NOTES.read_text(encoding='utf-8'),
                       'assets': [{'name': p.name, 'size': p.stat().st_size, 'state': 'uploaded',
                                   'digest': 'sha256:' + digest(p)} for p in self.output.iterdir()]}
        self.main_sha = self.commit
        self.tag_sha = self.commit

    def replies(self, path):
        if path.endswith('/git/ref/heads/main'):
            return {'object': {'sha': self.main_sha}}
        if path.endswith('/commits/v0.1.0'):
            return {'sha': self.tag_sha}
        return self.remote

    def download(self, *args, **_):
        if args[:3] == ('gh', 'release', 'download'):
            shutil.copytree(self.output, self.assets)

    def verify(self, attestations=None, definitions=DEFINITIONS, tag='v0.1.0'):
        with mock.patch.object(crates_io, 'api', side_effect=self.replies), \
                mock.patch.object(crates_io, 'run', side_effect=self.download), \
                mock.patch.object(release, 'pages', return_value=[self.remote]), \
                mock.patch.object(release, 'verify_attestations', attestations or mock.DEFAULT) as verify, \
                mock.patch.object(release, 'main_evidence') as main, \
                mock.patch.object(crates_io, 'definition_hashes', return_value=definitions):
            request, published = crates_io.verified_release(tag, self.commit, self.assets)
            if not attestations:
                verify.assert_called_once_with(self.assets, self.commit)
            main.assert_called_once_with(self.commit)
        return request, published

    def test_accepts_the_attested_tested_release_of_this_commit(self):
        request, published = self.verify()
        self.assertEqual(request, self.request)
        self.assertEqual(published, self.remote)

    def test_a_moved_main_or_tag_is_rejected_before_any_download(self):
        for name, message in (('main_sha', 'Main moved'), ('tag_sha', 'not at the current main')):
            setattr(self, name, 'f' * 40)
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, message):
                self.verify()
            setattr(self, name, self.commit)
            self.assertFalse(self.assets.exists())

    def test_a_failed_attestation_stops_verification(self):
        failing = mock.Mock(side_effect=subprocess.CalledProcessError(1, 'gh'))
        with self.assertRaises(subprocess.CalledProcessError):
            self.verify(attestations=failing)

    def test_release_evidence_must_match_this_checkout(self):
        with self.assertRaisesRegex(ValueError, 'differs from this checkout'):
            self.verify(definitions={'scripts/ci.py': 'e' * 64})

    def test_a_replaced_asset_or_changed_remote_metadata_is_rejected(self):
        for change in ({'draft': True}, {'prerelease': True}):
            saved = copy.deepcopy(self.remote)
            self.remote.update(change)
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'metadata'):
                self.verify()
            self.remote = saved
            shutil.rmtree(self.assets)
        self.remote['assets'][0]['digest'] = 'sha256:' + '0' * 64
        with self.assertRaisesRegex(ValueError, 'digest'):
            self.verify()

    def test_edited_release_text_does_not_block_the_attested_assets(self):
        # As happened to v0.1.0: generated notes were added on the release page after publication.
        self.remote['body'] = release.NOTES.read_text(encoding='utf-8') + "\r\n## What's Changed\r\n"
        self.assertEqual(self.verify()[0], self.request)

    def test_attested_release_notes_must_equal_this_checkout(self):
        other = self.root / 'other-notes.md'
        other.write_text('# OpenTLV Rust v0.1.0\n\n- Other notes\n', encoding='utf-8')
        with mock.patch.object(release, 'NOTES', other), \
                self.assertRaisesRegex(ValueError, 'differs from this checkout'):
            self.verify()

    def test_a_missing_release_is_rejected(self):
        with mock.patch.object(crates_io, 'api', side_effect=self.replies), \
                mock.patch.object(release, 'pages', return_value=[]), \
                self.assertRaisesRegex(ValueError, 'Missing or duplicate'):
            crates_io.verified_release('v0.1.0', self.commit, self.assets)


class Prepare(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.work = Path(temp.name) / 'work'
        self.output = Path(temp.name) / 'output'
        self.order = []
        request = {'version': VERSION, 'identity': IDENTITY}

        def verified(tag, commit, assets):
            self.order.append('release')
            assets.mkdir()
            for crate, data in ((SYS, b'tested sys'), (SAFE, b'tested safe')):
                (assets / f'{crate}-{VERSION}.crate').write_bytes(data)
            return request, {'html_url': 'release-url'}

        patches = [mock.patch.object(crates_io, 'WORK', self.work),
                   mock.patch.object(release, 'hosted_identity', return_value=IDENTITY),
                   mock.patch.object(crates_io, 'protected_environment',
                                     side_effect=lambda: self.order.append('environment')),
                   mock.patch.object(crates_io, 'verified_release', side_effect=verified),
                   mock.patch.object(crates_io, 'publisher', return_value='cargo'),
                   mock.patch.object(crates_io, 'approved_by', return_value=['marekcingel']),
                   mock.patch.object(crates_io, 'check_packaged'),
                   mock.patch.dict(os.environ, {'GITHUB_OUTPUT': str(self.output)}),
                   mock.patch('builtins.print')]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)

    def test_read_only_job_checks_the_protection_first_and_needs_no_approval(self):
        with mock.patch.object(crates_io, 'upload_plan', return_value=([SYS, SAFE], 'bootstrap')):
            crates_io.prepare('v0.1.0', False)
        self.assertEqual(self.order, ['environment', 'release'])
        crates_io.approved_by.assert_not_called()
        plan = read(self.work / 'plan.json')
        self.assertIsNone(plan['approved_by'])
        self.assertEqual(plan['uploads'], [SYS, SAFE])
        self.assertEqual(self.output.read_text(encoding='utf-8'), 'auth=bootstrap\n')
        crates_io.check_packaged.assert_called_once()

    def test_protected_job_records_the_approval_and_a_complete_publication_packs_nothing(self):
        with mock.patch.object(crates_io, 'upload_plan', return_value=([], 'none')):
            crates_io.prepare('v0.1.0', True)
        self.assertEqual(read(self.work / 'plan.json')['approved_by'], ['marekcingel'])
        crates_io.approved_by.assert_called_once_with(IDENTITY['run_id'])
        crates_io.check_packaged.assert_not_called()
        self.assertEqual(self.output.read_text(encoding='utf-8'), 'auth=none\n')

    def test_a_previous_plan_cannot_survive_a_failed_preparation(self):
        self.work.mkdir()
        (self.work / 'plan.json').write_text('{}', encoding='utf-8')
        crates_io.protected_environment.side_effect = ValueError('unprotected')
        with self.assertRaisesRegex(ValueError, 'unprotected'):
            crates_io.prepare('v0.1.0', True)
        self.assertFalse((self.work / 'plan.json').exists())

    def test_the_workflow_identity_is_the_publisher_not_the_release_workflow(self):
        with mock.patch.object(crates_io, 'upload_plan', return_value=([], 'none')):
            crates_io.prepare('v0.1.0', False)
        release.hosted_identity.assert_called_once_with(crates_io.SIGNER)


class Upload(unittest.TestCase):
    """Cargo is a recording stand-in that packs and uploads on request."""

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.work = Path(temp.name)
        self.packed = {SYS: b'tested sys', SAFE: b'tested safe'}
        self.packages = {crate: hashlib.sha256(data).hexdigest() for crate, data in self.packed.items()}
        self.events = []
        self.uploaded = {}
        for patch in (mock.patch.object(crates_io, 'WORK', self.work),
                      mock.patch.object(crates_io, 'publisher', return_value=self.cargo),
                      mock.patch.object(crates_io, 'index_entry', side_effect=self.index),
                      mock.patch.object(crates_io, 'fetch', side_effect=self.download),
                      mock.patch.object(crates_io.time, 'sleep')):
            patch.start()
            self.addCleanup(patch.stop)

    def plan(self, uploads=(SYS, SAFE)):
        return {'version': VERSION, 'packages': self.packages, 'uploads': list(uploads), 'auth': 'trusted'}

    def cargo(self, command, *args, env=None):
        output = self.work / 'target/package'
        output.mkdir(parents=True, exist_ok=True)
        if command == 'package':
            self.assertEqual(set(args) & {'--locked', '--no-verify'}, {'--locked', '--no-verify'})
            self.assertIsNone(env)
            crate = args[args.index('-p') + 1]
            (output / f'{crate}-{VERSION}.crate').write_bytes(self.packed[crate])
            self.events.append(('package', crate))
        else:
            crate = args[args.index('-p') + 1]
            self.assertEqual(env, {'CARGO_REGISTRY_TOKEN': TOKEN})
            self.assertEqual(set(args) & {'--locked', '--no-verify'}, {'--locked', '--no-verify'})
            self.assertNotIn('--dry-run', args)
            self.events.append(('publish', crate))
            self.uploaded[crate] = self.packed[crate]

    def index(self, crate, _version):
        if crate not in self.uploaded:
            return None
        self.events.append(('indexed', crate))
        return {'vers': VERSION, 'cksum': hashlib.sha256(self.uploaded[crate]).hexdigest(), 'yanked': False}

    def download(self, url, missing=(404,)):
        self.assertEqual(missing, (403, 404))
        crate = url.rsplit('/', 2)[1]
        return self.uploaded.get(crate)

    def test_the_dependency_is_published_and_served_before_the_safe_crate_is_packed(self):
        crates_io.upload(self.plan(), TOKEN)
        self.assertEqual(self.events, [('package', SYS), ('publish', SYS), ('indexed', SYS),
                                       ('package', SAFE), ('publish', SAFE), ('indexed', SAFE)])

    def test_different_bytes_stop_before_that_upload(self):
        self.packed[SAFE] = b'rebuilt differently'
        with self.assertRaisesRegex(ValueError, 'other bytes than the tested opentlv archive'):
            crates_io.upload(self.plan(), TOKEN)
        self.assertEqual(list(self.uploaded), [SYS])
        self.packed[SAFE] = b'tested safe'
        self.packed[SYS] = b'rebuilt differently'
        self.uploaded.clear()
        with self.assertRaisesRegex(ValueError, 'other bytes than the tested opentlv-sys archive'):
            crates_io.upload(self.plan(), TOKEN)
        self.assertEqual(self.uploaded, {})

    def test_a_retry_uploads_only_the_planned_crate(self):
        self.uploaded[SYS] = self.packed[SYS]
        crates_io.upload(self.plan([SAFE]), TOKEN)
        self.assertEqual(self.events[:2], [('package', SAFE), ('publish', SAFE)])
        self.assertNotIn(('publish', SYS), self.events)

    def test_missing_credential_fails_before_the_toolchain_or_cargo(self):
        with self.assertRaisesRegex(ValueError, 'credential'):
            crates_io.upload(self.plan(), '')
        self.assertEqual(self.events, [])
        crates_io.publisher.assert_not_called()

    def test_failed_upload_does_not_continue_with_the_safe_crate(self):
        def failing(command, *args, env=None):
            if command == 'publish':
                raise subprocess.CalledProcessError(101, 'cargo')
            self.cargo(command, *args, env=env)

        with mock.patch.object(crates_io, 'publisher', return_value=failing), \
                self.assertRaises(subprocess.CalledProcessError):
            crates_io.upload(self.plan(), TOKEN)
        self.assertNotIn(('package', SAFE), self.events)

    def test_a_version_that_never_appears_fails_the_upload(self):
        clock = iter([0, 1, crates_io.WAIT_SECONDS + 1])
        with mock.patch.object(crates_io.time, 'monotonic', side_effect=lambda: next(clock)), \
                self.assertRaisesRegex(ValueError, 'did not appear'):
            crates_io.wait_published(SYS, VERSION, self.packages[SYS])
        crates_io.time.sleep.assert_called_with(crates_io.POLL_SECONDS)

    def test_other_published_bytes_fail_at_once(self):
        self.uploaded[SYS] = b'other'
        with self.assertRaisesRegex(ValueError, 'lists other'):
            crates_io.published(SYS, VERSION, self.packages[SYS])
        with mock.patch.object(crates_io, 'index_entry', return_value={
                'vers': VERSION, 'cksum': self.packages[SYS], 'yanked': False}), \
                self.assertRaisesRegex(ValueError, 'serves other'):
            crates_io.published(SYS, VERSION, self.packages[SYS])
        with mock.patch.object(crates_io, 'index_entry', return_value={
                'vers': VERSION, 'cksum': self.packages[SYS], 'yanked': True}), \
                self.assertRaisesRegex(ValueError, 'lists other'):
            crates_io.published(SYS, VERSION, self.packages[SYS])

    def test_confirmation_covers_both_crates_even_without_an_upload(self):
        self.uploaded.update(self.packed)
        crates_io.confirm({**self.plan([]), 'auth': 'none'})
        receipt = read(self.work / 'receipt.json')
        self.assertEqual(receipt['status'], 'success')
        self.assertEqual(receipt['crates'][SAFE], f'https://crates.io/crates/opentlv/{VERSION}')
        self.uploaded.pop(SAFE)
        (self.work / 'receipt.json').unlink()
        clock = iter([0, 0, crates_io.WAIT_SECONDS + 1])
        with mock.patch.object(crates_io.time, 'monotonic', side_effect=lambda: next(clock)), \
                self.assertRaisesRegex(ValueError, 'did not appear'):
            crates_io.confirm(self.plan([]))
        self.assertFalse((self.work / 'receipt.json').exists())

    def test_the_token_leaves_the_environment_that_other_commands_inherit(self):
        with mock.patch.dict(os.environ, {'CARGO_REGISTRY_TOKEN': TOKEN, 'GITHUB_OUTPUT': 'unused'}), \
                mock.patch.object(sys, 'argv', ['crates_io.py', 'confirm']), \
                mock.patch.object(crates_io, 'read', return_value={**self.plan([]), 'auth': 'none'}), \
                mock.patch.object(crates_io, 'confirm') as confirm:
            crates_io.main()
            self.assertNotIn('CARGO_REGISTRY_TOKEN', os.environ)
        confirm.assert_called_once()


class Packaging(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.work = Path(temp.name)
        output = self.work / 'target/package'
        output.mkdir(parents=True)
        for crate in (SYS, SAFE):
            (output / f'{crate}-{VERSION}.crate').write_bytes(crate.encode())
        self.packages = {crate: digest(output / f'{crate}-{VERSION}.crate') for crate in (SYS, SAFE)}

    def check(self, crates):
        cargo = mock.Mock()
        with mock.patch.object(crates_io, 'WORK', self.work):
            crates_io.check_packaged(cargo, crates, VERSION, self.packages)
        return cargo.call_args.args

    def test_new_pairs_pack_together_and_a_single_crate_resolves_from_the_registry(self):
        # `cargo package -p opentlv` fails while opentlv-sys is not on the registry.
        self.assertEqual(self.check([SYS, SAFE]), ('package', '--workspace', '--locked', '--no-verify'))
        self.assertEqual(self.check([SAFE]), ('package', '-p', SAFE, '--locked', '--no-verify'))

    def test_no_build_script_runs_near_a_credential(self):
        self.assertIn('--no-verify', self.check([SYS]))


def workflow():
    return yaml.load((ROOT / crates_io.WORKFLOW).read_text(encoding='utf-8'), Loader=yaml.BaseLoader)


class Workflow(unittest.TestCase):
    def test_only_a_manual_dispatch_can_start_a_publication(self):
        self.assertEqual(set(workflow()['on']), {'workflow_dispatch'})
        self.assertEqual(workflow()['on']['workflow_dispatch']['inputs']['tag']['required'], 'true')

    def test_only_the_protected_job_after_the_check_has_a_credential(self):
        jobs = workflow()['jobs']
        self.assertEqual(set(jobs), {'prepare', 'publish'})
        self.assertNotIn('environment', jobs['prepare'])
        self.assertNotIn('id-token', jobs['prepare']['permissions'])
        self.assertEqual(jobs['publish']['needs'], 'prepare')
        self.assertEqual(jobs['publish']['environment']['name'], crates_io.ENVIRONMENT)
        self.assertEqual(jobs['publish']['permissions']['id-token'], 'write')
        self.assertEqual(workflow()['permissions'], {'contents': 'read'})
        for job in jobs.values():
            self.assertEqual(job['permissions']['contents'], 'read')

    def test_protected_job_requires_the_approval_and_stores_no_github_token_with_the_credential(self):
        steps = workflow()['jobs']['publish']['steps']
        prepare = next(step for step in steps if step.get('id') == 'prepare')
        self.assertTrue(prepare['run'].endswith('crates_io.py prepare --approved'))
        uploads = [step for step in steps if step.get('run', '').endswith('crates_io.py upload')]
        self.assertEqual([step['if'] for step in uploads], ["steps.prepare.outputs.auth == 'trusted'",
                                                           "steps.prepare.outputs.auth == 'bootstrap'"])
        for step in uploads:
            self.assertEqual(set(step['env']), {'CARGO_REGISTRY_TOKEN'})
        self.assertEqual(steps.index(prepare), 1)
        self.assertIn('auth.outputs.token', uploads[0]['env']['CARGO_REGISTRY_TOKEN'])
        self.assertIn('secrets.CRATES_IO_BOOTSTRAP_TOKEN', uploads[1]['env']['CARGO_REGISTRY_TOKEN'])
        text = (ROOT / crates_io.WORKFLOW).read_text(encoding='utf-8')
        self.assertEqual(text.count('secrets.'), 1)

    def test_the_token_exchange_is_pinned_and_conditional(self):
        steps = workflow()['jobs']['publish']['steps']
        auth = next(step for step in steps if step.get('id') == 'auth')
        self.assertRegex(auth['uses'], r'^rust-lang/crates-io-auth-action@[0-9a-f]{40}$')
        self.assertEqual(auth['if'], "steps.prepare.outputs.auth == 'trusted'")

    def test_no_other_workflow_can_publish(self):
        for path in (ROOT / '.github/workflows').glob('*.yml'):
            if path.name != Path(crates_io.WORKFLOW).name:
                text = path.read_text(encoding='utf-8')
                self.assertIsNone(re.search(r'cargo publish|crates-io-auth-action|CARGO_REGISTRY_TOKEN|'
                                            r'CRATES_IO', text), path.name)

    def test_attestations_stay_bound_to_the_release_workflow_and_the_dependency_comes_first(self):
        self.assertNotEqual(crates_io.SIGNER, release.SIGNER)
        self.assertEqual(crates_io.CRATES, (SYS, SAFE))


if __name__ == '__main__':
    unittest.main()
