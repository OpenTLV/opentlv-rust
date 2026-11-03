# crates.io Trusted Publishing validation

Validation for [issue #45](https://github.com/OpenTLV/opentlv-rust/issues/45) on
2026-11-03, which also prepares the `0.2.2` patch release after the interrupted
[`v0.2.1` release](#v021). **Nothing was published, no crates.io credential was
used, and the OIDC-only workflow has not run on GitHub.** The [procedure](../packaging.md#cratesio-publication) describes the
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
| `upload_plan` | `0.2.1`, never published | `opentlv-sys`, then `opentlv`, with `trusted` |
| `upload_plan` | `opentlv-sys` and a free name | Rejected: *does not exist on crates.io* |
| `published` | `opentlv-sys` `0.2.0` | Index and download serve the tested digest |

## Local validation

| Command | Result |
| --- | --- |
| `python -B -m unittest discover -s tests` | 179 tests passed, 41 of them in `tests/test_crates_io.py` and 21 in `tests/test_release.py` |
| `python -B scripts/check_quality.py` | Passed, REUSE compliant with 174/174 files |
| `actionlint` 1.7.7 | Passed |

These ran on Windows 11 with Python 3.11; the hosted source quality job uses Ubuntu
and Python 3.12.

`tests/test_crates_io.py` covers new versions of existing crates, already
published and immutable versions, a missing crate with no token fallback, a plan
that is not `trusted`, a missing credential, and `id-token: write` in the
`publish` job only. It mocks crates.io, GitHub and Cargo.

## v0.2.1

PR #47 prepared `0.2.1`, and its PR and Main CI passed. `Rust Release`
[run 41606751726](https://github.com/OpenTLV/opentlv-rust/actions/runs/41606751726)
passed Full CI, with the platform matrix, source quality, CodeQL and Core API
coverage, and assembled and attested the release. Its `publish` job then:

1. created the tag `v0.2.1` at `04227084765a369012fede8269c2cbe8f83a0e05`;
2. created the draft release `432479334` and uploaded all five assets;
3. stopped with `ValueError: Missing or duplicate draft release`, because the release
   list it read immediately afterwards did not contain the new draft.

Nothing was published on GitHub or crates.io. Before this fix, a new dispatch would
have stopped at *Tag already exists*, and the fix itself moves `main` past the tag,
so `v0.2.1` cannot be finished. Its disposition:

- The tag `v0.2.1` stays at `0422708`. It is never moved or reused, and
  `release_state` rejects it as a tag at another commit.
- The draft `432479334` stays unpublished and unchanged. A maintainer may delete it
  after recording its asset digests here; nothing in this repository deletes it.
- The next version is `0.2.2`, with the same crate contents apart from the version.

## Release publication fix

`scripts/release.py publish` no longer depends on the release list at the moment
of creation, and a failed publication can be finished:

- After `gh release create`, `find_draft` reads the release list every 10 seconds for
  up to five minutes until exactly one release has the tag. A duplicate or a
  non-draft fails at once. The draft is then published by its release ID, with
  `PATCH /releases/{id}`, instead of `gh release edit` looking it up by tag.
- `release_state` replaces `unused_tag` in `prepare` and before the first write.
  It accepts an unused tag, or the tag of this commit with no release or one draft,
  and the request records which. A tag at another commit, a published release or
  prerelease, several releases, or a release without its tag stop the run.
- For the tag alone, the run creates the draft. For an existing draft, it
  downloads the five assets by asset ID and publishes the draft unchanged only if
  the listing, attestations, own Full evidence and gate, tag, version, definition
  hashes, Core lock, notes and crate digests match this run. Otherwise it stops
  before any write.

`tests/test_release.py` covers the delayed listing, a duplicate, a missing draft, an
existing tag without a release, a matching draft, a partial draft and drafts with
other notes, digests, crate bytes or commit, a changed state between request and
publication, and every rejected state.

## Release 0.2.2

Both crates and `Cargo.lock` are `0.2.2`, and `opentlv` requires exactly
`opentlv-sys` `0.2.2`; the Core lock is unchanged. `release.version_for('v0.2.2')`
accepts the real manifests, lock and release notes. These ran on WSL Ubuntu 24.04 with
Rust 1.99.1 and the local `gh` stand-in of the
[registry build validation](registry-builds.md#method), from the uncommitted tree:

| Command | Result |
| --- | --- |
| `scripts/ci.py build --tier pr --configuration linux-stable` | Passed in 732 s: 300 and 277 tests, 10 examples, 171 Query fixtures with 2,133 native executions, attested `verified-source` Core `0.8.0` |
| `scripts/ci.py build --tier main --configuration linux-stable` | Passed in 83 s |
| `cargo package --workspace --locked --no-verify` | Both archives have the file lists of the published `0.2.0` archives, 33 and 77 files |

Compared with the `0.2.0` archives downloaded from crates.io, the `0.2.2` archives
differ only in the version in `Cargo.toml`, `Cargo.toml.orig` and `Cargo.lock`, the
`opentlv-sys` checksum in the `opentlv` lock, `.cargo_vcs_info.json`, and the new
`0.2.2` row of the Core table in the `opentlv-sys` README. Every source, build script
and Core selection file is identical, so the public API, features and linking are
unchanged. The release archives come from Full CI on the release commit, so these
local digests are not release evidence.

The tiers ran before two edits that change nothing they execute: a docstring in
`scripts/release.py`, and the restored closed `0.2.1` changelog section.

Full CI, including the Linux, Windows and macOS consumer builds, was not run
locally.

## Not verified

- A real publication through GitHub OIDC from a `workflow_dispatch` run. It waits
  for the publication of `0.2.2`, a real patch release; no release is made only to
  test it.
- The rejection of an API token by *Require trusted publishing for all new
  versions*.
- The `auth` step against the configured trusted publishers.
- `find_draft`, the asset download and the publication by release ID against
  GitHub; the tests mock the GitHub CLI.

## Open steps

Each is made by a maintainer after review, in this order. Issue #45 closes only
after step 5:

1. Merge this change, with the changelog closed under `## [0.2.2] - 2026-11-03`. If
   the release happens on a later day, correct the date before merging.
2. Wait for Rust Main CI on that commit, and keep `main` there until the publication.
3. Dispatch `Rust Release` with `tag: v0.2.2`. It runs Full CI on the release
   commit, then attests and publishes the GitHub release. If it stops after
   creating the tag or draft, dispatch it again with the same tag; see
   [interrupted publication](../packaging.md#interrupted-publication).
4. Dispatch `Rust crates.io` with `tag: v0.2.2`, review the plan, which must show
   `auth: trusted`, and approve the `crates-io` environment.
5. Record below the release and workflow run links, the `auth` step result, the
   published digests, the `verify-registry` and `verify-consumers` results, and the
   crates.io and docs.rs URLs.

## Publication

Not done yet.
