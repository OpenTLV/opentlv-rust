# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Build and test one Rust CI configuration, or aggregate exact CI evidence."""

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import sys
import tarfile
import time
import tomllib

from ci_support import ROOT, digest, read, require, run, stream, write
from ci_contract import (CRATES, REPOSITORY, TIERS, cargo_tests, definition_hashes,
                         expected_tests, identity, lock, matrix, policy, validate_dependency,
                         validate_records)
from registry_consumer import consumers

# Variables that select or replace Core; each check sets the ones it needs.
CORE_OVERRIDES = ("OPENTLV_LIB_DIR", "OPENTLV_LINK_KIND", "OPENTLV_CORE_SOURCE",
                  "OPENTLV_REQUIRE_VERIFIED_CORE", "RUSTFLAGS", "RUSTDOCFLAGS",
                  "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR", "CC", "CFLAGS")
# The Core CMake options of build.rs with every Cargo feature enabled.
CORE_OPTIONS = ("-DOPENTLV_READER=ON", "-DOPENTLV_WRITER=ON", "-DOPENTLV_QUERY=ON",
                "-DOPENTLV_SCHEMA=ON", "-DOPENTLV_CODEC=ON", "-DOPENTLV_DOCUMENT=ON",
                "-DOPENTLV_NFC=ON", "-DOPENTLV_LLDP=ON", "-DCMAKE_BUILD_TYPE=Release",
                "-DCMAKE_POSITION_INDEPENDENT_CODE=ON", "-DOPENTLV_BUILD_CXX=OFF",
                "-DOPENTLV_BUILD_TESTS=OFF", "-DOPENTLV_BUILD_EXAMPLES=OFF",
                "-DOPENTLV_WARNINGS_AS_ERRORS=OFF")
# Files Cargo adds to every package besides the tracked crate sources.
GENERATED = {".cargo_vcs_info.json", "Cargo.toml", "Cargo.toml.orig", "Cargo.lock"}
# Without these a registry build cannot select and authenticate Core.
BUILD_FILES = {
    "opentlv-sys": {"build.rs", "core/core-artifact.json", "core/select.rs", "core/verified_core.py",
                    "core/probe/CMakeLists.txt"},
    "opentlv": {"build.rs"},
}
# What crates.io shows and consumers resolve; a published version cannot be corrected.
REGISTRY_SHARED = {"edition": "2021", "rust-version": "1.82", "build": "build.rs", "publish": None,
                   "keywords": ["tlv", "ber", "asn1", "emv", "parser"]}
REGISTRY_CRATES = {
    "opentlv-sys": {
        "description": "Raw FFI bindings to the OpenTLV C API", "links": "tlv",
        "categories": ["external-ffi-bindings"],
        "features": {"default": ["lldp", "document", "nfc"], "lldp": [], "nfc": [], "document": []}},
    "opentlv": {
        "description": "Safe Rust bindings for OpenTLV", "links": None,
        "categories": ["parser-implementations", "encoding", "api-bindings"],
        "features": {"default": ["lldp", "document", "nfc"], "lldp": ["opentlv-sys/lldp"],
                     "nfc": ["opentlv-sys/nfc"], "document": ["opentlv-sys/document"]}},
}


class Cargo:
    """Runs Cargo for one configuration with an explicit, sanitized environment."""

    def __init__(self, configuration, work, require_release):
        self.work = work
        self.sanitizer = configuration["role"] == "sanitizer"
        env = {key: value for key, value in os.environ.items() if key not in CORE_OVERRIDES}
        env.update(RUSTUP_TOOLCHAIN=configuration["toolchain"], CARGO_TERM_COLOR="never",
                   RUSTFLAGS="-D warnings")
        if require_release:
            env["OPENTLV_REQUIRE_VERIFIED_CORE"] = "1"
        if self.sanitizer:
            sanitizer = policy()["sanitizer"]
            require(shutil.which(sanitizer["cc"]), sanitizer["cc"] + " is required")
            env.update(RUSTFLAGS="-D warnings " + sanitizer["rustflags"], CC=sanitizer["cc"],
                       CFLAGS=sanitizer["cflags"], ASAN_OPTIONS=sanitizer["asan_options"])
        self.env = env

    def __call__(self, *args, target="target", env=None):
        return stream("cargo", *args, cwd=ROOT,
                      env={**self.env, "CARGO_TARGET_DIR": str(self.work / target), **(env or {})})

    def rustc(self):
        output = run("rustc", "-vV", env=self.env, capture=True)
        return dict(line.split(": ", 1) for line in output.splitlines()[1:] if ": " in line)


def feature_args(name):
    features = policy()["features"][name]
    if features is None:
        return []
    return ["--no-default-features"] + (["--features", ",".join(features)] if features else [])


