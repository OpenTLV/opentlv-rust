# Layout normalization validation

Local validation for [issue #5](https://github.com/OpenTLV/opentlv-rust/issues/5)
on 2026-10-22, from the uncommitted working tree. Local checks are not hosted
workflow acceptance or release authorization.

## Scope

The Cargo workspace moved from `bindings/rust/`, its location in the Core
monorepository, to the repository root:

| Before | After |
| --- | --- |
| `bindings/rust/Cargo.toml`, `bindings/rust/Cargo.lock` | `Cargo.toml`, `Cargo.lock` |
| `bindings/rust/opentlv-sys/` | `opentlv-sys/` |
| `bindings/rust/opentlv/` | `opentlv/` |

All 76 files were moved with `git mv`, and Git records each one as a rename. 67 are
byte-identical, including the manifests, `Cargo.lock`, build scripts, the Core
lock, tests and the C ABI probe: `build.rs` and the tests resolve paths from
`CARGO_MANIFEST_DIR`, `OUT_DIR` or the selected Core tree, never from the
repository layout. The other 9 changed only documentation text:

| File | Change |
| --- | --- |
| `opentlv/src/lib.rs` | The crate-level Setup example depends on `opentlv-rust/opentlv` instead of `OpenTLV/bindings/rust/opentlv` |
| Six `opentlv/examples/*.rs` | The run hint says "from the repository root" instead of "from `bindings/rust`" |
| `opentlv/README.md`, `opentlv-sys/README.md` | Links and the path dependency name the new crate paths |

Outside the crates:

| Area | Change |
| --- | --- |
| `scripts/ci_support.py`, `scripts/ci.py`, `scripts/ci_contract.py` | Cargo runs from the repository root; the `WORKSPACE` path is gone. The `package` check expects `path_in_vcs` to be the crate directory. The CI definition hashes cover `Cargo.toml`, `Cargo.lock` and both crate directories instead of `bindings` |
| `scripts/check_quality.py` | Parses the Python files of both crate directories instead of `bindings` |
| `.gitignore` | `/bindings/rust/target/` removed; the root `/target/` already covers Cargo output |
| `tests/` | Root paths. Inventory paths map to the new locations through `SOURCE_DIRECTORY`. The examples may differ from the inventory only in the run hint. A new layout test rejects `bindings/rust` and `../../..` in the build configuration |
| README, [packaging](../packaging.md), [migration record](../migration.md), [standalone build](standalone-build.md) | Commands run from the repository root; links name the new paths; the sibling Core checkout for `OPENTLV_CORE_SOURCE` is `../opentlv` |

Workflows needed no change: they already ran `scripts/ci.py` from the
checkout root.

Unchanged on purpose:

- The frozen [inventory](../migration/source-inventory.json),
  `scripts/verify_baseline.py` and `scripts/verify_migration.py` keep
  `bindings/rust`. It is Core's source path and the path at import commit
  `95d6299099af7a7d774ed89ee223f2cda5035aa9`. The verifier still passes all 69
  files at that commit.
- Earlier validation records and their JSON evidence describe runs from
  `bindings/rust` and stay as recorded. The only edit is the lock link in
  [standalone build](standalone-build.md), so that it still resolves.
- The preserved guides `docs/guides/rust.md` and `docs/development/rust.md`
  keep their Core context; updating them is
  [documentation integration (#6)](https://github.com/OpenTLV/opentlv-rust/issues/6).

## Public API and Cargo features

Both crates were documented before and after the move with
`RUSTC_BOOTSTRAP=1 cargo rustdoc -p <crate> --lib --locked -- -Z unstable-options --output-format json`,
each with default features and with `--no-default-features`, in fresh target
directories:

| Crate | Features | Items | Public items | Result |
| --- | --- | ---: | ---: | --- |
| `opentlv-sys` | default | 2451 | 767 | Identical |
| `opentlv-sys` | none | 2347 | 720 | Identical |
| `opentlv` | default | 3763 | 758 | Identical, except the crate-level documentation |
| `opentlv` | none | 3513 | 674 | Identical, except the crate-level documentation |

The comparison ignored source spans and the path of the dependency's `.rmeta`
inside each target directory. Item IDs, paths, signatures, visibility and
attributes are equal. The only documentation difference is the Setup text of
`opentlv/src/lib.rs` described above.

`cargo metadata --no-deps --locked` reports the same packages before and after:
`0.7.1`, the features `default = ["lldp", "document", "nfc"]`, `lldp`, `nfc`
and `document` (forwarding to `opentlv-sys` in `opentlv`), `links = "tlv"`, the
dependencies and all 32 targets. Only `workspace_root` and the manifest paths
change.

## Checks

| Check | Result |
| --- | --- |
| PR CI `linux-stable` (`scripts/ci.py build --tier pr`) | Passed in 550 s: rustfmt, Clippy for `default` and `none`, 217 `default` and 201 `none` tests in 25 targets each (the [CI inventory](../../ci/matrix.json)), the six examples, the Query corpus (171 fixtures, 2133 native executions) and Rustdoc |
| Main CI `linux-stable` (`scripts/ci.py build --tier main`) | Passed in 141 s: workspace build and the `quick_start` smoke run |
| `python -B -m unittest discover -s tests` | 60 tests passed, including the new layout test |
| `python -B scripts/check_quality.py` | Passed |
| `python -B scripts/verify_migration.py --core ../opentlv --import-ref 95d6299099af7a7d774ed89ee223f2cda5035aa9` | 69 imported files verified |

Both CI configurations ran `scripts/ci.py` in WSL Ubuntu 24.04 x86-64 with Rust
1.99.1 and GCC 13.3.0, on a copy of the working tree. Their work directories
were inside the copy, under the root Cargo workspace, as in hosted CI. Each
selected the clean `pinned-git` Core `0.7.1` at
`476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` with `verified: false`. Their
records carry the definition hashes of the final tree. Full CI (all platforms, MSRV, sanitizer,
packaging and prebuilt linking) was not run locally; it runs before a release.
