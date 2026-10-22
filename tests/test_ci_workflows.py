# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Protect trigger, ownership and required-check contracts during CI maintenance."""

import hashlib
from pathlib import Path
import sys
import tomllib
import unittest

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from ci_contract import CRATES, matrix, policy  # noqa: E402
from ci_support import ROOT, read, run  # noqa: E402
from verify_baseline import SOURCE_DIRECTORY  # noqa: E402

# Imported tests intentionally changed by issue #2 to read the selected Core tree.
ADAPTED_TESTS = {'opentlv-sys/tests/fixed_abi.rs', 'opentlv/tests/corpus.rs'}
# Issue #5 moved the workspace to the repository root; examples changed only their run hint.
RUN_HINT = (b'from the repository root.', b'from `bindings/rust`.')
# Active build configuration; the migration verifiers and records name Core's original paths.
BUILD_CONFIGURATION = ('Cargo.toml', 'Cargo.lock', '.gitignore', '.gitattributes', *CRATES, 'ci',
                       '.github', 'scripts/ci.py', 'scripts/ci_contract.py', 'scripts/ci_support.py',
                       'scripts/ci_timings.py', 'scripts/check_quality.py', 'requirements-dev.txt')


def workflow(name):
    return yaml.load((ROOT / '.github/workflows' / name).read_text(encoding='utf-8'),
                     Loader=yaml.BaseLoader)


class Workflows(unittest.TestCase):
    def test_entry_points_do_not_duplicate_full_ci(self):
        self.assertEqual(workflow('rust-pr.yml')['on'], {'pull_request': {'branches': ['main']}})
        self.assertEqual(workflow('rust-main.yml')['on'], {'push': {'branches': ['main']}})
        full = workflow('rust-full.yml')
        self.assertEqual(set(full['on']), {'workflow_dispatch', 'workflow_call'})
        self.assertEqual(full['concurrency']['cancel-in-progress'], 'false')
        self.assertEqual(full['jobs']['checks']['with']['tier'], 'full')

    def test_always_running_gate_and_attempt_scoped_artifacts(self):
        checks = workflow('rust-checks.yml')
        gate = checks['jobs']['gate']
        self.assertEqual(gate['name'], 'Rust quality gate')
        self.assertEqual(gate['if'], 'always()')
        self.assertEqual(set(gate['needs']), {'build', 'quality', 'codeql'})
        self.assertEqual(checks['jobs']['build']['strategy']['fail-fast'], 'false')
        for path in (ROOT / '.github/workflows').glob('*.yml'):
            for job in workflow(path.name)['jobs'].values():
                for step in job.get('steps', []):
                    if step.get('uses', '').startswith('actions/checkout@'):
                        self.assertEqual(step['with']['persist-credentials'], 'false')
                    if step.get('uses', '').startswith('actions/upload-artifact@'):
                        self.assertIn('github.run_attempt', step['with']['name'])
                        self.assertEqual(step['with']['if-no-files-found'], 'error')

    def test_matrix_tests_declared_msrv_and_platforms(self):
        declared = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))[
            'workspace']['package']['rust-version']
        toolchains = policy()['toolchains']
        self.assertEqual(toolchains['msrv'], declared + '.0')
        self.assertNotIn('stable', toolchains['stable'])
        self.assertTrue(toolchains['sanitizer'].startswith('nightly-20'))
        full = matrix('full')
        self.assertEqual({item['toolchain'] for item in full if item['role'] == 'msrv'},
                         {toolchains['msrv']})
        self.assertEqual({item['system'] for item in full if item['role'] in ('stable', 'msrv')},
                         {'Linux', 'Windows', 'Darwin'})


class TestOwnership(unittest.TestCase):
    def test_core_suites_are_not_run_or_copied(self):
        for path in (ROOT / '.github/workflows').glob('*.yml'):
            text = path.read_text(encoding='utf-8')
            self.assertNotIn('OPENTLV_BUILD_TESTS=ON', text)
            self.assertNotIn('ctest', text)
            self.assertNotIn('gh workflow run', text)
        self.assertIn('"-DOPENTLV_BUILD_TESTS=OFF"',
                      (ROOT / 'opentlv-sys/build.rs').read_text(encoding='utf-8'))
        # Core's Query corpus and fuzz seeds are read from the selected Core tree only.
        tracked = run('git', 'ls-files', '--cached', '--others', '--exclude-standard',
                      capture=True).splitlines()
        self.assertFalse({path for path in tracked if '/query/' in path or '/fuzz/' in path
                          or path.endswith(('corpus.json', 'reference.py'))})

    def test_imported_tests_examples_and_abi_probe_are_unchanged(self):
        inventory = read(ROOT / 'docs/migration/source-inventory.json')
        checked = 0
        for entry in inventory['files']:
            path = entry['path']
            if '/tests/' in path or '/examples/' in path:
                local = Path(path).relative_to(SOURCE_DIRECTORY).as_posix()
                if local in ADAPTED_TESTS:
                    continue
                data = (ROOT / local).read_bytes()
                if '/examples/' in path:
                    data = data.replace(*RUN_HINT)
                self.assertEqual(hashlib.sha256(data).hexdigest(), entry['sha256'], path)
                checked += 1
        probe = [entry['path'] for entry in inventory['files'] if '/abi_probe/' in entry['path']]
        self.assertEqual(len(probe), 3)
        self.assertGreater(checked, len(probe))
        self.assertIn('fixed_abi', policy()['tests']['default'])


class Layout(unittest.TestCase):
    def test_root_workspace_without_monorepository_nesting(self):
        workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']
        self.assertEqual(workspace['members'], list(CRATES))
        for crate in CRATES:
            self.assertTrue((ROOT / crate / 'Cargo.toml').is_file(), crate)
        self.assertFalse((ROOT / SOURCE_DIRECTORY).exists())
        tracked = run('git', 'ls-files', '--cached', '--others', '--exclude-standard', '--',
                      *BUILD_CONFIGURATION, capture=True).splitlines()
        self.assertIn('opentlv-sys/build.rs', tracked)
        for path in tracked:
            text = (ROOT / path).read_text(encoding='utf-8')
            self.assertNotIn(SOURCE_DIRECTORY, text, path)
            self.assertNotIn('../../..', text, path)


if __name__ == '__main__':
    unittest.main()
