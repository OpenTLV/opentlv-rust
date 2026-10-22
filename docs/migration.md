# Rust migration baseline

This record fixes the pre-migration state of the Rust bindings in Core for
[OpenTLV/opentlv#671](https://github.com/OpenTLV/opentlv/issues/671), part of the
[Rust binding migration epic](https://github.com/OpenTLV/opentlv/issues/670).
It is the reference for every later migration step. No Rust sources are
imported yet; the source transfer is the next migration issue and must be
compared against this baseline.

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

## Follow-up boundaries

This baseline does not import, change or remove any Rust source, test,
example, feature, linking mode or Core file. The source transfer, standalone
Cargo build and Core dependency, CI, packaging, layout normalization,
documentation integration and Core cleanup are separate migration issues.
