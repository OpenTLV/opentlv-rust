# Using OpenTLV from Rust

Start with setup below, then run the [Rust quick start](../../opentlv/examples/quick_start.rs)
for input, expected output and the public API. You can use this guide without
studying native C implementation contracts.

The `opentlv` crate is an experimental safe Rust API over the OpenTLV C library.
This guide covers setup, reading, writing, composing formats, error handling
and ownership. For
crate layout, schemas, codecs, canonical validation and CI, see
[Rust development](../development/rust.md); every public item also has Rustdoc
(`cargo doc -p opentlv --open` from the repository root).

## Cargo setup

The crates are not published to crates.io yet. Depend on them by path from a
checkout of [opentlv-rust](https://github.com/OpenTLV/opentlv-rust):

```sh
git clone https://github.com/OpenTLV/opentlv-rust.git
```

```toml
[dependencies]
opentlv = { path = "opentlv-rust/opentlv" }
```

The build script of `opentlv-sys` fetches the OpenTLV Core commit pinned by
this repository, builds the C library with CMake and links it, so you need
Rust 1.82 or newer, CMake 3.16 or newer, a C99 compiler, Git and network
access to GitHub for the first build. On Windows, use the MSVC Rust toolchain
with Visual Studio. See [Build](../development/rust.md#build) for the Core
dependency. To link a library you built yourself, set `OPENTLV_LIB_DIR` (and
optionally `OPENTLV_LINK_KIND`); see
[Linking a prebuilt library](../development/rust.md#linking-a-prebuilt-library).

The Cargo features `lldp`, `nfc` and `document` are enabled by default; see
[Cargo features](../development/rust.md#cargo-features). The crate version is
independent of the Core version; see
[Versioning](../development/rust.md#versioning).

## Reading

The default Cargo feature `lldp` exposes `Format::Lldp` and enables the shared
LLDP C package in source builds. Use `default-features = false` to omit it.
For a prebuilt C library, match this feature to its `OPENTLV_LLDP` option.
See [LLDP framing and presets](https://github.com/OpenTLV/opentlv/blob/main/docs/formats/lldp/README.md).

`Reader` iterates over `Result<Element>`. Each `Element` has a `Tag` and a value
that is a `&[u8]` slice of your input. Constructed elements are not expanded
automatically; create a new `Reader` over `element.value()` to descend.

```rust
use opentlv::{Format, Reader};

for element in Reader::with_format(&data, Format::Ber) {
    let element = element?;
    println!("{:02X?}: {:02X?}", element.tag().as_bytes(), element.value());
}
```

`Reader::new` uses BER;
`Reader::with_format` selects another built-in `Format` (`Ber`, `Cer`, `Der`),
or takes a borrowed `&FixedFormat` for a runtime-configurable tag width, length
width and length byte order, or a `&ComposedFormat` (see
[Composing formats](#composing-formats)). `Reader::with_fixed_format` is
equivalent for a `&FixedFormat`:

```rust
use opentlv::{ByteOrder, FixedFormat, FixedFormatConfig, Reader};

let config = FixedFormatConfig::new(2, 1, ByteOrder::Big);
let format = FixedFormat::new(&config)?;
for element in Reader::with_fixed_format(&data, &format) {
    let element = element?;
    println!("{:02X?}: {:02X?}", element.tag().as_bytes(), element.value());
}
```

Runnable version with nesting and EMV tag names:
[reader.rs](../../opentlv/examples/reader.rs)
(`cargo run --example reader` from the repository root).

## Writing

`Writer` encodes into a buffer you own and never allocates. To build a nested
structure, encode the children into one buffer and write them as the value of
the parent.

```rust
use opentlv::{Format, Tag, Writer};

let mut buf = [0u8; 64];
let mut writer = Writer::with_format(&mut buf, Format::Ber);
writer.write(&Tag::from_bytes(&[0x50]), b"VISA")?;
let encoded: &[u8] = writer.written();
```

Use `encoded_size` to size the buffer beforehand. Both accept a `FixedFormat`
or `ComposedFormat` as well; `encoded_size_fixed` and
`Writer::with_fixed_format` are equivalent for a `FixedFormat`. Runnable
version: [writer.rs](../../opentlv/examples/writer.rs)
(`cargo run --example writer` from the repository root).

## Composing formats

A proprietary TLV format needs no C. `ComposedFormat` composes Core's generic
field encodings into a definite Format and owns the native descriptor, its
configuration and every table Core borrows:

```rust
use opentlv::{
    ByteOrder, ComposedFormat, ConstructedBit, EscapedLength, FieldComposition,
    FixedIdentifier, Reader,
};

// One tag byte; a length byte below 0xFF, or 0xFF and a two-byte count.
// Tags with the top bit set hold nested elements.
let format = ComposedFormat::fields(
    &FieldComposition::new(FixedIdentifier::new(1), EscapedLength::new(0xFF, 2, ByteOrder::Big))
        .with_constructed(ConstructedBit::new(0, 0x80, 0x80)),
)?;
for element in Reader::with_format(&data, &format) {
    let element = element?;
    println!("{:02X?}: {} bytes", element.tag().as_bytes(), element.value().len());
}
```

`FieldComposition::new` takes one identifier encoding, `FixedIdentifier` or
`VariableIdentifier` with an optional `IdentifierPolicy`, and one length
encoding, `FixedLength`, `VariableLength` with an optional `LengthPolicy`, or
`EscapedLength`. `with_order` selects TLV or LTV, `with_length_scope` whether
the count also covers the identifier, `with_tag_only` identifiers that omit
Length and Value, and `with_constructed` the `ConstructedBit` that classifies
nesting; without it every Value is opaque. The encodings select the Core
composition: the Fixed, tagged binary, Variable or escaped Format where one
exists, otherwise Core's generic field composition. `ComposedFormat::packed`
composes a definite packed Tag/Length header from a `PackedLayout` of two
`PackedField` bit fields, such as LLDP's 7-bit type above a 9-bit length; tag
fields are limited to 16 bits because the binding owns the identifier table.
`PackedField::read` and `write` also extract and insert one bit field of a
header integer directly. Core validates every configuration when the Format is
composed and reports `Error::InvalidArg` for an invalid one.

Pass `&format` wherever a format is accepted: `Reader`, `TreeReader`, `Writer`,
`TreeWriter`, `Document`, `decode`, `read`, the writer measurement functions
and `StructureSchema::validate`. These entry points take anything that converts
into a `FormatRef<'a>`, a borrowed descriptor: a builtin `Format`, or a
`&FixedFormat`, `&OwnedFixedFormat` or `&ComposedFormat`. The `_fixed` entry
points remain and are equivalent. For Query, set
`ProgramOptions::composed_format` or use `QuerySchema::with_composed_format`.
Clones of a `ComposedFormat` share one owner and its native identity; share
clones between a program and its Readers or Documents, because two independently
composed equal configurations are different Formats to Core.

`FormatRef` also exposes the `tlv_format_t` operations of any Format:
`can_read`, `can_write`, `decode`, `measure`, `encode` and `encode_into`.
Failures return `FormatError` with the Format's own detail: the operation, the
wire `Region`, the offset, the required size and the known tag, length and
value ranges. Runnable version with Tree Writer, Tree Reader, Query and
Document: [compose.rs](../../opentlv/examples/compose.rs)
(`cargo run --example compose` from the repository root).

## Error handling

Most fallible calls return `opentlv::Result<T>`, an alias for
`Result<T, opentlv::Error>`; operations with additional context use error
structures such as `DocumentError` and `WriterError`. There are no panics on bad input and no
`unsafe` in caller code.

- `Error` has one variant per C `TLV_ERR_*` code and implements
  `std::error::Error` and `Display` (the text comes from C `tlv_result_string`), so
  it works with `?` and `Box<dyn Error>`. It is `#[non_exhaustive]`: a code from
  a newer C library becomes `Error::Unknown(code)`, and `code()` returns the raw
  value.
- `Reader` yields an `Err` item on malformed input and then ends; it does not
  skip ahead.
- `Writer::write` returns `Error::BufferTooShort` when the element does not fit
  and leaves the position unchanged, so you can retry with a bigger buffer.
- Layers with more context use their own types: `SchemaError` and
  `ValidationError` carry the C `Error` plus the failing offset, and `CodecFailure`
  retains the shared `Error` and owned conversion diagnostic.

## Ownership and lifetimes

| Type | Owns | Borrows |
| --- | --- | --- |
| `Tag` | its bytes (`Clone`) | nothing |
| `Element<'a>` | its `Tag` | value `&'a [u8]` from the input |
| `Reader<'a>` | its cursor | the input `&'a [u8]` |
| `Writer<'a>` | its position and optional diagnostic | the output `&'a mut [u8]` |
| `Document<'f>` | C document nodes and storage | optional Fixed or composed Format/context |
| `Node`, `NodeMut` | a node handle | immutable or exclusive Document access |
| `LengthSchema`, `StructureSchema` | their rule tables | nothing |
| `FixedFormatConfig` | its fields (`Copy`) | nothing |
| `FixedFormat<'a>` | nothing | `&'a FixedFormatConfig` |
| `ComposedFormat` | its descriptor, configuration and tables, shared by clones | nothing |
| `FormatRef<'a>` | nothing (`Copy`) | a Format descriptor for `'a` |

- Element values are zero-copy. `Reader<'a>` yields `Element<'a>` tied to the
  input, not to the reader, so elements stay valid after the reader is dropped
  but not after the input is freed or mutated; the borrow checker enforces it.
  Copy with `to_vec()` when you need the bytes longer.
- While a `Writer` exists it holds the only `&mut` to its buffer. `written()`
  borrows the writer; `finish()` consumes it and returns the buffer with its
  full lifetime.
- `Tag` owns its bytes, while the C `tlv_tag_t` only borrows them. Reading an
  element copies its tag out of the input, so a `Tag` stays valid after the input
  is gone; the value stays zero-copy.
- Schemas own their C tables, and the tags those tables borrow, and free them
  on `Drop`.
- Readers, Writers, Documents and decoded sources borrow a Fixed or composed
  Format for their whole lifetime. A `ComposedFormat` is freed when its last
  clone drops; the borrow checker keeps it alive while anything uses it.
- The layout of `tlv_tag_t` is a pointer and a size, independent of any C build
  configuration, so the bindings work with a library built any way.

## Relationship to the C API

`opentlv-sys` declares the C functions as raw `extern "C"` items; `opentlv`
wraps them and contains no `extern` blocks. The wrappers call the C library for
all parsing, encoding, validation and codec work instead of reimplementing it,
so behavior matches the C API, and the C error codes map one to one onto
`Error`. What the safe API changes is memory handling: pointer and length pairs
become slices, initialization and `NULL` checks are done internally, and
lifetimes replace the borrowing rules the [C memory guide](https://github.com/OpenTLV/opentlv/blob/main/docs/guides/memory.md)
documents. Reader and Tree Reader `visit` methods support resumable callbacks;
panics are resumed only after returning from C. `QueryMatcher` retains canonical
matching state across STOP and input replacement. See the
[Reader and Query facade contract](https://github.com/OpenTLV/opentlv/blob/main/docs/concepts/bindings.md#rust-and-python-reader-and-query-facades).
Structure codecs and the DOL component are not bound yet; see the
[C API reference](https://github.com/OpenTLV/opentlv/blob/main/docs/reference/c-api.md).

## Tree measurement and owning Documents

`TreeWriter::measure` takes an iterator of `Result<TreeWriteItem>` records
and returns a borrowed staged encoding. Its length is the exact measured size;
C owns all parent closure and depth validation. `required_workspace()` reports
output/scratch lower bounds after exhaustion. Retry with a fresh source.
`Writer::preserve` uses C semantic equality checks before copying original
source bytes. `Writer::diagnostic` owns failure detail. Standalone
`measure_element` and `write_element` return `WriterError` with the same detail.

The default-enabled Cargo feature `document` exposes `Document`, `Node`,
`NodeMut` and `DocumentBuilder`. Disable it when linking a C library without
`OPENTLV_DOCUMENT`. `Document::parse` copies input through C, while node Values
borrow document storage. Mutation requires exclusive access; a retained value
or node prevents mutation or destruction at compile time. Query lookup and all
edits delegate to C. Destination-format sizing and output are available through
`encoded_size_as`, `encode_as` and `encode_into_as`.

`DocumentBuilder::new` materializes a fresh TreeReader stream;
`DocumentBuilder::next_subtree` pulls and materializes only the next subtree.
The builder exclusively borrows the reader. On `Error::NeedMoreData`, replace
input through `builder.set_input` and call `consume` again. Dropping the builder
releases the reader; completed Documents retain owned content and the reader's
conservative input/Format lifetime.

## Compiled Query

`QueryProgram::compile(text, &ProgramOptions)` owns the canonical C program;
`QueryProgram::load(image, &options)` validates an image with its original
compile options. `info`, `variables`, `format`, `explain` and `image` expose
native requirements and release-specific program data.

Independent `QueryExecution` values bind typed `QueryBinding` values, feed events,
pull or visit a `TreeReader`, or evaluate a Document. Scalars return `QueryValue`.
Use `feed_event(QueryEvent)` for source-less events, or `feed(TreeEvent)` to
preserve Reader Source metadata. After `finish()`, `next_result()` pulls retained
matches; `next_result_with_ordinal()` also returns native preorder identity.
Owning workspace and `execution_external` are available with explicit bounds;
the latter borrows caller storage. Rust lifetimes preserve program, input and
Document ownership; errors retain native Query spans and diagnostic detail.
`ProgramError.kind` is a `QueryErrorKind`; compare with `QueryErrorKind::Syntax`
or `QueryErrorKind::State` and use `name()` for the canonical C spelling.
`severity()`, `contexts()`, `path()` and `path_omitted()` read the common
evidence from the Reader cause when present, otherwise from the boxed
`metadata`, which stays `None` when nothing beyond error severity was reported.
`expected()` and `actual()` read the common texts from the same source.
`ProgramError.location` holds primary evidence inline. `source_offset()` projects its
start only for known INPUT locations, including zero; expression spans and
unknown locations return `None`. `expression` is the related expression span,
with kind `Unknown` when `location` already is that span. `limit` boxes the
exhausted resource name with its configured bound as `QueryLimit`, and
`codec_detail.reported` is the raw provider result. Categories are `#[non_exhaustive]`;
unrecognized native values become `Unrecognized(RawCategory)`.
`ProgramOptions.providers` holds `QueryProvider` values for the closed NUM, BCD,
TEXT and DATE conversions. Each owns a stable ID, scratch limit and `Send + Sync`
callback. Callbacks receive bytes and optional owned metadata; return
`QueryDecoded` or the shared `Error`. Provider panics become native codec errors,
and text is copied into bounded native scratch. Programs retain provider owners.

`QueryProgram::edit_document` evaluates and edits under one exclusive Document
borrow. `QueryEdit` selects removal, replacement or insertion;
`QueryEditOptions` bounds native work and target storage. `QueryEditError`
preserves the applied count, including partial failures. Short target storage
leaves the Document unchanged for a later retry with larger capacity.

`QuerySchema::new(rules, format)` owns `QueryRule` programs. Build each rule with
`QueryRule::new(context, assertion).named(name)` and validate a buffer or immutable
Document with `QuerySchemaLimits`. C performs contextual selection and Boolean
assertions; Rust owns workspace and failure context in `QuerySchemaError`.
Its `schema` is a `QuerySchemaDetail` with the failed assertion's `kind`, `tag`
and `field`; the location, path and expected text come from `failure`.
Borrowing prevents Document mutation while validation runs.

The [consumer example](../../opentlv/examples/query_conformance.rs)
runs the common corpus through the public facade. Remaining custom Format and
semantic Tag adapter coverage is tracked by the
[release requirements](https://github.com/OpenTLV/opentlv/blob/main/docs/development/query-release.md).

## Next step

Use [the basic model](https://github.com/OpenTLV/opentlv/blob/main/docs/concepts/learning-model.md) to choose borrowed processing
or owned editing, then [processing choices](https://github.com/OpenTLV/opentlv/blob/main/docs/guides/processing.md) for your next task.
