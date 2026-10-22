# Rust migration record

This document records the historical baseline and the
[#1](https://github.com/OpenTLV/opentlv-rust/issues/1) import. The standalone
build of [#2](https://github.com/OpenTLV/opentlv-rust/issues/2) later changed
`opentlv-sys/build.rs`, `opentlv-sys/tests/fixed_abi.rs`,
`opentlv/tests/corpus.rs` and `opentlv/Cargo.toml` so that they reach Core
through a pinned dependency instead of `../../..`. It preserved the crates,
features, linking modes, API, tests, examples and the frozen inventory. The
[layout normalization](validation/layout.md) of
[#5](https://github.com/OpenTLV/opentlv-rust/issues/5) then moved the
workspace from `bindings/rust/` to the repository root. The inventory and this
record keep the original `bindings/rust/` paths, which are Core's source paths
and those of the import commit. The documentation integration of
[#6](https://github.com/OpenTLV/opentlv-rust/issues/6) turned the imported
guides into the current standalone [user guide](guides/rust.md) and
[development guide](development/rust.md); the import commit and the pinned
Core links below keep the originals. Use the
[README](../README.md#building) to build and the
[standalone build validation](validation/standalone-build.md) for its results.
To verify the original import, rather than today's working tree, against
this record:

```sh
python -B scripts/verify_migration.py --core ../opentlv \
  --import-ref 95d6299099af7a7d774ed89ee223f2cda5035aa9
```

This record fixes the pre-migration state of the Rust bindings in Core for
[OpenTLV/opentlv#671](https://github.com/OpenTLV/opentlv/issues/671), part of the
[Rust binding migration epic](https://github.com/OpenTLV/opentlv/issues/670).
It is the reference for every later migration step. The baseline sections
below describe the state before any Rust source was imported and stay
unchanged. The source transfer for
[opentlv-rust#1](https://github.com/OpenTLV/opentlv-rust/issues/1) is recorded
in [Source import](#source-import) and compared against this baseline.

## Provenance

| Field | Value |
| --- | --- |
| Source repository | [OpenTLV/opentlv](https://github.com/OpenTLV/opentlv) |
| Source commit | `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` (`git describe`: `0.7.1-31-g476f1fbe`) |
| Source directory | [`bindings/rust/`](https://github.com/OpenTLV/opentlv/tree/476f1fbecde6b056419e43c9a6cea4d1ca8df1cf/bindings/rust) |
| Linked Core version | `0.7.1` |
| Destination | [OpenTLV/opentlv-rust](https://github.com/OpenTLV/opentlv-rust), initialized at `fa0cf127e5814964045b34fdbd3544aa365319d3` |

The source commit is the clean Core `main` checkout used for the baseline run.
It is newer than the Core revision pinned in the workspace integration
manifest; this record neither changes that pin nor selects a standalone Core
dependency.

### History

`bindings/rust` has 83 commits up to the source commit, from
`babcdc386a65d5bb20e4a1cc7ed170653f174bce` (#154, the initial experimental
crates) to `b9b51b1fa6f64f0af6363b06e411774aed39284b` (#622). The history stays
in Core and is retrievable with:

```sh
git -C ../opentlv log 476f1fbecde6b056419e43c9a6cea4d1ca8df1cf -- bindings/rust
```

## Crates and versions

The Cargo workspace (`resolver = "2"`) has two members with workspace-inherited
package metadata:

| Field | Value |
| --- | --- |
| Members | `opentlv-sys`, `opentlv` |
| Version (both crates, `Cargo.lock` v3) | `0.7.1` |
| Edition | `2021` |
| `rust-version` | `1.70` |
| License | `MIT` |
| Repository metadata | `https://github.com/marekcingel/OpenTLV` |

| Crate | Role | Build metadata |
| --- | --- | --- |
| `opentlv-sys` | Raw C FFI declarations and native linking | `links = "tlv"`, `build.rs`, no dependencies |
| `opentlv` | Safe Rust API | Path dependency on `opentlv-sys` with `default-features = false` |

The lockfile contains only these two packages; there are no third-party
dependencies.

## Cargo features and defaults

| Crate | `default` | Features |
| --- | --- | --- |
| `opentlv-sys` | `lldp`, `document`, `nfc` | `lldp = []`, `nfc = []`, `document = []` |
| `opentlv` | `lldp`, `document`, `nfc` | Each forwards to the same `opentlv-sys` feature |

In source builds, the three features set `OPENTLV_LLDP`, `OPENTLV_NFC` and
`OPENTLV_DOCUMENT`. The build script always enables `OPENTLV_READER`,
`OPENTLV_WRITER`, `OPENTLV_QUERY`, `OPENTLV_SCHEMA` and `OPENTLV_CODEC`, and passes
no `OPENTLV_FORMAT_*` or `OPENTLV_EMV` options. In Rust code, `document` gates
`Document`, `DocumentBuilder`, `DocumentError`, `Node`, `NodeMut`, the Query edit
API and document-dependent Program paths; `lldp` and `nfc` gate `Format::Lldp`
and `Format::NfcType2`.

## Native linking modes

| Mode | Selection | Behavior |
| --- | --- | --- |
| Source build (default) | `OPENTLV_LIB_DIR` unset | `build.rs` configures `../../..` (the Core root) with CMake into `OUT_DIR`, always as a Release static, position-independent library without C++, tests, examples or warnings-as-errors, and links `static=tlv` |
| Prebuilt static | `OPENTLV_LIB_DIR=<dir>`, `OPENTLV_LINK_KIND=static` | Links `static=tlv` from `<dir>` |
| Prebuilt shared | `OPENTLV_LIB_DIR=<dir>`, `OPENTLV_LINK_KIND` unset or `dylib` | Links `dylib=tlv`; the loader must find the library at run time |

Prebuilt libraries must be built with components matching the enabled Cargo
features. Any other `OPENTLV_LINK_KIND` value fails the build script. The
build script reruns when `OPENTLV_LIB_DIR`, `OPENTLV_LINK_KIND`, or Core's
`CMakeLists.txt`, `cmake/` or `tlv/` change.

## Supported platforms and toolchains

The documented requirements are Rust 1.70 or newer, CMake 3.16 or newer, a C99
compiler and, on Windows, the MSVC Rust toolchain with Visual Studio.

The [original workflow](https://github.com/OpenTLV/opentlv/blob/476f1fbecde6b056419e43c9a6cea4d1ca8df1cf/.github/workflows/rust.yml)
uses the current stable Rust toolchain. It runs on main pushes, version tags
and pull requests touching `bindings/rust`, `tlv`, `tests/fuzz`, `tests/query`,
`cmake`, any `CMakeLists.txt` or the workflow:

- `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` on Ubuntu 24.04;
- `cargo build --workspace --all-targets`, `cargo test --workspace`, the
  `quick_start` example and the shared Query corpus through `query_conformance`
  with `RUSTFLAGS=-D warnings` on Ubuntu 24.04, adding Windows and macOS on
  release tags.

It uses only default features and the source-build linking mode. It does not
check the declared `rust-version`, reduced features, prebuilt linking or Rustdoc.
This describes the preserved workflow, not evidence that every platform passed
at the source commit.

## Source inventory

The [inventory](migration/source-inventory.json) records all 69 files to transfer
with their Git blob ID, SHA-256 hash and size, plus the recorded workspace,
crate and feature metadata:

| Area | Files |
| --- | ---: |
| Workspace `Cargo.toml` and `Cargo.lock` | 2 |
| `opentlv-sys`: `Cargo.toml`, `build.rs` | 2 |
| `opentlv-sys/src` | 6 |
| `opentlv-sys/tests/fixed_abi.rs` | 1 |
| `opentlv-sys/tests/abi_probe` (C ABI probe) | 3 |
| `opentlv/Cargo.toml` | 1 |
| `opentlv/src` | 25 |
| `opentlv/tests` | 19 |
| `opentlv/examples` | 7 |
| **Complete `bindings/rust` tree** | **66** |
| `.github/workflows/rust.yml` | 1 |
| `docs/guides/rust.md`, `docs/development/rust.md` | 2 |

`opentlv/src/diagnostic_kind.rs` and `opentlv/tests/diagnostic_vocabulary.rs` are
generated by Core's `scripts/generate_diagnostic_vocabulary.py`. They are
inventoried as transferred files, but the generator stays Core-owned.

### Core-owned dependencies

These stay in Core. The Rust build and tests currently reach them through
relative paths from `bindings/rust`:

| Core path | Used by |
| --- | --- |
| `CMakeLists.txt`, `cmake/`, `tlv/` | `opentlv-sys/build.rs` source build (`../../..`) |
| `tlv/include/`, `tlv/resources/config.h.in` | `opentlv-sys/tests/fixed_abi.rs` C ABI probe (`OPENTLV_SOURCE_DIR`) |
| Seed folders under `tests/fuzz/` (`corpus/`, `builtins/*/corpus/`) | `opentlv/tests/corpus.rs` seed replay (`../../../tests/fuzz`) |
| `tests/query/` (`run.py`, `corpus.json`, `reference.py`, helpers) and `tests/property/` | Shared Query corpus driving `opentlv/examples/query_conformance.rs` |
| `scripts/generate_diagnostic_vocabulary.py` | Generates the two diagnostic vocabulary files |

### Core integration to update during cleanup

These Core files reference the Rust binding and need updating or removal once
the standalone repository passes its quality gates:

| Core path | Reference |
| --- | --- |
| `.github/workflows/rust.yml` | Rust workflow |
| `.gitignore` | `/bindings/rust/target/` |
| `tests/CMakeLists.txt` | `diagnostic-vocabulary-generated` checks the in-tree Rust output |
| `scripts/generate_diagnostic_vocabulary.py` | Writes `bindings/rust/opentlv/...` |
| `scripts/check_doc_examples.py`, `scripts/check_doc_inventory.py` | Rust example roots |
| `examples/tlv/CMakeLists.txt`, `examples/tlv++/CMakeLists.txt`, `bindings/wasm/CMakeLists.txt` | Comments naming the Rust examples |
| `examples/tlv/src/README.md`, `examples/tlv++/src/README.md` | Links to the Rust examples |
| `docs/concepts/bindings.md`, `docs/getting-started/README.md`, `docs/guides/examples.md`, `docs/guides/schemas.md` | Links to `bindings/rust` sources, tests or examples |
| `docs/formats/lldp/README.md`, `docs/guides/document.md`, `docs/guides/queries.md` | Rust code samples in shared documents |
| `README.md`, `CONTRIBUTING.md` and other shared documents | Mention Rust as an in-tree binding |

## Test ownership

| Area | Ownership |
| --- | --- |
| `opentlv` unit, integration and doc tests | Rust-owned; transfer unchanged |
| `opentlv-sys` unit tests and C ABI probe | Rust-owned; the probe compiles the matching Core headers |
| `opentlv/tests/corpus.rs` | Rust-owned test over Core-owned fuzz seeds |
| `query_conformance` example | Rust-owned adapter for the Core-owned shared Query corpus |
| Diagnostic vocabulary generation and its consistency check | Core-owned |
| C library, fuzz harnesses and Query reference semantics | Core-owned; not transferred |

No missing Core behavioral coverage was found while recording this baseline.

## Recorded validation

Local validation on 2026-10-22 used Windows x64, Rust 1.98.1 stable
(`x86_64-pc-windows-msvc`), CMake 3.31.7 with Visual Studio 17 2022
(MSVC 19.44.35222.0), Python 3.11.4, and the clean Core checkout at the source
commit. Each configuration used its own `CARGO_TARGET_DIR` under the ignored
`build/671/` directory of this repository, so no existing build output was
reused. Every Cargo command except Rustdoc ran with `RUSTFLAGS=-D warnings`.
The Core worktree was still clean after the run.

| Check | Default features (source build) | `--no-default-features` (source build) | Prebuilt static | Prebuilt shared |
| --- | --- | --- | --- | --- |
| `cargo fmt --all --check` | Passed | | | |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | | | |
| `cargo build --workspace --all-targets` | Passed | Passed | | |
| `cargo test --workspace` | 202 passed | 186 passed | 202 passed | Link failure |
| Examples | 6 passed | | `quick_start` passed | Link failure |
| Shared Query corpus | 171 fixtures passed | | | |
| `cargo doc --workspace --no-deps` | Passed, 3 warnings | | | |

The 202 default-feature tests are 26 `opentlv` unit tests, 159 integration
tests across 19 files, 2 `opentlv-sys` unit tests, the C ABI layout test and
14 doc tests. No test failed or was ignored. Without default features, the
document, LLDP and document-dependent Program, Query Schema and doc tests are
compiled out (16 fewer); nothing else changes.

The examples `parse`, `quick_start`, `reader`, `validate`, `write` and `writer`
completed with exit code 0; their output is in the compact log.
`query_conformance` ran through Core's `tests/query/run.py --property-cases 0`:
171 fixtures, 2133 native executions and no unavailable-provider fixtures.

The prebuilt modes linked Core libraries built separately by CMake with
the same options as `build.rs` and all three feature components enabled.

### Known baseline failures

- **Prebuilt shared library on Windows MSVC.** Linking the tests and the
  `quick_start` example fails with `LNK2019` for the data symbols
  `tlv_format_ber`, `tlv_format_emv`, `tlv_format_cer`, `tlv_format_der`,
  `tlv_format_lldp` and `tlv_format_nfc_type2`. `opentlv-sys` declares them as
  `extern` statics without a `#[link(name = "tlv", kind = "dylib")]` attribute,
  so rustc does not import them through `__imp_` symbols from the DLL import
  library. Functions link correctly. This is pre-existing: the original
  workflow never exercises prebuilt linking. Linux and macOS were not tested.
  The migration preserves this behavior. Any fix is separate work and must
  not be mixed into the source transfer.
- **Rustdoc intra-doc links.** Three links in `opentlv/src/codec.rs` (lines 548,
  851, 861) of the form ``[`CodecFailure::from(crate::Error::...)`]`` do not
  resolve. Rustdoc still succeeds; the original workflow does not build Rustdoc.

### Observations

- An earlier attempt with a deeper target directory failed every source build
  before Rust compilation. MSBuild could not create a 245-character CMake
  try-compile `.tlog` path; Windows long paths are disabled on this machine.
  Core's in-tree `bindings/rust/target` stays below the limit. The run was
  repeated from scratch with a shorter build root. A standalone layout that
  nests Core builds more deeply can hit this limit on Windows.
- The C ABI probe test creates its CMake build directories under the system
  temporary directory, not Cargo's target directory.
- Rust links with the newest installed MSVC linker (14.50.35717, Visual
  Studio 18), while the source-built Core library used the CMake-selected
  Visual Studio 17 2022 toolset. This mix linked and passed.

The [machine-readable report](migration/validation.json) records exact commands,
working directories, environment overrides, exit codes, durations, per-target
test counts and log hashes. The [compact log](migration/windows-baseline.log)
keeps test summaries, example and Query output, Rustdoc warnings and the link
errors. Full logs remain in the ignored local `build/671/` directory.

The declared `rust-version` 1.70, Linux, macOS, 32-bit targets, other feature
subsets and hosted CI were not tested. These results establish the baseline in
the original Core build context, not standalone build readiness.

## Recorded metadata discrepancies

These are recorded as they are and are not changed by the migration baseline:

- Both crates are versioned `0.7.1`, the same as Core, although the
  development guide describes the crate version as independent and gives
  `0.1.0` as an example.
- `repository` metadata and several guide links point to
  `https://github.com/marekcingel/OpenTLV`.
- The Rust guides document the `lldp` and `document` features but not `nfc`.

## Verifying the baseline

With a sibling Core checkout that contains the source commit:

```sh
python -B scripts/verify_baseline.py --core ../opentlv
```

The verifier confirms that the commit is retrievable and that the inventory
matches the complete `bindings/rust` tree plus the explicit workflow and guides.
It checks each Git blob ID, SHA-256 hash and size, the recorded history range,
workspace metadata, locked crate versions, crate metadata and Cargo features.
It does not build or run anything. Checks on temporary copies confirmed that it
rejects a changed hash, a missing inventory entry, a changed feature default
and a changed history count. Later migration steps should be compared
against this frozen inventory rather than updating it to hide changes.

## Reproducing the runtime baseline

From a clean Core checkout at the source commit, run each command in
`bindings/rust` with `CARGO_TARGET_DIR` set to a fresh, short directory outside
the Core tree and `RUSTFLAGS=-D warnings`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets
cargo test --workspace
cargo run -q -p opentlv --example quick_start   # likewise parse, reader, validate, write, writer
python -B ../../tests/query/run.py --native "$CARGO_TARGET_DIR/debug/examples/query_conformance" --property-cases 0
cargo doc --workspace --no-deps                 # without RUSTFLAGS
cargo test --workspace --no-default-features    # separate target directory
```

For the prebuilt modes, build Core's `tlv` target with CMake using the options
listed in `build.rs`, all three feature components on, and
`OPENTLV_BUILD_SHARED_LIBS` set to `OFF` or `ON`. Then run `cargo test --workspace`
with `OPENTLV_LIB_DIR` pointing at the library directory and
`OPENTLV_LINK_KIND` set to `static` or `dylib`. For `dylib`, also add that directory to
the loader path. [validation.json](migration/validation.json) has the exact
commands.

## Source import

[opentlv-rust#1](https://github.com/OpenTLV/opentlv-rust/issues/1) imports all
69 inventoried files from the source commit. The import copies committed Git
blobs and preserves their bytes and original copyright and SPDX notices. It
does not transplant Git history; the source commit and the history command
above keep the original history retrievable.

| Source | Destination | Files |
| --- | --- | ---: |
| `bindings/rust/` (complete tree) | Same path | 66 |
| `docs/guides/rust.md`, `docs/development/rust.md` | Same paths | 2 |
| `.github/workflows/rust.yml` | `docs/migration/original-rust.yml` | 1 |

### Preserved contracts

- Both crates, their names, versions (`0.7.1`), metadata and the
  `Cargo.lock` are unchanged.
- Public Rust API, FFI declarations, ownership and lifetime behavior are
  unchanged.
- Cargo features `lldp`, `nfc` and `document`, and their defaults, are
  unchanged.
- `build.rs` and all three native linking modes are unchanged, including the
  known prebuilt shared-library failure on Windows MSVC.
- Unit, integration, doc and ABI probe tests and all seven examples are copied
  without changes.
- The generated diagnostic vocabulary files are copied as they are; their
  generator stays in Core.
- No Core file is changed or removed.

### Migration-specific differences

These are the only changes outside the imported files:

- The workspace stays at `bindings/rust/`. Keeping the original layout keeps
  every Core-relative path in `build.rs`, `fixed_abi.rs` and `corpus.rs`
  unchanged. Layout normalization is
  [opentlv-rust#5](https://github.com/OpenTLV/opentlv-rust/issues/5).
- The original workflow is kept as `docs/migration/original-rust.yml`. It is
  historical evidence, not an active workflow; standalone CI is
  [opentlv-rust#3](https://github.com/OpenTLV/opentlv-rust/issues/3).
- `.gitignore` ignores `/bindings/rust/target/`, as Core does for the same
  path.
- The README, this record, the changelog and `scripts/verify_migration.py` are
  new.

The imported workspace still resolves Core through `../../..` from its crates.
In this repository that is the repository root, which has no Core sources, so
the source build, the C ABI probe and the corpus seed replay do not work here
yet. Do not run Cargo in `bindings/rust` before the standalone build and
verified Core dependency of
[opentlv-rust#2](https://github.com/OpenTLV/opentlv-rust/issues/2) exist.

The preserved guides describe the original Core checkout. Their commands and
links to other Core documents keep that context and are not standalone
instructions. For working cross-links, use the pinned
[user guide](https://github.com/OpenTLV/opentlv/blob/476f1fbecde6b056419e43c9a6cea4d1ca8df1cf/docs/guides/rust.md)
and
[developer guide](https://github.com/OpenTLV/opentlv/blob/476f1fbecde6b056419e43c9a6cea4d1ca8df1cf/docs/development/rust.md).
Documentation integration is
[opentlv-rust#6](https://github.com/OpenTLV/opentlv-rust/issues/6), which
has since replaced the imported guides with standalone instructions.

### Verifying the import

With a sibling Core checkout that contains the source commit:

```sh
python -B scripts/verify_migration.py --core ../opentlv
```

The verifier first runs the baseline verification. It then compares every
imported file byte-for-byte with its original Git blob and with the SHA-256
hash and size in the frozen inventory, and rejects missing, extra or modified
files in `bindings/rust`. Only Cargo's `bindings/rust/target/` is ignored. It
does not build or run anything. Checks on temporary copies confirmed that it
rejects a modified source file, an extra file in `bindings/rust` and a missing
guide, and accepts Cargo build output.

### Imported-source validation

The runtime comparison repeated the complete baseline matrix on 2026-10-22,
on the same machine and with the same toolchain as the baseline. Core at the
source commit was exported with `git archive` (with `core.autocrlf=false`)
into the ignored `build/1/c/` directory. The 69 imported files were copied
over their source paths, and an `opentlv-source.txt` with `version=0.7.1` and
the source commit was added. That file is Core's archive version metadata; it
keeps the linked library at `0.7.1` instead of reading the enclosing
repository's Git tags. The overlay changed no file, so the build consumed
exactly the imported bytes in the original Core build context.

| Check | Default features (source build) | `--no-default-features` (source build) | Prebuilt static | Prebuilt shared |
| --- | --- | --- | --- | --- |
| `cargo fmt --all --check` | Passed | | | |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | | | |
| `cargo build --workspace --all-targets` | Passed | Passed | | |
| `cargo test --workspace` | 202 passed | 186 passed | 202 passed | Link failure |
| Examples | 6 passed | | `quick_start` passed | Link failure |
| Shared Query corpus | 171 fixtures passed | | | |
| `cargo doc --workspace --no-deps` | Passed, 3 warnings | | | |

Every step has the same exit code as the baseline. Per-target test counts,
example output, the Query corpus summary (2133 native executions) and the three
Rustdoc warnings are identical. The prebuilt shared-library run fails with the
same unresolved data symbols as the baseline: 6 in the tests and 12 in
`quick_start`. `Cargo.lock` was unchanged after the run and the Core checkout
stayed clean.

On disk, the imported files are LF on Windows (`eol=lf` in `.gitattributes`),
while the baseline Core checkout uses `core.autocrlf=true`. No source includes
files with `include_str!` or `include_bytes!`, and `cargo fmt` passed in both
runs.

The [machine-readable report](migration/import-validation.json) records exact
commands, working directories, environment overrides, exit codes, durations,
per-target test counts and log hashes. The
[compact log](migration/windows-imported.log) has the same sections as the
baseline log. Full logs remain in the ignored local `build/1/` directory.

These results compare the imported sources with the baseline in the original
Core build context. They are not standalone build, hosted CI, MSRV, Linux or
macOS validation.

## Follow-up boundaries

The baseline does not import, change or remove any Rust source, test,
example, feature, linking mode or Core file. The source import adds the files
listed in [Source import](#source-import) without changing them. The
standalone Cargo build and Core dependency, CI, packaging, layout
normalization, documentation integration and Core cleanup are separate
migration issues.

## Migration completion

[Issue #7](https://github.com/OpenTLV/opentlv-rust/issues/7) validated the
completed migration across `opentlv`, `opentlv-rust`, `opentlv-workspace` and
`opentlv-docs` on 2026-10-22, using the merged pull requests and hosted runs
below. The sections above remain the frozen baseline and import record.

### Identities

| Item | Value |
| --- | --- |
| Original Core source commit | `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` (`0.7.1-31-g476f1fbe`) |
| Import commit | `95d6299099af7a7d774ed89ee223f2cda5035aa9` ([#1](https://github.com/OpenTLV/opentlv-rust/issues/1)) |
| Final validated Rust commit | `d4ba3ff27ff2a0a5934bdc9cb4bf54019c2274bc` (`main` after [#14](https://github.com/OpenTLV/opentlv-rust/issues/14)) |
| Crates | `opentlv-sys` and `opentlv`, both `0.7.1`, unpublished |
| Core dependency lock | `OpenTLV/opentlv` `0.7.1` at `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf`, no release tag; every build records a clean `pinned-git` selection with `verified: false` |
| Core cleanup | [OpenTLV/opentlv#679](https://github.com/OpenTLV/opentlv/issues/679), merged as `7596cdfeef4775609588b7424949aa917fbf4ecb` |

### Implementation

`python -B scripts/verify_migration.py --core ../opentlv --import-ref
95d6299099af7a7d774ed89ee223f2cda5035aa9` passed: the 69 imported files match
their Core blobs, and the 83-commit history, workspace metadata, crate
versions and Cargo features match the inventory. A file-by-file comparison of
`d4ba3ff` with Core's `bindings/rust` at the source commit found only the
recorded changes:

| Change | Recorded in |
| --- | --- |
| Core reached through the selected Core tree instead of `../../..` in `build.rs`, `fixed_abi.rs` and `corpus.rs` | [#2](https://github.com/OpenTLV/opentlv-rust/issues/2) |
| `rust-version` 1.70 → 1.82, `is_none_or` in `corpus.rs`, `opentlv-sys` requirement with a version, three Rustdoc links in `codec.rs` | [CI validation](validation/ci.md#scope) |
| `repository`, `keywords`, `categories`, exact `=0.7.1` requirement, crate README and LICENSE files | [Packaging validation](validation/packaging.md#scope) |
| Workspace at the repository root; example and crate documentation comments name the repository root and an `opentlv-rust` checkout | [Layout validation](validation/layout.md) |
| `FixedFormatConfig` Rustdoc link to `OpenTLV/opentlv` | [#6](https://github.com/OpenTLV/opentlv-rust/issues/6) |
| Added Core selection module, lock, verifier, probe and `core_dependency.rs` tests | [#2](https://github.com/OpenTLV/opentlv-rust/issues/2) |

The public API, FFI declarations, the `lldp`, `nfc` and `document` features
and their defaults, native linking modes, `Cargo.lock`, the C ABI probe, the
imported tests and all seven examples are unchanged. The default suite grows
from 202 to 217 tests only by the 15 `core_dependency` tests; without default
features it grows from 186 to 201.

### Quality gates

| Gate | Run | Commit | Result |
| --- | --- | --- | --- |
| PR CI | [39796625185](https://github.com/OpenTLV/opentlv-rust/actions/runs/39796625185) | `c9e3a90` | Passed |
| Main CI | [39804845893](https://github.com/OpenTLV/opentlv-rust/actions/runs/39804845893) | `d4ba3ff` | Passed |
| Full CI | [39810822148](https://github.com/OpenTLV/opentlv-rust/actions/runs/39810822148) | `d4ba3ff` | Passed: all seven configurations, packaging, static and shared linking, AddressSanitizer, source quality and CodeQL |

Every Full configuration ran the C ABI probe in each feature-set suite,
`cargo package --workspace --locked` with the archive checks of the
[packaging validation](validation/packaging.md), and Rustdoc on pinned
stable. A local rerun of the Full `windows-stable` configuration at `d4ba3ff`
(Windows 11 x64, Rust 1.99.1, MSVC 19.44.35222.0) passed in 678 s with 217,
201, 202, 201 and 216 tests for the `default`, `none`, `lldp`, `nfc` and
`document` feature sets, six examples, 171 Query fixtures with 2133 native
executions, packaging, and static linking; shared linking is recorded as
unsupported on Windows. The first Full run,
[39771040778](https://github.com/OpenTLV/opentlv-rust/actions/runs/39771040778)
at `f6cc931`, predates packaging, layout and documentation changes and is not
completion evidence.

Test ownership follows the [quality gates](quality-gates.md#test-ownership):
Rust owns API, lifetime, error-mapping, FFI, ABI and linking tests; the fuzz
seed replay and the shared Query corpus read Core-owned data from the selected
Core tree. No Core test suite is copied.

### Core cleanup and compatibility

Core `7596cdfe` contains no `bindings/rust`, Cargo workspace, Rust workflow or
Rust user guide. Its remaining Rust references point to this repository, and it
keeps the shared Query corpus, fuzz seeds and the diagnostic vocabulary
generator with `--rust-output-root`. With that Core selected locally through
`OPENTLV_CORE_SOURCE`, `d4ba3ff` passed all 217 default tests and the 171 Query
fixtures, so the cleanup removed nothing the Rust tests read.

### Documentation

| Step | Evidence |
| --- | --- |
| Portal integration | [OpenTLV/opentlv-docs#74](https://github.com/OpenTLV/opentlv-docs/pull/74) selects the standalone manuals |
| Notification | Main CI [39804845893](https://github.com/OpenTLV/opentlv-rust/actions/runs/39804845893) → notifier [39809300668](https://github.com/OpenTLV/opentlv-rust/actions/runs/39809300668) → Docs source update [39809324870](https://github.com/OpenTLV/opentlv-docs/actions/runs/39809324870) → [OpenTLV/opentlv-docs#76](https://github.com/OpenTLV/opentlv-docs/pull/76), merged with the Rust pin `d4ba3ff` |
| Core pin after cleanup | [OpenTLV/opentlv-docs#75](https://github.com/OpenTLV/opentlv-docs/pull/75) selects Core `7596cdfe` after dropping the archived Core Rust user guide |

The only earlier notification, for `2157a08`, reached Docs before the Rust
source was registered and was rejected as expected. Issue #7 also presents
Core's retained `development/rust.md` as current Core ownership guidance in
the portal instead of an archived copy of the Rust guide.

### Workspace

`opentlv-workspace` registered this repository in
[OpenTLV/opentlv-workspace#37](https://github.com/OpenTLV/opentlv-workspace/issues/37),
pinned to the initial scaffold. Issue #7 pins it to `d4ba3ff` and adds a Rust
smoke to both Workspace Full scenarios: `cargo run --locked -p opentlv
--example quick_start` with Rust 1.99.1, after which the gate requires a clean
Core selected inside the scenario's Cargo target at exactly the commit in
`opentlv-sys/core/core-artifact.json`. Rust rejects its local Core override in
CI, so the sibling scenario also uses the lock; building against a sibling
Core remains the documented
[local workspace development](development/rust.md#local-workspace-development)
workflow.

Workspace Full
[39817682277](https://github.com/OpenTLV/opentlv-workspace/actions/runs/39817682277)
passed both scenarios and the evidence gate for that change at
`d446afd54031cf7168b952c6556e1b9d2992dcb5`. A local run on WSL Ubuntu 24.04
x86-64 had passed before it in 147 s (sibling) and 159 s (remote): CLI, Lua,
Rust and documentation checks passed, Rust printed `Hello, world!` and
selected Core `476f1fbe` (`pinned-git`) in both.

### Known limitations and follow-up work

These are outside the migration and are tracked separately:

- **Verified Core.** No approved Core release with schema-2 assets exists, so
  the lock has no release tag and Verified Source and Verified Binary
  selection, including GitHub Artifact Attestation verification, has not run
  against a real release. Lock one once it exists.
- **Core pin.** The lock stays at the migration source commit, which still
  contains Core's former `bindings/rust`. Moving it past the cleanup is a
  normal reviewed dependency update.
- **Windows shared linking.** Prebuilt shared linking on Windows MSVC still
  fails on the data symbols recorded in the
  [baseline](#known-baseline-failures).
- **CodeQL.** Full reports that CodeQL Action v3 is deprecated in December
  2026; `rust-codeql.yml` needs v4.
- **Deferred by the migration epic.** crates.io publication, a release
  workflow, Cargo feature normalization and API evolution.
