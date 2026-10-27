# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Protect the REUSE annotations, the license texts and the copyright holder."""

from pathlib import Path
import re
import sys
import tomllib
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from ci_support import ROOT, run  # noqa: E402

HOLDER = '2026 Marek Cingel'
# Copyright lines of headers, REUSE metadata and LICENSE files, in any comment syntax.
NOTICE = re.compile(r'^[\s#/*;-]*(?:Copyright \(c\)|SPDX-FileCopyrightText:?\s*=?)\s*"?(.+)$',
                    re.MULTILINE)
# REUSE does not read examples between these markers as the file's own metadata.
IGNORED = re.compile(rb'REUSE-IgnoreStart.*?REUSE-IgnoreEnd', re.DOTALL)


def tracked():
    return run('git', 'ls-files', '--cached', '--others', '--exclude-standard',
               capture=True).splitlines()


def annotations():
    return tomllib.loads((ROOT / 'REUSE.toml').read_text(encoding='utf-8'))['annotations']


class Licensing(unittest.TestCase):
    def test_annotations_list_existing_unheadered_files_explicitly(self):
        files = set(tracked())
        listed = [path for annotation in annotations() for path in annotation['path']]
        self.assertEqual(len(listed), len(set(listed)))
        for path in listed:
            # A glob would also cover new sources, which must carry their own header.
            self.assertFalse(set('*?[') & set(path), path)
            self.assertIn(path, files)
            data = IGNORED.sub(b'', (ROOT / path).read_bytes())
            self.assertNotIn(b'SPDX-License-Identifier', data, path)
        for annotation in annotations():
            self.assertEqual(annotation['SPDX-FileCopyrightText'], HOLDER)
            self.assertEqual(annotation['SPDX-License-Identifier'], 'MIT')

    def test_preserved_migration_evidence_is_annotated(self):
        listed = {path for annotation in annotations() for path in annotation['path']}
        evidence = {path for path in tracked() if path.startswith('docs/migration/')}
        self.assertTrue(evidence)
        self.assertLessEqual(evidence | {'opentlv-sys/tests/abi_probe/CMakeLists.txt'}, listed)

    def test_reuse_license_text_matches_repository_license(self):
        license_text = (ROOT / 'LICENSE').read_text(encoding='utf-8')
        template = (ROOT / 'LICENSES/MIT.txt').read_text(encoding='utf-8')
        # The texts differ only in the copyright line and their line wrapping.
        license_text = license_text.replace(f'Copyright (c) {HOLDER}',
                                            'Copyright (c) <year> <copyright holders>')
        self.assertEqual(license_text.split(), template.split())
        self.assertEqual(sorted(path.name for path in (ROOT / 'LICENSES').iterdir()), ['MIT.txt'])

    def test_organization_is_not_a_copyright_holder(self):
        notices = 0
        for path in tracked():
            data = (ROOT / path).read_bytes()
            if b'\0' in data:
                continue
            for holder in NOTICE.findall(data.decode('utf-8')):
                notices += 1
                self.assertNotIn('OpenTLV', holder, path)
        self.assertGreater(notices, 100)


if __name__ == '__main__':
    unittest.main()
