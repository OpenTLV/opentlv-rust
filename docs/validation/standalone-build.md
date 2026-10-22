# Standalone Rust build validation

Local validation for [issue #2](https://github.com/OpenTLV/opentlv-rust/issues/2)
on 2026-10-22. This records local evidence, not hosted CI or release acceptance.

## Dependency and preserved behavior

The [lock](../../bindings/rust/opentlv-sys/core/core-artifact.json) selects Core
`0.7.1` at `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf`, the
[migration source commit](../migration.md#provenance), with no release tag.
Every default build cloned that commit from GitHub into its own `OUT_DIR` and
recorded a clean `pinned-git` selection with `verified: false`. None used a
sibling Core checkout.

Both crates remain at `0.7.1`. The Cargo features, their defaults, the CMake
options of the source build, the static and shared prebuilt linking modes,
the public API, the FFI declarations, `Cargo.lock`, the tests and the examples
are unchanged. Of the imported files, `opentlv-sys/build.rs`,
`opentlv-sys/tests/fixed_abi.rs`, `opentlv/tests/corpus.rs` and
`opentlv/Cargo.toml` changed so that they take Core from the selection
instead of `../../..`. The migration verifier passed all 69 files at import commit
`95d6299099af7a7d774ed89ee223f2cda5035aa9`, and the frozen inventory is
unchanged.

## Completed checks

Each configuration used a fresh `CARGO_TARGET_DIR`. Every Cargo command except
Rustdoc ran with `RUSTFLAGS=-D warnings`.

| Check | Windows x64 | Linux x64, WSL Ubuntu 24.04 |
| --- | --- | --- |
| Rust | 1.98.1 `x86_64-pc-windows-msvc` | 1.98.1 `x86_64-unknown-linux-gnu` |
| C compiler, CMake | MSVC 19.44.35222.0, CMake 3.31.7 | GCC 13.3.0, CMake 3.28.3 |
| `cargo fmt --all --check` | Passed | |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | |
| `cargo build --workspace --all-targets` | Passed | Passed |
| `cargo test --workspace` | 217 passed | 217 passed |
| `cargo test --workspace --no-default-features` | 201 passed | 201 passed |
| Examples | 6 passed | `quick_start` passed |
| Shared Query corpus | 171 fixtures passed | 171 fixtures passed |
| `cargo doc --workspace --no-deps` | Passed, 3 warnings | |
| Unchanged rebuild | No build script rerun | |
| Copy outside the repository | Build and 217 tests passed | Copy in the Linux home directory |
| Local sibling Core (`OPENTLV_CORE_SOURCE`) | Build and 217 tests passed | |
| Prebuilt static library | 217 passed, `quick_start` passed | |
| Prebuilt shared library | Link failure (known) | |

The default-feature count is the baseline's 202 tests plus the 15 new Core
selection tests in `opentlv-sys/tests/core_dependency.rs`. The reduced-feature
count is the baseline's 186 plus the same 15. Per-target counts, the 2133
native Query executions and the three Rustdoc warnings are identical to the
[baseline](../migration.md#recorded-validation). The C ABI probe and
the corpus seed replay compiled and read the selected Core tree.

The prebuilt libraries were built separately from the pinned clone with the
options of `build.rs`. The shared-library run fails with the same unresolved data
symbols as the baseline: 6 in the tests and 12 in `quick_start`. On Linux,
the Core build used `-fPIC`.

The copy outside the repository had no sibling Core and no enclosing
repository; it resolved Core only through the lock. The local-source build
reported a clean sibling checkout at the locked commit and printed the Cargo
warning about an unverified local Core.

## Rejected configurations

Each case stopped in the build script before any Core build file ran, and no
Core clone was created:

| Configuration | Result |
| --- | --- |
| `OPENTLV_CORE_SOURCE` with `CI=true` | Development-only override rejected |
| `OPENTLV_CORE_SOURCE` with `GITHUB_ACTIONS=true` | Development-only override rejected |
| Relative `OPENTLV_CORE_SOURCE` | Absolute path required |
| `OPENTLV_REQUIRE_VERIFIED_CORE=1` with the committed lock | A published Core release must be locked |
| Copy with `release_tag: "v0.7.1"`, no GitHub CLI | Verification failed in `verified_core.py`; no fallback to Git |

## Dependency tests

`cargo test -p opentlv-sys --test core_dependency` (15 tests) exercises the
module that `build.rs` uses. It covers lock parsing, floating and malformed
pins, CI detection, the local override, Git root, version tag and dirty-state
rules, pinned clone reuse, interrupted clones, commit and version mismatches,
verified results that differ from the lock, public header validation and the
evidence record.

`python -B -m unittest discover -s tests` (29 tests) exercises
`verified_core.py`. It covers release locks, manifest identity and Full CI
evidence, workflow and configuration hashes, checksums, archive traversal,
binary compatibility (toolchain, target, cross-compilation, `crt-static`, MSVC
runtime, Linux PIC and glibc, Cargo feature components), Verified Binary and
Verified Source selection, disabled fallback, the package layout, attestation
failure before the manifest is parsed, moved tags and the result file read by
`build.rs`.

The authenticated download tests use fixtures and mocked `gh` responses. They
check signer and source-digest arguments and failure behavior; they do not prove
authentication against a real published release.

## Local evidence and limitations

The [machine-readable report](standalone-build.json) records commands,
environment overrides, exit codes, durations, test counts, selections and log
hashes. Logs remain under the ignored `build/2/logs/` directory. The Linux
toolchain was installed with rustup into the ignored `build/2/linux-rust/`
directory; the WSL system was not changed.

- Cargo builds `opentlv-sys` as a separate unit for `-p opentlv` and for
  `--workspace`, as in the baseline, where the first `-p opentlv` example
  also rebuilt Core. Each unit's `OUT_DIR` now also holds its own Core clone,
  so the first build of each configuration needs network access to GitHub.
  Within a unit, the clone and the Core build are reused.
- No approved Core release with schema-2 assets exists, so the lock has no
  release tag. Verified Source and Verified Binary resolution cannot succeed
  until one is locked. Linux release binaries do not declare position-independent
  code, so on Linux a locked release falls back to Verified Source.
- The declared `rust-version` 1.70, macOS, 32-bit targets, other feature
  subsets, hosted CI and real release-artifact attestations were not tested.