def install(configuration):
    components = []
    if "lint" in configuration["checks"]:
        components = ["--component", "rustfmt", "--component", "clippy"]
    run("rustup", "toolchain", "install", configuration["toolchain"], "--profile", "minimal",
        "--no-self-update", *components)


def selections(work, require_release):
    """Validate every Core selection recorded by an opentlv-sys build script."""
    found = sorted(work.rglob("core-selection.json"))
    require(found, "No Core selection evidence")
    result = []
    for path in found:
        selected = read(path)
        source = Path(selected["source_dir"]).resolve()
        # Only Core resolved inside this configuration's fresh OUT_DIRs is accepted.
        require(source.is_relative_to(work), "Core was not resolved inside the CI work directory")
        if selected.get("verified"):
            evidence = read(source.parents[1] / "verification.json")
            selected["release"] = {key: evidence.get(key) for key in (
                "mode", "release_tag", "release_id", "manifest_sha256", "signer_workflow")}
        validate_dependency(selected, require_release)
        result.append(selected)
    return result


def core_identity(selected):
    return {key: value for key, value in selected.items()
            if key not in ("source_dir", "lib_dir", "include_dir")}


def compilers(work):
    found = set()
    for path in work.rglob("CMakeCCompiler.cmake"):
        if "cmake-build" in path.parts or "core-probe" in path.parts:
            text = path.read_text(encoding="utf-8")
            values = [re.search(rf'set\(CMAKE_C_COMPILER_{key} "([^"]*)"\)', text)
                      for key in ("ID", "VERSION")]
            require(all(values), "Incomplete CMake compiler probe")
            found.add((values[0][1], values[1][1]))
    return [{"id": compiler, "version": version} for compiler, version in sorted(found)]


def core_source(selected):
    """The verified Core tree that provides headers, Query corpus and prebuilt sources."""
    source = Path(selected["source_dir"])
    if selected["mode"] == "pinned-git":
        require(run("git", "-C", source, "rev-parse", "HEAD", capture=True) == selected["commit"]
                and not run("git", "-C", source, "status", "--porcelain", capture=True),
                "Core corpus revision changed")
    else:
        require((source / "opentlv-source.txt").read_text(encoding="utf-8") ==
                f'version={selected["version"]}\ncommit={selected["commit"]}\n',
                "Core corpus archive identity differs")
    return source


def examples(cargo):
    names = policy()["examples"]
    present = sorted(path.stem for path in (ROOT / "opentlv/examples").glob("*.rs"))
    # query_conformance is the native driver of Core's Query corpus runner.
    require(present == sorted(names + ["query_conformance"]), "Example inventory differs")
    for name in names:
        cargo("run", "--locked", "-q", "-p", "opentlv", "--example", name)
    return names


def query(cargo, source):
    # The uplifted example is from the last build; rebuild it with the default features.
    cargo("build", "--locked", "-p", "opentlv", "--example", "query_conformance")
    native = cargo.work / "target/debug/examples/query_conformance"
    if os.name == "nt":
        native = native.with_suffix(".exe")
    require(native.is_file(), "Missing Query conformance driver")
    output = stream(sys.executable, "-B", source / "tests/query/run.py",
                    "--native", native, "--property-cases", "0", cwd=cargo.work)
    match = re.search(r"(\d+) Query fixtures and (\d+) generated comparisons passed "
                      r"\((\d+) native executions, (\d+) unavailable-provider fixtures\)", output)
    require(match is not None, "Missing Query corpus summary")
    result = dict(zip(("fixtures", "comparisons", "executions", "unavailable"), map(int, match.groups())))
    require(result == policy()["query"], "Query corpus result differs from the inventory")
    return result


def packaged_manifest(crate, version, manifest):
    """Check the normalized manifest that a registry user of the archive receives."""
    package = manifest["package"]
    require((package["name"], package["version"], package["license"], package.get("readme")) ==
            (crate, version, "MIT", "README.md"), f"Wrong packaged {crate} metadata")
    require(package.get("repository") == f"https://github.com/{REPOSITORY}",
            f"Wrong packaged {crate} repository")
    expected = {**REGISTRY_SHARED, **REGISTRY_CRATES[crate]}
    require({key: manifest.get(key) if key == "features" else package.get(key) for key in expected} ==
            expected, f"Wrong packaged {crate} registry identity")
    tables = [manifest.get(key, {}) for key in ("dependencies", "dev-dependencies", "build-dependencies")]
    tables += [target.get(key, {}) for target in manifest.get("target", {}).values()
               for key in ("dependencies", "dev-dependencies", "build-dependencies")]
    dependencies = {name: spec for table in tables for name, spec in table.items()}
    require(not any("path" in spec for spec in dependencies.values()),
            f"Packaged {crate} keeps a local dependency path")
    # opentlv is released only together with the opentlv-sys version it was tested with.
    expected = {"opentlv-sys": "=" + version} if crate == "opentlv" else {}
    require({name: spec.get("version") for name, spec in dependencies.items()} == expected,
            f"Wrong packaged {crate} dependencies")


