# crates.io Trusted Publishing validation

Validation for [issue #45](https://github.com/OpenTLV/opentlv-rust/issues/45) on
2026-11-03. **Nothing was published, no crates.io credential was used, and the
OIDC-only workflow has not run on GitHub.** The
[procedure](../packaging.md#cratesio-publication) describes the workflow.

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

The Rust crates, the build script and the CI matrix are unchanged, so the PR and
Main `scripts/ci.py build` tiers were not run locally. Full CI was not run locally.

## Not verified

- A real publication through GitHub OIDC from a `workflow_dispatch` run. It is
  deferred to the next legitimate Rust release; no release is made only to test it.
- The rejection of an API token by *Require trusted publishing for all new
  versions*.
- The `auth` step against the configured trusted publishers.
