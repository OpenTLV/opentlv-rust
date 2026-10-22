# Rust bindings (experimental)

The Rust bindings live in `bindings/rust/`, a Cargo workspace with two crates:

| Crate | Purpose |
| --- | --- |
| `opentlv-sys` | Raw `extern "C"` declarations of the OpenTLV C API and the build script that links the C library |
| `opentlv` | Safe Rust API built on `opentlv-sys` |

For setup, examples, error handling and ownership as a user, see
[Using OpenTLV from Rust](../guides/rust.md). For the naming and shape this
crate follows and adapts, see the [language bindings conceptual
model](../concepts/bindings.md).

Raw C declarations live in `opentlv-sys`; safe facade implementations in
`opentlv` contain the audited unsafe calls and callback adapters. Only the part of the C API the safe crate needs is bound
so far.

The `opentlv-sys` test suite builds a small C ABI probe with CMake and a native C
compiler. It compares the size, alignment and every member offset of Fixed field
and format configurations with the Rust `#[repr(C)]` mirrors. Offset checks catch
layout changes even when the overall structure size stays the same. This test
uses the matching source headers; consumers must still pair them with the
matching native library.

## Core types

The `opentlv` crate exposes these safe types; none of them exposes a raw pointer:

| Type | Wraps | Notes |
| --- | --- | --- |
| `Tag` | `tlv_tag_t` | Owned bytes of any length, `Clone` but not `Copy`; built with `from_bytes`, read with `as_bytes` |
| `Element<'a>` | `tlv_element_t` | A `Tag` (copied out of the input) plus a value borrowed as `&'a [u8]` |
| `Error` | `tlv_result_t` | One variant per `TLV_ERR_*` code, plus `Unknown(code)`; implements `std::error::Error` |
| `Result<T>` | | Alias for `std::result::Result<T, Error>` |
| `FixedFormatConfig` | `tlv_fixed_format_t` | Runtime-configurable tag width, length width (1-8 bytes) and length byte order; a plain `Copy` value the caller owns |
| `FixedFormat<'a>` | `tlv_format_t` | Borrows a `&'a FixedFormatConfig`, mirroring the C `tlv_fixed_format_t`/`tlv_format_t` split directly; no heap allocation |
| `ByteOrder` | `tlv_byte_order_t` | `Big` or `Little`, the length field's byte order for `FixedFormatConfig` |

The C `tlv_tag_t` is a borrowed pointer and size (`opentlv_sys::tlv_tag_t`), so
its layout does not depend on how the library was built. `Tag` owns its bytes
because the safe API cannot hand out a pointer whose lifetime C does not track;
it converts to a borrowed C tag only for the duration of each call. A tag has no
length limit; a format may reject some lengths.

## Reader

`Reader<'a>` parses a `&'a [u8]` and iterates over `Result<Element<'a>>`. It wraps
the C `tlv_reader_t`; element values are zero-copy slices of the input, and the
borrow checker keeps the input alive for as long as the reader or any element.

```rust
let reader = opentlv::Reader::new(&data);

for element in reader {
    let element = element?;
    println!("{:?}: {:?}", element.tag(), element.value());
}
```

`Reader::new` uses BER;
`Reader::with_format` takes a `Format` (`Ber`, `Emv`, `Cer`, `Der`, plus optional `Lldp`), and
`Reader::with_fixed_format` takes a borrowed `&FixedFormat` for a
runtime-configurable tag width, length width and length byte order (wraps the C
`tlv_fixed_format_t` and `tlv_fixed_format_init()`: `FixedFormatConfig::new(tag_size,
length_size, order)` builds the caller-owned configuration, and
`FixedFormat::new(&config)` returns `Result<FixedFormat<'_>>` borrowing it;
`config` must outlive `format`, and `format` must outlive every reader or
writer built from it). Malformed input yields an `Err(Error)` item, after
which the iterator ends, since the C reader does not advance past bad data.
Library users need no `unsafe`.

## Writer

`Writer<'a>` encodes elements into a caller-owned `&'a mut [u8]`. It wraps the C
`tlv_writer_t` and never allocates. Tags are `Tag`s and values are `&[u8]`.

```rust
let mut buf = [0u8; 64];
let mut writer = opentlv::Writer::new(&mut buf);
writer.write(&opentlv::Tag::from_bytes(&[0x01]), b"abc")?;
let encoded: &[u8] = writer.written();
```

`Writer::with_format` takes the same `Format` as the reader. `write_element`
appends an `Element` (for example one produced by a `Reader`), `position`,
`remaining` and `capacity` report buffer usage, and `finish` returns the written
bytes with the buffer's lifetime. `encoded_size` computes the size of an element
without writing it. Failures use the common `Error`; an element that does not fit
gives `Error::BufferTooShort` and leaves the position unchanged. Library users
need no `unsafe`.

## Schemas

Both schema types wrap the C library's tables; validation runs in C.

| Type | Wraps | Purpose |
| --- | --- | --- |
| `LengthSchema` | `tlv_schema_t` | Per-tag length rules: `find` and `validate_length` |
| `StructureSchema` | `tlv_structure_schema_t` | Required, optional, duplicate, kind and child-membership rules for a whole buffer |

`LengthSchema::new` and `StructureSchema::new` build owned tables from
`LengthRule` and `StructureRule` values; `LengthSchema::emv`,
`LengthSchema::emv_for` and `StructureSchema::emv` return the built-in EMV
schemas. `StructureSchema::validate` takes the data, a `Format` (which decides
which tags are constructed) and `ValidationLimits`, and returns
`Result<(), SchemaError>`. `SchemaError` carries the C `Error`
(`Schema`, `InvalidSchema`, `Limit`, ...) and the optional failing
offset plus Schema kind and anchor.