def packaged_files(crate, names, expected):
    """Check the files of an archive; the build files are required even if untracked."""
    require(BUILD_FILES[crate] <= names, f"Packaged {crate} lacks its build files")
    require(names == expected, f"Packaged {crate} files differ from its tracked sources")


def package(cargo, record):
    arguments = ["package", "--workspace", "--locked"]
    if record["identity"]["dirty"]:
        require(not record["identity"]["hosted"], "Hosted packages require a clean checkout")
        arguments.append("--allow-dirty")
    cargo(*arguments)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))[
        "workspace"]["package"]["version"]
    output = cargo.work / "target/package"
    require(sorted(path.name for path in output.glob("*.crate")) ==
            sorted(f"{crate}-{version}.crate" for crate in CRATES), "Unexpected crate packages")
    digests = {}
    for crate in CRATES:
        path = output / f"{crate}-{version}.crate"
        prefix = f"{crate}-{version}/"
        tracked = run("git", "ls-files", "--cached", "--others", "--exclude-standard", "--",
                      crate, capture=True).splitlines()
        expected = {name.removeprefix(f"{crate}/") for name in tracked} | GENERATED
        with tarfile.open(path) as archive:
            members = archive.getmembers()
            require(all(member.isfile() and member.name.startswith(prefix) for member in members),
                    "Unexpected crate archive entry")
            packaged_files(crate, {member.name.removeprefix(prefix) for member in members}, expected)
            vcs = json.load(archive.extractfile(prefix + ".cargo_vcs_info.json"))
            manifest = tomllib.load(archive.extractfile(prefix + "Cargo.toml"))
            require(archive.extractfile(prefix + "LICENSE").read() == (ROOT / "LICENSE").read_bytes(),
                    f"Packaged {crate} license differs from the repository license")
        require(vcs.get("path_in_vcs") == crate, "Wrong packaged crate path")
        packaged_manifest(crate, version, manifest)
        if record["identity"]["hosted"]:
            require(vcs.get("git", {}).get("sha1") == record["identity"]["commit"],
                    "Package was not built from the CI commit")
        digests[crate] = digest(path)
    return digests


def linking(cargo, source, configuration):
    expected = expected_tests("default")
    result = {}
    for kind, shared in (("static", "OFF"), ("dylib", "ON")):
        if kind == "dylib" and configuration["system"] == "Windows":
            # Known Core limitation: Windows DLL data symbols are not imported.
            result[kind] = "unsupported"
            continue
        build = cargo.work / f"core-{kind}"
        run("cmake", "-S", source, "-B", build, *CORE_OPTIONS, f"-DOPENTLV_BUILD_SHARED_LIBS={shared}")
        run("cmake", "--build", build, "--target", "tlv", "--config", "Release")
        patterns = (("libtlv.a", "tlv.lib") if kind == "static"
                    else ("libtlv.so", "libtlv.dylib"))
        directory = next((build / sub for sub in ("tlv", "tlv/Release")
                          if any((build / sub / name).exists() for name in patterns)), None)
        require(directory is not None, f"CMake did not produce the {kind} Core library")
        env = {"OPENTLV_LIB_DIR": str(directory), "OPENTLV_LINK_KIND": kind}
        if kind == "dylib":
            variable = "DYLD_LIBRARY_PATH" if configuration["system"] == "Darwin" else "LD_LIBRARY_PATH"
            env[variable] = os.pathsep.join(filter(None, (str(directory), os.environ.get(variable))))
        tests = cargo_tests(cargo("test", "--workspace", "--locked", target=f"target-{kind}", env=env))
        require(tests == expected, f"Rust test inventory differs with a {kind} Core library")
        cargo("run", "--locked", "-q", "-p", "opentlv", "--example", "quick_start",
              target=f"target-{kind}", env=env)
        result[kind] = {"tests": tests["total"], "quick_start": True}
    return result


def sanitized(work):
    caches = [path for path in work.rglob("CMakeCache.txt") if path.parent.name == "cmake-build"]
    flags = policy()["sanitizer"]["cflags"]
    require(caches and all(f"CMAKE_C_FLAGS:STRING={flags}" in path.read_text(encoding="utf-8")
                           for path in caches), "Core was not built with the sanitizer")
    return {"sanitizer": "address", "core_instrumented": True}


