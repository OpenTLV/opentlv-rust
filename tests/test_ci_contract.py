# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Partial, stale or foreign CI evidence must fail the Rust quality gate."""

import argparse
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import ci  # noqa: E402
import ci_contract as contract  # noqa: E402

IDENTITY = {'repository': contract.REPOSITORY, 'commit': 'a' * 40, 'run_id': '100',
            'run_attempt': '2', 'hosted': True, 'dirty': False}
DEFINITIONS = {'scripts/ci.py': 'b' * 64}
CHECKS = {name: {'result': 'success'} for name in ('build', 'quality', 'codeql', 'api_coverage')}
SYSTEMS = {'Linux': 'x86_64-unknown-linux-gnu', 'Windows': 'x86_64-pc-windows-msvc',
           'Darwin': 'aarch64-apple-darwin'}
CARGO_OUTPUT = r'''
     Running unittests src\lib.rs (target\debug\deps\opentlv-0c54fd9c07d7d685.exe)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/fixed_abi.rs (target/debug/deps/fixed_abi-c0c57a225336563b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.16s

   Doc-tests opentlv
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.30s
'''


def record(configuration, tier):
    pinned = contract.lock()
    sanitizer = configuration['role'] == 'sanitizer'
    default = contract.expected_tests('default')['total']
    value = {
        'schema': 1, 'tier': tier, 'status': 'success', 'identity': copy.deepcopy(IDENTITY),
        'definitions': dict(DEFINITIONS), 'configuration': copy.deepcopy(configuration),
        'require_release': False,
        'runtime': {'rustc': '1.99.0-nightly' if sanitizer else configuration['toolchain'],
                    'host': SYSTEMS[configuration['system']], 'system': configuration['system']},
        'c_compiler': [{'id': 'GNU', 'version': '13.3.0'}],
        'core': [{'mode': 'verified-source', 'verified': True, 'repository': 'OpenTLV/opentlv',
                  'version': pinned['version'], 'commit': pinned['commit'], 'dirty': False,
                  'locked_version': pinned['version'], 'locked_commit': pinned['commit'],
                  'release_tag': pinned['release_tag'],
                  'release': {'mode': 'verified-source', 'release_tag': pinned['release_tag'],
                              'release_id': 7, 'manifest_sha256': 'd' * 64,
                              'signer_workflow': contract.SIGNER}}],
        'tests': {name: contract.expected_tests(name, sanitizer)
                  for name in configuration['features']}}
    checks = {
        'smoke': True, 'lint': True, 'docs': True, 'examples': contract.policy()['examples'],
        'query': contract.policy()['query'],
        'package': {crate: 'c' * 64 for crate in contract.CRATES},
        'linking': {'static': {'tests': default, 'quick_start': True},
                    'dylib': 'unsupported' if configuration['system'] == 'Windows'
                    else {'tests': default, 'quick_start': True}},
        'sanitizer': {'sanitizer': 'address', 'core_instrumented': True}}
    value.update({name: checks[name] for name in configuration['checks']})
    return value


def records(tier='pr'):
    return [record(item, tier) for item in contract.matrix(tier)]


def checks_for(tier):
    checks = copy.deepcopy(CHECKS)
    if tier == 'main':
        for name in ('quality', 'codeql', 'api_coverage'):
            checks[name]['result'] = 'skipped'
    return checks


def mutate(data, path, value):
    target = data
    for key in path[:-1]:
        target = target[key]
    if value is KeyError:
        del target[path[-1]]
    else:
        target[path[-1]] = value


