# Packaging and versioning

The two crates of this repository are prepared as independent Cargo packages
([issue #4](https://github.com/OpenTLV/opentlv-rust/issues/4)). They are not
published to crates.io: registry publication is a separate step. The first
GitHub release is prepared as `v0.1.0` against Core `v0.8.0` (#33). The
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
| `version` | `0.1.0` |
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

- The first standalone Rust release is `0.1.0`. The inherited `0.7.1` was
  an unpublished migration baseline, not a previous Rust release. There were
  no Rust repository tags or GitHub releases when checked on 2026-10-27.
  The Rust version is independent of Core's `0.8.0` dependency version.
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
opentlv-sys = { version = "=0.1.0", path = "../opentlv-sys", default-features = false }
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
| 0.1.0 | 0.8.0 | `345c77acb29266036df5f27d04ce315bb61853cc` | v0.8.0 |

The packages do not contain Core. A build from an archive selects Core exactly
as a build from the repository does, as described in the
[README](../README.md#core-dependency): it authenticates the locked Core release
and selects its compatible static binary or builds its verified source.
`opentlv::version()` reports the
version of the Core library actually linked at run time, which can differ from
the pinned one when `OPENTLV_LIB_DIR` links a prebuilt library.

## Package contents

Each `.crate` archive contains exactly the crate's tracked files plus Cargo's
generated `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock` and
`.cargo_vcs_info.json`:

| Crate | Files |
| --- | --- |
| `opentlv-sys` | `README.md`, `LICENSE`, `build.rs`, `src/`, the native integration files in `core/` (lock, selection module, release verifier and CMake toolchain probe) and the tests with the C ABI probes in `tests/abi_probe/`, `tests/codec_abi_probe/` and `tests/compose_abi_probe/` |
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
URL, the registry identity in `scripts/ci.py` (description, keywords, categories,
edition, `rust-version`, `links`, build script, features and no `publish`
restriction), no dependency `path` and the dependency
`opentlv-sys = "=<version>"` for `opentlv` only.
[`tests/test_packaging.py`](../tests/test_packaging.py) checks
the manifests, the shared version, the unchanged features against the
migration baseline, the license copies and the Core compatibility table
without building.

The `consumer` check then builds a new project against those exact archives for
each feature set, through a local Cargo registry that replaces crates.io; see the
[quality gates](quality-gates.md#checks) and the
[registry build validation](validation/registry-builds.md). To run it on any two
archives, for example those of a release:

```sh
python -B scripts/registry_consumer.py --packages <directory with both .crate files> \
  --work build/consumers --output build/consumers.json
```

The target directories are `<work>/<n>`, so on Windows `--work` must fit the
[path limit](#remaining-cratesio-prerequisites).

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

## Licensing

Both crates declare the SPDX expression `MIT` in `license` and carry a
`LICENSE` copy equal to the root `LICENSE`, which names Marek Cingel as the
copyright holder. Packaged Rust, Python and CMake sources keep their SPDX
headers. The repository's `REUSE.toml` and `LICENSES/` are not packaged: in an
archive, the crate `LICENSE` covers the files without a header, which are the
README, the Core lock, the C ABI probe's `CMakeLists.txt` imported from Core
and Cargo's generated files. Neither crate has Cargo dependencies beyond `opentlv-sys`, and no
third-party code is vendored.

The archives contain no Core sources. `opentlv-sys` authenticates the pinned Core
release at build time, or links a library selected with `OPENTLV_LIB_DIR`, and
the resulting `tlv` library becomes part of the Rust binary. At the pinned
commit, Core's `LICENSE` is byte-identical to this repository's and its
library sources carry only `MIT` and `Copyright (c) 2026 Marek Cingel`
notices. A binary distribution therefore needs that one MIT notice; recheck
Core's licensing notices when changing the pinned commit, and add the license
of every new bundled or linked dependency here and to the crate archives.

## GitHub release

The manual `Rust Release` workflow accepts an unused `vMAJOR.MINOR.PATCH` tag
matching both Cargo manifests, `Cargo.lock` and `ci/release-notes.md`.
Changes stay under `Unreleased` during development; close the changelog under
the matching version without the tag's `v` prefix and the release date
before dispatching the release (for example, `## [0.1.0] - 2026-10-28`). The
versioned release notes are the publication text and are included in the
hashed CI definitions. No crates.io credentials or uploads are involved.

1. Review and merge the release preparation. Wait for successful Rust Main CI
   on that exact commit. Dispatch `Rust Release` on `main` with `tag: v0.1.0`.
2. The workflow rejects a foreign, dirty or non-main checkout, a changed main
   tip, an existing tag/release, wrong versions or missing Main evidence.
   It runs Full CI with `require_verified_core: true`: all seven configurations,
   source quality, CodeQL, API coverage, packaging and the aggregate gate.
3. Assembly revalidates every record against the current commit, run, attempt,
   Core lock and source hashes. It copies the exact two Linux stable packages
   whose digests the Full record contains. It does not rebuild them.
4. The workflow attests all five assets: `opentlv-sys-0.1.0.crate`,
   `opentlv-0.1.0.crate`, `release-manifest.json`, `release-notes.md` and
   `SHA256SUMS`. Publication rechecks evidence, checksums, attestations and
   unused tag status, then creates the tag and a draft release. Only after the
   uploaded asset names, sizes, SHA-256 digests, notes and tag commit match
   does it publish the draft and verify the public result.
5. Record the published URL and exact commit, Main and Release/Full run links
   in [issue #33](https://github.com/OpenTLV/opentlv-rust/issues/33) and
   [Core epic #718](https://github.com/OpenTLV/opentlv/issues/718). Close the
   tracking only after publication and asset verification. The workflow emits
   `rust-published-<attempt>` with the URL, source and asset identities.

If a failure occurs after creating a tag or draft, inspect that state before
retrying; the workflow deliberately refuses to overwrite either. Earlier
attempts' artifacts cannot satisfy a new attempt. A new main commit or a new
Main run requires a new release attempt with matching evidence.

For GitHub release archives before crates.io publication, extract both `.crate`
files into adjacent directories, then use a Cargo path dependency and patch
the exact registry dependency back to the extracted raw crate:

```toml
[dependencies]
opentlv = { path = "vendor/opentlv-0.1.0" }

[patch.crates-io]
opentlv-sys = { path = "vendor/opentlv-sys-0.1.0" }
```

The normal Core build requirements still apply. Rust's crate version is
`0.1.0`; `opentlv::version()` should report linked Core `0.8.0`.

### Local preparation evidence (2026-10-27)

Windows/MSVC with Rust 1.99.1 passed the exact test inventory in all five
feature configurations: default 299, none 276, LLDP 277, NFC 276 and
Document 298, including the C ABI probes and compile-fail lifetime tests.
rustfmt, Clippy with `-D warnings` for default and no-default features,
Rustdoc with `-D warnings`, all ten examples, 171 Query fixtures with 2,133
native executions, and `cargo package --workspace --locked --allow-dirty`
passed. Both archives were built from their packaged contents; the package
inventory, normalized manifests, licenses and crate digests were checked.
All 100 infrastructure tests, actionlint and source quality/REUSE passed.

Core API coverage remains 215/215 generic symbols with 82 exclusions;
the Core pin diff has no added or removed public symbols. The actual Core
v0.8.0 manifest, `SHA256SUMS` and source archive were verified against their
saved Sigstore bundles, enforcing the Core release signer and exact source
commit. Their checksums, source identity and recorded configuration passed
the Rust verifier. The 431 Core build and corpus files compared with the
clean pinned checkout matched after line-ending normalization.

Cargo used the development-only `OPENTLV_CORE_SOURCE` override to that exact
clean Core commit. The GitHub CLI was not authenticated for the default online
resolver; separate artifact verification does not turn those Cargo runs into
verified-mode or hosted evidence. Linux, macOS, MSRV, sanitizer, prebuilt-linking
Full jobs, release attestations and publication still require the hosted
release run on the merged commit. Local logs and the machine-readable result
are under `build/issue33-*`; they are not committed release evidence.

The subsequent [Full run 40559017343, attempt 1](https://github.com/OpenTLV/opentlv-rust/actions/runs/40559017343/attempts/1)
on `df4faeb87c923a042944977eea5e96cf2f0017b5` failed all seven build
configurations: a second Verified Source resolution changed the extracted
source path while reusing the first CMake cache. The follow-up gives each
resolution its own build directory. A regression reproduced the original
failure and passed after the fix; the 101-test infrastructure suite, Clippy,
rustfmt and source quality/REUSE passed locally. Two successive builds also
passed with the actual Core v0.8.0 source archive and an offline acquisition
fixture. These checks cover compilation and reruns, not online authentication
or hosted acceptance; a new Full run on the fix commit is still required.

## Remaining crates.io prerequisites

These are not part of this preparation and must be resolved before or at the
first publication. The [crates.io validation](validation/crates-io.md) records
the identity and archive checks of `v0.1.0`:

- **Version.** The `v0.1.0` archives must not be published, because their
  contents have the documentation and docs.rs problems below. The first
  crates.io version needs a new commit and a new version, such as `0.1.1`, so
  that each version has one archive.
- **Names and owners.** Neither `opentlv` nor `opentlv-sys` existed on
  crates.io on 2026-10-30, and crates.io does not reserve names. The first
  publication claims both names; crate owners, such as an OpenTLV GitHub team,
  are set then.
- **Order.** `opentlv-sys` must be published before `opentlv`, whose exact
  requirement must resolve from the registry.
- **Core availability.** A build needs CMake, a C99 compiler, Python 3.11,
  an authenticated GitHub CLI and network access to GitHub, even when
  `OPENTLV_LIB_DIR` links a prebuilt library. The locked
  Core v0.8.0 release assets and attestations must remain available. A
  compatible Verified Binary avoids compiling Core; otherwise the verified
  source is built locally. The
  [registry build validation](validation/registry-builds.md#consumer-requirements)
  shows how each requirement fails, and that a clean machine cannot build the
  crates; this needs a decision before publishing.
- **Windows paths.** With the Visual Studio generator, the Cargo target directory
  can have at most 49 characters, or 47 with `--release`. A short
  `CARGO_TARGET_DIR` works around it; a deep project otherwise fails with an
  MSBuild error.
- **Targets.** `build.rs` gives CMake no target, so cross compilation is not
  supported.
- **docs.rs.** docs.rs builds without network access, so the default build
  script cannot fetch Core there; an offline build of the released archive
  fails. Rendering the documentation on docs.rs needs a documentation-only
  build path in `build.rs`.
- **Documentation.** The crate-level Rustdoc, the crate READMEs and the
  [user guide](guides/rust.md) describe a path dependency on an opentlv-rust
  checkout. They need the crates.io dependency once the crates are published.
- **Registry process.** A crates.io token or trusted publishing configuration
  and a registry-specific publish gate remain to be defined. The GitHub
  release workflow publishes source packages only to GitHub Releases.
