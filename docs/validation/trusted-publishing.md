# crates.io Trusted Publishing validation

Validation for [issue #45](https://github.com/OpenTLV/opentlv-rust/issues/45) on
2026-11-03, which also prepares the `0.2.1` patch release. **Nothing was tagged or
published, no crates.io credential was used, and the OIDC-only workflow has not run
on GitHub.** The [procedure](../packaging.md#cratesio-publication) describes the
workflow, and [Open steps](#open-steps) lists what remains.

## Configuration

A maintainer made these settings on 2026-11-03. The evidence is the owner-confirmed
crates.io and GitHub settings screenshots of the configuration walkthrough that day.
They confirm the settings, not a successful OIDC publication.

| Setting | State |
| --- | --- |
| Trusted publisher of `opentlv-sys` and `opentlv` | GitHub: owner `OpenTLV`, repository `opentlv-rust`, workflow `rust-crates-io.yml`, environment `crates-io` |
| *Require trusted publishing for all new versions* | Enabled on both crates |
| GitHub environment `crates-io` | Required reviewer `marekcingel`, deployment limited to `main`, no administrator bypass; self-review prevention stays disabled for the sole reviewer |
| `OpenTLV Rust Bootstrap` API token on crates.io | Revoked; the account's token list is empty |
| `CRATES_IO_BOOTSTRAP_TOKEN` environment secret | Deleted; the environment has no secrets |

## Changes

- `rust-crates-io.yml` has a single upload step, which receives only the token of
  `rust-lang/crates-io-auth-action`. The workflow references no secret.
- `upload_plan` requires both crates to exist with `marekcingel` as an owner, and
  selects `trusted`, or `none` for a complete version. A missing crate fails in
  `prepare`, before any upload, and points to the
  [recovery procedure](../packaging.md#recovery).
- `upload` accepts only a `trusted` plan and a non-empty token, before it installs
  the toolchain or runs Cargo.
- The archive digest, attestation, approval, exact-package and post-publication
  checks, and the order `opentlv-sys` then `opentlv`, are unchanged.

## Checks on real data

`upload_plan` and its helpers ran read-only against crates.io, with the package
digests from the `SHA256SUMS` of the
[`v0.2.0` release](https://github.com/OpenTLV/opentlv-rust/releases/tag/v0.2.0):

| Check | Input | Result |
| --- | --- | --- |
| `owners` | `opentlv-sys`, `opentlv` | `marekcingel` for both |
| `upload_plan` | `0.2.0`, already published | No uploads, `none`: the index has the tested digests |
| `upload_plan` | `0.2.1`, not published | `opentlv-sys`, then `opentlv`, with `trusted` |
| `upload_plan` | `opentlv-sys` and a free name | Rejected: *does not exist on crates.io* |
| `published` | `opentlv-sys` `0.2.0` | Index and download serve the tested digest |

## Local validation

| Command | Result |
| --- | --- |
| `python -B -m unittest discover -s tests` | 173 tests passed, 41 of them in `tests/test_crates_io.py` |
| `python -B scripts/check_quality.py` | Passed, REUSE compliant with 174/174 files |
| `actionlint` 1.7.7 | Passed |

These ran on Windows 11 with Python 3.11; the hosted source quality job uses Ubuntu
and Python 3.12.

`tests/test_crates_io.py` covers new versions of existing crates, already
published and immutable versions, a missing crate with no token fallback, a plan
that is not `trusted`, a missing credential, and `id-token: write` in the
`publish` job only. It mocks crates.io, GitHub and Cargo.

## Release 0.2.1

Both crates and `Cargo.lock` are `0.2.1`, and `opentlv` requires exactly
`opentlv-sys` `0.2.1`; the Core lock is unchanged. `release.version_for('v0.2.1')`
accepts the real manifests, lock and release notes. These ran on WSL Ubuntu 24.04 with
Rust 1.99.1 and the local `gh` stand-in of the
[registry build validation](registry-builds.md#method), from the uncommitted tree:

| Command | Result |
| --- | --- |
| `scripts/ci.py build --tier pr --configuration linux-stable` | Passed in 766 s: 300 and 277 tests, 10 examples, 171 Query fixtures with 2,133 native executions, attested `verified-source` Core `0.8.0` |
| `scripts/ci.py build --tier main --configuration linux-stable` | Passed in 184 s |
| `cargo package --workspace --locked --no-verify` | Both archives have the file lists of the published `0.2.0` archives, 33 and 77 files |

Compared with the `0.2.0` archives downloaded from crates.io, the `0.2.1` archives
differ only in the version in `Cargo.toml`, `Cargo.toml.orig` and `Cargo.lock`, the
`opentlv-sys` checksum in the `opentlv` lock, `.cargo_vcs_info.json`, and the new
`0.2.1` row of the Core table in the `opentlv-sys` README. Every source, build script
and Core selection file is identical, so the public API, features and linking are
unchanged. The release archives come from Full CI on the release commit, so these
local digests are not release evidence.

Full CI, including the Linux, Windows and macOS consumer builds, was not run
locally.

## Not verified

- A real publication through GitHub OIDC from a `workflow_dispatch` run. It waits
  for the publication of `0.2.1`, a real patch release; no release is made only to
  test it.
- The rejection of an API token by *Require trusted publishing for all new
  versions*.
- The `auth` step against the configured trusted publishers.

## Open steps

Each is made by a maintainer after review, in this order. Issue #45 closes only
after step 5:

1. Merge this change, with the changelog closed under `## [0.2.1] - 2026-11-03`. If
   the release happens on a later day, correct the date before merging.
2. Wait for Rust Main CI on that commit, and keep `main` there until the publication.
3. Dispatch `Rust Release` with `tag: v0.2.1`. It runs Full CI on the release
   commit, then attests and publishes the GitHub release.
4. Dispatch `Rust crates.io` with `tag: v0.2.1`, review the plan, which must show
   `auth: trusted`, and approve the `crates-io` environment.
5. Record below the release and workflow run links, the `auth` step result, the
   published digests, the `verify-registry` and `verify-consumers` results, and the
   crates.io and docs.rs URLs.

## Publication

Not done yet.
