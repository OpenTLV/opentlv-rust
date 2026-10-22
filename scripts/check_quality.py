# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Check Python syntax, workflow structure and whitespace of the CI infrastructure."""

import ast

import yaml

from ci_support import ROOT, require, run

# Git's empty tree: diffing against it checks every tracked line, not only local edits.
EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"


def main():
    for directory in ("bindings", "scripts", "tests"):
        for path in (ROOT / directory).rglob("*.py"):
            if "__pycache__" not in path.parts and "target" not in path.parts:
                ast.parse(path.read_text(encoding="utf-8-sig"), filename=str(path))
    for path in (ROOT / ".github/workflows").glob("*.yml"):
        # BaseLoader retains the GitHub Actions `on` key instead of YAML 1.1 booleans.
        workflow = yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)
        require(workflow.get("name") and workflow.get("on") and workflow.get("jobs"),
                "Invalid workflow: " + str(path))
    # Migration evidence is preserved byte-for-byte, including its whitespace.
    run("git", "diff", "--check", EMPTY_TREE, "--", ".", ":(exclude)docs/migration")


if __name__ == "__main__":
    main()
