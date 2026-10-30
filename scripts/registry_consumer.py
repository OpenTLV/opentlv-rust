# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Build isolated consumers of the packaged crates, as a crates.io user would.

A local Cargo registry replaces crates.io, so each consumer resolves the exact
`.crate` archives through an index, checks their digests and unpacks them into
its own Cargo home. Nothing is published and no checkout path is involved.
"""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import tarfile
import tomllib

from ci_contract import CRATES, policy
from ci_support import digest, require, stream, write

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"
CONSUMER = "opentlv-registry-consumer"
# Uses the Rust API of each Cargo feature, so a feature lost in packaging cannot compile.
PROGRAM = """\
use opentlv::{{ByteOrder, FixedFormat, FixedFormatConfig, Format, Reader, Tag, Writer}};

fn main() -> Result<(), Box<dyn std::error::Error>> {{
    let config = FixedFormatConfig::new(1, 1, ByteOrder::Big);
    let format = FixedFormat::new(&config)?;
    let mut buf = [0u8; 64];
    let mut writer = Writer::with_fixed_format(&mut buf, &format);
    writer.write(&Tag::from_bytes(&[0x01]), b"registry")?;
    for element in Reader::with_fixed_format(writer.written(), &format) {{
        assert_eq!(element?.value(), b"registry");
    }}
    assert_eq!(Format::ALL.len(), {formats});
{features}    println!("linked Core {{}}", opentlv::version());
    Ok(())
}}
"""
FEATURE_CODE = {
    "lldp": '    assert_eq!("lldp".parse::<Format>().unwrap(), Format::Lldp);\n',
    "nfc": '    assert_eq!("nfc-type2".parse::<Format>().unwrap(), Format::NfcType2);\n',
    "document": "    assert_eq!(opentlv::Document::parse(&[4, 1, 42], Format::Ber, 2, 2)?.len(), 1);\n",
}


def find_archives(packages):
    """The single archive of each crate, as {crate: (version, path)}."""
    found = {}
    for crate in CRATES:
        matches = [(match[1], path) for path in Path(packages).glob("*.crate")
                   if (match := re.fullmatch(re.escape(crate) + r"-(\d.*)\.crate", path.name))]
        require(len(matches) == 1, f"Expected one {crate} archive in {packages}")
        found[crate] = matches[0]
    require(len({version for version, _ in found.values()}) == 1, "Crate archive versions differ")
    return found


def archive_file(path, crate, version, name):
    with tarfile.open(path) as archive:
        return archive.extractfile(f"{crate}-{version}/{name}").read()


def index_entry(crate, version, path):
    """The index record that crates.io would derive from the normalized manifest."""
    manifest = tomllib.loads(archive_file(path, crate, version, "Cargo.toml").decode("utf-8"))
    require(not ({"dev-dependencies", "build-dependencies", "target"} & set(manifest)),
            f"Packaged {crate} has dependencies this local registry does not model")
    dependencies = [{"name": name, "req": spec["version"], "features": spec.get("features", []),
                     "optional": spec.get("optional", False),
                     "default_features": spec.get("default-features", True),
                     "target": None, "kind": "normal"}
                    for name, spec in manifest.get("dependencies", {}).items()]
    return {"name": crate, "vers": version, "deps": dependencies, "cksum": digest(path),
            "features": manifest.get("features", {}), "yanked": False,
            "links": manifest["package"].get("links")}


def local_registry(archives, root):
    """Create the local registry that serves exactly these archives."""
    for crate, (version, path) in archives.items():
        shutil.copyfile(path, root / path.name)
        entry = root / "index" / crate[:2] / crate[2:4] / crate
        entry.parent.mkdir(parents=True, exist_ok=True)
        entry.write_text(json.dumps(index_entry(crate, version, path)) + "\n", encoding="utf-8")


def consumer_manifest(version, features):
    # `features` as in ci/matrix.json: None keeps the default features.
    requirement = json.dumps("=" + version)
    if features is not None:
        requirement = (f"{{ version = {requirement}, default-features = false, "
                       f"features = {json.dumps(features)} }}")
    return (f'[package]\nname = "{CONSUMER}"\nversion = "0.0.0"\nedition = "2021"\n'
            f"publish = false\n\n[dependencies]\nopentlv = {requirement}\n\n[workspace]\n")


def consumer_program(features):
    enabled = list(FEATURE_CODE) if features is None else features
    formats = 4 + sum(name in enabled for name in ("lldp", "nfc"))
    return PROGRAM.format(formats=formats,
                          features="".join(FEATURE_CODE[name] for name in enabled))


def check_lock(path, packages):
    """Require the lock a crates.io user gets: registry source and the archive digests."""
    locked = {item["name"]: item for item in tomllib.loads(
        Path(path).read_text(encoding="utf-8"))["package"]}
    require(set(locked) == {CONSUMER, *packages}, "Consumer resolved unexpected packages")
    for crate, expected in packages.items():
        require(locked[crate].get("source") == CRATES_IO, f"{crate} did not resolve from the registry")
        require(locked[crate].get("checksum") == expected, f"{crate} differs from its archive digest")


def selection(target):
    found = list(Path(target).rglob("core-selection.json"))
    require(len(found) == 1, "Expected one Core selection of the consumer")
    return json.loads(found[0].read_text(encoding="utf-8"))


def consumers(packages, work, env=None):
    """Build and run one fresh consumer per feature set; returns the per-set results."""
    archives = find_archives(packages)
    work = Path(work).resolve()
    require(not work.exists(), "Use a fresh consumer work directory")
    registry = work / "registry"
    home = work / "cargo-home"
    registry.mkdir(parents=True)
    home.mkdir()
    local_registry(archives, registry)
    (home / "config.toml").write_text(
        '[source.crates-io]\nreplace-with = "opentlv-packages"\n\n[source.opentlv-packages]\n'
        f"local-registry = {json.dumps(registry.as_posix())}\n", encoding="utf-8")
    version = archives["opentlv"][0]
    digests = {crate: digest(path) for crate, (_, path) in archives.items()}
    core = json.loads(archive_file(archives["opentlv-sys"][1], "opentlv-sys", version,
                                   "core/core-artifact.json"))["version"]
    results = {}
    for index, (name, features) in enumerate(policy()["features"].items()):
        directory = work / name
        # Short: a Core build on Windows allows at most 49 characters in this path.
        target = work / str(index)
        (directory / "src").mkdir(parents=True)
        (directory / "Cargo.toml").write_text(consumer_manifest(version, features), encoding="utf-8")
        (directory / "src/main.rs").write_text(consumer_program(features), encoding="utf-8")
        cargo_env = {**(os.environ if env is None else env), "CARGO_HOME": str(home),
                     "CARGO_TARGET_DIR": str(target)}
        stream("cargo", "generate-lockfile", "--offline", cwd=directory, env=cargo_env)
        check_lock(directory / "Cargo.lock", digests)
        output = stream("cargo", "run", "--locked", "--offline", cwd=directory, env=cargo_env)
        require(f"linked Core {core}\n" in output, "Consumer linked a different Core version")
        results[name] = {"packages": digests, "core": core,
                         "selection": selection(target)["mode"]}
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages", type=Path, required=True, help="Directory with both .crate files")
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.unlink(missing_ok=True)
    write(args.output, consumers(args.packages, args.work))


if __name__ == "__main__":
    main()
