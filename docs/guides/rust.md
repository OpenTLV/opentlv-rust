# Using OpenTLV from Rust

Start with setup below, then run the [Rust quick start](../getting-started/README.md#quick-start)
for input, expected output and the public API. You can use this guide without
studying native C implementation contracts.

The `opentlv` crate is an experimental safe Rust API over the OpenTLV C library.
This guide covers setup, reading, writing, error handling and ownership. For
crate layout, schemas, codecs, canonical validation and CI, see
[Rust bindings](../development/rust.md); every public item also has Rustdoc
(`cargo doc -p opentlv --open` from `bindings/rust`).

## Cargo setup

The crates are not published to crates.io yet. Depend on them by path from a
checkout of the repository:

```toml
[dependencies]
opentlv = { path = "OpenTLV/bindings/rust/opentlv" }
```

The build script of `opentlv-sys` builds the C library with CMake and links it,
so you need a Rust toolchain (1.70 or newer), CMake 3.16 or newer and a C99
compiler. To link a library you built yourself, set `OPENTLV_LIB_DIR` (and
optionally `OPENTLV_LINK_KIND`); see
[Linking a prebuilt library](../development/rust.md#linking-a-prebuilt-library).

## Reading

The default Cargo feature `lldp` exposes `Format::Lldp` and enables the shared
LLDP C package in source builds. Use `default-features = false` to omit it.
For a prebuilt C library, match this feature to its `OPENTLV_LLDP` option.
See [LLDP framing and presets](../formats/lldp/README.md).

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
and `Reader::with_fixed_format` takes a `&FixedFormat` for a
runtime-configurable tag width, length width and length byte order:

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
[reader.rs](https://github.com/marekcingel/OpenTLV/blob/main/bindings/rust/opentlv/examples/reader.rs)
(`cargo run --example reader`).

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

Use `encoded_size` to size the buffer beforehand, or `encoded_size_fixed`/
`Writer::with_fixed_format` for a `FixedFormat`. Runnable version:
[writer.rs](https://github.com/marekcingel/OpenTLV/blob/main/bindings/rust/opentlv/examples/writer.rs)
(`cargo run --example writer`).

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
| `Document<'f>` | C document nodes and storage | optional Fixed Format/context |
| `Node`, `NodeMut` | a node handle | immutable or exclusive Document access |
| `LengthSchema`, `StructureSchema` | their rule tables | nothing |
| `FixedFormatConfig` | its fields (`Copy`) | nothing |
| `FixedFormat<'a>` | nothing | `&'a FixedFormatConfig` |

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
- The layout of `tlv_tag_t` is a pointer and a size, independent of any C build
  configuration, so the bindings work with a library built any way.

## Relationship to the C API

`opentlv-sys` declares the C functions as raw `extern "C"` items; `opentlv`
wraps them and contains no `extern` blocks. The wrappers call the C library for
all parsing, encoding, validation and codec work instead of reimplementing it,
so behavior matches the C API, and the C error codes map one to one onto
`Error`. What the safe API changes is memory handling: pointer and length pairs
become slices, initialization and `NULL` checks are done internally, and
lifetimes replace the borrowing rules the [C memory guide](memory.md)
documents. Reader and Tree Reader `visit` methods support resumable callbacks;
panics are resumed only after returning from C. `QueryMatcher` retains canonical
matching state across STOP and input replacement. See the
[Reader and Query facade contract](../concepts/bindings.md#rust-and-python-reader-and-query-facades).
Structure codecs and the DOL component are not bound yet; see the
[C API reference](../reference/c-api.md).

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

The [consumer example](../../bindings/rust/opentlv/examples/query_conformance.rs)
runs the common corpus through the public facade. Remaining custom Format and
semantic Tag adapter coverage is tracked by the
[release requirements](../development/query-release.md).

## Next step

Use [the basic model](../concepts/learning-model.md) to choose borrowed processing
or owned editing, then [processing choices](processing.md) for your next task.
