# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Exercise build.rs reruns with fresh resolver outputs and a real CMake build.

The resolver is a fixture, not attestation evidence; verification has its own
tests. This regression covers the boundary between selection and compilation.
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
RESOLVER = r"""
import argparse
import json
import os
from pathlib import Path
import tempfile

parser = argparse.ArgumentParser()
for key in ('lock', 'work', 'output', 'target', 'host', 'components', 'probe'):
    parser.add_argument('--' + key)
args = parser.parse_args()
if os.environ.get('TEST_RESOLVER_FAIL'):
    raise SystemExit('fixture attestation failure')
lock = json.loads(Path(args.lock).read_text())
work = Path(args.work)
work.mkdir(parents=True, exist_ok=True)
attempt = Path(tempfile.mkdtemp(prefix='release-', dir=work))
source = attempt / 'source' / ('OpenTLV-' + lock['version'])
source.mkdir(parents=True)
(source / 'tlv').mkdir()
(source / 'cmake').mkdir()
(source / 'tlv/fixture.c').write_text('int tlv_fixture(void) { return 0; }\n')
(source / 'tlv/CMakeLists.txt').write_text('add_library(tlv STATIC fixture.c)\n')
headers = source / 'headers'
headers.mkdir()
(headers / 'version.h').write_text(
    '#define OPENTLV_VERSION_STRING "' + lock['version'] + '"\n'
    '#define OPENTLV_GIT_COMMIT_HASH "' + lock['commit'] + '"\n')
(headers / 'config.h').write_text(''.join(
    '#define ' + key + ' ' + ('1' if value == 'ON' else '0') + '\n'
    for key, value in (item.split('=') for item in args.components.split(','))))
(source / 'CMakeLists.txt').write_text('''
cmake_minimum_required(VERSION 3.16)
project(core_fixture C)
configure_file(headers/version.h generated/include/tlv/version.h COPYONLY)
configure_file(headers/config.h generated/include/tlv/config.h COPYONLY)
add_subdirectory(tlv)
''')
Path(args.output).write_text(
    'mode=verified-source\nversion=' + lock['version'] + '\ncommit=' + lock['commit']
    + '\nsource_dir=' + str(source) + '\n')
"""


class BuildScript(unittest.TestCase):
    def test_reverified_sources_build_in_one_out_dir_and_failed_verification_stops(self):
        with tempfile.TemporaryDirectory(prefix='rust-build-rerun-') as directory:
            root = Path(directory)
            manifest, output = root / 'manifest', root / 'out'
            output.mkdir()
            core = manifest / 'core'
            core.mkdir(parents=True)
            shutil.copyfile(ROOT / 'opentlv-sys/core/core-artifact.json', core / 'core-artifact.json')
            shutil.copytree(ROOT / 'opentlv-sys/core/probe', core / 'probe')
            (core / 'verified_core.py').write_text(RESOLVER, encoding='utf-8')
            executable = root / ('build-script.exe' if os.name == 'nt' else 'build-script')
            compile_result = subprocess.run(
                ['rustc', '--edition=2021', str(ROOT / 'opentlv-sys/build.rs'), '-o', str(executable)],
                capture_output=True, text=True)
            self.assertEqual(compile_result.returncode, 0, compile_result.stdout + compile_result.stderr)
            env = {key: value for key, value in os.environ.items()
                   if not key.startswith('OPENTLV_') and key not in ('CC', 'CFLAGS', 'CMAKE_GENERATOR')}
            env.update(CARGO_MANIFEST_DIR=str(manifest), OUT_DIR=str(output),
                       TARGET='fixture-target', HOST='fixture-target',
                       OPENTLV_PYTHON=sys.executable, OPENTLV_REQUIRE_VERIFIED_CORE='1')
            sources, libraries = [], []
            for _ in range(2):
                result = subprocess.run([str(executable)], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                selected = json.loads((output / 'core-selection.json').read_text())
                self.assertEqual(selected['mode'], 'verified-source')
                self.assertTrue(selected['verified'])
                source = Path(selected['source_dir'])
                self.assertTrue(source.resolve().is_relative_to(output.resolve()))
                sources.append(source)
                directories = [Path(line.removeprefix('cargo:rustc-link-search=native='))
                               for line in result.stdout.splitlines()
                               if line.startswith('cargo:rustc-link-search=native=')]
                self.assertEqual(len(directories), 1)
                library = next(p for p in directories[0].iterdir() if p.name in ('tlv.lib', 'libtlv.a'))
                libraries.append(library)
            self.assertNotEqual(sources[0], sources[1])
            self.assertNotEqual(libraries[0], libraries[1])
            previous = [path.read_bytes() for path in libraries]
            result = subprocess.run([str(executable)], env={**env, 'TEST_RESOLVER_FAIL': '1'},
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('fixture attestation failure', result.stderr)
            self.assertFalse((output / 'core-selection.json').exists())
            self.assertFalse((output / 'core-release.txt').exists())
            self.assertNotIn('cargo:rustc-link-lib=', result.stdout)
            self.assertEqual([path.read_bytes() for path in libraries], previous)


if __name__ == '__main__':
    unittest.main()
