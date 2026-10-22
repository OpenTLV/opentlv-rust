# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- Own the Rust user and development guides as standalone manuals for the OpenTLV documentation portal: setup from an opentlv-rust checkout with Rust 1.82 and the pinned Core, all three Cargo features including `nfc`, the independent `0.7.1` crate version, the current CI, local workspace development and contribution routing, with examples linked from this repository and Core references routed to its current documentation; technical content and the frozen migration evidence are preserved. (#6)
- Move the Cargo workspace from the inherited `bindings/rust/` to the repository root, with `opentlv-sys/` and `opentlv/` as top-level crates; CI, the packaging check, tests and documentation use the new paths, and a regression test rejects the old layout in the build configuration. Crate sources, public API, Cargo features, `Cargo.lock`, native linking and the frozen migration inventory are unchanged. (#5)

### Fixed

- Point the `FixedFormatConfig` Rustdoc link to Core's memory guide in `OpenTLV/opentlv` instead of the former `marekcingel/OpenTLV` monorepository. (#6)
- Declare Rust 1.82, the oldest toolchain that compiles the safe crate, as `rust-version` instead of the never-working 1.70, and use `Option::is_none_or` where Clippy now suggests it; give `opentlv`'s `opentlv-sys` dependency a version so both crates can be packaged; repair three broken Rustdoc links in `codec.rs`. (#3)

### Added

- Notify the documentation portal after successful Main CI when Rust documentation, examples or crate READMEs change, with exact source identity, attempt-bound evidence, an isolated dispatch credential and regression coverage. (#6)
- Prepare `opentlv-sys` and `opentlv` as independent Cargo packages without publishing them: `repository` metadata points to `OpenTLV/opentlv-rust`, keywords and categories, a README and MIT license copy per crate, both crates versioned together from the `0.7.1` migration baseline independently of Core, `opentlv` requiring exactly its `opentlv-sys` version, the compatible Core dependency recorded per release, packaged manifests checked in CI for local paths and metadata, and the remaining crates.io prerequisites documented; crate names, Cargo features and native linking unchanged. (#4)
- Add standalone Rust PR, Main and Full CI: rustfmt, Clippy, per-target test inventories for every Cargo feature set including the unchanged C ABI probe, examples, Core's shared Query corpus, Rustdoc, Cargo packaging, prebuilt static and shared linking, AddressSanitizer, a Linux/Windows/macOS matrix with pinned stable and tested MSRV, verified Core provenance and an exact-evidence quality gate. (#3)
- Build both crates standalone against Core pinned to an exact commit, with recorded and header-validated Core identity, a development-only local Core override, and Verified Source/Binary support for locked Core releases with checksum and GitHub Artifact Attestation verification; Cargo features and native linking modes unchanged. (#2)
- Import the existing Rust Cargo workspace from Core byte-for-byte: both crates, sources, tests, examples, FFI declarations, Cargo features and native linking unchanged, with the original guides and workflow preserved as evidence, a migration verifier against the frozen baseline, and an identical imported-source rerun of the baseline matrix in the Core build context. (#1)
- Record the Rust migration baseline from Core: source commit, history, crate versions, Cargo features and defaults, native linking modes, supported platforms, the complete source and integration inventory, and Windows baseline build and test results, with a verifier against the pinned Core tree. No Rust sources are imported yet. (OpenTLV/opentlv#671)
