# OpenTLV Rust v0.2.0

First Rust release for crates.io: `opentlv-sys` and `opentlv` are versioned
together as **0.2.0**, independently of Core. The public API is unchanged from
0.1.0, which remains a GitHub release only.

- Pins OpenTLV Core **v0.8.0** at
  `345c77acb29266036df5f27d04ce315bb61853cc`. A build authenticates the release
  through the SHA-256 of its `SHA256SUMS` in the lock, so it no longer needs a
  GitHub login; with a logged-in GitHub CLI it verifies the attestations as well.
- Requires Rust 1.82 or newer, CMake, a C99 compiler, Python 3.11 or newer and
  network access to GitHub. Windows uses MSVC, with at most 76 characters in the
  Cargo target directory, or 74 with `--release`.
- Builds its documentation on docs.rs, without Core.
- Distributes the two tested `.crate` archives, release evidence and SHA-256
  checksums through GitHub Releases. These archives contain Rust sources and
  build scripts; Core is resolved when building. See
  [packaging](https://github.com/OpenTLV/opentlv-rust/blob/v0.2.0/docs/packaging.md).

The crates.io publication of these same archives is a separate, approved step.
Cross compilation is not supported. Windows
shared linking remains unsupported; the release tests static linking there.
Generic Core symbol coverage is complete, but symbol references do not prove
functional parity and Builtins-specific parity remains outside this release.
