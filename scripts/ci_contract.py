# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Shared matrix and fail-closed evidence checks for Rust's CI entry points."""

import os
from pathlib import Path
import re

from ci_support import ROOT, digest, read, require, run

REPOSITORY = "OpenTLV/opentlv-rust"
CORE_REPOSITORY = "OpenTLV/opentlv"
SIGNER = CORE_REPOSITORY + "/.github/workflows/core-release.yml"
TIERS = ("pr", "main", "full")
POLICY = ROOT / "ci/matrix.json"
LOCK = ROOT / "opentlv-sys/core/core-artifact.json"
# Optional per-configuration checks; build and Core provenance always run.
CHECKS = ("smoke", "lint", "docs", "examples", "query", "package", "linking", "sanitizer")
CRATES = ("opentlv-sys", "opentlv")
SYSTEMS = {"Linux": "-linux-", "Windows": "-windows-", "Darwin": "-apple-darwin"}
HEX = re.compile(r"[0-9a-f]{64}")


def policy():
    return read(POLICY)


def matrix(tier):
    require(tier in TIERS, "Unknown CI tier")
    data = policy()
    toolchains = data["toolchains"]
    linux = data["platforms"][0]

    def configuration(platform, role, features, checks):
        return {"id": f'{platform["id"]}-{role}', "os": platform["os"],
                "system": platform["system"], "role": role, "toolchain": toolchains[role],
                "features": features, "checks": checks}

    if tier == "main":
        return [configuration(linux, "stable", [], ["smoke"])]
    if tier == "pr":
        return [configuration(linux, "stable", ["default", "none"],
                              ["lint", "docs", "examples", "query"])]
    result = []
    for platform in data["platforms"]:
        result.append(configuration(platform, "stable", list(data["features"]),
                                    ["lint", "docs", "examples", "query", "package", "linking"]))
        result.append(configuration(platform, "msrv", ["default", "none"], []))
    result.append(configuration(linux, "sanitizer", ["default"], ["sanitizer"]))
    return result


def expected_tests(name, sanitizer=False):
    targets = dict(policy()["tests"][name])
    if sanitizer:
        # Rustdoc tests are not instrumented; the sanitizer runs libraries and test targets.
        targets = {key: value for key, value in targets.items() if not key.startswith("doc:")}
    return {"total": sum(targets.values()), "targets": targets}


def cargo_tests(output):
    """Per-target counts from `cargo test` output; any skip or filter is a failure."""
    results = {}
    current = None
    for line in output.splitlines():
        line = line.strip()
        running = re.fullmatch(r"Running (?:unittests )?\S+ \((.+)\)", line)
        doc = re.fullmatch(r"Doc-tests (\S+)", line)
        if running or doc:
            require(current is None, "Test target ended without a result")
            if running:
                # Binary stem without Cargo's metadata hash, on any host's path syntax.
                name = re.split(r"[\\/]", running[1])[-1].removesuffix(".exe").rsplit("-", 1)[0]
            else:
                name = "doc:" + doc[1]
            require(name not in results, "Duplicate test target: " + name)
            current = name
            continue
        summary = re.match(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
                           r"(\d+) measured; (\d+) filtered out", line)
        if summary:
            require(current is not None, "Test result without a target")
            require(summary[1] == "ok" and all(int(value) == 0 for value in summary.groups()[2:]),
                    "Failed, ignored, measured or filtered tests in " + current)
            results[current] = int(summary[2])
            current = None
    require(current is None and results, "Missing or incomplete Rust test results")
    return {"total": sum(results.values()), "targets": results}


def definition_hashes():
    listed = run("git", "ls-files", "--cached", "--others", "--exclude-standard", "--",
                 "requirements-dev.txt", "CHANGELOG.md", "Cargo.toml", "Cargo.lock", *CRATES, "ci", "scripts",
                 "tests", ".github/workflows", capture=True).splitlines()
    paths = sorted({path for path in listed if (ROOT / path).is_file()})
    return {path: digest(ROOT / path) for path in paths}


def identity():
    hosted = os.environ.get("GITHUB_ACTIONS") == "true"
    commit = run("git", "rev-parse", "HEAD", capture=True)
    dirty = bool(run("git", "status", "--porcelain", "--untracked-files=all", capture=True))
    if hosted:
        require(commit == os.environ["GITHUB_SHA"] and not dirty, "CI checkout identity changed")
    return {"repository": os.environ.get("GITHUB_REPOSITORY", REPOSITORY),
            "commit": commit, "run_id": os.environ.get("GITHUB_RUN_ID", "local"),
            "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "local"),
            "hosted": hosted, "dirty": dirty}


def lock(require_release=False):
    value = read(LOCK)
    require(value.get("schema") == 1 and value.get("repository") == CORE_REPOSITORY
            and re.fullmatch(r"[0-9a-f]{40}", str(value.get("commit")))
            and isinstance(value.get("version"), str), "Unsupported Core dependency lock")
    require(not require_release or value.get("release_tag") is not None,
            "A published Core release must be locked for verified builds")
    return value


