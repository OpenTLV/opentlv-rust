# Packaging and versioning

The two crates of this repository are prepared as independent Cargo packages
([issue #4](https://github.com/OpenTLV/opentlv-rust/issues/4)). They are not
published: crates.io publication is a separate step after the migration. The
[packaging validation](validation/packaging.md) records the local results.

## Package identities

| Crate | Role | Native metadata | Categories |
| --- | --- | --- | --- |
| `opentlv-sys` | Raw C FFI declarations, Core selection and native linking | `links = "tlv"`, `build.rs` | `external-ffi-bindings` |
| `opentlv` | Safe Rust API | `build.rs`, which only forwards the selected Core tree to the tests | `parser-implementations`, `encoding`, `api-bindings` |

The crate names, descriptions, `links` value, Cargo features and their defaults
are unchanged from the [migration baseline](migration.md#crates-and-versions);
the `opentlv` build script was added by the
[standalone build](validation/standalone-build.md).

Both crates inherit this metadata from `[workspace.package]` in
the root `Cargo.toml`:

| Field | Value |
| --- | --- |
| `version` | `0.7.1` |
| `edition` | `2021` |
| `rust-version` | `1.82` |
| `license` | `MIT` |
| `repository` | `https://github.com/OpenTLV/opentlv-rust` |
| `keywords` | `tlv`, `ber`, `asn1`, `emv`, `parser` |

Each crate has its own `README.md`, which Cargo uses as the package readme,
and a copy of the repository `LICENSE`, so that every archive carries the MIT
license text. Both copies must equal the root `LICENSE`.

## Versioning

The Rust bindings have their own version, independent of OpenTLV Core:

- The current workspace version `0.7.1` is the migration baseline. It equals
  Core's version only because the crates were imported from Core `0.7.1`; it is
  not synchronized with Core and not derived from Git tags in either
  repository.
- Both crates always have the same version, the single
  `[workspace.package]` `version`, and are released together. `Cargo.lock`
  records it for both.
- Versions follow [SemVer](https://semver.org/) as Cargo applies it. Before
  1.0, a breaking change of either crate's public API, Cargo features, native
  linking contract or minimum supported Rust version increments the minor
  version; any other change increments the patch version.
- Changing the Core lock in `core/core-artifact.json` always produces a new
  version, because the lock determines the Core that each build of a
  published version uses. It increments the minor version when the Core
  change breaks one of the contracts above.

### Version relationship

`opentlv` depends on exactly the `opentlv-sys` version it is released with:

```toml
opentlv-sys = { version = "=0.7.1", path = "../opentlv-sys", default-features = false }
```

The exact requirement is deliberate. `links = "tlv"` permits only one
`opentlv-sys` in a dependency graph, the CI tests the two crates only as a
pair, and the `opentlv-sys` version identifies the Core dependency, so a
caret requirement would allow untested combinations with a different Core.
Inside the workspace, Cargo uses the `path`; in the package, Cargo removes the
`path` and keeps the version requirement.

## Core compatibility

Each `opentlv-sys` version pins exactly one Core dependency, recorded in
[`core/core-artifact.json`](../opentlv-sys/core/core-artifact.json),
which is part of the package:

| Crates | Core version | Core commit | Core release |
| --- | --- | --- | --- |
| 0.7.1 | 0.7.1 | `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` | none locked |

The packages do not contain Core. A build from an archive selects Core exactly
as a build from the repository does, as described in the
[README](../README.md#core-dependency): while no release is locked it clones
the pinned commit from `OpenTLV/opentlv`. `opentlv::version()` reports the
version of the Core library actually linked at run time, which can differ from
the pinned one when `OPENTLV_LIB_DIR` links a prebuilt library.

## Package contents

Each `.crate` archive contains exactly the crate's tracked files plus Cargo's
generated `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock` and
`.cargo_vcs_info.json`:

| Crate | Files |
| --- | --- |
| `opentlv-sys` | `README.md`, `LICENSE`, `build.rs`, `src/`, the native integration files in `core/` (lock, selection module, release verifier and CMake toolchain probe) and the tests with the C ABI probes in `tests/abi_probe/` and `tests/compose_abi_probe/` |
| `opentlv` | `README.md`, `LICENSE`, `build.rs`, `src/`, `tests/` and the eleven examples |

The build scripts and tests read only files inside their own package, the
Cargo `OUT_DIR`, the selected Core tree and their own temporary directories.
The only local path in a manifest
is the declared `opentlv-sys` workspace dependency, which carries a version.

## Validation

The `package` check of [`scripts/ci.py`](../scripts/ci.py), part of the
stable [CI configurations](quality-gates.md), runs
`cargo package --workspace --locked`, which builds both crates from their
archives. It requires the exact file lists above, a `LICENSE` equal to the
repository license and, in each archive's normalized `Cargo.toml`, the crate
name, the workspace version, `MIT`, the `README.md` readme, this repository
URL, no dependency `path` and the dependency `opentlv-sys = "=<version>"` for
`opentlv` only. [`tests/test_packaging.py`](../tests/test_packaging.py) checks
the manifests, the shared version, the unchanged features against the
migration baseline, the license copies and the Core compatibility table
without building.

To inspect the packages locally, from the repository root:

```sh
cargo metadata --format-version 1 --no-deps
cargo package -p opentlv-sys --list
cargo package -p opentlv --list
cargo package --workspace --allow-dirty
```

`cargo package -p opentlv` alone fails, even with `--no-verify`, while
`opentlv-sys` is not on crates.io: Cargo resolves an unpublished dependency
from a local archive only for crates packaged in the same invocation. Use
`--workspace`, as CI does.

## Remaining crates.io prerequisites

These are not part of this preparation and must be resolved before or at the
first publication:

- **Names and owners.** Neither `opentlv` nor `opentlv-sys` existed on
  crates.io on 2026-10-22. The first publication claims both names; crate
  owners, such as an OpenTLV GitHub team, are set then.
- **Order.** `opentlv-sys` must be published before `opentlv`, whose exact
  requirement must resolve from the registry.
- **Core availability.** A build from crates.io needs Git, CMake, a C99
  compiler and network access to GitHub, and the pinned commit must stay
  reachable in `OpenTLV/opentlv`. Locking an attested Core release with
  Verified Binaries before publication reduces this, but Verified Binary and
  Verified Source selection additionally need Python 3.11 and an
  authenticated GitHub CLI.
- **docs.rs.** docs.rs builds without network access, so the default build
  script cannot fetch Core there. Rendering the documentation on docs.rs needs
  a documentation-only build path in `build.rs`.
- **Documentation.** The crate-level Rustdoc, the crate READMEs and the
  [user guide](guides/rust.md) describe a path dependency on an opentlv-rust
  checkout. They need the crates.io dependency once the crates are published.
- **Release process.** Release tags, the release workflow and the crates.io
  token or trusted publishing configuration are defined with publication. The
  CI never publishes.