class Gate(unittest.TestCase):
    def test_complete_gate(self):
        for tier, count in (('pr', 1), ('main', 1), ('full', 7)):
            with self.subTest(tier=tier):
                report = contract.validate_records(records(tier), tier, IDENTITY, DEFINITIONS,
                                                   checks_for(tier))
                self.assertEqual(report['status'], 'success')
                self.assertEqual(len(report['configurations']), count)

    def test_rejects_incomplete_or_foreign_record(self):
        cases = [
            (('schema',), 2), (('tier',), 'full'), (('status',), 'failed'),
            (('identity', 'run_attempt'), '1'), (('identity', 'run_id'), '99'),
            (('identity', 'commit'), 'e' * 40), (('identity', 'repository'), 'other/repository'),
            (('identity', 'hosted'), False), (('identity', 'dirty'), True),
            (('definitions', 'scripts/ci.py'), 'e' * 64),
            (('configuration', 'toolchain'), '1.98.0'), (('configuration', 'id'), 'unknown'),
            (('configuration', 'features'), ['default']), (('configuration', 'checks'), ['lint']),
            (('runtime', 'system'), 'Windows'), (('runtime', 'rustc'), '1.98.0'),
            (('runtime', 'host'), 'x86_64-pc-windows-msvc'), (('c_compiler',), []),
            (('core',), []), (('core', 0, 'commit'), 'f' * 40), (('core', 0, 'dirty'), True),
            (('core', 0, 'mode'), 'local-source'), (('core', 0, 'verified'), False),
            (('core', 0, 'locked_commit'), 'f' * 40), (('core', 0, 'repository'), 'other/core'),
            (('tests', 'default', 'total'), 1), (('tests', 'none', 'targets', 'fixed_abi'), 0),
            (('tests', 'lldp'), {'total': 0, 'targets': {}}), (('tests', 'none'), KeyError),
            (('lint',), False), (('docs',), KeyError), (('examples',), ['quick_start']),
            (('query', 'fixtures'), 170), (('query', 'unavailable'), 1),
            (('package',), {'opentlv': 'c' * 64}), (('smoke',), True),
            (('require_release',), True),
        ]
        for path, value in cases:
            with self.subTest(path=path):
                data = records()
                mutate(data[0], path, value)
                with self.assertRaises(ValueError):
                    contract.validate_records(data, 'pr', IDENTITY, DEFINITIONS, CHECKS)

    def test_rejects_incomplete_full_checks(self):
        by_id = {item['configuration']['id']: index for index, item in enumerate(records('full'))}
        cases = [
            ('linux-stable', ('package', 'opentlv'), 'bad'),
            ('linux-stable', ('linking', 'static', 'tests'), 1),
            ('linux-stable', ('linking', 'dylib'), 'unsupported'),
            ('windows-stable', ('linking', 'dylib'), {'tests': 1, 'quick_start': True}),
            ('macos-stable', ('linking', 'static', 'quick_start'), False),
            ('linux-msrv', ('runtime', 'rustc'), '1.99.1'),
            ('linux-sanitizer', ('runtime', 'rustc'), '1.99.1'),
            ('linux-sanitizer', ('sanitizer', 'core_instrumented'), False),
            ('linux-sanitizer', ('tests', 'default'), contract.expected_tests('default')),
        ]
        for name, path, value in cases:
            with self.subTest(configuration=name, path=path):
                data = records('full')
                mutate(data[by_id[name]], path, value)
                with self.assertRaises(ValueError):
                    contract.validate_records(data, 'full', IDENTITY, DEFINITIONS, CHECKS)

    def test_rejects_partial_full_matrix(self):
        for kind in ('missing', 'duplicate', 'substitution'):
            with self.subTest(kind=kind):
                data = records('full')
                if kind == 'missing':
                    data.pop()
                elif kind == 'duplicate':
                    data.append(copy.deepcopy(data[0]))
                else:
                    data[-1] = copy.deepcopy(data[0])
                with self.assertRaises(ValueError):
                    contract.validate_records(data, 'full', IDENTITY, DEFINITIONS, CHECKS)

    def test_rejects_unsuccessful_prerequisites(self):
        for job in ('build', 'quality', 'codeql', 'api_coverage'):
            for result in ('failure', 'cancelled', 'skipped', None):
                with self.subTest(job=job, result=result):
                    checks = copy.deepcopy(CHECKS)
                    checks[job]['result'] = result
                    with self.assertRaises(ValueError):
                        contract.validate_records(records(), 'pr', IDENTITY, DEFINITIONS, checks)
        with self.assertRaises(ValueError):
            contract.validate_records(records(), 'pr', IDENTITY, DEFINITIONS,
                                      {'build': CHECKS['build']})
        # Main permits only source quality and CodeQL to be skipped.
        with self.assertRaises(ValueError):
            contract.validate_records(records('main'), 'main', IDENTITY, DEFINITIONS, CHECKS)

    def test_release_fails_with_null_lock(self):
        data = records('full')
        for item in data:
            item['require_release'] = True
        null_lock = {**contract.lock(), 'release_tag': None}
        original_read = contract.read
        with mock.patch.object(contract, 'read', side_effect=lambda path: (
                null_lock if path == contract.LOCK else original_read(path))), \
                self.assertRaisesRegex(ValueError, 'published Core'):
            contract.validate_records(data, 'full', IDENTITY, DEFINITIONS, CHECKS, True)
        with self.assertRaisesRegex(ValueError, 'Full CI'):
            contract.validate_records(records(), 'pr', IDENTITY, DEFINITIONS, CHECKS, True)

    def test_release_requires_attestation_even_after_lock_update(self):
        pinned = contract.lock()
        released = {**pinned, 'release_tag': 'v' + pinned['version']}
        selected = {**records()[0]['core'][0], 'release_tag': released['release_tag'],
                    'mode': 'pinned-git', 'verified': False}
        release = {'mode': 'verified-source', 'release_tag': released['release_tag'],
                   'release_id': 7, 'manifest_sha256': 'd' * 64, 'signer_workflow': contract.SIGNER}
        with mock.patch.object(contract, 'lock', lambda _=False: released):
            with self.assertRaisesRegex(ValueError, 'attestation'):
                contract.validate_dependency(selected, True)
            verified = {**selected, 'mode': 'verified-source', 'verified': True, 'release': release}
            contract.validate_dependency(verified, True)
            for key, value in (('signer_workflow', 'other.yml'), ('manifest_sha256', 'bad'),
                               ('release_id', None), ('mode', 'verified-binary')):
                with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'incomplete'):
                    contract.validate_dependency(
                        {**verified, 'release': {**release, key: value}}, True)

    def test_failed_build_invalidates_stale_success(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'record.json'
            output.write_text('{"status": "success"}')
            args = argparse.Namespace(output=output, require_release=True, tier='full')
            with mock.patch.object(ci, 'lock', side_effect=ValueError('published Core missing')), \
                    self.assertRaisesRegex(ValueError, 'published Core'):
                ci.build(args)
            self.assertFalse(output.exists())


class CargoOutput(unittest.TestCase):
    def test_counts_every_target_on_any_host_path_syntax(self):
        self.assertEqual(contract.cargo_tests(CARGO_OUTPUT), {
            'total': 17, 'targets': {'opentlv': 2, 'fixed_abi': 1, 'doc:opentlv': 14}})

    def test_rejects_skipped_failed_or_missing_results(self):
        cases = {
            'ignored': CARGO_OUTPUT.replace('0 ignored; 0 measured; 0 filtered out; finished in 0.00s',
                                            '1 ignored; 0 measured; 0 filtered out; finished in 0.00s'),
            'filtered': CARGO_OUTPUT.replace('0 filtered out; finished in 11.16s',
                                             '3 filtered out; finished in 11.16s'),
            'failed': CARGO_OUTPUT.replace('test result: ok. 14 passed; 0 failed',
                                           'test result: FAILED. 13 passed; 1 failed'),
            'missing result': CARGO_OUTPUT.rsplit('test result', 1)[0],
            'duplicate target': CARGO_OUTPUT + CARGO_OUTPUT,
            'empty': 'Finished `test` profile\n',
        }
        for name, output in cases.items():
            with self.subTest(case=name), self.assertRaises(ValueError):
                contract.cargo_tests(output)

    def test_sanitizer_excludes_only_doctests(self):
        default = contract.expected_tests('default')
        sanitized = contract.expected_tests('default', sanitizer=True)
        self.assertEqual(set(default['targets']) - set(sanitized['targets']),
                         {'doc:opentlv', 'doc:opentlv_sys'})
        self.assertEqual(default['total'] - sanitized['total'],
                         sum(default['targets'][name] for name in ('doc:opentlv', 'doc:opentlv_sys')))


class QueryCorpus(unittest.TestCase):
    SUMMARY = ('{} Query fixtures and 0 generated comparisons passed '
               '({} native executions, {} unavailable-provider fixtures)\n')

    def run_query(self, summary):
        calls = []
        with tempfile.TemporaryDirectory() as directory:
            cargo = mock.Mock(side_effect=lambda *args, **kwargs: calls.append(args))
            cargo.work = Path(directory)
            native = cargo.work / 'target/debug/examples/query_conformance'
            native = native.with_suffix('.exe') if ci.os.name == 'nt' else native
            native.parent.mkdir(parents=True)
            native.touch()
            with mock.patch.object(ci, 'stream', return_value=summary):
                result = ci.query(cargo, Path(directory) / 'core')
        # A reduced-feature test build must not leave the driver that is run.
        self.assertEqual(calls, [('build', '--locked', '-p', 'opentlv', '--example',
                                  'query_conformance')])
        return result

    def test_requires_the_inventory_result(self):
        query = contract.policy()['query']
        self.assertEqual(self.run_query(self.SUMMARY.format(
            query['fixtures'], query['executions'], query['unavailable'])), query)
        # The result of a driver built without the default features.
        with self.assertRaisesRegex(ValueError, 'inventory'):
            self.run_query(self.SUMMARY.format(163, 1693, 8))


class Matrix(unittest.TestCase):
    def test_tiers(self):
        pr, main, full = (contract.matrix(tier) for tier in contract.TIERS)
        self.assertEqual([item['id'] for item in pr], ['linux-stable'])
        self.assertEqual(main[0]['checks'], ['smoke'])
        self.assertEqual(main[0]['features'], [])
        self.assertEqual(sorted(item['id'] for item in full), sorted(
            [f'{system}-{role}' for system in ('linux', 'windows', 'macos')
             for role in ('stable', 'msrv')] + ['linux-sanitizer']))
        self.assertEqual({item['system'] for item in full}, {'Linux', 'Windows', 'Darwin'})

    def test_feature_arguments(self):
        self.assertEqual(ci.feature_args('default'), [])
        self.assertEqual(ci.feature_args('none'), ['--no-default-features'])
        self.assertEqual(ci.feature_args('lldp'), ['--no-default-features', '--features', 'lldp'])
        policy = contract.policy()
        self.assertEqual(set(policy['features']), set(policy['tests']))


if __name__ == '__main__':
    unittest.main()
