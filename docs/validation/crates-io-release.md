# crates.io release validation

Validation for [issue #40](https://github.com/OpenTLV/opentlv-rust/issues/40), which
owns the first crates.io publication. **On 2026-11-01, nothing is published yet.** The
issue requires explicit approval first, so this record has the repository side and
the [steps left](#open-steps); the published results belong in
[Publication](#publication).

The public crates.io owners endpoints returned HTTP 404 for both names on
2026-11-01. This is an availability observation, not a name reservation; the
publication workflow checks again immediately before upload.

## Decisions

The [registry build findings](registry-builds.md#findings-for-the-first-publication)
needed these decisions before a version that cannot change:

| Finding | Decision |
| --- | --- |
| A default build needs a GitHub login, so a clean machine and docs.rs cannot build | The Core lock records the SHA-256 of the release's `SHA256SUMS`. A build downloads the assets from the public release URLs and accepts them by digest. A logged-in GitHub CLI also verifies the attestations, and a failure stops the build. All CI tiers still require attested Core |
| docs.rs has no network access | With `DOCS_RS`, the build script selects and links no Core, and `[package.metadata.docs.rs]` documents all features for `x86_64-unknown-linux-gnu` |
| Windows target directories are limited to 49 characters | Shorter Core build directories raise the limit to 76 |
| The documentation describes a path dependency | The crate READMEs, Rustdoc and user guide describe `cargo add opentlv` |
| Verified Binaries are not selected in practice | Unchanged: fixing it needs a new Core release. Consumers compile Core |
| Cross compilation is not supported | Unchanged, and now stated in the crate READMEs |

The version is `0.1.1`, with the public API unchanged, because the `v0.1.0` archives
[must not be published](crates-io.md#findings-that-block-publishing-these-archives).

### What the digest does and does not prove

The lock is inside the crate, and crates.io never changes a published version, so
the digest pins the exact Core assets for everyone who trusts that crate. It adds no
trust of its own: unlike an attestation, it does not show on the user's machine who
built the assets. The digest was taken from the public `SHA256SUMS`, whose listed
manifest digest was compared, and every CI tier verifies the attestations of those
same bytes.

## Consumer requirements

Measured on 2026-10-30 with these consumers, with exact toolchains as in the
[registry build validation](registry-builds.md#method):

| Requirement | Result |
| --- | --- |
| GitHub CLI and login | Not needed: Linux ran with neither `gh` nor `git` on `PATH`, Windows with no `gh` on `PATH` |
| Git | Not needed, as above |
| Network access to GitHub | Needed, except for docs.rs |
| Python, CMake, C compiler | Needed, except for docs.rs |
| Windows target directory, debug | 76 characters passed, 77 failed |
| Windows target directory, `--release` | 74 characters passed, 75 failed |

The Windows limit was measured with `LongPathsEnabled` set to 0, as in issue #38, with
MSVC and the Visual Studio 17 2022 generator.

## Local validation

The initial consumer measurements below were made on 2026-10-30 with Rust 1.99.1,
on WSL Ubuntu 24.04 and Windows 11. Repository validation was completed on
2026-11-01; its fresh results are recorded separately below.

| Check | Result |
| --- | --- |
| `cargo package --workspace --locked --allow-dirty` without a GitHub CLI, Linux and Windows | Passed: `verified-source` |
| `scripts/registry_consumer.py` with those archives, Linux and Windows | Passed: `default`, `none`, `lldp`, `nfc`, `document` and `opentlv-sys` alone, each `verified-source`, `attested: false`, linked Core 0.8.0 |
| `DOCS_RS=1 RUSTDOCFLAGS="-D warnings" cargo doc --offline --locked --no-deps -p opentlv -p opentlv-sys` of the registry packages, with neither CMake nor Python on `PATH` | Passed |
| The same without `DOCS_RS` | Failed as expected: `failed to run "cmake"` |
| `gh auth status` of the real `gh` 2.103.3 with the consumers' empty configuration | Exit 1, so the build uses the locked digest |

### Completion checks on 2026-11-01

The local `.crate` archives are working-tree candidates, not published release
artifacts. The regression tests use mocked crates.io/docs.rs responses; the live
post-publication jobs still need to run after approval.

| Check | Result |
| --- | --- |
| `python -B -m unittest discover -s tests` on Windows | Passed: 172 tests |
| `python -B scripts/check_quality.py` | Passed: syntax, workflow structure, whitespace and REUSE (173 files) |
| `cargo fmt --all --check` and actionlint | Passed |
| `cargo package --workspace --locked --allow-dirty` on Windows | Passed: both 0.1.1 archives built from their packaged sources |
| Isolated Windows registry consumers | Passed: all five feature sets and `opentlv-sys` alone; verified Core 0.8.0 with `attested: false` |
| Windows `cargo test --workspace --locked`, default and no default features | Passed: 300 and 277 tests, matching the CI inventories |
| Registry-package Rustdoc with `DOCS_RS=1`, `--offline --locked --no-deps`, and warnings denied | Passed for both crates |
| Local Linux PR tier, with the `gh` stand-in | Passed: both Clippy configurations, 300/277 tests, 10 examples, 171 Query fixtures / 2,133 native executions, Rustdoc and attested Core selections |
| Local Linux Main tier, with the `gh` stand-in | Passed: workspace build, `quick_start` and attested Core selections |

Local logs and reports are under `build/issue40-*` (ignored by Git). Tests also
cover mismatched Core identity, accidental GitHub authentication, inherited Core
overrides, missing owners, stale success reports and the shared docs.rs deadline.
Both Linux reports' definition hashes match the completed implementation.

The Windows candidate archive hashes, also checked in every consumer's lockfile:

| Candidate archive | SHA-256 |
| --- | --- |
| `opentlv-sys-0.1.1.crate` | `18c8125d5d4713d89675804d4fe6fa2186f3333b8968e8c998f7082402cb4a44` |
| `opentlv-0.1.1.crate` | `a64783735a39dd65dc7a620c9ae6dcf2b7eacc5ed53d4bf30067821e736e478a` |

These are local candidate hashes. The approved release workflow must record and
verify the hashes of the archives it actually produces on the release revision.

The PR and Main tiers use the local stand-in for `gh` of the
[registry build validation](registry-builds.md#method), which reads the public release
data and passes `attestation verify` to the real `gh` with each public Sigstore bundle.
Full CI was not run locally, and `tests/test_verify_published.py` mocks crates.io and
docs.rs.

## Not verified

Only hosted runs and the publication itself can prove these:

- Full CI with the `consumer` check on Linux, Windows and macOS, and the `Rust Release`
  and `Rust crates.io` workflows on GitHub, including everything
  [the publication validation lists](crates-io-publication.md#not-verified).
- macOS builds without a login. Python from python.org needs its certificates
  installed for the public download.
- The real docs.rs build, and `scripts/verify_published.py` against real answers. Its
  metadata fields were read from a real crates.io answer for `libz-sys`, and
  `status.json` from a real docs.rs answer for `serde`.

## Open steps

Each is made by a maintainer, in this order. Steps 3 and 5 are the approval that the
issue requires, and nothing in this repository takes them:

1. Merge this change, and close the changelog under `## [0.1.1] - <date>` in a
   following commit, as the [release procedure](../packaging.md#github-release) requires.
2. Wait for Rust Main CI on that commit, and keep `main` there until the publication.
3. Dispatch `Rust Release` with `tag: v0.1.1`.
4. Make the [one-time configuration](../packaging.md#one-time-configuration).
5. Dispatch `Rust crates.io` with `tag: v0.1.1`, review the plan and approve the
   `crates-io` environment. This claims both crate names permanently.
6. Finish the one-time configuration: add the trusted publishers, delete the secret
   and revoke the token.
7. Record the results below, in a commit after the publication, and close the issue.

## Publication

Not done yet. To record: the release and workflow run links, the published digests, the
approving reviewer, the `verify-registry` and `verify-consumers` results, and the
crates.io and docs.rs URLs.
