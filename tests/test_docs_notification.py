# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Exercise Main change detection and the actual notifier without real dispatches."""

import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

import yaml

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import docs_notification as producer


def workflow(name):
    return yaml.load((ROOT / ".github/workflows" / name).read_text(), Loader=yaml.BaseLoader)


class ChangeDetectionTests(unittest.TestCase):
    def test_documentation_inputs_and_unrelated_changes(self):
        for path in ("docs/guides/rust.md", "docs/quality-gates.md", "docs/validation/ci.json",
                     "opentlv/examples/reader.rs", "README.md", "CHANGELOG.md",
                     "opentlv/README.md", "opentlv-sys/README.md", producer.WORKFLOW,
                     ".github/workflows/docs-notify.yml", "scripts/docs_notification.py"):
            self.assertTrue(producer.relevant(path), path)
        for path in ("opentlv/src/lib.rs", "opentlv-sys/build.rs", "opentlv/tests/reader.rs",
                     "opentlv/Cargo.toml", "Cargo.lock", "ci/matrix.json", "docs_other/file.md",
                     "examples/reader.rs", ".github/workflows/rust-full.yml"):
            self.assertFalse(producer.relevant(path), path)

    def test_previous_success_includes_failed_cancelled_pushes_and_excludes_future(self):
        good = {"run_number": 10, "head_sha": "a" * 40, "event": "push", "status": "completed",
                "conclusion": "success", "head_branch": "main", "path": producer.WORKFLOW,
                "head_repository": {"full_name": producer.REPOSITORY}}
        invalid = [dict(good, run_number=11, conclusion="cancelled"),
                   dict(good, run_number=12, conclusion="failure"), dict(good, run_number=13),
                   dict(good, run_number=14), dict(good, run_number=12, event="pull_request"),
                   dict(good, run_number=12, head_repository={"full_name": "attacker/fork"})]
        self.assertEqual(producer.baseline([good, *invalid], 13), "a" * 40)
        self.assertEqual(producer.baseline(invalid, 13), producer.ZERO)

    def test_main_records_exact_push_attempt_and_does_not_dispatch(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            env = {"GITHUB_REPOSITORY": producer.REPOSITORY, "GITHUB_EVENT_NAME": "push",
                   "GITHUB_REF": "refs/heads/main", "GITHUB_SHA": "a" * 40,
                   "GITHUB_RUN_NUMBER": "10", "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2",
                   "GH_TOKEN": "read-only-fixture-token"}
            def api(request, timeout):
                self.assertEqual(request.get_method(), "GET")
                self.assertIn("OpenTLV/opentlv-rust/actions/workflows/rust-main.yml/runs?", request.full_url)
                self.assertEqual(timeout, 30)
                return io.BytesIO(b'{"workflow_runs": []}')
            for paths, expected in ((["docs/guides/rust.md", "opentlv/src/lib.rs"], True),
                                    (["opentlv/src/lib.rs"], False)):
                with patch.dict(os.environ, env), patch.object(producer, "urlopen", api), \
                        patch.object(producer, "__file__", str(root / "scripts/docs_notification.py")), \
                        patch.object(producer, "changed_files", return_value=paths) as changed:
                    producer.main()
                    changed.assert_called_once_with(root, producer.ZERO, "a" * 40)
                    self.assertEqual(json.loads((root / "build/docs-notification/notification.json").read_text()), {
                        "schema": 1, "repository": producer.REPOSITORY, "commit": "a" * 40, "ref": "main",
                        "run_id": 123, "run_attempt": 2, "changed": expected})
            for change in ({"GITHUB_REPOSITORY": "fork/repo"}, {"GITHUB_EVENT_NAME": "pull_request"},
                           {"GITHUB_REF": "refs/heads/feature"}):
                with patch.dict(os.environ, dict(env, **change)), self.assertRaisesRegex(ValueError, "official"):
                    producer.main()

    def test_real_git_multicommit_rename_deletion_and_first_run(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def git(*args):
                return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE).decode().strip()
            def commit():
                git("add", ".")
                git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                    "-c", "commit.gpgsign=false", "commit", "-m", "fixture")
                return git("rev-parse", "HEAD")
            git("init", "-b", "main")
            (root / "README.md").write_text("old\n")
            first = commit()
            (root / "README.md").write_text("new\n")
            docs = commit()
            (root / "build.rs").write_text("// unrelated\n")
            last = commit()
            self.assertEqual(producer.changed_files(root, docs, last), ["build.rs"])
            self.assertIn("README.md", producer.changed_files(root, first, last))
            git("mv", "README.md", "old.txt")
            renamed = commit()
            self.assertIn("README.md", producer.changed_files(root, last, renamed))
            git("rm", "old.txt")
            deleted = commit()
            self.assertEqual(producer.changed_files(root, renamed, deleted), ["old.txt"])
            self.assertTrue(any(map(producer.relevant, producer.changed_files(root, producer.ZERO, last))))
            self.assertEqual(producer.changed_files(root, last, last), [])
            with self.assertRaises(ValueError):
                producer.changed_files(root, "--bad-ref", last)


class NotificationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.work = Path(temporary.name)
        before = Path.cwd()
        os.chdir(self.work)
        self.addCleanup(os.chdir, before)
        (self.work / "evidence").mkdir()
        env = patch.dict(os.environ, {"GITHUB_REPOSITORY": producer.REPOSITORY, "DOCS_TOKEN": "fixture-secret",
                                     "GITHUB_EVENT_PATH": str(self.work / "event.json")})
        env.start()
        self.addCleanup(env.stop)
        self.run = {"id": 123, "run_attempt": 2, "head_sha": "a" * 40, "status": "completed",
                    "conclusion": "success", "event": "push", "head_branch": "main",
                    "path": producer.WORKFLOW, "head_repository": {"full_name": producer.REPOSITORY}}
        self.record = {"schema": 1, "repository": producer.REPOSITORY, "commit": "a" * 40,
                       "ref": "main", "run_id": 123, "run_attempt": 2, "changed": True}
        self.sent = []

    def execute(self, failure=None):
        (self.work / "event.json").write_text(json.dumps({"workflow_run": self.run}))
        (self.work / "evidence/notification.json").write_text(json.dumps(self.record))
        class Response:
            status = 204
            def __enter__(self):
                return self
            def __exit__(self, *args):
                pass
        def send(request, **kwargs):
            self.assertEqual(kwargs, {"timeout": 30})
            if failure:
                raise failure
            self.sent.append(request)
            return Response()
        with patch("urllib.request.urlopen", send), patch("sys.stdout", new_callable=io.StringIO):
            exec(compile(workflow("docs-notify.yml")["jobs"]["notify"]["steps"][-1]["run"],
                         "docs-notify.yml", "exec"), {})

    def test_exact_payload_and_destination(self):
        self.execute()
        self.assertEqual(len(self.sent), 1)
        self.assertEqual(self.sent[0].full_url, "https://api.github.com/repos/OpenTLV/opentlv-docs/dispatches")
        self.assertEqual(self.sent[0].get_method(), "POST")
        self.assertEqual(self.sent[0].get_header("Authorization"), "Bearer fixture-secret")
        self.assertEqual(json.loads(self.sent[0].data), {"event_type": "component-docs-updated", "client_payload": {
            "source": "rust", "repository": producer.REPOSITORY, "commit": "a" * 40, "ref": "main", "run_id": 123}})

    def test_unrelated_changes_need_no_token_or_dispatch(self):
        self.record["changed"] = False
        os.environ.pop("DOCS_TOKEN")
        self.execute()
        self.assertFalse(self.sent)

    def test_untrusted_and_mismatched_runs_never_dispatch(self):
        for change in ({"event": "pull_request"}, {"conclusion": "failure"}, {"status": "in_progress"},
                       {"path": ".github/workflows/evil.yml"}, {"head_branch": "feature"},
                       {"head_repository": {"full_name": "attacker/fork"}}, {"head_sha": "b" * 40},
                       {"run_attempt": 3}):
            with self.subTest(change=change), patch.dict(self.run, change), self.assertRaises(SystemExit):
                self.execute()
        self.assertFalse(self.sent)

    def test_stale_or_malformed_evidence_never_dispatches(self):
        for change in ({"commit": "b" * 40}, {"run_id": 99}, {"run_attempt": 1}, {"ref": "v0.1.0"},
                       {"repository": "attacker/fork"}, {"changed": "false"}, {"schema": True}, {"extra": "data"}):
            with self.subTest(change=change), patch.dict(self.record, change), self.assertRaises(SystemExit):
                self.execute()
        self.assertFalse(self.sent)

    def test_authentication_and_network_failures_never_expose_secret(self):
        with patch.dict(os.environ, {"DOCS_TOKEN": ""}), self.assertRaisesRegex(SystemExit, "TOKEN"):
            self.execute()
        for error in [HTTPError("fixture", code, "fixture-secret", {}, None) for code in (401, 403, 404, 500)] + [
                URLError("fixture-secret"), TimeoutError("fixture-secret")]:
            with self.assertRaisesRegex(SystemExit, "Docs dispatch failed") as failure:
                self.execute(error)
            self.assertNotIn("fixture-secret", str(failure.exception))

    def test_security_routing_and_main_dependencies(self):
        notify = workflow("docs-notify.yml")
        self.assertEqual(notify["on"], {"workflow_run": {"workflows": ["Rust Main CI"],
                                                        "types": ["completed"], "branches": ["main"]}})
        self.assertEqual(notify["permissions"], {"contents": "read", "actions": "read"})
        self.assertEqual(notify["concurrency"]["cancel-in-progress"], "false")
        job = notify["jobs"]["notify"]
        for guard in ("conclusion == 'success'", "event == 'push'", "head_branch == 'main'",
                      "path == '.github/workflows/rust-main.yml'", "head_repository.full_name"):
            self.assertIn(guard, job["if"])
        self.assertFalse(any("checkout@" in step.get("uses", "") for step in job["steps"]))
        download, dispatch = job["steps"]
        self.assertEqual(dispatch["shell"], "python")
        self.assertEqual(download["with"]["name"], "docs-notification-${{ github.event.workflow_run.run_attempt }}")
        self.assertEqual(download["with"]["run-id"], "${{ github.event.workflow_run.id }}")
        self.assertEqual(dispatch["env"], {"DOCS_TOKEN": "${{ secrets.OPENTLV_DOCS_DISPATCH_TOKEN }}"})
        jobs = workflow("rust-main.yml")["jobs"]
        # The reusable checks end in the always-running quality gate; a failed gate fails `checks`.
        self.assertEqual(jobs["docs_changes"]["needs"], ["checks"])
        self.assertEqual(jobs["docs_changes"]["permissions"], {"contents": "read", "actions": "read"})
        self.assertEqual(jobs["docs_changes"]["steps"][0]["with"], {"fetch-depth": "0", "persist-credentials": "false"})
        for name in ("rust-pr.yml", "rust-main.yml", "docs-notify.yml"):
            self.assertNotIn("docs-full.yml", (ROOT / ".github/workflows" / name).read_text())
        self.assertNotIn("DOCS_DISPATCH_TOKEN", (ROOT / ".github/workflows/rust-main.yml").read_text())


if __name__ == "__main__":
    unittest.main()