```rust
let tag = opentlv::Tag::from_bytes(&[0x84]);
let schema = opentlv::StructureSchema::new(
    [opentlv::StructureRule::new(tag).length(5, 16).required_once()],
    false,
);
schema.validate(&data, opentlv::Format::Ber, &opentlv::ValidationLimits::default())?;
```

## Codecs and the EMV dictionary

`Codec` converts a raw value to a typed `Value` (`Number`, `Flags`, `Digits`,
`Date`, `Time`, `Afl`, `Track2`, ...) with `decode`, and back with `encode`,
`encode_into` and `encoded_size`. Errors use `CodecFailure`, mapped from
the shared `Error` and optional owned `CodecDiagnostic`. `Codec::amount`
is the amount codec; every other codec comes from the EMV dictionary:

```rust
use opentlv::emv::{self, Context};

let tag = opentlv::Tag::from_bytes(&[0x9A]);
let definition = emv::find(Context::Base, &tag).unwrap();
let date = definition.codec().unwrap().decode(&[0x25, 0x12, 0x31])?;
```

`emv::find` returns a `Definition` (name, `ValueKind`, length bounds and step,
`validate_length`, optional `Codec`); `Context::child` follows template
contexts. Opaque bytes, text and templates have no codec; use the reader's
borrowed value.

## Formats and canonical validation

`Format` (`Ber`, `Emv`, `Cer`, `Der`, plus optional `Lldp`) selects the wire
encoding; it implements `FromStr` and `Display` for names such as `"der"`.
`Format` (`Der`, `Cer`) adds canonical-encoding checks and resource `Limits`
on top of the format: `validate`, `read`, `encoded_size` and `write`, each with
a `Strictness` (`Canonical` or `Strict`, which also validates UNIVERSAL
content). `default_limits()` returns `Result<Limits, ValidationError>`.
These bounded operations return `InvalidArg` with unknown location for formats other
than DER/CER; ordinary framing remains available through Reader/Writer. Failures are
`ValidationError` values with an explicit location domain, kind and bounds.

Reader and Tree Reader callbacks and resumable Query matching delegate to C.
The DOL component and structure codecs are not bound yet.

## Build

Requirements: a Rust toolchain (1.70 or newer), CMake 3.16 or newer and a C99
compiler. On Windows use the MSVC Rust toolchain with Visual Studio.

```sh
cd bindings/rust
cargo build
cargo test
```

By default the build script configures and builds the OpenTLV C library from the
repository root with CMake, as a static Release library inside Cargo's `target`
directory, and links it. The C++ layer, CLI, tests and examples are not built.

The build script passes no `OPENTLV_FORMAT_*` or `OPENTLV_EMV` options, so the
C library uses its default components except LLDP: the default `lldp` Cargo
feature enables `OPENTLV_LLDP`; `--no-default-features` disables it. For prebuilt
libraries, match the feature to the C build's option. Component selection for
other built-ins remains future work. See [LLDP presets](../formats/lldp/README.md).

## Versioning

The Cargo workspace version (`version.workspace = true` in each crate's
`Cargo.toml`) is independent of the OpenTLV C library's version and is not
derived from the repository's Git tags; it follows its own release cadence,
the same way the Python package's `pyproject.toml` version does (see
[Python bindings](python.md#versioning)). `opentlv::version()` is a separate
thing: it calls `tlv_version_string()` at run time and reports the version of
the *linked C library*, not the crate's own version. The two can differ, for
example crate `0.1.0` built against OpenTLV C library `0.7.1`.

## Continuous integration and quality checks

The [Rust Bindings workflow](https://github.com/marekcingel/OpenTLV/blob/main/.github/workflows/rust.yml)
runs on every push and pull request to `main` and fails on any of:

- `cargo fmt --all --check`, formatting differs from rustfmt;
- `cargo clippy --workspace --all-targets -- -D warnings`, any clippy or
  compiler warning;
- `cargo build` and `cargo test` on Linux, Windows and macOS, with warnings
  denied.

Run the same checks locally from `bindings/rust`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`opentlv/tests/corpus.rs` replays the C fuzz seeds (each harness's `corpus/`
folder under `tests/fuzz/`) through the safe API (reader on every format, DER
and CER rules, writer and reader round trips, every EMV codec), so seeds
added for the C harnesses also cover the bindings without being copied.

## Linking a prebuilt library

To link a library you built yourself, point `OPENTLV_LIB_DIR` at the directory
containing it:

```sh
OPENTLV_LIB_DIR=/path/to/build/tlv cargo build
```

`OPENTLV_LINK_KIND` selects `static` or `dylib` (default `dylib`, matching the
default CMake build). With a shared library, the platform's loader must find it
at run time (`PATH` on Windows, `LD_LIBRARY_PATH` on Linux, `DYLD_LIBRARY_PATH`
on macOS).

## Document component

The default-enabled `document` feature forwards to `opentlv-sys/document`.
Source builds set `OPENTLV_DOCUMENT` to match. Prebuilt C libraries must enable
that component when the Rust feature is enabled. Document runtime tests and a
compile-fail lifetime example cover ownership, mutation, Query lookup, limits,
destination-format output and resumable whole-stream/subtree builders.
