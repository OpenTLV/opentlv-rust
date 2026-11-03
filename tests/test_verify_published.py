# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""A published version is immutable, so its verification must see what a new user sees."""

import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from ci import REGISTRY_CRATES, REGISTRY_SHARED  # noqa: E402
from ci_contract import matrix  # noqa: E402
from ci_support import ROOT  # noqa: E402
import crates_io  # noqa: E402
import verify_published as verify  # noqa: E402

SYS, SAFE = 'opentlv-sys', 'opentlv'
VERSION = '0.2.1'
DIGEST = 'a' * 64


def crates_api(crate, changes=None):
    """The crates.io answers for a crate that matches its registry identity."""
    identity = {**REGISTRY_SHARED, **REGISTRY_CRATES[crate]}
    version = {'checksum': DIGEST, 'yanked': False, 'license': 'MIT',
               'repository': 'https://github.com/OpenTLV/opentlv-rust',
               'description': identity['description'], 'edition': identity['edition'],
               'rust_version': identity['rust-version'], 'lib_links': identity['links'],
               'features': identity['features'], **(changes or {})}
    listed = {'keywords': list(reversed(identity['keywords'])), 'categories': identity['categories'],
              **{key: value for key, value in (changes or {}).items() if key in ('keywords', 'categories')}}

    def fetch(url, missing=(404,)):
        return json.dumps({'version': version} if url.endswith('/' + VERSION) else {'crate': listed}).encode()
    return fetch


class Metadata(unittest.TestCase):
    def check(self, crate=SYS, changes=None, logins=(crates_io.OWNER,)):
        with mock.patch.object(verify, 'fetch', crates_api(crate, changes)), \
                mock.patch.object(verify, 'owners', return_value=set(logins)):
            verify.metadata(crate, VERSION, DIGEST)

    def test_matching_identity_of_both_crates(self):
        self.check(SYS)
        self.check(SAFE)

    def test_every_differing_field_is_rejected(self):
        for field, value in (('checksum', 'b' * 64), ('yanked', True), ('license', 'Apache-2.0'),
                             ('repository', 'https://example.com'), ('description', 'other'),
                             ('edition', '2024'), ('rust_version', '1.80'), ('lib_links', None),
                             ('features', {}), ('keywords', ['tlv']), ('categories', [])):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'other opentlv-sys metadata'):
                self.check(changes={field: value})

    def test_another_owner_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'not owned'):
            self.check(logins=('someone-else',))

    def test_missing_owner_is_rejected(self):
        with mock.patch.object(verify, 'fetch', crates_api(SYS)), \
                mock.patch.object(verify, 'owners', return_value=None), \
                self.assertRaisesRegex(ValueError, 'not owned'):
            verify.metadata(SYS, VERSION, DIGEST)


class Documentation(unittest.TestCase):
    def test_only_a_successful_build_counts(self):
        for data, expected in ((None, False), (b'{"doc_status":false}', False), (b'{"doc_status":true}', True)):
            with self.subTest(data=data), mock.patch.object(verify, 'fetch', return_value=data) as fetch:
                self.assertEqual(verify.documented(SYS, VERSION), expected)
                fetch.assert_called_once_with(f'https://docs.rs/crate/{SYS}/{VERSION}/status.json')

    def test_missing_documentation_fails_after_the_wait(self):
        with mock.patch.object(verify, 'documented', return_value=False), \
                mock.patch.object(verify.time, 'sleep'), \
                mock.patch.object(verify.time, 'monotonic', side_effect=[0, verify.DOCS_WAIT_SECONDS + 1]), \
                self.assertRaisesRegex(ValueError, 'docs.rs'):
            verify.wait_documented(SYS, VERSION)


class Evidence(unittest.TestCase):
    def test_registry_records_both_crates_and_uses_one_documentation_deadline(self):
        with tempfile.TemporaryDirectory() as temp, \
                mock.patch.object(verify, 'REGISTRY_OUTPUT', Path(temp) / 'registry.json'), \
                mock.patch.object(verify, 'wait_published') as published, \
                mock.patch.object(verify, 'metadata', side_effect=lambda *args: {'checksum': DIGEST}), \
                mock.patch.object(verify, 'wait_documented') as documented, \
                mock.patch.object(verify.time, 'monotonic', return_value=10):
            verify.registry(VERSION, {crate: DIGEST for crate in (SYS, SAFE)})
            record = json.loads(verify.REGISTRY_OUTPUT.read_text())
            self.assertEqual(record['status'], 'success')
            self.assertEqual(record['version'], VERSION)
            self.assertEqual(set(record['crates']), {SYS, SAFE})
            for crate in (SYS, SAFE):
                published.assert_any_call(crate, VERSION, DIGEST)
                documented.assert_any_call(crate, VERSION, 10 + verify.DOCS_WAIT_SECONDS)
                self.assertEqual(record['crates'][crate]['documentation'],
                                 f'https://docs.rs/{crate}/{VERSION}')

    def test_failed_verification_removes_previous_success(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'result.json'
            for output, command, failure, args in (
                    ('REGISTRY_OUTPUT', verify.registry, 'wait_published',
                     (VERSION, {SYS: DIGEST, SAFE: DIGEST})),
                    ('OUTPUT', verify.consumers, 'install',
                     (VERSION, {SYS: DIGEST, SAFE: DIGEST}, 'linux-stable'))):
                with self.subTest(output=output):
                    path.write_text('{"status":"success"}')
                    with mock.patch.object(verify, output, path), \
                            mock.patch.object(verify, failure, side_effect=ValueError('failed')), \
                            self.assertRaises(ValueError):
                        command(*args)
                    self.assertFalse(path.exists())


class Workflow(unittest.TestCase):
    def setUp(self):
        text = (ROOT / crates_io.WORKFLOW).read_text(encoding='utf-8')
        self.jobs = yaml.load(text, Loader=yaml.BaseLoader)['jobs']

    def test_verification_follows_the_publication_without_any_credential(self):
        for name in ('verify-registry', 'verify-consumers'):
            job = self.jobs[name]
            self.assertEqual(job['needs'], ['prepare', 'publish'])
            self.assertEqual(job['permissions'], {'contents': 'read'})
            self.assertNotIn('environment', job)
            text = json.dumps(job)
            for secret in ('GH_TOKEN', 'github.token', 'secrets.', 'CARGO_REGISTRY_TOKEN'):
                self.assertNotIn(secret, text)

    def test_consumers_run_on_every_stable_platform(self):
        expected = [{'id': item['id'], 'os': item['os']} for item in matrix('full') if item['role'] == 'stable']
        self.assertEqual(self.jobs['verify-consumers']['strategy']['matrix']['include'], expected)

    def test_verification_reports_are_uploaded(self):
        for job, path in (('verify-registry', 'build/published-registry.json'),
                          ('verify-consumers', 'build/published-consumers.json')):
            uploads = [step for step in self.jobs[job]['steps']
                       if step.get('uses', '').startswith('actions/upload-artifact@')]
            self.assertEqual(len(uploads), 1)
            self.assertEqual(uploads[0]['with']['path'], path)
            self.assertEqual(uploads[0]['with']['if-no-files-found'], 'error')


if __name__ == '__main__':
    unittest.main()
