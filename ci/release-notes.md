# OpenTLV Rust v0.1.0

First standalone Rust release: `opentlv-sys` and `opentlv` are versioned
together as **0.1.0**, independently of Core. The inherited, unpublished
0.7.1 migration version is not a previous Rust release.

- Pins OpenTLV Core **v0.8.0** at
  `345c77acb29266036df5f27d04ce315bb61853cc` and requires its attested release
  artifacts for default builds.
- Includes the safe Reader/Writer, composed Formats, Schema, Document, Query,
  generic Codec, StructureCodec and deterministic Generator APIs. The `lldp`,
  `nfc` and `document` features remain enabled by default.
- Requires Rust 1.82 or newer, CMake, a C99 compiler, Python 3.11 or newer and
  an authenticated GitHub CLI for Core verification. Windows uses MSVC.
- Distributes the two tested `.crate` archives, release evidence and SHA-256
  checksums through GitHub Releases. These archives contain Rust sources and
  build scripts; Core is resolved when building. See
  [packaging](https://github.com/OpenTLV/opentlv-rust/blob/v0.1.0/docs/packaging.md)
  for using the pair locally.

crates.io publication and docs.rs hosting remain separate follow-ups. Windows
shared linking remains unsupported; the release tests static linking there.
Generic Core symbol coverage is complete, but symbol references do not prove
functional parity and Builtins-specific parity remains outside this release.
