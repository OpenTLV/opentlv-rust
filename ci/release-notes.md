# OpenTLV Rust v0.2.2

Patch release: `opentlv-sys` and `opentlv` are versioned together as **0.2.2**,
independently of Core. The public API, Cargo features and native linking are
unchanged from 0.2.0. Version 0.2.1 was tagged but never published: its GitHub
release stopped before publication, and 0.2.2 contains its changes.

- Is the first version published to crates.io only through Trusted Publishing:
  after approval of the protected `crates-io` environment, the `Rust crates.io`
  workflow exchanges a GitHub OIDC token for a short-lived crates.io token. No
  stored API token exists for either crate, and crates.io requires Trusted
  Publishing for every new version.
- Finishes an interrupted GitHub release safely: publication waits for a new
  draft to appear, publishes it by its release ID, and a new run of the same
  commit publishes an existing draft only after verifying its assets, attestations
  and evidence, without overwriting anything.
- Pins OpenTLV Core **v0.8.0** at
  `345c77acb29266036df5f27d04ce315bb61853cc`, unchanged. A build authenticates
  the release through the SHA-256 of its `SHA256SUMS` in the lock, so it needs no
  GitHub login; with a logged-in GitHub CLI it verifies the attestations as well.
- Requires Rust 1.82 or newer, CMake, a C99 compiler, Python 3.11 or newer and
  network access to GitHub. Windows uses MSVC, with at most 76 characters in the
  Cargo target directory, or 74 with `--release`.
- Distributes the two tested `.crate` archives, release evidence and SHA-256
  checksums through GitHub Releases. These archives contain Rust sources and
  build scripts; Core is resolved when building. See
  [packaging](https://github.com/OpenTLV/opentlv-rust/blob/v0.2.2/docs/packaging.md).

The crates.io publication of these same archives is a separate, approved step.
Cross compilation is not supported. Windows
shared linking remains unsupported; the release tests static linking there.
Generic Core symbol coverage is complete, but symbol references do not prove
functional parity and Builtins-specific parity remains outside this release.
