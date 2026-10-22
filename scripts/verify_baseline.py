#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Verify the recorded migration baseline against the pinned Core Git tree."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib


ROOT = Path(__file__).resolve().parents[1]
SOURCE_COMMIT = "476f1fbecde6b056419e43c9a6cea4d1ca8df1cf"
SOURCE_DIRECTORY = "bindings/rust"
EXTRA_FILES = [
    ".github/workflows/rust.yml",
    "docs/development/rust.md",
    "docs/guides/rust.md",
]


def git(core, *args, text=True):
    return subprocess.check_output(["git", "-C", str(core), *args], text=text)


def manifest(core, path):
    return tomllib.loads(git(core, "show", f"{SOURCE_COMMIT}:{path}"))


def verify_files(core, inventory):
    paths = git(core, "ls-tree", "-r", "--name-only", SOURCE_COMMIT,
                "--", SOURCE_DIRECTORY).splitlines()
    expected = set(paths) | set(EXTRA_FILES)
    entries = inventory["files"]
    recorded = {entry["path"] for entry in entries}
    if len(recorded) != len(entries) or recorded != expected:
        raise ValueError("Inventory differs from the complete Rust tree and explicit files")
    for entry in entries:
        data = git(core, "show", f"{SOURCE_COMMIT}:{entry['path']}", text=False)
        blob = git(core, "rev-parse", f"{SOURCE_COMMIT}:{entry['path']}").strip()
        if (blob, hashlib.sha256(data).hexdigest(), len(data)) != (
                entry["blob"], entry["sha256"], entry["bytes"]):
            raise ValueError(f"Recorded identity differs: {entry['path']}")
    return len(paths)


def verify_metadata(core, inventory):
    history = git(core, "log", "--format=%H", SOURCE_COMMIT, "--", SOURCE_DIRECTORY).split()
    recorded = inventory["history"]
    if (len(history), history[-1], history[0]) != (
            recorded["commits"], recorded["first_commit"], recorded["last_commit"]):
        raise ValueError("Recorded Rust history differs")

    workspace = inventory["workspace"]
    root = manifest(core, workspace["manifest"])["workspace"]
    lock = tomllib.loads(git(core, "show", f"{SOURCE_COMMIT}:{SOURCE_DIRECTORY}/Cargo.lock"))
    if (root["resolver"], root["members"], root["package"]) != (
            workspace["resolver"], workspace["members"], workspace["package"]):
        raise ValueError("Recorded workspace metadata differs")
    if {p["name"]: p["version"] for p in lock["package"]} != workspace["locked_packages"]:
        raise ValueError("Recorded locked crate versions differ")

    crates = {}
    for member in root["members"]:
        path = f"{SOURCE_DIRECTORY}/{member}/Cargo.toml"
        data = manifest(core, path)
        package = data["package"]
        crates[package["name"]] = {
            "manifest": path,
            "description": package["description"],
            "links": package.get("links"),
            "build": package.get("build"),
            "features": data.get("features", {}),
            "dependencies": data.get("dependencies", {}),
        }
    if crates != inventory["crates"]:
        raise ValueError("Recorded crate metadata or Cargo features differ")


def verify(core):
    inventory = json.loads(
        (ROOT / "docs/migration/source-inventory.json").read_text(encoding="utf-8"))
    if inventory["source_commit"] != SOURCE_COMMIT:
        raise ValueError("The migration baseline must not change")
    if inventory["source_repository"] != "https://github.com/OpenTLV/opentlv":
        raise ValueError("Unexpected source repository")
    git(core, "cat-file", "-e", f"{SOURCE_COMMIT}^{{commit}}")
    tree_files = verify_files(core, inventory)
    verify_metadata(core, inventory)
    print(f"Verified {len(inventory['files'])} files against Core {SOURCE_COMMIT}")
    print(f"Complete {SOURCE_DIRECTORY} tree: {tree_files} files; "
          f"explicit CI and documentation files: {len(EXTRA_FILES)}")
    print(f"Verified {inventory['history']['commits']} {SOURCE_DIRECTORY} history commits, "
          f"workspace, crate versions and Cargo features")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core", type=Path, default=ROOT.parent / "opentlv",
                        help="Core Git checkout containing the recorded commit")
    args = parser.parse_args()
    try:
        verify(args.core)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Baseline verification failed: {error}\n")
