# opentlv-sys

Raw, unsafe FFI bindings to the [OpenTLV](https://github.com/OpenTLV/opentlv)
C API, and the build script that builds or links the native `tlv` library
(`links = "tlv"`). Use the safe
[`opentlv`](https://github.com/OpenTLV/opentlv-rust/tree/main/opentlv)
crate unless you need the C API directly. Only the part of the C API used by
`opentlv` is declared.

## Core dependency

Each `opentlv-sys` version pins one OpenTLV Core dependency in
[`core/core-artifact.json`](core/core-artifact.json), which is part of the
package:

| `opentlv-sys` | Core version | Core commit | Core release |
| --- | --- | --- | --- |
| 0.1.0 | 0.8.0 | `345c77acb29266036df5f27d04ce315bb61853cc` | v0.8.0 |

The package does not contain Core. `core/verified_core.py` authenticates the
locked Core release artifacts, selects a compatible static binary or builds
the verified source with CMake as a static Release library in Cargo's `OUT_DIR`.
It never falls back to an unauthenticated Git checkout. The build script checks that
Core's public headers report the pinned version, commit and components.

Requirements: Rust 1.82 or newer, CMake 3.16 or newer, a C99 compiler, Git,
Python 3.11 or newer and an authenticated GitHub CLI (`gh auth login`),
plus network access to GitHub. On Windows, use the MSVC
Rust toolchain with Visual Studio.

## Build environment

| Variable | Effect |
| --- | --- |
| `OPENTLV_LIB_DIR` | Links a prebuilt library from this directory instead of building Core. The pinned Core tree is still selected for its headers and test data; the prebuilt library is not verified and must be built with the Core components of the enabled features. |
| `OPENTLV_LINK_KIND` | `static` or `dylib` (default) for `OPENTLV_LIB_DIR` |
| `OPENTLV_CORE_SOURCE` | Absolute path of a local Core checkout, for development only; rejected in CI |
| `OPENTLV_REQUIRE_VERIFIED_CORE` | Requires an attested Core release; fails while none is locked |

## Cargo features

`lldp`, `nfc` and `document`, all on by default, enable the Core components
`OPENTLV_LLDP`, `OPENTLV_NFC` and `OPENTLV_DOCUMENT` in source builds and the
matching declarations.

## Versioning

`opentlv-sys` and `opentlv` are released together with the same version. A
change of the pinned Core dependency is released as a new version. See
[packaging and versioning](https://github.com/OpenTLV/opentlv-rust/blob/main/docs/packaging.md).

## License

[MIT](LICENSE). The build script builds or links the OpenTLV Core C library,
also MIT licensed by Marek Cingel; keep the license notice when distributing
binaries that contain it.
