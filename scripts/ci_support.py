# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Small CI helpers, using only the Python standard library."""

import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args, cwd=ROOT, env=None, capture=False):
    command = list(map(str, args))
    print("+ " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, check=True, text=True,
                            encoding="utf-8", errors="replace",
                            stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def stream(*args, cwd=ROOT, env=None):
    """Run a command, echo its combined output and return it for parsing."""
    command = list(map(str, args))
    print("+ " + " ".join(command), flush=True)
    # Cargo prints `Running` on stderr and test binaries print results on
    # stdout; one pipe keeps them in order.
    with subprocess.Popen(command, cwd=cwd, env=env, text=True, encoding="utf-8",
                          errors="replace", stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT) as process:
        lines = []
        for line in process.stdout:
            print(line, end="", flush=True)
            lines.append(line)
    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, command)
    return "".join(lines)


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def api(path):
    return json.loads(run("gh", "api", path, capture=True))
