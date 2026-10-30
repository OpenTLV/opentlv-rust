# crates.io publication validation

Validation for [issue #39](https://github.com/OpenTLV/opentlv-rust/issues/39) on
2026-10-30. **Nothing was published, no crates.io credential was used and the
workflow has not run on GitHub.** The [procedure](../packaging.md#cratesio-publication)
describes the workflow.

## Registry support

Checked against the source of the
[crates.io Trusted Publishing documentation](https://crates.io/docs/trusted-publishing),
last changed on 2026-10-02:

| Question | Answer | Consequence here |
| --- | --- | --- |
| Is GitHub OIDC supported? | Yes, through `rust-lang/crates-io-auth-action`, with 30-minute tokens | Used once both crates exist, pinned to `v1.0.6` at `3268eb41` |
| Can it publish a new crate? | No: "initial publish requires an API token" | The first publication needs the `CRATES_IO_BOOTSTRAP_TOKEN` environment secret, which is removed afterwards |
| What does it trust? | Owner, repository, workflow filename and optional environment | One dedicated workflow file and the `crates-io` environment |
| Can tokens and OIDC coexist? | Yes, during a transition | The workflow still selects exactly one for each run |

## Package reproduction

`cargo publish` always packs the crate itself, so the tested archive cannot be
uploaded directly. These commands ran with Cargo 1.99.1 on WSL Ubuntu 24.04, from a
new clone at the `v0.1.0` tag, and were compared with the
[released assets](crates-io.md#release-archives):

| Command | `opentlv-sys` | `opentlv` |
| --- | --- | --- |
| Released assets | `16522a9f...` | `4723697a...` |
| `cargo publish --workspace --dry-run --no-verify --locked` | Identical | Identical |
| `cargo publish -p opentlv-sys --dry-run --no-verify --locked` | Identical | |
| `cargo package --workspace --no-verify --locked` | Identical | Identical |
| `cargo package -p opentlv-sys --no-verify --locked` | Identical | |
| `cargo package -p opentlv --no-verify --locked`, with `opentlv-sys` served from a local registry that replaces crates.io | | Identical |

The last row is the state after the first upload. `cargo publish -p opentlv`
itself cannot run against the local registry, because Cargo refuses to publish
through a replaced crates.io, and before the first upload it stops with
`no matching package named opentlv-sys found`. This is why the workflow checks
with `cargo package` before each `cargo publish`, and checks the index entry and
the download afterwards. `cargo package --workspace` with a replaced crates.io
stops with the same refusal, so a retry packs only the crates still missing.

## Checks on real data

The publisher's functions ran read-only against public services, with only the
GitHub CLI call replaced by the public API:

| Check | Input | Result |
| --- | --- | --- |
| `owners`, `index_entry` | `opentlv`, `opentlv-sys`, `opentlv_sys` on crates.io | No owners, no version: all free |
| `upload_plan` | Both crates on crates.io | `opentlv-sys`, then `opentlv`, with `bootstrap` |
| `owners`, `index_entry`, `published` | `serde` 1.0.0 | Formats read; the served bytes accepted, and a wrong digest rejected |
| `protected_environment` | `luarocks` in `opentlv-lua`, the configuration to copy | Accepted |
| `protected_environment` | `release` in `astral-sh/uv`, which has no reviewer | Rejected: *must require a reviewer* |
| `approved_by` | A real approved run of `pypa/pip`, environment `PyPI` | Returned the reviewer; rejected for any other environment |
| `verified_release` | `v0.1.0` from the `main` tip | Rejected: *tag is not at the current main commit* |
| `verified_release` | `v0.1.0` at its tag commit | Rejected: *Main moved* |
| `check_gate` | The Full run of `v0.1.0` | Accepted |
| `verify_contents` | The `v0.1.0` assets | Rejected: *Configuration differs*, because that evidence has no `consumer` check |

The `v0.1.0` release also showed a defect that the mocked tests had not. Its page
text was edited after publication, with generated notes and CRLF line endings, so
comparing it with `ci/release-notes.md` rejected a valid release. The publisher
therefore compares only the release state and assets; the attested
`release-notes.md` must still equal the checkout.

## Local validation

| Command | Result |
| --- | --- |
| `python -B -m unittest discover -s tests` | 153 tests passed, 40 of them in `tests/test_crates_io.py` |
| `python -B scripts/check_quality.py` | Passed, REUSE compliant with 170/170 files |
| `actionlint` 1.7.7 | Passed |
| `scripts/ci.py build --tier pr --configuration linux-stable` | Passed in 585 s: 299 and 276 tests, 171 Query fixtures with 2,133 native executions, `verified-source` |
| `scripts/ci.py build --tier main --configuration linux-stable` | Passed in 70 s |

`tests/test_crates_io.py` mocks crates.io, GitHub and Cargo. The evidence checks
run on a really assembled release payload.

The PR and Main tiers ran on WSL Ubuntu 24.04 with the local `gh` stand-in of the
[registry build validation](registry-builds.md#method), before the final split of
the release page check. That split changes neither the crates nor anything these
tiers execute. Full CI was not run locally.

## Not verified

Only the first hosted run can prove these, and issue
[#40](https://github.com/OpenTLV/opentlv-rust/issues/40) owns the publication:

- The workflow on GitHub, including the environment approval, the token exchange
  and the permissions of the job's `GITHUB_TOKEN` for the environment and approval
  endpoints.
- A real `cargo publish`, the crates.io server checks, and `cargo package -p opentlv`
  against the real registry.
- Trusted Publishing from a `workflow_dispatch` run. The crates.io documentation
  only shows a tag push, and names no restriction.
- A `publish-new` token limited to the two names.
- A positive `verified_release` from start to finish on a real release: no release
  can pass it yet, because none is at the `main` tip with `consumer` evidence.

## Open before the first publication

This issue does not resolve the
[remaining prerequisites](../packaging.md#remaining-cratesio-prerequisites), which
the workflow cannot detect: the documentation that still says the crates are
unpublished, docs.rs, and the GitHub login that a default build needs. The
environment and the token have not been configured. Without the environment, the
`prepare` job fails and nothing can be published.
