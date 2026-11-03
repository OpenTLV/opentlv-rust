# Packaging and versioning

The two crates of this repository are prepared as independent Cargo packages
([issue #4](https://github.com/OpenTLV/opentlv-rust/issues/4)). The first
GitHub release was `v0.1.0` against Core `v0.8.0` (#33), and `0.2.0` is the first
version for crates.io: [registry publication](#cratesio-publication) is a
separate, approved step. The
[packaging validation](validation/packaging.md) records the local results.

## Package identities

| Crate | Role | Native metadata | Categories |
| --- | --- | --- | --- |
| `opentlv-sys` | Raw C FFI declarations, Core selection and native linking | `links = "tlv"`, `build.rs` | `external-ffi-bindings` |
| `opentlv` | Safe Rust API | `build.rs`, which only forwards the selected Core tree to the tests | `parser-implementations`, `encoding`, `api-bindings` |

The crate names, descriptions, `links` value, Cargo features and their defaults
are unchanged from the [migration baseline](migration.md#crates-and-versions);
the `opentlv` build script was added by the
[standalone build](validation/standalone-build.md).

Both crates inherit this metadata from `[workspace.package]` in
the root `Cargo.toml`:

| Field | Value |
| --- | --- |
| `version` | `0.2.0` |
| `edition` | `2021` |
| `rust-version` | `1.82` |
| `license` | `MIT` |
| `repository` | `https://github.com/OpenTLV/opentlv-rust` |
| `keywords` | `tlv`, `ber`, `asn1`, `emv`, `parser` |

Each crate has its own `README.md`, which Cargo uses as the package readme,
and a copy of the repository `LICENSE`, so that every archive carries the MIT
license text. Both copies must equal the root `LICENSE`.

## Versioning

The Rust bindings have their own version, independent of OpenTLV Core:

- The first standalone Rust release is `0.1.0`, and `0.2.0` is the first version
  meant for crates.io. The inherited `0.7.1` was
  an unpublished migration baseline, not a previous Rust release. There were
  no Rust repository tags or GitHub releases when checked on 2026-10-27.
  The Rust version is independent of Core's `0.8.0` dependency version.
- Both crates always have the same version, the single
  `[workspace.package]` `version`, and are released together. `Cargo.lock`
  records it for both.
- Versions follow [SemVer](https://semver.org/) as Cargo applies it. Before
  1.0, a breaking change of either crate's public API, Cargo features, native
  linking contract or minimum supported Rust version increments the minor
  version; any other change increments the patch version.
- Changing the Core lock in `core/core-artifact.json` always produces a new
  version, because the lock determines the Core that each build of a
  published version uses. It increments the minor version when the Core
  change breaks one of the contracts above.

### Version relationship

`opentlv` depends on exactly the `opentlv-sys` version it is released with:

```toml
opentlv-sys = { version = "=0.2.0", path = "../opentlv-sys", default-features = false }
```

The exact requirement is deliberate. `links = "tlv"` permits only one
`opentlv-sys` in a dependency graph, the CI tests the two crates only as a
pair, and the `opentlv-sys` version identifies the Core dependency, so a
caret requirement would allow untested combinations with a different Core.
Inside the workspace, Cargo uses the `path`; in the package, Cargo removes the
`path` and keeps the version requirement.

## Core compatibility

Each `opentlv-sys` version pins exactly one Core dependency, recorded in
[`core/core-artifact.json`](../opentlv-sys/core/core-artifact.json),
which is part of the package:

| Crates | Core version | Core commit | Core release |
| --- | --- | --- | --- |
| 0.1.0 | 0.8.0 | `345c77acb29266036df5f27d04ce315bb61853cc` | v0.8.0 |
| 0.2.0 | 0.8.0 | `345c77acb29266036df5f27d04ce315bb61853cc` | v0.8.0 |

The packages do not contain Core. A build from an archive selects Core exactly
as a build from the repository does, as described in the
[README](../README.md#core-dependency): it authenticates the locked Core release
through the digest in the lock, and through its attestations with a logged-in GitHub
CLI, and selects its compatible static binary or builds its verified source.
`opentlv::version()` reports the
version of the Core library actually linked at run time, which can differ from
the pinned one when `OPENTLV_LIB_DIR` links a prebuilt library.

## Package contents

Each `.crate` archive contains exactly the crate's tracked files plus Cargo's
generated `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock` and
`.cargo_vcs_info.json`:

| Crate | Files |
| --- | --- |
| `opentlv-sys` | `README.md`, `LICENSE`, `build.rs`, `src/`, the native integration files in `core/` (lock, selection module, release verifier and CMake toolchain probe) and the tests with the C ABI probes in `tests/abi_probe/`, `tests/codec_abi_probe/` and `tests/compose_abi_probe/` |
| `opentlv` | `README.md`, `LICENSE`, `build.rs`, `src/`, `tests/` and the eleven examples |

The build scripts and tests read only files inside their own package, the
Cargo `OUT_DIR`, the selected Core tree and their own temporary directories.
The only local path in a manifest
is the declared `opentlv-sys` workspace dependency, which carries a version.

## Validation

The `package` check of [`scripts/ci.py`](../scripts/ci.py), part of the
stable [CI configurations](quality-gates.md), runs
`cargo package --workspace --locked`, which builds both crates from their
archives. It requires the exact file lists above, a `LICENSE` equal to the
repository license and, in each archive's normalized `Cargo.toml`, the crate
name, the workspace version, `MIT`, the `README.md` readme, this repository
URL, the registry identity in `scripts/ci.py` (description, keywords, categories,
edition, `rust-version`, `links`, build script, features and no `publish`
restriction), no dependency `path` and the dependency
`opentlv-sys = "=<version>"` for `opentlv` only.
[`tests/test_packaging.py`](../tests/test_packaging.py) checks
the manifests, the shared version, the unchanged features against the
migration baseline, the license copies and the Core compatibility table
without building.

The `consumer` check then builds a new project against those exact archives for
each feature set and for `opentlv-sys` alone, through a local Cargo registry that
replaces crates.io and without a GitHub login; see the
[quality gates](quality-gates.md#checks) and the
[registry build validation](validation/registry-builds.md). To run it on any two
archives, for example those of a release:

```sh
python -B scripts/registry_consumer.py --packages <directory with both .crate files> \
  --work build/consumers --output build/consumers.json
```

The target directories are `<work>/<n>`, so on Windows `--work` must fit the
[path limit](#consumer-requirements).

To inspect the packages locally, from the repository root:

```sh
cargo metadata --format-version 1 --no-deps
cargo package -p opentlv-sys --list
cargo package -p opentlv --list
cargo package --workspace --allow-dirty
```

`cargo package -p opentlv` alone fails, even with `--no-verify`, while
`opentlv-sys` is not on crates.io: Cargo resolves an unpublished dependency
from a local archive only for crates packaged in the same invocation. Use
`--workspace`, as CI does.

## Licensing

Both crates declare the SPDX expression `MIT` in `license` and carry a
`LICENSE` copy equal to the root `LICENSE`, which names Marek Cingel as the
copyright holder. Packaged Rust, Python and CMake sources keep their SPDX
headers. The repository's `REUSE.toml` and `LICENSES/` are not packaged: in an
archive, the crate `LICENSE` covers the files without a header, which are the
README, the Core lock, the C ABI probe's `CMakeLists.txt` imported from Core
and Cargo's generated files. Neither crate has Cargo dependencies beyond `opentlv-sys`, and no
third-party code is vendored.

The archives contain no Core sources. `opentlv-sys` authenticates the pinned Core
release at build time, or links a library selected with `OPENTLV_LIB_DIR`, and
the resulting `tlv` library becomes part of the Rust binary. At the pinned
commit, Core's `LICENSE` is byte-identical to this repository's and its
library sources carry only `MIT` and `Copyright (c) 2026 Marek Cingel`
notices. A binary distribution therefore needs that one MIT notice; recheck
Core's licensing notices when changing the pinned commit, and add the license
of every new bundled or linked dependency here and to the crate archives.

## GitHub release

The manual `Rust Release` workflow accepts an unused `vMAJOR.MINOR.PATCH` tag
matching both Cargo manifests, `Cargo.lock` and `ci/release-notes.md`.
Changes stay under `Unreleased` during development; close the changelog under
the matching version without the tag's `v` prefix and the release date
before dispatching the release (for example, `## [0.1.0] - 2026-10-28`). The
versioned release notes are the publication text and are included in the
hashed CI definitions. No crates.io credentials or uploads are involved.

1. Review and merge the release preparation. Wait for successful Rust Main CI
   on that exact commit. Dispatch `Rust Release` on `main` with `tag: v0.2.0`.
2. The workflow rejects a foreign, dirty or non-main checkout, a changed main
   tip, an existing tag/release, wrong versions or missing Main evidence.
   It runs Full CI with `require_verified_core: true`: all seven configurations,
   source quality, CodeQL, API coverage, packaging and the aggregate gate.
3. Assembly revalidates every record against the current commit, run, attempt,
   Core lock and source hashes. It copies the exact two Linux stable packages
   whose digests the Full record contains. It does not rebuild them.
4. The workflow attests all five assets: `opentlv-sys-0.2.0.crate`,
   `opentlv-0.2.0.crate`, `release-manifest.json`, `release-notes.md` and
   `SHA256SUMS`. Publication rechecks evidence, checksums, attestations and
   unused tag status, then creates the tag and a draft release. Only after the
   uploaded asset names, sizes, SHA-256 digests, notes and tag commit match
   does it publish the draft and verify the public result.
5. Record the published URL and exact commit, Main and Release/Full run links
   in [issue #33](https://github.com/OpenTLV/opentlv-rust/issues/33) and
   [Core epic #718](https://github.com/OpenTLV/opentlv/issues/718). Close the
   tracking only after publication and asset verification. The workflow emits
   `rust-published-<attempt>` with the URL, source and asset identities.

If a failure occurs after creating a tag or draft, inspect that state before
retrying; the workflow deliberately refuses to overwrite either. Earlier
attempts' artifacts cannot satisfy a new attempt. A new main commit or a new
Main run requires a new release attempt with matching evidence.

To use GitHub release archives instead of crates.io, extract both `.crate`
files into adjacent directories, then use a Cargo path dependency and patch
the exact registry dependency back to the extracted raw crate:

```toml
[dependencies]
opentlv = { path = "vendor/opentlv-0.2.0" }

[patch.crates-io]
opentlv-sys = { path = "vendor/opentlv-sys-0.2.0" }
```

The normal Core build requirements still apply. Rust's crate version is
`0.2.0`; `opentlv::version()` should report linked Core `0.8.0`.

### Local preparation evidence (2026-10-27)

Windows/MSVC with Rust 1.99.1 passed the exact test inventory in all five
feature configurations: default 299, none 276, LLDP 277, NFC 276 and
Document 298, including the C ABI probes and compile-fail lifetime tests.
rustfmt, Clippy with `-D warnings` for default and no-default features,
Rustdoc with `-D warnings`, all ten examples, 171 Query fixtures with 2,133
native executions, and `cargo package --workspace --locked --allow-dirty`
passed. Both archives were built from their packaged contents; the package
inventory, normalized manifests, licenses and crate digests were checked.
All 100 infrastructure tests, actionlint and source quality/REUSE passed.

Core API coverage remains 215/215 generic symbols with 82 exclusions;
the Core pin diff has no added or removed public symbols. The actual Core
v0.8.0 manifest, `SHA256SUMS` and source archive were verified against their
saved Sigstore bundles, enforcing the Core release signer and exact source
commit. Their checksums, source identity and recorded configuration passed
the Rust verifier. The 431 Core build and corpus files compared with the
clean pinned checkout matched after line-ending normalization.

Cargo used the development-only `OPENTLV_CORE_SOURCE` override to that exact
clean Core commit. The GitHub CLI was not authenticated for the default online
resolver; separate artifact verification does not turn those Cargo runs into
verified-mode or hosted evidence. Linux, macOS, MSRV, sanitizer, prebuilt-linking
Full jobs, release attestations and publication still require the hosted
release run on the merged commit. Local logs and the machine-readable result
are under `build/issue33-*`; they are not committed release evidence.

The subsequent [Full run 40559017343, attempt 1](https://github.com/OpenTLV/opentlv-rust/actions/runs/40559017343/attempts/1)
on `df4faeb87c923a042944977eea5e96cf2f0017b5` failed all seven build
configurations: a second Verified Source resolution changed the extracted
source path while reusing the first CMake cache. The follow-up gives each
resolution its own build directory. A regression reproduced the original
failure and passed after the fix; the 101-test infrastructure suite, Clippy,
rustfmt and source quality/REUSE passed locally. Two successive builds also
passed with the actual Core v0.8.0 source archive and an offline acquisition
fixture. These checks cover compilation and reruns, not online authentication
or hosted acceptance; a new Full run on the fix commit is still required.

## crates.io publication

The manual `Rust crates.io` workflow,
[`rust-crates-io.yml`](../.github/workflows/rust-crates-io.yml), publishes the
crate pair of an existing GitHub release. It is the only workflow that can obtain a
registry credential, and only through Trusted Publishing; `Rust Release` holds none.
It has no pull request, push or tag trigger. The
[publication validation](validation/crates-io-publication.md) and the
[Trusted Publishing validation](validation/trusted-publishing.md) record what has
and has not been exercised.

### Procedure

1. Finish the [GitHub release](#github-release) of the new version, and keep `main`
   at its commit: any later commit, even a documentation change, requires a new
   version.
2. Check that the [one-time configuration](#one-time-configuration) is unchanged and
   review the [consumer requirements](#consumer-requirements). The workflow cannot
   check the crates.io settings or these decisions for you.
3. Dispatch `Rust crates.io` on `main` with the release tag. The `prepare` job runs
   every check below without a credential and uploads
   `rust-crates-io-plan-<attempt>`, which lists the crates, digests and credential
   of this publication.
4. Review that plan, then approve the `crates-io` environment. After approval the
   `publish` job repeats every check, because an approval can come much later.
5. It uploads `opentlv-sys`, waits until crates.io serves it, and only then uploads
   `opentlv`. It emits `rust-crates-io-<attempt>` with the plan, the approving
   reviewers and the published URLs.
6. The [verification jobs](#verification-after-publication) check the published
   versions as a new user sees them. Record the result in the
   [release validation](validation/crates-io-release.md).

### What the workflow checks

Before any upload, `scripts/crates_io.py prepare` requires:

- a manual dispatch of this workflow from `main` on the official clean checkout,
  with `main` and the tag both at that commit;
- an environment with a required reviewer, no administrator bypass and only `main`
  as its deployment branch. A job that references a missing environment would
  create it without protection, so `prepare` runs first and stops the run;
- in the `publish` job, a recorded approval of this run for that environment;
- the public, non-draft GitHub release: its asset names, sizes and digests, the
  attestation of every asset by `rust-release.yml` at this commit, and its
  `SHA256SUMS`. The text on the release page is not compared, because maintainers
  can edit it; the attested `release-notes.md` is;
- the release's complete Full evidence and attempt-bound quality gate, including
  the [consumer builds](#validation) of the same archive digests, with Main CI
  successful on this commit and the definition hashes, Core lock and release notes
  equal to this checkout;
- both crates on crates.io with `marekcingel` among their owners. Trusted
  Publishing cannot create a crate, so a missing crate, like a name taken by someone
  else, stops the publication before the first upload;
- for each version already on crates.io, the tested digest and no yank.

Cargo cannot upload an existing archive: `cargo publish` packs the crate again.
The published bytes stay the tested ones because packaging is reproducible, which
the [validation](validation/crates-io-publication.md#package-reproduction) measured
on the `v0.1.0` assets. Each `cargo package` must produce the tested digest
before that crate's `cargo publish` runs. Both use the Linux stable toolchain that
built the archives, with `--locked` and `--no-verify`: the archives were tested in
Full CI, and no build script runs near a credential. Afterwards, the index entry and
the downloaded archive must both have the tested digest.

### Verification after publication

[`scripts/verify_published.py`](../scripts/verify_published.py) runs in two jobs
after `publish`. They hold no credential and no GitHub token:

- `verify-registry` requires the tested digests in the index and the download. It
  compares the crates.io metadata and owner with the
  [registry identity](#validation), and waits up to 30 minutes for docs.rs to report
  a successful documentation build of both crates.
- `verify-consumers` runs the [consumer builds](#validation) against crates.io
  itself on Linux, Windows and macOS stable. It requires the tested digests in
  `Cargo.lock`, the locked Core version and commit, and `attested: false`, which
  proves the build needed no login. Both crates are libraries, so there is no
  `cargo install` to check.

The jobs upload `published-registry.json` and one `published-consumers.json` per
platform. The registry report records verified metadata, owners, archive digests
and versioned documentation URLs. Consumer reports list the packages each project
actually resolved; the raw-crate project lists only `opentlv-sys`. A failed rerun
removes an earlier success report before verification starts.

A failure cannot change the published bytes. Re-run a job for a temporary cause, such
as a docs.rs queue. For a defect, release a new version, and yank the faulty one with
`cargo yank` if it harms users.

### Credentials

The workflow stores no registry credential. Trusted Publishing is its only way to
upload, and crates.io requires it for every new version of both crates, so no API
token could publish one either:

| Registry state | `auth` | Credential |
| --- | --- | --- |
| Both crates exist, and this version is missing from at least one | `trusted` | A 30-minute token from [`rust-lang/crates-io-auth-action`](https://github.com/rust-lang/crates-io-auth-action), exchanged for the job's GitHub OIDC token and revoked at the end of the job |
| This version is already complete | `none` | None; the job only confirms the published bytes |
| Either crate does not exist | | None: `prepare` fails before any upload, because Trusted Publishing cannot create a crate |

Only the `publish` job has `id-token: write`, and it starts only after the approval.
Only the upload step receives the registry token, without `GH_TOKEN`, and only the
`cargo publish` call receives it from that step. `scripts/crates_io.py upload`
refuses a plan whose `auth` is not `trusted`, and a missing token, before it
installs the toolchain or runs Cargo.

The first versions, `0.2.0`, were published on 2026-11-01 with a `publish-new` API
token, because crates.io requires one for a crate's first version. Issue
[#45](https://github.com/OpenTLV/opentlv-rust/issues/45) revoked that token, deleted
its environment secret and removed its path from the workflow.

### Retry and immutability

A published version cannot be changed. The workflow never replaces one: a version
already on crates.io with other bytes, or yanked, fails with *release a new version*.
To finish an interrupted publication, re-run the job or dispatch again while `main`
is still at the release commit. Crates that already have the tested bytes are
confirmed, and only the missing ones are uploaded. If `main` has moved, publish
a new version of both crates, because `opentlv` requires exactly its `opentlv-sys`
version.

The `v0.1.0` archives cannot pass this workflow: its tag is not at the current
`main` commit, and its Full evidence predates the `consumer` check.

### Recovery

Every failure below stops before an upload or leaves the published bytes as they
are. Fix the cause, then re-run the `publish` job, which needs a new approval, or
dispatch again while `main` is still at the release commit. Never store a crates.io
API token in the repository or the environment to get past a failure: the crates
require Trusted Publishing, and a stored token would outlive the publication it
served.

| Failure | Cause | Recovery |
| --- | --- | --- |
| `prepare` stops: *does not exist on crates.io* | A crate in `CRATES` has no crates.io entry: a renamed or new crate, or a deleted one | Nothing was uploaded. Trusted Publishing cannot create a crate, so a new crate name is a separate decision with its own issue, not a retry |
| `prepare` stops: *not owned by `marekcingel`* | The crate's owners changed | Restore the owner, or change `OWNER` in a reviewed commit, which needs a new version |
| `prepare` stops on the environment | Its reviewer, administrator bypass or branch rule changed | Restore the [configuration](#one-time-configuration). Never weaken it to let a run through |
| `publish` waits for approval | The required reviewer has not approved | Review the plan artifact, then approve `crates-io`. A rejected or expired approval publishes nothing; dispatch again |
| The `auth` step fails | The trusted publisher does not match the run: owner, repository, workflow file name or environment differ, or a crate lacks a publisher | Compare each crate's trusted publisher on crates.io with `OpenTLV`, `opentlv-rust`, `rust-crates-io.yml` and `crates-io`. Correct the side that changed, in both crates when a name in the workflow changes, then re-run |
| `upload` stops: *Missing crates.io credential*, or `cargo publish` reports an authentication error | The OIDC exchange produced no token, `id-token: write` is missing, or crates.io rejected or expired the 30-minute token | Re-run the `publish` job, which requests a new OIDC token. If the error stays, check the publisher identity as above and the [crates.io status](https://status.crates.io) |
| `upload` or `confirm` stops: *did not appear on crates.io* | The index or the download did not serve the version within 10 minutes | The upload may have succeeded. Re-run: `prepare` confirms a version that crates.io now serves with the tested bytes and uploads only what is missing. A delay never needs a new version |
| Only `opentlv-sys` is published | A failure between the two uploads | Re-run or dispatch again while `main` is at the release commit; only `opentlv` is uploaded. If `main` has moved, publish a new version of both crates, and yank the lone `opentlv-sys` version only if it harms users |
| *other bytes* or *yanked* | A version on crates.io differs from the tested archive | crates.io versions are immutable: release a new version |

### One-time configuration

These settings live outside this repository. A maintainer made them on 2026-11-03,
and the [Trusted Publishing validation](validation/trusted-publishing.md) records
them:

1. The repository environment `crates-io`, configured as `luarocks` in
   `opentlv-lua`: the required reviewer `marekcingel`, no administrator bypass and
   deployment limited to the branch `main`. It holds no secrets.
2. A GitHub trusted publisher in the crates.io settings of each crate: owner
   `OpenTLV`, repository `opentlv-rust`, workflow `rust-crates-io.yml`, environment
   `crates-io`. Renaming any of them breaks the publisher until both crates are
   changed with it.
3. *Require trusted publishing for all new versions* is enabled on both crates.
4. The `0.2.0` bootstrap token is revoked, and the `marekcingel` crates.io account
   has no other API token.

Further crate owners, such as an OpenTLV GitHub team, can be added; `marekcingel`
must stay an owner, or `OWNER` in `scripts/crates_io.py` must change with it.

## Consumer requirements

What a crates.io user needs, as the
[release validation](validation/crates-io-release.md) measured it:

- **Core availability.** A build needs CMake, a C99 compiler, Python 3.11 and
  network access to GitHub, even when `OPENTLV_LIB_DIR` links a prebuilt library.
  It needs neither Git nor a GitHub login. The locked Core v0.8.0 release assets
  must remain available with the same bytes, because a published version cannot
  change its lock. A compatible Verified Binary avoids compiling Core; otherwise
  the verified source is built locally.
- **Authentication.** Without a logged-in GitHub CLI, the `SHA256SUMS` digest in the
  lock authenticates the Core assets, and it is as trustworthy as the published
  crate that contains it. With a logged-in GitHub CLI, the build verifies the
  attestations as well, and a failure stops it.
- **Windows paths.** With the Visual Studio generator, the Cargo target directory
  can have at most 76 characters, or 74 with `--release`. A short
  `CARGO_TARGET_DIR` works around it; a deep project otherwise fails with an
  MSBuild error.
- **Targets.** `build.rs` gives CMake no target, so cross compilation is not
  supported.
- **Documentation.** docs.rs builds without network access, so `DOCS_RS` selects no
  Core, and only Rustdoc runs there.
