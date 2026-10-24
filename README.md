# opentlv-rust

Rust bindings for [OpenTLV](https://github.com/OpenTLV/opentlv), a universal
library for parsing, encoding, and working with TLV and TLV-like binary formats.

The Rust bindings were migrated out of Core
([migration epic](https://github.com/OpenTLV/opentlv/issues/670)). This
repository contains the Cargo workspace imported from Core in
[issue #1](https://github.com/OpenTLV/opentlv-rust/issues/1). Since
[issue #2](https://github.com/OpenTLV/opentlv-rust/issues/2) it builds standalone
against an explicitly pinned Core dependency, and since
[issue #3](https://github.com/OpenTLV/opentlv-rust/issues/3) it has its own
[PR, Main and Full CI](docs/quality-gates.md). Since
[issue #4](https://github.com/OpenTLV/opentlv-rust/issues/4) both crates are
prepared as independent Cargo packages with their own
[versioning](docs/packaging.md); they are not published yet. Since
[issue #5](https://github.com/OpenTLV/opentlv-rust/issues/5) the Cargo
workspace is at the repository root, and since
[issue #6](https://github.com/OpenTLV/opentlv-rust/issues/6) this repository
owns the Rust [documentation](#documentation) published through the OpenTLV
documentation portal. Core removed its in-tree `bindings/rust` and the workflow
that tested it in
[OpenTLV/opentlv#679](https://github.com/OpenTLV/opentlv/issues/679), and
[issue #7](https://github.com/OpenTLV/opentlv-rust/issues/7) validated the
completed migration across repositories; see the
[migration completion record](docs/migration.md#migration-completion). Crate
publication is separate post-migration work.

## Layout

| Path | Content |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Cargo workspace of the two crates |
| `opentlv-sys/` | Raw FFI crate: sources, `build.rs`, tests and the C ABI probe |
| `opentlv-sys/core/` | Core dependency lock, selection module, release verifier and toolchain probe used by `build.rs` |
| `opentlv/` | Safe Rust crate: sources, tests and eight examples |
| `opentlv-sys/README.md`, `opentlv/README.md`, `*/LICENSE` | Package readme and license text of each crate |
| `.github/workflows/` | PR, Main and Full CI, source quality, CodeQL and the documentation portal notification |
| `ci/matrix.json` | CI toolchains, platforms, feature sets and test inventory |
| `scripts/ci.py` | Runs one CI configuration, or checks the evidence of all of them |
| `scripts/docs_notification.py` | Records documentation changes for the portal notification |
| `tests/` | Tests for the release verifier, the CI infrastructure and the documentation notification |
| `docs/guides/rust.md` | Rust user guide |
| `docs/development/rust.md` | Rust development guide: crates, build, features, versioning and contributing |
| `docs/migration.md` | Migration baseline, source import, validation results and completion record |
| `docs/migration/` | Source inventory, original CI configuration and validation evidence |
| `docs/quality-gates.md` | CI entry points, supported configurations, test ownership and evidence |
| `docs/packaging.md` | Package identities, versioning, Core compatibility and crates.io prerequisites |
| `docs/validation/` | Standalone build, CI, packaging and layout validation |
| `scripts/verify_baseline.py` | Verifies the inventory against the pinned Core tree |
| `scripts/verify_migration.py` | Verifies the imported files against the inventory and the pinned Core tree |

## Documentation

The [user guide](docs/guides/rust.md) covers setup, reading, writing, errors,
ownership, Documents and Query; the [development guide](docs/development/rust.md)
covers the crates, build, Cargo features, versioning, CI and contributing. Every
public item also has Rustdoc (`cargo doc -p opentlv --open`). The
[OpenTLV documentation portal](https://github.com/OpenTLV/opentlv-docs)
assembles `docs/` at a reviewed commit and links the examples in
`opentlv/examples/` from this repository. Change Rust documentation here, not
in Core or the portal. Successful Main builds notify the portal when
documentation inputs change; see the
[notification contract and token permissions](docs/quality-gates.md#documentation-portal-notifications).
The [migration record](docs/migration.md) links the original Core guides.

## Building

Requirements: Rust 1.82 or newer, CMake 3.16 or newer,
a C99 compiler and Git. On Windows, use the MSVC Rust toolchain with Visual
Studio. The first build of each configuration downloads Core from GitHub.

```sh
cargo build --workspace
cargo test --workspace
```

The Cargo features `lldp`, `nfc` and `document` (all on by default) and the
native linking modes are the same as in Core.

Before a pull request, run the checks of the PR CI from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The [quality gates](docs/quality-gates.md) list every CI check, the supported
toolchains and platforms, and how to run a complete CI configuration locally.

### Core dependency

[`core-artifact.json`](opentlv-sys/core/core-artifact.json) pins
Core:

| Field | Value |
| --- | --- |
| `version` | `0.7.1` |
| `commit` | `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` (`0.7.1-31-g476f1fbe`, the migration source commit) |
| `release_tag` | `null`: no approved Core release with attested artifacts is locked yet |
| `allow_verified_source` | `true` |

The `opentlv-sys` build script selects Core before it runs any Core build
file:

| Mode | Selected when | Verification |
| --- | --- | --- |
| `pinned-git` | Default while `release_tag` is `null` | Clones `OpenTLV/opentlv` into `OUT_DIR` once, checks out the locked commit and requires a matching commit, version tag and clean tree. Git commit IDs identify the content; no archive is involved. |
| `verified-binary` or `verified-source` | `release_tag` is set, or `OPENTLV_REQUIRE_VERIFIED_CORE=1` | [`verified_core.py`](opentlv-sys/core/verified_core.py) checks the release and that its tag resolves to the locked commit. It verifies the GitHub Artifact Attestations of the manifest, `SHA256SUMS` and every downloaded archive (signer `core-release.yml`, source digest of the locked commit), along with the archive checksums, Core's Full CI evidence and the configuration bound to the source. It uses a static Verified Binary only when it matches the target, the C compiler of a CMake probe, the MSVC runtime or declared Linux PIC and glibc, and the Core components of the enabled Cargo features. Otherwise it uses the Verified Source, if `allow_verified_source` permits. It never falls back to Git. Needs Python 3.11 or newer and an authenticated [GitHub CLI](https://cli.github.com/). |
| `local-source` | `OPENTLV_CORE_SOURCE` is set | Records the commit, version and dirty state of the local checkout without verifying them, and prints a Cargo warning. Rejected when `CI` or `GITHUB_ACTIONS` is set, or with `OPENTLV_REQUIRE_VERIFIED_CORE`. |

After building Core, or before linking a Verified Binary, the build script
checks that the public `tlv/version.h` and `tlv/config.h` report the selected
version, commit and Core components. Each build records its selection in
`core-selection.json` in the `opentlv-sys` `OUT_DIR`
(`target/<profile>/build/opentlv-sys-*/out/`).

Core is built in `OUT_DIR` with CMake as before: as a Release static,
position-independent library without C++, tests, examples or
warnings-as-errors. Within an `OUT_DIR`, the clone and the Core build are
reused; Core is rebuilt only when the lock, the selected Core sources or the
build environment variables change. Cargo configurations with their own
`OUT_DIR`, such as another profile or feature set, clone and build Core once
each.

`OPENTLV_LIB_DIR` and `OPENTLV_LINK_KIND` still link a prebuilt library.
The build script still selects Core for its headers and test data, but does not
build it, and the prebuilt library is not verified.

The C ABI probe test and the corpus seed replay read the selected Core tree.
To run the shared Query corpus, use the `tests/query/run.py` of the selected
Core, given as `source_dir` in `core-selection.json`:

```sh
python -B <Core>/tests/query/run.py --native target/debug/examples/query_conformance --property-cases 0
```

To test the dependency selection itself:

```sh
cargo test -p opentlv-sys --test core_dependency
python -B -m unittest discover -s tests
```

### Local Core development

To build against a local Core checkout, for example the sibling `opentlv`
checkout of the [workspace](https://github.com/OpenTLV/opentlv-workspace), set
`OPENTLV_CORE_SOURCE` to the absolute path of its repository root. From the
repository root:

```sh
export OPENTLV_CORE_SOURCE="$(cd ../opentlv && pwd)"
cargo test --workspace
```

```powershell
$env:OPENTLV_CORE_SOURCE = (Resolve-Path ../opentlv).Path
cargo test --workspace
```

Unset the variable to return to the pinned Core. Local builds are for
development only. To change the Core dependency, update the lock to a new
approved commit, or to a release once one is published, and record its
validation.

## Migration verification

With a sibling Core checkout that contains the recorded commit:

```sh
python -B scripts/verify_migration.py --core ../opentlv \
  --import-ref 95d6299099af7a7d774ed89ee223f2cda5035aa9
```

This needs Python 3.11 or newer and Git. It runs the baseline verification,
then checks every file of the import commit byte-for-byte against its original
Git blob and rejects missing, extra or modified files in the imported
`bindings/rust`. It does not build anything. Without `--import-ref` it checks
the working tree, which no longer matches: the standalone build intentionally
changed `opentlv-sys/build.rs`, `opentlv-sys/tests/fixed_abi.rs`,
`opentlv/tests/corpus.rs` and `opentlv/Cargo.toml`, and added files, and the
[layout normalization](docs/validation/layout.md) moved the workspace out of
`bindings/rust`. See the
[migration record](docs/migration.md) for the baseline and imported-source
results and the [standalone build validation](docs/validation/standalone-build.md).

## License

[MIT](LICENSE)
