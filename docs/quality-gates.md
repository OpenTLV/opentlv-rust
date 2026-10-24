# Rust quality gates

Rust follows the PR, Main and Full entry points of the standalone Lua and
Python repositories. Full is reusable by a future release workflow; this
repository does not publish crates or create tags through these workflows.
Core no longer contains a Rust binding or Rust workflow; these gates are the
only Rust CI.

| Entry point | Trigger | Required work |
| --- | --- | --- |
| `rust-pr.yml` | Pull request to `main` | Linux with pinned stable Rust: rustfmt, Clippy, build, the default and reduced-feature test suites (including the C ABI probe), examples, the shared Query corpus, Rustdoc, Core provenance, infrastructure regressions and CodeQL; informational Core API coverage |
| `rust-main.yml` | Push to `main` | Linux with pinned stable Rust: workspace build, `quick_start` smoke and Core provenance, then documentation change detection |
| `rust-full.yml` | Manual dispatch or `workflow_call` | All seven configurations below, Cargo packaging, prebuilt static and shared linking, AddressSanitizer, source quality and CodeQL; informational Core API coverage |
| `docs-notify.yml` | Successful `Rust Main CI` on `main` | [Documentation portal notification](#documentation-portal-notifications) when documentation inputs changed |

PR has no path filters, so a documentation-only pull request still produces its
required check. Feature branch pushes do not duplicate PR builds. Main does not
repeat the matrix or source analysis. Full has no push, tag or schedule trigger
and never publishes. A failed configuration does not cancel the remaining Full
matrix. PR/Main superseded runs can be cancelled.

## Documentation portal notifications

The [OpenTLV documentation portal](https://github.com/OpenTLV/opentlv-docs)
assembles the Rust manuals from this repository's `docs/` directory at a
reviewed, pinned commit. After the checks of `Rust Main CI` pass on the official
repository's `main`, its `docs_changes` job records whether documentation
inputs changed. It compares against the most recent earlier successful Main
run, so cancelled or failed pushes do not lose documentation changes. If no
earlier successful run is available in the latest 100 successful runs, it
checks the complete source tree. Detection includes `docs/`,
`opentlv/examples/`, the root README and changelog, the crate READMEs, and
notification workflow/tool changes. Unrelated implementation changes produce a
record with `changed: false`.

`Notify Rust documentation portal` runs after Main finishes successfully. It
downloads only `docs-notification-<producer attempt>`, validates its repository,
commit, run and attempt, and skips dispatch when no relevant files changed.
The notifier performs no checkout and executes no artifact code. PR, fork,
failed and cancelled runs cannot use its cross-repository credential.

Configure `OPENTLV_DOCS_DISPATCH_TOKEN` as a repository secret holding a
fine-grained GitHub token restricted to **OpenTLV/opentlv-docs**, with
**Contents: write** for repository dispatch. The producer uses only its
read-only `GITHUB_TOKEN`; the dispatch secret is available solely to the
notifier's final step. Missing credentials, HTTP authentication/permission
errors and network failures fail the notifier clearly without printing the
token. They do not change the successful Main result.

The event is `component-docs-updated` with exactly these payload fields:

```json
{
  "source": "rust",
  "repository": "OpenTLV/opentlv-rust",
  "commit": "<full producer commit SHA>",
  "ref": "main",
  "run_id": 123
}
```

Install the paired Docs receiver configuration on Docs `main` first. Docs
independently verifies the configured `rust-main.yml` push run and commit
ancestry, then proposes a monotonic Rust source pin update. Duplicate and
older pending or merged updates do not replace newer pins. This documentation
mode makes no runtime compatibility claim and consumes no compatibility feed.
The proposal runs normal Docs PR checks; neither Rust PRs nor this event invoke
Docs Full or publish Pages, so a rejected or failed proposal leaves the last
published documentation unchanged. New manual pages still require reviewed
portal navigation changes.

If dispatch fails, rerun the notifier for the same successful producer while
its artifact remains available. Replaying a delivered event is safe. After
artifact expiry, rerun the producer. A 204 response means GitHub accepted the
event; confirm the Docs receiver run and source-update PR separately. Local
mocked HTTP tests do not establish hosted delivery or publication.

Independent Rust documentation releases are not configured: this repository
has no release workflow yet, so no release receipt is sent and the portal
shows the manuals of its selected source pin.

## Supported configurations

[`ci/matrix.json`](../ci/matrix.json) holds the toolchains, platforms, Cargo
feature sets and the test inventory used by both the workflow planner and the
evidence verifier.

| Configuration | Runner | Toolchain | Feature sets | Additional checks |
| --- | --- | --- | --- | --- |
| `linux-stable` | Ubuntu 24.04 | 1.99.1 | all five | rustfmt, Clippy, Rustdoc, examples, Query corpus, packaging, linking |
| `windows-stable` | Windows latest, MSVC | 1.99.1 | all five | as Linux; shared linking unsupported |
| `macos-stable` | macOS latest | 1.99.1 | all five | as Linux |
| `linux-msrv`, `windows-msrv`, `macos-msrv` | as above | 1.82.0 | `default`, `none` | |
| `linux-sanitizer` | Ubuntu 24.04, Clang 18 | `nightly-2026-10-21` | `default` | AddressSanitizer and LeakSanitizer |

PR runs `linux-stable` with the `default` and `none` feature sets and without
packaging or linking. Main runs only the `linux-stable` smoke.

- **Toolchains.** Stable is pinned to the exact release, so new Clippy lints or
  compiler warnings arrive through a reviewed update of the matrix, not a
  floating `stable`. `rust-version` was 1.70 when imported from Core, but the
  safe crate never compiled with it: taking the address of an extern static
  without `unsafe` requires 1.82. The workspace now declares 1.82, and every
  MSRV configuration proves it with the full default and reduced-feature
  suites. Clippy reads `rust-version`, so it now suggests 1.82 APIs; the one
  resulting finding in `opentlv/tests/corpus.rs` uses `Option::is_none_or`. Clippy, rustfmt and Rustdoc run only on pinned stable, where their
  lint sets are defined.
- **Feature sets.** `default` (`lldp`, `nfc` and `document`), `none`
  (`--no-default-features`) and each feature alone. These are the existing
  Cargo features; no feature or default changed. Clippy checks `default` and
  `none`.
- **Platforms.** The hosted Ubuntu and Windows runners are x86-64 and the macOS
  runner is Apple silicon. Each record stores the Rust host triple, the C
  compiler that built Core and the runner architecture. 32-bit and
  cross-compiled targets are not tested.

## Test ownership

Rust owns the public Rust API, ownership and lifetime guarantees (including the
compile-fail Rustdoc examples), Rust error mapping, FFI interoperability, the
Rust/C ABI layout and native linking. The imported test suites stay as they are;
no test framework or unrelated test rewrite is introduced.

Core owns TLV parsing and writing, format, schema, codec and Query semantics,
its regression corpus, fuzzing, its own ABI checks and performance tests. Core
tests, examples and other bindings remain disabled in the Core builds of
`build.rs` and the linking checks. Two existing Rust tests read Core-owned data
from the selected Core tree instead of copies: the fuzz seed replay in
`opentlv/tests/corpus.rs`, and the shared Query corpus that Core's
`tests/query/run.py` drives through the `query_conformance` example. No Core
test suite is copied or run here, and establishing this CI identified no Core
behavior that only these tests cover; any such gap belongs in a Core issue.

The C ABI probe (`opentlv-sys/tests/fixed_abi.rs` with
`tests/abi_probe/`) is unchanged. It builds a C program with CMake and compares
the size, alignment and every member offset of the Fixed format structures with
the `#[repr(C)]` mirrors. The composition probe (`opentlv-sys/tests/compose_abi.rs`
with `tests/compose_abi_probe/`, #20) does the same for the field encodings, the
Format compositions, `tlv_format_error_t` and `tlv_encoding_t`, leaving the
imported probe intact. Both run in every `default` and reduced-feature suite,
on every platform and toolchain.

## Checks

Each configuration runs [`scripts/ci.py`](../scripts/ci.py) in a fresh work
directory with its own Cargo target directories, the pinned toolchain from
`RUSTUP_TOOLCHAIN` and an environment without inherited Core overrides,
`RUSTFLAGS` or `CC`/`CFLAGS`. Every Cargo command uses `--locked` and, except
Rustdoc, `RUSTFLAGS=-D warnings`.

- **Formatting and Clippy.** `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` for `default` and
  `none`.
- **Build and tests.** `cargo build --workspace --all-targets`, then
  `cargo test --workspace` for each feature set. The runner counts the tests of
  every target, including Rustdoc tests, and compares them with the inventory.
  A missing, extra or renamed target, a different count, or any failed,
  ignored, measured or filtered test fails.
- **Examples.** The seven runnable examples. `query_conformance` is the native
  driver of the Query corpus; a new example fails until it is added.
- **Query corpus.** Core's `tests/query/run.py` of the selected Core tree
  drives `query_conformance` with `--property-cases 0`: 171 fixtures with 2133
  native executions and no unavailable providers. The driver is rebuilt with
  the default features first, because Cargo leaves the example binary of the
  last test build, possibly a reduced-feature one, in `target/debug/examples`.
- **Rustdoc.** `cargo doc --workspace --no-deps` with
  `RUSTDOCFLAGS=-D warnings`. Three broken intra-doc links in `codec.rs` were
  repaired; the rendered text is unchanged.
- **Packaging.** `cargo package --workspace` builds both crates from their
  `.crate` archives. `opentlv` resolves `opentlv-sys` from the packaged archive,
  not the path dependency. Each archive must contain exactly the crate's tracked
  files and Cargo's generated `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock` and
  `.cargo_vcs_info.json`; in hosted CI the latter must name the checkout
  commit. Each archive's `LICENSE` must equal the repository license, and its
  normalized `Cargo.toml` must keep the [package identity](packaging.md)
  without a dependency `path`, with `opentlv` requiring exactly the
  `opentlv-sys` version. The archive SHA-256 digests are recorded.
  Publication is separate post-migration work; the dependency on `opentlv-sys`
  now also states its version so that `opentlv` can be packaged at all.
- **Native linking.** Core from the selected tree is built once as a static
  and once as a shared library, with the options of `build.rs`. The default
  test suite and `quick_start` then run against each through `OPENTLV_LIB_DIR`
  and `OPENTLV_LINK_KIND`, with the shared library on `LD_LIBRARY_PATH` or
  `DYLD_LIBRARY_PATH`. On Windows the shared library is recorded as
  `unsupported`: the bindings do not import Core's exported data symbols from a
  DLL, so the link fails, as in the [migration
  baseline](validation/standalone-build.md). Fixing that is not part of this
  CI change.
- **AddressSanitizer.** The pinned nightly builds the Rust code with
  `-Zsanitizer=address` for the explicit host target, so build scripts stay
  uninstrumented. Clang 18 builds Core with
  `-fsanitize=address -fno-omit-frame-pointer`, checked in each Core
  `CMakeCache.txt`. The library and integration tests of the default features
  run with `detect_leaks=1`; Rustdoc tests are not instrumented and are excluded
  from this inventory only. Memory and thread sanitizers, Miri (which cannot
  execute the C library), and sanitizers on Windows and macOS are not run.
- **Core provenance.** Every `core-selection.json` written by an
  `opentlv-sys` build script in the work directory is checked against the
  lock: repository, commit, version, release tag and a clean tree. With the
  current `release_tag: null` this requires `pinned-git` with
  `verified: false`, which is exact source identity, not an authenticated Core
  release. Core must have been resolved inside the work directory, so a local
  `OPENTLV_CORE_SOURCE` is never accepted; `build.rs` also rejects it in CI.

Source quality runs [`scripts/check_quality.py`](../scripts/check_quality.py)
(Python syntax, workflow structure and whitespace of all files except the
preserved migration evidence), the existing release-verifier tests, the CI and
documentation notification regression tests with
`python -m unittest discover -s tests`, and actionlint.
CodeQL analyzes Rust and Python from source.

Each build configuration also executes the workflow's build and gate shell
steps with a stub runner before building. These eight regression cases verify
optional release arguments, quoting, logs and failure propagation, including
through `tee`. Both commands use nonempty arrays so `set -u` also works with
macOS Bash 3.2. For local shell-specific checks, set `OPENTLV_TEST_BASH` to the
desired Bash executable and run `tests/test_ci_shell.py`.

## Core API coverage

`Core API coverage` reports how much of the pinned Core's public API the safe
`opentlv` crate uses. It runs in PR and Full, not Main. The job is
informational: it uses `continue-on-error`, is not a `Rust quality gate`
prerequisite and has no threshold yet. The coverage epic (#18) tracks raising
it. `Rust timings` waits for it, so its duration is recorded.

[`scripts/api_coverage.py`](../scripts/api_coverage.py) is the binding-agnostic
script of the Python repository, with two changes: its default configuration is
`ci/api-coverage.json`, and the report states that a referenced symbol does not
prove functional parity. It fetches only the locked Core commit and reads every
`TLV_API` function and data declaration under `tlv/include`. A declaration
shape it does not recognize fails the report instead of being skipped. A symbol
counts as referenced when its identifier appears in `opentlv/src` outside
comments, including Rustdoc comments; calls through Core macros are not counted.
Coverage is reported for core and `tlv/builtins` separately, in total and per
header, with every unreferenced symbol listed. The Markdown summary goes to the
job log and job summary. `api-coverage.json` is uploaded as an attempt-scoped
artifact.

`opentlv-sys` is not a reference directory. Its `extern` declarations make a
symbol callable only through `unsafe` raw bindings, without the safe crate's
ownership, error and lifetime guarantees, so a declaration alone does not count.
For Core 0.7.1, 11 symbols such as `tlv_document_encode`, `tlv_query_compile`
and `tlv_node_encode` are declared in `opentlv-sys` but not used by
`opentlv`, and are reported as not referenced.

On a pull request, the job also reads the Core lock at the base commit and lists
the symbols added and removed between the two pins. Added symbols show whether
the safe crate already references or ignores them. An unchanged pin is reported
as such.

[`ci/api-coverage.json`](../ci/api-coverage.json) is the configuration and
ignore list. It names the Core lock `opentlv-sys/core/core-artifact.json` and
the reference directory. Each ignore entry has a `reason` and matches header
paths relative to `tlv/include` (`headers`) or symbol names (`symbols`); both
accept `fnmatch` patterns. Ignored symbols are excluded from the counts, even
when the safe crate references them, such as `tlv_tag_compare` in a test that
compares `Tag` ordering with Core. The current entries are the endian, tag,
value, size and copy primitives, which Rust represents natively, the
diagnostic producers (`tlv_diagnostic_init`, setters, path builders,
`tlv_location_set` and `tlv_location_resolve`) for C format and codec
implementations, and the decode, measure, encode and constructed callbacks
that the bound initializers such as `tlv_fields_format_init()` and
`tlv_variable_format_init()` install, which Core calls through the descriptor.
The packed callbacks stay counted: `ComposedFormat::packed` passes them to
`tlv_format_init()` itself. The last entries are Reader and Writer entry points
that duplicate a bound path: wrappers that pass a NULL diagnostic to a bound
`_diag` counterpart, such as `tlv_reader_next` and `tlv_tree_writer_begin`;
`tlv_write` and `tlv_write_diag`, the Tag/Value forms of
`tlv_write_element_diag`; and `tlv_writer_copy_element`, which is
`tlv_writer_write_element` without diagnostics. Diagnostic consumers such as the `*_string` lookups and
`tlv_diagnostic_path_string` remain counted. Patterns that match nothing in the
pinned Core are reported, so stale entries are visible after a pin bump. Change
the ignore list only for symbols the safe crate intentionally leaves unbound.

The first report, for Core 0.7.1 at `476f1fbecde6`, referenced 136 of 254 core
symbols (53.5 %) with 43 ignored, and 34 of 291 builtins symbols (11.7 %).
With the field encodings, Format compositions and generic Format operations
(#20), it references 164 of 235 core symbols (69.8 %) with 62 ignored; every
`tlv/field/*.h`, `tlv/format.h` and `tlv/formats/*.h` symbol is referenced or
ignored. With the single-element reads and source-free Reader pulls (#24), it
references 167 of 218 core symbols (76.6 %) with 79 ignored; every
`tlv/reader/*.h` and `tlv/writer/*.h` symbol is referenced or ignored.
Builtins-specific parity is outside the coverage epic. The report reads the
headers, not a build, and is unaffected by Cargo features; with the default
features `opentlv-sys/build.rs` builds Core with every component and builtin
enabled.

Run it locally from the repository root:

```sh
python -B scripts/api_coverage.py
python -B scripts/api_coverage.py --core ../opentlv --commit HEAD --base-commit <old-sha>
python -B scripts/api_coverage.py --base origin/main --output build/api-coverage.json
```

Without `--core`, the script caches the shallow Core fetch in
`build/api-coverage/core.git`. `--core` reads an existing Core checkout instead;
the requested commits must already be there. `--commit` measures a different
Core revision than the lock. `--base` reads the lock at a binding revision, and
`--base-commit` names a previous Core revision directly. The parsing, ignore
and pin-diff logic is covered by `tests/test_api_coverage.py`.

## Dependency verification

The `opentlv-sys` build script remains the sole Core selection mechanism; CI
adds no second resolver. A release caller must invoke:

```yaml
jobs:
  full:
    uses: ./.github/workflows/rust-full.yml
    with:
      require_verified_core: true
```

That mode requires Full CI and rejects the current null release lock before
building. Once an approved release is locked, every Cargo command runs with
`OPENTLV_REQUIRE_VERIFIED_CORE=1`. Each selection must then be
`verified-binary` or `verified-source`, and its `verification.json` must name
the locked release tag, a release ID, the manifest SHA-256 and the
`core-release.yml` signer. Failed attestations, checksums or producer evidence
stop the build in `build.rs`; they never fall back to Git. Testing fixtures is
not evidence that a real published Core attestation was consumed.

## Evidence and required check

Successful records include the repository, checkout SHA, run ID and attempt,
the matrix configuration, the Rust release, host triple and LLVM version, the
runner system and architecture, the C compiler that built Core, every Core
selection, per-target test counts for each feature set, the results of the
configured checks, the crate digests, the elapsed time and hashes of all CI
definitions, sources, tests and dependency configuration. Local runs explicitly
record `hosted: false` and their checkout's dirty state.

`Rust quality gate` runs even after failed prerequisites. It requires the exact
matrix, all applicable job results and, per configuration, exactly the
configured checks with the inventory results. Main explicitly permits only the
source-quality and CodeQL jobs to be skipped. Missing, duplicate, stale,
foreign, cancelled, unexpectedly skipped or unsuccessful evidence fails. Success
evidence is removed before each attempt, so a failed rerun cannot reuse an old
success. The gate CLI accepts only an official clean hosted checkout.

After the first hosted PR run, configure branch protection/rulesets to require
the check ending in `Rust quality gate` from GitHub Actions; use the exact
displayed reusable-workflow context (normally `checks / Rust quality gate`).
Do not require individual Full matrix jobs on every PR. This change does not
alter repository settings. A workflow file alone does not enforce merging rules.

Artifacts include attempt-scoped records and build logs. The gate uploads
`quality.json` only after acceptance. Timing collection uses the same
run/attempt, excludes itself and retries delayed API completion metadata.
Reported job seconds are elapsed runner time, not billed minutes. This is the
first standalone CI baseline; no hosted speedup or measured latency is claimed.

## Changing the inventory

Intentional changes to tests, examples, features, toolchains or the pinned
Core corpus must update [`ci/matrix.json`](../ci/matrix.json) in the same
change, with reviewed evidence. Updating the stable toolchain or the sanitizer
nightly is a deliberate matrix change; raising the MSRV also changes
`rust-version`.

## Local validation

The runner installs the configured toolchain with rustup; use an isolated
`RUSTUP_HOME` and `CARGO_HOME` to leave your global setup unchanged. It needs
Python 3.11 or newer, CMake, a C compiler, Git and network access to GitHub and
crates.io:

```sh
python -m pip install -r requirements-dev.txt
python -B scripts/check_quality.py
python -B -m unittest discover -s tests
actionlint
python -B scripts/ci.py matrix --tier full
python -B scripts/ci.py build --tier full --configuration linux-stable \
  --work build/ci-full-linux --output build/full-linux-stable.json
```

Choose a configuration that matches the host system, such as `windows-msrv` or
`macos-stable`; `linux-sanitizer` needs `clang-18`. Every invocation requires a
new `--work` directory.

See [local validation and remaining hosted acceptance](validation/ci.md).
