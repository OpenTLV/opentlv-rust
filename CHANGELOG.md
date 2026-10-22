# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Build both crates standalone against Core pinned to an exact commit, with recorded and header-validated Core identity, a development-only local Core override, and Verified Source/Binary support for locked Core releases with checksum and GitHub Artifact Attestation verification; Cargo features and native linking modes unchanged. (#2)
- Import the existing Rust Cargo workspace from Core byte-for-byte: both crates, sources, tests, examples, FFI declarations, Cargo features and native linking unchanged, with the original guides and workflow preserved as evidence, a migration verifier against the frozen baseline, and an identical imported-source rerun of the baseline matrix in the Core build context. (#1)
- Record the Rust migration baseline from Core: source commit, history, crate versions, Cargo features and defaults, native linking modes, supported platforms, the complete source and integration inventory, and Windows baseline build and test results, with a verifier against the pinned Core tree. No Rust sources are imported yet. (OpenTLV/opentlv#671)
