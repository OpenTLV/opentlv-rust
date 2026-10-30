# crates.io identity and package validation

Validation for [issue #37](https://github.com/OpenTLV/opentlv-rust/issues/37)
on 2026-10-30, against the assets of the
[`v0.1.0` GitHub release](https://github.com/OpenTLV/opentlv-rust/releases/tag/v0.1.0)
at `f297ab3899bafb95716a39a38dfaf7df14e8768d`, built by
[run 40646933091, attempt 1](https://github.com/OpenTLV/opentlv-rust/actions/runs/40646933091/attempts/1).
Nothing was published and no crates.io credential was used.

## Result

The names are free, and the archives' metadata, license, dependencies and file
lists are correct. **These `0.1.0` archives must still not be published**: see
[findings](#findings-that-block-publishing-these-archives).

## Registry identity

| Check | Result |
| --- | --- |
| `https://index.crates.io/op/en/opentlv` and `.../opentlv-sys` | 404: neither crate exists |
| `https://crates.io/api/v1/crates/<name>` and `.../owners` for both | 404: no crate, therefore no owners |
| Index entries for `opentlv_sys`, `open-tlv`, `open_tlv`, `open-tlv-sys`, `open_tlv_sys` | 404: no similar name is taken |
| `https://crates.io/api/v1/users/marekcingel` | Account exists (id 446444), `github_username_matches: true` |
| `https://crates.io/api/v1/categories/<slug>` | All four categories used by the crates exist |
| `https://crates.io/api/v1/keywords/<keyword>` | All five keywords are valid existing keywords |

crates.io does not reserve names, so anyone can claim either one until the
first publication. That publication makes the publishing account the owner.

## Release archives

`SHA256SUMS` matches all four listed assets, and `release-manifest.json` records
the same two package digests.

| Archive | SHA-256 | Size | Unpacked | Entries |
| --- | --- | --- | --- | --- |
| `opentlv-sys-0.1.0.crate` | `16522a9fd904c673650a66e1bfc0776f552fca43df9fe17741e4a335cd198547` | 48,298 B | 240,347 B | 33 |
| `opentlv-0.1.0.crate` | `4723697a61b1542fc2c35d112367a144d8337601f122b48a8308d337169f01d5` | 162,033 B | 745,779 B | 77 |

Both are far below the default 10 MiB crates.io upload limit. Reading each archive
without extracting it, and comparing it with the `v0.1.0` tag, gave:

| Property | Result |
| --- | --- |
| Entries | Regular files under the single `<crate>-0.1.0/` prefix |
| Files | Every tracked file of the crate; the only additions are `.cargo_vcs_info.json`, `Cargo.lock` and `Cargo.toml.orig` |
| Content | Byte-identical to the tag, except Cargo's normalized `Cargo.toml` |
| Source identity | `.cargo_vcs_info.json` records the tag commit, not dirty, with the crate's `path_in_vcs` |
| Unwanted content | No CI files, build output, credentials or local checkout paths found by name and content search |
| Metadata | The [packaging identities](../packaging.md#package-identities): name, description, version, edition, `rust-version`, license, readme, repository, keywords, categories, `links`, build script and features; no `publish` restriction |
| Dependencies | `opentlv-sys` has none. `opentlv` has only `opentlv-sys = "=0.1.0"` with `default-features = false` and no `path`. Neither has build or development dependencies |
| Pair binding | The `Cargo.lock` in `opentlv` records checksum `16522a9f...`, the digest of the released `opentlv-sys` archive |
| License | `MIT`; each `LICENSE` equals the root `LICENSE`, the same Git blob as Core's `LICENSE` at the pinned commit |
| SPDX headers | Missing only from Cargo's generated files, `LICENSE`, the READMEs, `core/core-artifact.json` and `tests/abi_probe/CMakeLists.txt`, as the [packaging document](../packaging.md#licensing) states |

The GitHub attestations API returns one SLSA provenance statement for each crate
digest, from `.github/workflows/rust-release.yml` on `refs/heads/main` in that
run. This reads the statement only; the signature was not verified locally.

`cargo publish --workspace --dry-run --no-verify --locked` with Rust 1.99.1 from
a clean `v0.1.0` checkout, without a token, packaged both crates and aborted both
uploads. It covers Cargo's client checks only. The crates.io server checks run at
upload.

## Findings that block publishing these archives

A published version cannot be changed, and these problems are inside the
archives. The fixes need a new commit, so the first crates.io version must be a
new version such as `0.1.1`. Publishing different bytes as `0.1.0` would give one
version two archives, because the GitHub release keeps its attested `0.1.0`.

1. **Stale installation text.** `opentlv/README.md` says the crates are not
   published and shows a path dependency, as does the crate documentation in
   `opentlv/src/lib.rs`. crates.io and docs.rs would display this text.
2. **docs.rs cannot build the crates.** Building the documentation of the
   released `opentlv-sys` archive with `DOCS_RS=1`, offline and in a network
   namespace without network access, stops with `Core dependency verification
   failed: gh is required to verify a Core release` (exit code 101). `build.rs`
   has no documentation-only path. This simulates the docs.rs conditions; docs.rs
   itself was not used. No epic issue names this fix yet.
3. **Registry consumers need an authenticated GitHub CLI.** A default build uses
   `gh` to authenticate Core, as the README states. Whether this requirement is
   acceptable for registry users is an input for
   [issue #38](https://github.com/OpenTLV/opentlv-rust/issues/38).

## Changes

- The CI `package` check now also compares each archive's description, keywords,
  categories, edition, `rust-version`, `links`, build script, features and
  `publish` with the expected registry identity. Applied to the real `v0.1.0`
  archives, it accepts both.
- The packaging document now lists `tests/codec_abi_probe/`, which the archive
  contains and the document omitted.

## Local validation

| Command | Result |
| --- | --- |
| `python -B scripts/check_quality.py` | Passed, REUSE compliant with 163/163 files |
| `python -B -m unittest discover -s tests` | 103 tests passed |
| `scripts/ci.py build --tier pr --configuration linux-stable` | Could not run: the WSL host has no `gh`, so the first build stops at Core verification, and `ci.py` removes the development override |

No crate source, manifest, lock or workflow changed, so the PR tier's Cargo checks
would not exercise this change. Hosted PR CI runs them. Full CI, which contains
the `package` check, was not run locally; the real-archive check above covers the
new comparison.

## Not verified

- Publication permission and token scopes: no credential was used, and only a
  real first publication proves them.
- The verified account email and GitHub organization access: the public API does
  not expose them, so they rest on the record in
  [epic #36](https://github.com/OpenTLV/opentlv-rust/issues/36).
- Publishing authentication and owner configuration, which belong to
  [issue #39](https://github.com/OpenTLV/opentlv-rust/issues/39).
- Consumer builds from the archives, which belong to issue #38.
