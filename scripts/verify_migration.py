#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Verify the imported Rust sources against the frozen baseline and pinned Core tree."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from verify_baseline import SOURCE_COMMIT, SOURCE_DIRECTORY, verify as verify_baseline


ROOT = Path(__file__).resolve().parents[1]
# Files outside bindings/rust are preserved as evidence, not active configuration.
DESTINATIONS = {
    ".github/workflows/rust.yml": "docs/migration/original-rust.yml",
}
# Ignore only local Cargo build output.
IGNORED = {"target"}


def destination(path):
    return DESTINATIONS.get(path, path)


def verify(core, import_ref=None):
    verify_baseline(core)
    inventory = json.loads(
        (ROOT / "docs/migration/source-inventory.json").read_text(encoding="utf-8"))
    entries = inventory["files"]
    destinations = {destination(entry["path"]) for entry in entries}
    if len(destinations) != len(entries):
        raise ValueError("Duplicate destination")
    for entry in entries:
        original = subprocess.check_output(
            ["git", "-C", str(core), "show", f"{SOURCE_COMMIT}:{entry['path']}"])
        imported = subprocess.check_output(
            ["git", "-C", str(ROOT), "show", f"{import_ref}:{destination(entry['path'])}"]
        ) if import_ref else (ROOT / destination(entry["path"])).read_bytes()
        if (hashlib.sha256(imported).hexdigest(), len(imported)) != (
                entry["sha256"], entry["bytes"]) or imported != original:
            raise ValueError(f"Modified imported file: {destination(entry['path'])}")
    tree = {entry["path"] for entry in entries
            if entry["path"].startswith(f"{SOURCE_DIRECTORY}/")}
    imported_paths = set(subprocess.check_output(
        ["git", "-C", str(ROOT), "ls-tree", "-r", "--name-only", import_ref,
         "--", SOURCE_DIRECTORY], text=True
    ).splitlines()) if import_ref else {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / SOURCE_DIRECTORY).rglob("*")
        if path.is_file()
        and not IGNORED.intersection(path.relative_to(ROOT / SOURCE_DIRECTORY).parts[:1])
    }
    if imported_paths != tree:
        raise ValueError(f"Missing or extra files in {SOURCE_DIRECTORY}")
    print(f"Verified {len(entries)} imported files byte-for-byte against Core {SOURCE_COMMIT}"
          + (f" at import commit {import_ref}" if import_ref else ""))
    print(f"Complete {SOURCE_DIRECTORY} tree: {len(tree)} files; "
          f"preserved CI and documentation files: {len(entries) - len(tree)}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core", type=Path, default=ROOT.parent / "opentlv",
                        help="Core Git checkout containing the recorded commit")
    parser.add_argument("--import-ref",
                        help="Verify a historical import commit instead of the working tree")
    args = parser.parse_args()
    try:
        if args.import_ref and not re.fullmatch("[0-9a-f]{40}", args.import_ref):
            raise ValueError("--import-ref must be a full immutable commit SHA")
        verify(args.core, args.import_ref)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Migration verification failed: {error}\n")
