# Packaging validation

Local validation for [issue #4](https://github.com/OpenTLV/opentlv-rust/issues/4)
on 2026-10-22, on Windows 11 x64 with Rust 1.98.1, MSVC and CMake 3.31.7, from
the uncommitted working tree. Local checks are not hosted workflow acceptance
or release authorization. Nothing was published.

## Scope

| Change | Reason |
| --- | --- |
| `repository` `https://github.com/marekcingel/OpenTLV` → `https://github.com/OpenTLV/opentlv-rust` | The recorded [metadata discrepancy](../migration.md#recorded-metadata-discrepancies) pointed to the original repository. |
| `keywords` for both crates, `categories` per crate | Registry discovery metadata. |
| `README.md` and a `LICENSE` copy in each crate | Package readme and the MIT license text in every archive. |
| `opentlv-sys` requirement `0.7.1` → `=0.7.1` | The crates are released together; see [version relationship](../packaging.md#version-relationship). |
| `scripts/ci.py` `package` check | Also checks each archive's `LICENSE` and normalized `Cargo.toml`. |

The crate names, version `0.7.1`, descriptions, `links`, build scripts, Cargo
features and defaults, sources, tests, examples, native linking, the Core lock
and `Cargo.lock` are unchanged. The [CI evidence](ci.json) of issue #3 was
recorded with the previous CI definitions.

## Results

`CARGO_HTTP_CHECK_REVOKE=false` was set for every Cargo command that reads the
crates.io index, because the local network blocks Windows certificate
revocation checks (`CRYPT_E_NO_REVOCATION_CHECK`). Without it, Cargo cannot read
the index; this is a property of the local network, not of the packages.

| Command (from `bindings/rust`) | Result |
| --- | --- |
| `cargo metadata --format-version 1 --no-deps --locked` | Both crates `0.7.1`, `MIT`, `https://github.com/OpenTLV/opentlv-rust`, readme `README.md`, the keywords and categories of [packaging](../packaging.md); `opentlv` depends only on `opentlv-sys` `=0.7.1`, `opentlv-sys` has no dependencies; features `default`, `document`, `lldp`, `nfc` for both |
| `cargo package -p opentlv-sys --list` | 22 files: `README.md`, `LICENSE`, `build.rs`, `core/` (4 files), `src/` (6), `tests/` (5), Cargo's 4 generated files |
| `cargo package -p opentlv --list` | 58 files: `README.md`, `LICENSE`, `build.rs`, `src/` (25), `examples/` (7), `tests/` (19), Cargo's 4 generated files |
| `cargo package -p opentlv-sys --allow-dirty --locked` | Packaged and verified: the archive built in 39 s |
| `cargo package -p opentlv --allow-dirty --locked`, with or without `--no-verify` | Fails as expected: `no matching package named opentlv-sys found` in the crates.io index |
| `scripts/ci.py` `package` check (`cargo package --workspace --locked --allow-dirty`) | Passed in 101 s: both archives built from their contents, exact file lists, `LICENSE` equal to the repository license, normalized manifests without a dependency `path` and with `opentlv-sys = "=0.7.1"` |
| `cargo test --workspace --locked` | 217 tests in 25 targets passed, as in the [CI validation](ci.md) `default` set |
| `python -B -m unittest discover -s tests` (repository root) | 59 tests passed, including [`test_packaging.py`](../../tests/test_packaging.py) |
| `python -B scripts/check_quality.py` | Passed |

Every package verification build selected the clean `pinned-git` Core `0.7.1`
at `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf` with `verified: false`. The
normalized `opentlv` manifest declares:

```toml
[dependencies.opentlv-sys]
version = "=0.7.1"
default-features = false
```

`tests/test_packaging.py` checks the packaged manifest validation against
local dependency paths, including target-specific and development
dependencies, caret requirements, extra dependencies, the previous repository
URL, a different version and a missing readme.

Neither `opentlv` nor `opentlv-sys` was found in the crates.io index
(`cargo info`). The [remaining crates.io prerequisites](../packaging.md#remaining-cratesio-prerequisites)
were not addressed.
