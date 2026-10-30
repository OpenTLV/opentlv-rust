# Registry-style build validation

Validation for [issue #38](https://github.com/OpenTLV/opentlv-rust/issues/38) on
2026-10-30, against the
[`v0.1.0` GitHub release](https://github.com/OpenTLV/opentlv-rust/releases/tag/v0.1.0)
assets whose digests the [crates.io validation](crates-io.md) records, with the
locked Core `v0.8.0` release. Nothing was published and no crates.io credential
was used.

## Result

The archives work as registry packages: a consumer outside the workspace resolves
both crates through a registry index, checks their digests, unpacks them and
authenticates, builds and links the locked Core with every feature set, on Linux
and Windows. `opentlv-sys` contains its build script and the Core selection files.

**A clean machine still cannot simply `cargo add opentlv`.** The default build
needs an authenticated GitHub CLI, network access, CMake and a C compiler; on
Windows the Cargo target directory must also have a short path. See the
[requirements](#consumer-requirements) and the
[findings](#findings-for-the-first-publication). macOS and the hosted GitHub CLI
path are [not verified here](#not-verified); the Full CI `consumer` check added by
this issue covers them.

## Method

[`scripts/registry_consumer.py`](../../scripts/registry_consumer.py) copies the two
unchanged `.crate` files into a local Cargo registry whose index entries follow
each archive's normalized `Cargo.toml`, and replaces crates.io with it in a new
Cargo home. For each feature set, a new project outside the workspace depends on
`opentlv = "=0.1.0"`. Cargo runs offline, so nothing else can resolve.
`Cargo.lock` must name crates.io as the source and the archive digests as
checksums, as it would for a crates.io user. The program uses the Rust API of its
features, and must report the Core version locked in the archive.

The hosts have no GitHub CLI login, so `core/verified_core.py` ran unchanged
against a local stand-in for `gh`, which is not committed. It read the same public
release data over HTTPS, and passed `attestation verify` to the real `gh` 2.103.3
with the public Sigstore bundle of each asset. Signature, signer workflow and
source digest were therefore really verified, while authenticated `gh api` and
`gh release download` were not exercised.

| Host | Rust | C compiler | CMake | Python |
| --- | --- | --- | --- | --- |
| WSL Ubuntu 24.04, x86-64, glibc 2.39 | 1.99.1, 1.82.0 | GCC 13.3.0 | 3.28.3 | 3.12.3 |
| Windows 11, x86-64 | 1.99.1 | MSVC 19.44.35222.0, Visual Studio 17 2022 generator | 3.31.7 | 3.11.4 |

## Package verification

`cargo package --workspace --locked` with Rust 1.99.1 from a clean `v0.1.0`
checkout packaged 33 and 77 files and built both crates from their archives, each
selecting `verified-source`. The resulting archives are **byte-identical to the
released assets**:

| Archive | SHA-256 |
| --- | --- |
| `opentlv-sys-0.1.0.crate` | `16522a9fd904c673650a66e1bfc0776f552fca43df9fe17741e4a335cd198547` |
| `opentlv-0.1.0.crate` | `4723697a61b1542fc2c35d112367a144d8337601f122b48a8308d337169f01d5` |

The released `opentlv-sys` archive contains `build.rs`, `core/core-artifact.json`,
`core/select.rs`, `core/verified_core.py` and `core/probe/CMakeLists.txt`, and
`opentlv` contains its `build.rs`, all byte-identical to the tag.

## Consumer builds

Each row is a new project using the released archives.

| Build | Linux | Windows |
| --- | --- | --- |
| Features `default`, `none`, `lldp`, `nfc`, `document`, Rust 1.99.1 | All passed: `verified-source`, linked Core 0.8.0 | All passed: `verified-source`, linked Core 0.8.0 |
| `default` and `none`, Rust 1.82.0 | Passed | Not run |
| `--release` | Passed | Passed, with the [path limit](#consumer-requirements) |
| `--target` naming the host | Passed | Not run |
| Unpacked sources read-only | Passed: the build writes nothing there | Not run |
| Archive changed after indexing | Rejected: `failed to verify the checksum` | Not run |

Each first build took about 50 to 120 seconds. The build script recorded the Core
components of each feature set, and checked them against the public Core headers.

### Targets

Neither crate declares `[package.metadata.docs.rs]` targets or a target list. The
[supported configurations](../quality-gates.md#supported-configurations) are the
native Linux, Windows and macOS builds. `build.rs` gives CMake no target, so Core
is always compiled for the host:

| Target from an x86-64 Linux host | Result |
| --- | --- |
| `x86_64-unknown-linux-musl` | Built and ran, with Core compiled by the host's glibc toolchain. It works only because x86-64 objects happen to link; it is not a supported configuration. |
| `aarch64-unknown-linux-gnu` | Failed at linking; this host has no cross linker. CMake had built `libtlv.a` for x86-64, so a cross linker would not help. |

## Consumer requirements

| Requirement | Without it |
| --- | --- |
| Rust 1.82 or newer, the declared `rust-version` | Not tested |
| Python 3.11 or newer (`OPENTLV_PYTHON` selects it) | `failed to run "python3"` |
| GitHub CLI on `PATH` | `gh is required to verify a Core release` |
| GitHub CLI authenticated with `gh auth login` or `GH_TOKEN` | `gh api` exits with status 4 |
| Network access to GitHub on each first build of a configuration | `gh api` fails; Cargo's `--offline` does not help |
| CMake 3.16 or newer, even for a Verified Binary, because it probes the C toolchain | `failed to run "cmake"` |
| A C99 compiler | Not tested; no [compatible binary](#findings-for-the-first-publication) was available on these hosts, so Core was compiled |
| Windows, Visual Studio generator: a Cargo target directory of at most **49** characters, **47** with `--release` | CMake reports `is not able to compile a simple test program`, caused by MSBuild's `MSB6003` (`Could not find a part of the path`) or, on longer paths, `exceeds the OS max path limit` |

The Windows limit was measured with real builds and `LongPathsEnabled` set to 0;
50 and 48 characters failed. It counts from the drive to the end of the target
directory, so a project directly in `C:\Users\<name>\source\repos\<name>` fits only
when that path has at most 42 characters. The location of the project and of
`CARGO_HOME` do not matter themselves: a 104-character project passed with a
41-character `CARGO_TARGET_DIR`. Setting `CARGO_TARGET_DIR` to a short path is the
tested workaround. Enabling long paths and other CMake generators were not tested.

`OPENTLV_LIB_DIR` does not remove these requirements: the build script still fetches
Core for its headers and test data, so a prebuilt library without `gh` stops with
the same error, before it links anything.

## Findings for the first publication

These follow the [crates.io findings](crates-io.md#findings-that-block-publishing-these-archives),
and need decisions before [issue #40](https://github.com/OpenTLV/opentlv-rust/issues/40).
No epic issue names their fixes yet, and this issue changes no crate source.

1. **The default build needs a GitHub login.** This answers the question left by
   issue #37: a registry user without `gh` and a token cannot build, even with a
   prebuilt library. The same applies to docs.rs and to offline or vendored builds.
   Publishing as is gives a crate that fails on a clean machine with a clear
   message. The authentication itself worked, so a fix should keep it where the
   tools are available. Options are a pinned public download with digests in the
   lock, a bundled Core source, or a documented system-library path that needs
   neither `gh` nor network.
2. **Windows paths are tight.** A deep project fails with an MSBuild error that does
   not mention the target directory. `build.rs` and `verified_core.py` spend 41
   characters on `core-release\release-XXXXXXXX\cmake-build`, so shorter directories
   would raise the limit.
3. **Verified Binaries are not selected in practice.** The Core v0.8.0 manifest has
   no `position_independent_code` field, which `compatible()` requires, so the Linux
   binary was rejected despite the same GCC and glibc. The Windows binary requires
   exactly MSVC 19.44.35229.0; the local 19.44.35222.0 differs. Core has no macOS
   binary. Consumers therefore compile Core.
4. **Cross compilation is unsupported.** Core is built for the host regardless of
   `--target`, as the repository README now states. The crate READMEs are not
   changed here and need the same text.

## Changes

- The CI `package` check requires the build and Core selection files in the
  archives, even if they are no longer tracked.
- A new Full CI `consumer` check runs these consumer builds with the archives of the
  same run, on Linux, Windows and macOS stable. The quality gate requires all five
  feature sets, with the recorded package digests and a verified Core selection.
  Rust 1.82.0 is not part of this check.
- The check keeps its target directories at `build\ci\c\<n>`: 43 characters on the
  hosted Windows runner, within the limit above.
- [`docs/packaging.md`](../packaging.md#remaining-cratesio-prerequisites) lists the
  requirements.

## Local validation

| Command | Result |
| --- | --- |
| `package` and `consumer` checks of `scripts/ci.py`, with the gate's `validate_checks`, on the working tree on Linux | Passed: both archives and five consumers, all `verified-source` |
| `scripts/ci.py build --tier pr --configuration linux-stable`, with the `gh` stand-in | Passed: 299 and 276 tests, 171 Query fixtures with 2,133 native executions, `verified-source` |
| `scripts/ci.py build --tier main --configuration linux-stable`, with the `gh` stand-in | Passed |
| `python -B -m unittest discover -s tests` | 113 tests passed |
| `python -B scripts/check_quality.py` | Passed, REUSE compliant with 166/166 files |

Full CI was not run locally.

## Not verified

- macOS, and Windows and Linux with a real authenticated `gh`: the `consumer` check
  runs them in hosted Full CI, which has not yet run on this change.
- Downloads from crates.io itself. The local registry follows the index format, but
  only the real publication in issue #40 exercises the crates.io server.
- Rust 1.82.0 on Windows and macOS, other compilers, and other CMake or Python
  versions than listed.
- Real builds on docs.rs. An offline `cargo doc` of the released archive with
  `DOCS_RS=1` stops at the same Core verification as in the
  [crates.io validation](crates-io.md#findings-that-block-publishing-these-archives).
