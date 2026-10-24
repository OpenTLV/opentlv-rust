# Rust bindings (experimental)

The root of [opentlv-rust](https://github.com/OpenTLV/opentlv-rust) is a Cargo
workspace with two crates. Core remains the source of truth for shared
architecture and TLV semantics; this repository owns the Rust implementation,
its documentation, tests and development.

| Crate | Purpose |
| --- | --- |
| `opentlv-sys` | Raw `extern "C"` declarations of the OpenTLV C API and the build script that links the C library |
| `opentlv` | Safe Rust API built on `opentlv-sys` |

For setup, examples, error handling and ownership as a user, see
[Using OpenTLV from Rust](../guides/rust.md). For the naming and shape this
crate follows and adapts, see the [language bindings conceptual
model](https://github.com/OpenTLV/opentlv/blob/main/docs/concepts/bindings.md).

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
| `FormatRef<'a>` | `const tlv_format_t*` | A `Copy` borrow of a builtin, Fixed or composed descriptor, accepted by every format entry point; exposes `tlv_format_can_read()`, `tlv_format_can_write()`, `tlv_format_decode()`, `tlv_format_measure()` and `tlv_format_encode()` with `FormatError` detail |
| `FixedIdentifier`, `VariableIdentifier`, `FixedLength`, `VariableLength`, `EscapedLength`, `PackedField`, `ConstructedBit` | `tlv/field/*.h`, `tlv_constructed_bit_t` | Immutable field encoding configurations; `PackedField` also reads and writes one bit field |
| `ComposedFormat` | `tlv_format_t` and a `tlv/formats/*.h` composition | Built from a `FieldComposition` or `PackedLayout`; owns the descriptor, configuration and every borrowed table behind an `Arc`, so clones share one native identity |

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
`Reader::with_format` takes a `Format` (`Ber`, `Emv`, `Cer`, `Der`, plus optional `Lldp` and `NfcType2`)
or any other `FormatRef`, such as a borrowed `&ComposedFormat`, and
`Reader::with_fixed_format` takes a borrowed `&FixedFormat` for a
runtime-configurable tag width, length width and length byte order (wraps the C
`tlv_fixed_format_t` and `tlv_fixed_format_init()`: `FixedFormatConfig::new(tag_size,
length_size, order)` builds the caller-owned configuration, and
`FixedFormat::new(&config)` returns `Result<FixedFormat<'_>>` borrowing it;
`config` must outlive `format`, and `format` must outlive every reader or
writer built from it). Malformed input yields an `Err(Error)` item, after
which the iterator ends, since the C reader does not advance past bad data.
Library users need no `unsafe`.

Iteration and `Reader::read_element()` pull through `tlv_reader_next_diag()`
without source metadata; `Reader::read_source()` uses
`tlv_reader_next_source_diag()`. Both explicit pulls stay callable after an
error, so incremental input can be extended with `set_input` after
`Error::NeedMoreData`. For one element, `read_element()` calls `tlv_read_diag()`
and `read()` calls `tlv_read_source_diag()` without opening a Reader; their
`ReaderError` owns the failure diagnostic.

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

The schema types wrap the C library's tables; checks and validation run in C.

| Type | Wraps | Purpose |
| --- | --- | --- |
| `LengthSchema` | `tlv_schema_t` | Per-tag length rules: `find` and `validate_length` |
| `StructureSchema` | `tlv_structure_schema_t` | Required, optional, duplicate, kind and child-membership rules for a whole buffer |
| `CheckedStructureSchema<'s>` | `tlv_schema_checked_t` | A `StructureSchema` checked once, for repeated validation |
| `ValueConstraint` | `tlv_value_constraint_t` | Range, allowed-value or named-number constraint on a decoded scalar |

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

`StructureSchema::check_definition` checks every reachable rule table through
`tlv_schema_check()` without input; a definition error has `definition_kind`
and `definition_index` but no offset. `StructureSchema::check` does the same
check through `tlv_schema_prepare()` and returns a `CheckedStructureSchema`
that borrows the schema, whose tables are immutable. Its `validate` and
`validate_diagnostics` call `tlv_schema_validate_checked()` and
`tlv_schema_validate_all_checked()`. Schema diagnostics are initialized with
`tlv_schema_diagnostic_init()`. `ValueConstraint` owns the allowed values and
names its native definition borrows; construction runs
`tlv_value_constraint_check()`, and `validate` and `name` call
`tlv_value_constraint_validate()` and `tlv_value_constraint_name()`.

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

`Format` (`Ber`, `Emv`, `Cer`, `Der`, plus optional `Lldp` and `NfcType2`) selects the wire
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

Requirements: Rust 1.82 or newer, CMake 3.16 or newer, a C99 compiler and Git.
On Windows use the MSVC Rust toolchain with Visual Studio. The first build of
each configuration downloads Core from GitHub.

From the repository root:

```sh
cargo build --workspace
cargo test --workspace
```

By default the build script of `opentlv-sys` clones OpenTLV Core into Cargo's
`OUT_DIR`, checks out the commit locked in
[`opentlv-sys/core/core-artifact.json`](../../opentlv-sys/core/core-artifact.json)
and verifies its identity. It builds Core with CMake as a static,
position-independent Release library and links it. The C++ layer, CLI, tests
and examples of Core are not built. After building, it checks that Core's
public `tlv/version.h` and `tlv/config.h` report the locked version, commit and
components. The [repository README](../../README.md#core-dependency) describes
every Core selection mode, including verified Core releases, and the recorded
selection evidence.

## Cargo features

The build script always enables the Core Reader, Writer, Query, Schema and Codec
components. Each Cargo feature of `opentlv` forwards to the `opentlv-sys`
feature of the same name, which enables one more Core component in source
builds:

| Feature | Default | Enables | Core option |
| --- | --- | --- | --- |
| `lldp` | yes | `Format::Lldp` | `OPENTLV_LLDP` |
| `nfc` | yes | `Format::NfcType2` | `OPENTLV_NFC` |
| `document` | yes | `Document`, `DocumentBuilder` and the Query edit API | `OPENTLV_DOCUMENT` |

`--no-default-features` disables all three. The build script passes no
`OPENTLV_FORMAT_*` or `OPENTLV_EMV` options, so the other built-ins use Core's
defaults. For prebuilt libraries, match the features to the C build's options.
See [LLDP presets](https://github.com/OpenTLV/opentlv/blob/main/docs/formats/lldp/README.md).

## Local workspace development

In [opentlv-workspace](https://github.com/OpenTLV/opentlv-workspace), Core and
Rust are sibling checkouts. To build against the local Core checkout, set
`OPENTLV_CORE_SOURCE` to the absolute path of its repository root. From
`opentlv-rust`:

```sh
export OPENTLV_CORE_SOURCE="$(cd ../opentlv && pwd)"
cargo test --workspace
```

```powershell
$env:OPENTLV_CORE_SOURCE = (Resolve-Path ../opentlv).Path
cargo test --workspace
```

The build records the local commit, version and dirty state without verifying
them and prints a Cargo warning. CI and `OPENTLV_REQUIRE_VERIFIED_CORE` reject
this override. Unset the variable to return to the locked Core. Local builds are
for development only and are not release evidence.

## Versioning

`opentlv` and `opentlv-sys` share the Cargo workspace version
(`version.workspace = true` in each crate's `Cargo.toml`). It started from the
`0.7.1` migration baseline and is independent of the OpenTLV C library's
version; it is not derived from Core's Git tags. `opentlv` requires exactly the
`opentlv-sys` version it is released with, and each `opentlv-sys` version
records its compatible Core dependency. `opentlv::version()` is a separate
thing: it calls `tlv_version_string()` at run time and reports the version of
the *linked C library*, not the crate's own version. The two are equal at the
migration baseline and can differ after either is released. See
[packaging and versioning](../packaging.md).

## Continuous integration and quality checks

Pull request, Main and Full CI fail on any of:

- `cargo fmt --all --check`, formatting differs from rustfmt;
- `cargo clippy --workspace --all-targets -- -D warnings`, any clippy or
  compiler warning;
- `cargo build` and `cargo test` with warnings denied. Full CI covers Linux,
  Windows and macOS with the pinned stable toolchain and the tested MSRV.

Run the same checks locally from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The [quality gates](../quality-gates.md) list every CI check, the supported
toolchains and platforms, and how to run a complete CI configuration locally.

`opentlv/tests/corpus.rs` replays the C fuzz seeds (each harness's `corpus/`
folder under `tests/fuzz/` of the selected Core tree) through the safe API
(reader on every format, DER and CER rules, writer and reader round trips,
every EMV codec), so seeds added for the C harnesses also cover the bindings
without being copied.

## Contributing

Report Rust binding issues and send changes to
[opentlv-rust](https://github.com/OpenTLV/opentlv-rust/issues). Changes to
shared TLV semantics, the C API or Core components belong to
[OpenTLV Core](https://github.com/OpenTLV/opentlv/issues); this repository then
selects the Core revision that contains them.

## Linking a prebuilt library

To link a library you built yourself, point `OPENTLV_LIB_DIR` at the directory
containing it:

```sh
OPENTLV_LIB_DIR=/path/to/build/tlv cargo build
```

`OPENTLV_LINK_KIND` selects `static` or `dylib` (default `dylib`, matching the
default CMake build). With a shared library, the platform's loader must find it
at run time (`PATH` on Windows, `LD_LIBRARY_PATH` on Linux, `DYLD_LIBRARY_PATH`
on macOS). The build script still selects Core for its headers and test data,
but does not build it, and the prebuilt library is not verified.

## Document component

The default-enabled `document` feature forwards to `opentlv-sys/document`.
Source builds set `OPENTLV_DOCUMENT` to match. Prebuilt C libraries must enable
that component when the Rust feature is enabled. Document runtime tests and a
compile-fail lifetime example cover ownership, mutation, Query lookup, limits,
destination-format output and resumable whole-stream/subtree builders, and the
revision and retirement epoch, `NodeHandle` resolution after edits, retained
source locations and Query visits of Document nodes.
