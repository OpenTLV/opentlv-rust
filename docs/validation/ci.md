# Rust CI validation

Local validation for [issue #3](https://github.com/OpenTLV/opentlv-rust/issues/3)
on 2026-10-22. Local checks are not hosted workflow acceptance or release
authorization.

## Scope

The public API, FFI declarations, Cargo features and their defaults, native
linking modes, `Cargo.lock`, the C ABI probe and the imported tests and examples
are unchanged, except for these corrections found while establishing CI:

| Change | Reason |
| --- | --- |
| `rust-version` 1.70 → 1.82 | 1.70 and 1.81 fail to compile `opentlv`: `ptr::addr_of!` of the extern static `tlv_asn1_query_tags` in `program.rs` needs `unsafe` before 1.82. 1.82.0 builds all targets and passes both MSRV suites. |
| `opentlv/tests/corpus.rs`: `map_or(true, …)` → `is_none_or(…)` | Clippy reads `rust-version`; with 1.82 its `unnecessary_map_or` lint applies. Same semantics. |
| `opentlv/Cargo.toml`: `opentlv-sys` dependency gains `version = "0.7.1"` | `cargo package` rejects a path dependency without a version. |
| Three intra-doc links in `opentlv/src/codec.rs` | Unresolved links were the three Rustdoc warnings of the [baseline](../migration.md#recorded-validation); explicit targets keep the rendered text. |

CI regression tests verify that every other imported test, example and ABI
probe file still matches the frozen migration inventory. The Core lock remains
`0.7.1` at `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` with a null release tag.
Real Core release attestations were not exercised.

## Completed local checks

Every configuration was run by the final `scripts/ci.py` from a snapshot copy
of the uncommitted working tree, in a fresh work directory, with Rust toolchains
installed by the runner into an isolated, ignored rustup home. All records carry
the same definition hashes as the final tree.

| Configuration | Host | Rust | C compiler | Result |
| --- | --- | --- | --- | --- |
| PR `linux-stable` | WSL Ubuntu 24.04 x86-64 | 1.99.1 | GCC 13.3.0 | Passed in 753 s |
| Main `linux-stable` | WSL Ubuntu 24.04 x86-64 | 1.99.1 | GCC 13.3.0 | Passed in 136 s |
| Full `linux-stable` | WSL Ubuntu 24.04 x86-64 | 1.99.1 | GCC 13.3.0 | Passed in 1401 s |
| Full `linux-msrv` | WSL Ubuntu 24.04 x86-64 | 1.82.0 | GCC 13.3.0 | Passed in 268 s |
| Full `linux-sanitizer` | WSL Ubuntu 24.04 x86-64 | nightly-2026-10-21 | Clang 18.1.3 | Passed in 141 s |
| Full `windows-stable` | Windows 11 x64 | 1.99.1 | MSVC 19.44.35222.0 | Passed in 921 s |
| Full `windows-msrv` | Windows 11 x64 | 1.82.0 | MSVC 19.44.35222.0 | Passed in 242 s |

Durations are local wall-clock times with the WSL and Windows runs sharing one
machine; they are not hosted CI estimates. The nightly reports itself as
`1.101.0-nightly`.

Test counts per feature set, identical on every platform and toolchain:

| Feature set | Tests | Targets |
| --- | --- | --- |
| `default` | 217 | 25 |
| `none` | 201 | 25 |
| `lldp` | 202 | 25 |
| `nfc` | 201 | 25 |
| `document` | 216 | 25 |
| `default` under AddressSanitizer, without Rustdoc tests | 203 | 23 |

`default` and `none` equal the [standalone build](standalone-build.md) counts.
`nfc` equals `none` because `nfc_type2.rs` has one test for each state of the
feature.

On both stable configurations:

- rustfmt and Clippy for `default` and `none`, and Rustdoc with
  `-D warnings`, passed.
- The six examples passed. Core's Query runner reported 171 fixtures, 2133
  native executions and no unavailable providers.
- `cargo package --workspace` built both crates from their archives. Each
  archive contained exactly the crate's tracked files plus Cargo's four
  generated files.
- The static prebuilt Core library passed all 217 tests and `quick_start`. On
  Linux the shared library also passed them with `LD_LIBRARY_PATH`. On Windows
  the shared library is recorded as unsupported, as before.

Every Cargo build script, including the package verification and prebuilt
linking builds, cloned and selected the same clean `pinned-git` Core with
`verified: false`; none used a local override.

The sanitizer configuration built Rust code with `-Zsanitizer=address` and
Core with `-fsanitize=address -fno-omit-frame-pointer`, and ran the library and
integration tests with LeakSanitizer enabled. A separate negative control with
the same nightly and flags confirmed that the sanitizers report and fail on
both a leaked allocation and a heap buffer overflow.

Infrastructure checks on Windows, Python 3.11.4:

- `python -B -m unittest discover -s tests`: 52 tests passed. These are the 29
  existing release-verifier tests and 23 CI tests: gate acceptance and
  rejection of every incomplete, foreign, stale or unsuccessful record field
  and job result, Cargo output parsing, the Query driver rebuild, timing
  retries, workflow contracts, the frozen test inventory and the eight
  workflow shell cases under Git Bash.
- `scripts/check_quality.py` and actionlint 1.7.7 passed; shellcheck was not
  installed locally, so the hosted quality job is the first shell lint.
- A real `--require-release` invocation rejected the null Core release lock
  before creating its work directory and removed a stale success record.

The [machine-readable summary](ci.json) records the policy, definition hashes
and, per configuration, the runtime, C compiler, Core selection, test counts,
check results, crate digests, durations and log hashes. Raw logs and records
remain under the ignored `build/3/` directory.

## Diagnostics resolved during validation

- The first Clippy run after raising `rust-version` failed on
  `opentlv/tests/corpus.rs`. Clippy had suppressed `unnecessary_map_or` while
  the declared MSRV lacked `Option::is_none_or`.
- In the first PR run, Core's Query runner drove a `query_conformance` binary
  from the preceding `--no-default-features` test build: 163 fixtures passed and
  8 needed unavailable providers. The runner accepted it and only the gate
  rejected it. The runner now rebuilds the driver with the default features and
  checks the result itself.
- On the nightly toolchain Cargo stores build-script output under
  `build/<package>/<hash>/out`. Selection and CMake evidence are therefore found
  recursively instead of through a fixed `OUT_DIR` pattern.

## Remaining acceptance

- A hosted PR run and a manually dispatched Full run must pass all
  configurations and the quality gate. macOS (Apple silicon) has not run at all;
  CodeQL for Rust and Python, actionlint's shellcheck integration and the
  timing collection run only in hosted CI.
- Branch protection and rulesets have not been modified. After the first hosted
  PR run, require the observed `Rust quality gate` check as described in
  [quality gates](../quality-gates.md).
- A real approved Core release must be locked before strict release mode can
  succeed. Crate publication, signing and release automation remain separate
  work.
- Windows shared-library linking remains an explicitly unsupported check.