def build(args):
    # Invalidate a previous success before any check/build can fail.
    args.output.unlink(missing_ok=True)
    lock(args.require_release)
    require(not args.require_release or args.tier == "full", "Release requires Full CI")
    configurations = {item["id"]: item for item in matrix(args.tier)}
    require(args.configuration in configurations, "Configuration is outside this tier")
    configuration = configurations[args.configuration]
    require(platform.system() == configuration["system"], "Platform does not match requested configuration")
    work = args.work.resolve()
    require(not work.exists(), "Use a fresh --work directory; cached evidence is not accepted")
    work.mkdir(parents=True)
    started = time.monotonic()
    install(configuration)
    cargo = Cargo(configuration, work, args.require_release)
    rustc = cargo.rustc()
    record = {"schema": 1, "tier": args.tier, "configuration": configuration,
              "identity": identity(), "definitions": definition_hashes(),
              "require_release": args.require_release,
              "runtime": {"rustc": rustc.get("release"), "host": rustc.get("host"),
                          "rustc_commit": rustc.get("commit-hash"), "llvm": rustc.get("LLVM version"),
                          "system": platform.system(), "machine": platform.machine()}}
    checks = configuration["checks"]
    # Sanitized code must not leak into build scripts, so name the target explicitly.
    target = ["--target", rustc["host"]] if cargo.sanitizer else []
    if "lint" in checks:
        cargo("fmt", "--all", "--check")
        for name in ("default", "none"):
            cargo("clippy", "--workspace", "--all-targets", "--locked", *feature_args(name),
                  "--", "-D", "warnings")
        record["lint"] = True
    if "smoke" in checks:
        cargo("build", "--workspace", "--locked")
        cargo("run", "--locked", "-q", "-p", "opentlv", "--example", "quick_start")
        record["smoke"] = True
    else:
        cargo("build", "--workspace", "--all-targets", "--locked", *target)
    record["tests"] = {}
    for name in configuration["features"]:
        selected = ["--lib", "--tests"] if cargo.sanitizer else []
        tests = cargo_tests(cargo("test", "--workspace", "--locked", *feature_args(name),
                                  *target, *selected))
        require(tests == expected_tests(name, cargo.sanitizer), "Rust test inventory differs: " + name)
        record["tests"][name] = tests
    source = core_source(selections(work, args.require_release)[0])
    if "examples" in checks:
        record["examples"] = examples(cargo)
    if "query" in checks:
        record["query"] = query(cargo, source)
    if "docs" in checks:
        cargo("doc", "--workspace", "--no-deps", "--locked", env={"RUSTDOCFLAGS": "-D warnings"})
        record["docs"] = True
    if "package" in checks:
        record["package"] = package(cargo, record)
    if "consumer" in checks:
        # Short: MSBuild's path limit also binds the hosted Windows work directory.
        record["consumer"] = consumers(work / "target/package", work / "c", cargo.env)
    if "linking" in checks:
        record["linking"] = linking(cargo, source, configuration)
    if "sanitizer" in checks:
        record["sanitizer"] = sanitized(work)
    # Packaging and prebuilt linking resolve Core again; validate every selection.
    found = selections(work, args.require_release)
    unique = {json.dumps(core_identity(item), sort_keys=True) for item in found}
    record["core"] = [json.loads(item) for item in sorted(unique)]
    record["core_selections"] = len(found)
    record["c_compiler"] = compilers(work)
    record.update({"seconds": time.monotonic() - started, "status": "success"})
    require(record["identity"] == identity() and record["definitions"] == definition_hashes(),
            "Checkout changed during validation")
    write(args.output, record)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    plan = commands.add_parser("matrix")
    plan.add_argument("--tier", choices=TIERS, required=True)
    runner = commands.add_parser("build")
    runner.add_argument("--tier", choices=TIERS, required=True)
    runner.add_argument("--configuration", required=True)
    runner.add_argument("--work", type=Path, required=True)
    runner.add_argument("--output", type=Path, required=True)
    runner.add_argument("--require-release", action="store_true")
    gate = commands.add_parser("gate")
    gate.add_argument("--tier", choices=TIERS, required=True)
    gate.add_argument("--evidence", type=Path, required=True)
    gate.add_argument("--output", type=Path, required=True)
    gate.add_argument("--require-release", action="store_true")
    args = parser.parse_args()
    if args.command == "matrix":
        print(json.dumps({"include": matrix(args.tier)}, separators=(",", ":")))
    elif args.command == "build":
        build(args)
    else:
        args.output.unlink(missing_ok=True)
        current = identity()
        require(current["hosted"] and not current["dirty"] and current["repository"] == REPOSITORY,
                "Hosted quality evidence requires the official clean checkout")
        records = [read(path) for path in sorted(args.evidence.rglob("record.json"))]
        report = validate_records(records, args.tier, current, definition_hashes(),
                                  json.loads(os.environ["CHECK_RESULTS"]), args.require_release)
        write(args.output, report)


if __name__ == "__main__":
    main()
