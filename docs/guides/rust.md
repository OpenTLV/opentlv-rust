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

The build script of `opentlv-sys` authenticates the pinned Core v0.8.0 release
and links its compatible static binary or builds the verified source with CMake.
You need Rust 1.82 or newer, CMake 3.16 or newer, a C99 compiler, Git,
Python 3.11 or newer, an authenticated GitHub CLI (`gh auth login`) and network
access to GitHub. On Windows, use the MSVC Rust toolchain
with Visual Studio, and keep the Cargo target directory at 49 characters or
fewer, 47 with `--release`, or set a short `CARGO_TARGET_DIR`; longer paths fail
in MSBuild. Core is built for the host, so cross compilation is not supported.
See [Build](../development/rust.md#build) for the Core
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

To decode a single element, `read_element(&data, format)` returns the `Element`
and the consumed byte count, and `read(&data, format)` returns a `Decoded` with
the original encoding and `Layout`. Trailing bytes stay unread, empty input
fails with `Error::End`, and other failures return a `ReaderError` that owns the
`ReaderDiagnostic`:

```rust
use opentlv::{read_element, Format};

let (element, consumed) = read_element(&data, Format::Ber)?;
let rest = &data[consumed..];
```

`Reader::read_element()` and `Reader::read_source()` pull the next element
without or with source metadata. Unlike iteration they stay callable after an
error: a `Reader::incremental` reader returns `Error::NeedMoreData` at the end
of its window, and continues after `set_input` supplies more bytes.

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
- Diagnostics keep the enclosing Tags of a failure in `path`, outermost first,
  and `path_text()` formats them as Core does, for example `"6F > A5"`. A
  truncated path ends in `" > ..."`. In Core 0.7.1 only Schema failures,
  including Query Schema rules, track a path; otherwise both are `None`.

## Ownership and lifetimes

| Type | Owns | Borrows |
| --- | --- | --- |
| `Tag` | its bytes (`Clone`) | nothing |
| `Element<'a>` | its `Tag` | value `&'a [u8]` from the input |
| `Reader<'a>` | its cursor | the input `&'a [u8]` |
| `Writer<'a>` | its position and optional diagnostic | the output `&'a mut [u8]` |
| `Document<'f>` | C document nodes and storage | optional Fixed or composed Format/context |
| `Node`, `NodeMut` | a node handle | immutable or exclusive Document access |
| `NodeHandle` | a node address and identity (`Copy`) | nothing |
| `LengthSchema`, `StructureSchema` | their rule tables | nothing |
| `CheckedStructureSchema<'s>` | a native checked handle (`Copy`) | its `&'s StructureSchema` |
| `ValueConstraint` | its values and names | nothing |
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
Generic Value codecs, structure codecs and wire generation are described below.
The DOL component is not bound yet; see the
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
edits delegate to C. `encode`, `encode_into` and `encoded_size` use the
Document's own Format through `tlv_document_encode()` and its siblings;
destination-format sizing and output are available through `encoded_size_as`,
`encode_as` and `encode_into_as`.

Because a borrowed `Node` cannot survive an edit, `Node::handle()` returns a
`NodeHandle` that is not tied to any borrow. After edits,
`Document::resolve(handle)` or `resolve_mut` returns the node again, or `None`
once an erase or a constructed Value replacement has retired it. C compares the
node's never-reused identity, so a node created later at the same address is
not mistaken for it. `revision()` counts every successful edit and
`retire_epoch()` the edits that retired existing nodes:

```rust
use opentlv::{Document, Format, Query, Tag};

let mut document = Document::parse(&data, Format::Ber, 64, 1024)?;
let amount = document
    .find_path(&Query::parse("6F/A5/9F02")?)?
    .ok_or("no amount")?
    .handle();
document.append(&Tag::from_bytes(&[0x9F, 0x36]), &[0, 1])?;
if let Some(mut node) = document.resolve_mut(amount) {
    node.set_value(&[0x01, 0x2C])?;
}
```

`Document::parse_with_source_locations` retains original input coordinates:
`Node::source_location()` returns the offset and header length, and an edit
clears them for the changed node and its ancestors. `Document::visit(&query,
visitor)` delivers every path match as a `Node` in Document order, without
allocating; the nodes borrow the Document and may be kept after the visit.

`DocumentBuilder::new` materializes a fresh TreeReader stream;
`DocumentBuilder::next_subtree` pulls and materializes only the next subtree.
The builder exclusively borrows the reader. On `Error::NeedMoreData`, replace
input through `builder.set_input` and call `consume` again. Dropping the builder
releases the reader; completed Documents retain owned content and the reader's
conservative input/Format lifetime.

## Compiled Query

`QueryProgram::compile(text, &ProgramOptions)` owns the canonical C program.
Static `names` compile directly through `tlv_query_compile()`; a `resolver`
uses checked preparation and commit instead, which detects a resolver that
answers differently while the program is sized and written. Both produce the
same image. `QueryProgram::load(image, &options)` validates an image with its
original compile options, and `QueryProgram::open_plan(image, &options)`
validates only the native plan, without parsing, name resolution or planning;
the options then supply just the Format, semantic Tags and providers that
executions use. `info`, `variables`, `format`, `explain` and `image` expose
native requirements and release-specific program data, and `plan_info` reads
the requirements from the plan itself.

Independent `QueryExecution` values bind typed `QueryBinding` values, feed events,
pull or visit a `TreeReader`, or evaluate a Document. Scalars return `QueryValue`.
Use `feed_event(QueryEvent)` for source-less events, or `feed(TreeEvent)` to
preserve Reader Source metadata. After `finish()`, `next_result()` pulls retained
matches; `next_result_with_ordinal()` also returns native preorder identity.
`reset()` resets an execution in place through `tlv_query_exec_reset()`: it
keeps the program, providers, workspace and budgets and clears bindings,
results and counters, so one execution serves a whole batch of inputs, even
after a failure. After `evaluate_document`, `next_document()` pulls result
nodes and `visit_document(visitor)` visits the remaining ones through the same
cursor. Programs that read constructed Values need a canonical Value snapshot;
without `value_capacity`, its size is discovered with
`tlv_document_query_value_size()` and the discovery output becomes the snapshot.
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
`QueryProvider::number(conversion, id, NumberCodec)` decodes NUM, BCD or DATE
Values with Core's own number codec through `tlv_query_codec_decode()`, without
a Rust callback; values above `i64::MAX` fail with `Error::InvalidValue`.

`Query::visit_buffer(data, format, max_depth, max_elements, visitor)` visits the
elements a parsed path addresses in complete input, without opening a Reader or
matcher.

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

## Validating and querying batches

Work that does not depend on the input can be done once. `StructureSchema::check()`
checks the definition, including every child schema, through
`tlv_schema_prepare()` and returns a `CheckedStructureSchema` that borrows the
schema. Its `validate` and `validate_diagnostics` behave like the schema's own,
without repeating the definition check, so they never fail with
`Error::InvalidSchema`. `check_definition()` runs only the check. Combined with
one compiled program and one execution reset per message:

```rust
let schema = definition.check()?;
let mut execution = program.execution(8, 64, 100_000, true)?;
for message in &messages {
    schema.validate(message, Format::Ber, &limits)?;
    execution.reset()?;
    execution.bind("minimum", QueryBinding::Integer(250))?;
    let mut reader = TreeReader::new(message, Format::Ber, 8, 8, 64, true)?;
    execution.visit(&mut reader, |_| Visit::Continue)?;
    println!("{:?}", execution.result()?);
}
```

`ValueConstraint` checks a decoded scalar, such as an ASN.1 INTEGER, against a
`range`, an `allowed` set or `named` numbers. Core checks the definition when it
is built (`Error::InvalidSchema` for reversed bounds), `validate` fails with
`Error::Schema` for a violating value, and `name` returns a named number's name.
Runnable version, with a number codec provider, a buffer path visit and the
Document view: [batch.rs](../../opentlv/examples/batch.rs)
(`cargo run --example batch` from the repository root).

## Generic Value codecs

Choose a codec explicitly for each element's Value. `IntegerCodec` covers
`u8`, big- and little-endian `u16`/`u32`, and minimal big-endian `i64`;
`NumberCodec` covers configurable unsigned numeric representations.
`SchemaNumberCodec` combines a `NumberCodec` with a `LengthRule` to select a
permitted wire width. These conversions call the canonical C codecs.

```rust
use opentlv::{DigitsCodec, IntegerCodec, TextAlphabet, TextCodec};

assert_eq!(IntegerCodec::uint16_be().decode(&[1, 2])?, 258);
let label = TextCodec::new(TextAlphabet::AsciiPrintable, 4, true);
assert_eq!(label.encode("OK")?, b"OK\0\0");
assert_eq!(label.decode(b"OK\0\0")?, "OK");
// Digit strings retain leading zeros; trailing F nibbles are padding.
assert_eq!(DigitsCodec::default().decode(&[0x01, 0x23, 0x4f])?, "01234");
```

`BytesCodec::decode` and `TextCodec::decode` return slices borrowed from the
input. `Ipv4Codec` returns `std::net::Ipv4Addr`; `Ipv4ListCodec` returns an
`Ipv4List` borrowing the input, with `get`, `iter` and `as_bytes`. Encoding
accepts a slice of addresses and preserves their order and duplicates.
`DigitsCodec::decode` returns an owned `String`.

Each codec exposes `decode`, `encoded_size`, `encode_into` and `encode`.
`encode_into` reports `Error::BufferTooShort` for insufficient output space;
`encode` allocates owned output. Errors are `CodecFailure` values with the
shared `Error` and owned native diagnostic evidence, including Schema causes.
The [value_codecs example](../../opentlv/examples/value_codecs.rs) demonstrates
selection per Value (`cargo run --example value_codecs`).

## Structure codecs

`StructureCodec` validates a complete TLV sequence and converts it to or from
an application object. It borrows a Format and optionally a `StructureSchema`;
`with_limits` sets the maximum nesting depth and element count. Zero is a real
limit. Decode validates the whole sequence in Core before calling your closure;
the returned object may borrow the input:

```rust
use opentlv::{Format, StructureCodec, StructureRule, StructureSchema, Tag};

let schema = StructureSchema::new(
    [StructureRule::new(Tag::from_bytes(&[0x50])).required_once()], false,
);
let codec = StructureCodec::new(Format::Ber).with_schema(&schema);
let label = codec.decode(&[0x50, 0x02, b'O', b'K'], |bytes| Ok(&bytes[2..]))?;
assert_eq!(label, b"OK");
let encoded = codec.encode(|| Ok([0x50, 0x01, b'A']))?;
assert_eq!(encoded, [0x50, 0x01, b'A']);
```

Encoding runs the closure once and validates its produced bytes in Core before
returning them. `encode_into` copies into caller storage; on a validation
failure that storage contains rejected output and the diagnostic uses output
coordinates. `encoded_size` runs the closure and measures its result, without
validating the sequence. Closure failures are returned unchanged. Closures run
outside C, so their panics unwind normally through Rust.

## Reproducible wire generation

`Generator` wraps Core's wire generator for fuzz corpora and property tests.
It owns the ordered candidates and borrows the Format. Record
`Generator::VERSION`, the Format/configuration, candidates, limits, seed and
case index to replay a case independently of earlier calls:

```rust
use opentlv::{Format, Generator, GeneratorCandidate, GeneratorLimits, Tag};

let generator = Generator::new(
    Format::Ber,
    [GeneratorCandidate::new(Tag::from_bytes(&[0x5a]), 0, 16)],
    GeneratorLimits {
        max_elements: 8, max_depth: 2, max_value_size: 64, max_case_size: 512,
    },
    2026,
);
let case = generator.generate(42)?;
assert_eq!(generator.generate(42)?, case);
```

The Format must support reading and writing. Core validates all limits and
candidate ranges when a case is requested. `generate` allocates output and
workspace fallibly; `generate_into` allocates nothing and uses caller storage
of at least `max_case_size` output bytes and `workspace_size()` scratch bytes.
The scratch size is `(max_depth + 1) * max_case_size`; a depth above 64 is
unsupported, and size arithmetic overflow is an error. Generation may return
`Error::Limit` when the bounded search cannot produce a nonempty case.

Generated streams decode and re-encode through the selected Format. Primitive
Values are arbitrary bytes: Schema, application and protocol-specific content
validity are not implied. Exclude terminators and candidates requiring contextual
ordering. Seed and index zero are valid, and distinct pairs may produce the
same bytes. See [generate.rs](../../opentlv/examples/generate.rs)
(`cargo run --example generate`).

## Next step

Use [the basic model](https://github.com/OpenTLV/opentlv/blob/main/docs/concepts/learning-model.md) to choose borrowed processing
or owned editing, then [processing choices](https://github.com/OpenTLV/opentlv/blob/main/docs/guides/processing.md) for your next task.
