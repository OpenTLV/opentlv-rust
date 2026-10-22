# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Timing collection must wait for the same attempt and exclude itself."""

from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import ci_timings as timings  # noqa: E402


def job(name, complete=True):
    return {'name': 'checks / ' + name, 'status': 'completed' if complete else 'in_progress',
            'conclusion': 'success' if complete else None,
            'started_at': '2026-10-20T10:00:00Z',
            'completed_at': '2026-10-20T10:00:10Z' if complete else None}


class Timings(unittest.TestCase):
    def setUp(self):
        patcher = mock.patch.object(timings.time, 'sleep', lambda _: None)
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_waits_for_metadata_excludes_collector(self):
        calls = []

        def fetch(repo, run, attempt):
            calls.append((repo, run, attempt))
            return [job('Rust quality gate', len(calls) > 1), job('Rust / linux-stable'),
                    job('Rust timings', False)]
        with mock.patch.object(timings, 'fetch_jobs', fetch):
            result = timings.measure('OpenTLV/opentlv-rust', '100', '2', 'rust')
        self.assertEqual(calls, [('OpenTLV/opentlv-rust', '100', '2')] * 2)
        self.assertEqual((result['job_count'], result['job_seconds']), (2, 20))

    def test_rejects_partial_report_after_bounded_retry(self):
        calls = []
        with mock.patch.object(timings, 'fetch_jobs', lambda *args: calls.append(args) or [
                job('Rust quality gate'), job('build', False)]):
            with self.assertRaisesRegex(ValueError, 'incomplete'):
                timings.measure('repo', '100', '2', 'rust')
        self.assertEqual(len(calls), 6)

    def test_jobs_api_pagination_is_attempt_scoped(self):
        urls = []

        def api(url):
            urls.append(url)
            return {'jobs': [job('build')] * (100 if len(urls) == 1 else 1)}
        with mock.patch.object(timings, 'api', api):
            self.assertEqual(len(timings.fetch_jobs('repo', '100', '2')), 101)
        self.assertTrue(all('/attempts/2/jobs?' in url for url in urls))
        self.assertTrue(urls[-1].endswith('page=2'))


if __name__ == '__main__':
    unittest.main()
