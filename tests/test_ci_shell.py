# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Execute the real workflow shell steps, including macOS Bash 3.2 in the matrix."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[1]


def bash_executable():
    override = os.environ.get('OPENTLV_TEST_BASH')
    if override:
        return str(Path(override).resolve())
    if os.name == 'nt':
        # GitHub uses Git Bash; System32/bash.exe is a WSL launcher, not this shell.
        git = shutil.which('git')
        if git:
            # git.exe is in Git's cmd/ or, inside Git Bash, mingw64/bin/.
            for parent in Path(git).resolve().parents[1:3]:
                candidate = parent / 'bin/bash.exe'
                if candidate.is_file():
                    return str(candidate)
    else:
        executable = shutil.which('bash')
        if executable:
            return executable
    raise AssertionError('Bash is required; set OPENTLV_TEST_BASH to the intended executable')


class WorkflowShell(unittest.TestCase):
    def test_runner_arguments_and_exit_status(self):
        workflow = yaml.load((ROOT / '.github/workflows/rust-checks.yml').read_text(encoding='utf-8'),
                             Loader=yaml.BaseLoader)
        for job in ('build', 'gate'):
            steps = [step for step in workflow['jobs'][job]['steps']
                     if 'scripts/ci.py ' + job in step.get('run', '')]
            self.assertEqual(len(steps), 1)
            interpreter = 'python' if job == 'build' else 'python3'
            for release in ('false', 'true'):
                for runner_exit in (0, 23):
                    with self.subTest(job=job, release=release, runner_exit=runner_exit):
                        self.check(steps[0]['run'], interpreter, job, release, runner_exit)

    def check(self, script, interpreter, job, release, runner_exit):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory) / 'workflow with spaces'
            work.mkdir()
            # Record the argument vector without building or fabricating CI evidence.
            stub = interpreter + r"""() {
  printf '%s\0' "$@" > arguments.bin
  printf '%s\n' 'runner stdout'
  printf '%s\n' 'runner stderr' >&2
  return "$RUNNER_EXIT"
}
"""
            (work / 'step.sh').write_text(stub + script, encoding='utf-8', newline='\n')
            result = subprocess.run(
                [bash_executable(), '--noprofile', '--norc', '-e', '-o', 'pipefail', 'step.sh'],
                cwd=work, capture_output=True, text=True,
                env={**os.environ, 'TIER': 'full', 'CONFIGURATION': 'configuration with spaces',
                     'REQUIRE_RELEASE': release, 'RUNNER_EXIT': str(runner_exit)})
            self.assertEqual(result.returncode, runner_exit, result.stdout + result.stderr)
            expected = ['-B', 'scripts/ci.py', job, '--tier', 'full']
            if job == 'build':
                expected += ['--configuration', 'configuration with spaces', '--work', 'build/ci',
                             '--output', 'build/record.json']
                log = (work / 'build/ci.log').read_text()
                self.assertIn('runner stdout\n', log)
                self.assertIn('runner stderr\n', log)
            else:
                expected += ['--evidence', 'build/evidence', '--output', 'build/quality.json']
            if release == 'true':
                expected.append('--require-release')
            self.assertEqual((work / 'arguments.bin').read_bytes().split(b'\0'),
                             [*[argument.encode() for argument in expected], b''])


if __name__ == '__main__':
    unittest.main()