def validate_dependency(selected, require_release=False):
    pinned = lock(require_release)
    require(selected.get("repository") == CORE_REPOSITORY
            and all(selected.get(key) == pinned[key] for key in ("commit", "version"))
            and selected.get("locked_commit") == pinned["commit"]
            and selected.get("locked_version") == pinned["version"]
            and selected.get("release_tag") == pinned["release_tag"],
            "Core selection differs from the dependency lock")
    require(selected.get("dirty") is False, "Core selection is modified")
    mode = selected.get("mode")
    if pinned["release_tag"] is None:
        require(mode == "pinned-git" and selected.get("verified") is False,
                "Expected a clean pinned Core checkout")
    else:
        require(mode in ("verified-source", "verified-binary") and selected.get("verified") is True,
                "Core release attestation is required")
        release = selected.get("release", {})
        require(release.get("mode") == mode and release.get("release_tag") == pinned["release_tag"]
                and release.get("release_id") and release.get("signer_workflow") == SIGNER
                and HEX.fullmatch(str(release.get("manifest_sha256", ""))),
                "Core release identity is incomplete")


def validate_records(records, tier, expected_identity, definitions, checks, require_release=False):
    require(tier in TIERS, "Unknown CI tier")
    require(not require_release or tier == "full", "Release requires Full CI")
    required = {"build", "quality", "codeql", "api_coverage"}
    require(set(checks) == required, "Missing or unexpected prerequisite job")
    for name in required:
        expected = "skipped" if tier == "main" and name != "build" else "success"
        require(checks[name].get("result") == expected, f"Unsuccessful prerequisite: {name}")
    expected = {item["id"]: item for item in matrix(tier)}
    require(len(records) == len(expected), "Missing or duplicate configuration evidence")
    data = policy()
    seen = set()
    for record in records:
        name = record.get("configuration", {}).get("id")
        require(name in expected and name not in seen, "Unexpected or duplicate configuration")
        seen.add(name)
        configuration = expected[name]
        require(record.get("schema") == 1 and record.get("tier") == tier
                and record.get("status") == "success", "Unsuccessful build evidence")
        require(record.get("identity") == expected_identity, "Wrong repository/commit/run/attempt")
        require(record.get("definitions") == definitions, "CI definitions differ")
        require(record.get("configuration") == configuration, "Configuration differs")
        require(record.get("require_release") is require_release, "Release policy differs")
        validate_runtime(record.get("runtime", {}), configuration)
        compilers = record.get("c_compiler", [])
        require(len(compilers) == 1 and compilers[0].get("id") and compilers[0].get("version"),
                "Missing or inconsistent C compiler evidence")
        require(record.get("core"), "Missing Core selection evidence")
        for selected in record["core"]:
            validate_dependency(selected, require_release)
        sanitizer = configuration["role"] == "sanitizer"
        require(set(record.get("tests", {})) == set(configuration["features"]),
                "Missing or unexpected feature configuration")
        for feature in configuration["features"]:
            require(record["tests"][feature] == expected_tests(feature, sanitizer),
                    "Incomplete Rust test suite: " + feature)
        for check in CHECKS:
            require((check in record) == (check in configuration["checks"]),
                    "Missing or unexpected check: " + check)
        validate_checks(record, configuration, data)
    return {"schema": 1, "tier": tier, "identity": expected_identity,
            "definitions": definitions, "require_release": require_release,
            "checks": checks, "configurations": sorted(seen), "status": "success"}


def validate_runtime(runtime, configuration):
    release = runtime.get("rustc", "")
    if configuration["role"] == "sanitizer":
        require(release.endswith("-nightly"), "Sanitizer requires the pinned nightly toolchain")
    else:
        require(release == configuration["toolchain"], "Rust toolchain differs from matrix")
    require(runtime.get("system") == configuration["system"]
            and SYSTEMS[configuration["system"]] in runtime.get("host", ""),
            "Runtime differs from matrix")


def validate_checks(record, configuration, data):
    default = expected_tests("default")["total"]
    for check in configuration["checks"]:
        value = record[check]
        if check in ("smoke", "lint", "docs"):
            require(value is True, "Unsuccessful check: " + check)
        elif check == "examples":
            require(value == data["examples"], "Incomplete examples")
        elif check == "query":
            require(value == data["query"], "Incomplete Query corpus run")
        elif check == "package":
            require(set(value) == set(CRATES) and all(HEX.fullmatch(str(item)) for item in value.values()),
                    "Missing crate package digests")
        elif check == "linking":
            dylib = ("unsupported" if configuration["system"] == "Windows"
                     else {"tests": default, "quick_start": True})
            require(value == {"static": {"tests": default, "quick_start": True}, "dylib": dylib},
                    "Incomplete native linking checks")
        elif check == "sanitizer":
            require(value == {"sanitizer": "address", "core_instrumented": True},
                    "Incomplete sanitizer evidence")
