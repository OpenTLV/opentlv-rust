# Generic Core API coverage

This is the reviewed baseline of the generic Core surface that the safe
`opentlv` crate supports. It was recorded for issue
[#23](https://github.com/OpenTLV/opentlv-rust/issues/23), which closes the
[coverage epic #18](https://github.com/OpenTLV/opentlv-rust/issues/18), against
Core `0.7.1`, commit `476f1fbecde6b056419e43c9a6cea4d1ca8df1cf`, as pinned by
[`core-artifact.json`](../../opentlv-sys/core/core-artifact.json). The
comparison baseline is Rust commit `01c1c977ec9e6be7aeaadd8d5d0e11ddd10d7b63`,
`main` before this issue's changes.

Core coverage is **215/215 (100.0 %)** with 82 reviewed exclusions, up from
214/216 (99.1 %) with 81. Builtins stay at **34/291 (11.7 %)**. Builtins parity
is outside the epic.
[`Core API coverage`](../quality-gates.md#core-api-coverage) now fails a pull
request or Full run when a generic Core symbol is neither referenced nor
excluded, or when an exclusion no longer matches the pinned Core.

## Closing the remaining gaps

Before this issue, the report listed two `tlv/diagnostic.h` symbols as not
referenced:

- `tlv_diagnostic_path_string` was a real gap. The diagnostic types that carry
  an enclosing path now have `path_text()`, which returns Core's formatting of
  that path, such as `"6F > A5"`: `ReaderDiagnostic`, `DiagnosticMetadata`,
  `CodecDiagnostic`, `SchemaDiagnostic`, `WriterDiagnostic` and
  `ProgramError`. Python's equivalent is `OpenTLVError.path_text`. The method
  formats the owned path again through Core, so it does not depend on the
  input or Schema staying alive. A truncated path ends in `" > ..."` and a
  tracked empty path is `""`. It returns `None` without a tracked path, and also
  for a path Core rejects: one with an empty Tag, or one with more than the 32
  Tags a native path can hold.
- `tlv_location_translate` adds a byte origin to a known location. Python uses
  it because its `write_element(storage, ..., offset=...)` writes at an offset
  into caller storage. Rust entry points take the exact input or output slice
  and report Core's coordinates in that slice unchanged, so the binding never
  adds an origin. The symbol is now a reviewed exclusion in
  [`ci/api-coverage.json`](../../ci/api-coverage.json).

## Per-header inventory

`Referenced` counts the `TLV_API` symbols that `opentlv/src` names, excluding
the reviewed exclusions. A referenced symbol shows that the binding can reach
it, not that it covers all the functionality. This table records that mapping
and the binding tests behind it. The named test files are in `opentlv/tests/`,
the ABI probes in `opentlv-sys/tests/`.

| Core header | Referenced | Excluded | Public Rust API | Binding tests |
| --- | ---: | ---: | --- | --- |
| `tlv/codec/codec.h` | 2/2 | 0 | Shared decode and encode dispatch of every codec | `codec`, `value_codecs` |
| `tlv/codec/diagnostic.h` | 4/4 | 1 | `CodecDiagnostic`, `CodecOperation`, `CodecCause`, `CodecViolation`, `CodecReason` | `codec`, `diagnostic_vocabulary` |
| `tlv/codec/digits.h` | 2/2 | 0 | `DigitsCodec` | `value_codecs` |
| `tlv/codec/ipv4.h` | 3/3 | 0 | `Ipv4Codec`, `Ipv4ListCodec`, `Ipv4List` | `value_codecs` |
| `tlv/codec/number.h` | 2/2 | 0 | `NumberCodec`, `QueryProvider::number()` | `codec`, `query_reuse` |
| `tlv/codec/structure.h` | 2/2 | 0 | `StructureCodec` | `structure_codec` |
| `tlv/codec/text.h` | 2/2 | 0 | `TextCodec`, `TextAlphabet` | `value_codecs` |
| `tlv/codec/values.h` | 7/7 | 0 | `IntegerCodec`, `BytesCodec` | `value_codecs`, `codec_abi` |
| `tlv/definition.h` | 1/1 | 0 | `Definition`, `DefinitionRegistry` | `definition` |
| `tlv/diagnostic.h` | 4/4 | 10 | `Location`, `LocationDomain`, `LocationKind`, `Severity`, `DiagnosticContext`, `path_text()` | `diagnostic`, `diagnostic_path`, `diagnostic_vocabulary` |
| `tlv/document/document.h` | 41/41 | 0 | `Document`, `Node`, `NodeMut`, `NodeHandle`, `DocumentSourceLocation`, `QueryEdit` | `document`, `document_state`, `program` |
| `tlv/field/escaped.h` | 2/2 | 0 | `EscapedLength` | `composed_format` |
| `tlv/field/fixed.h` | 4/4 | 0 | `FixedIdentifier`, `FixedLength` | `composed_format` |
| `tlv/field/packed.h` | 2/2 | 0 | `PackedField::read()`, `PackedField::write()` | `composed_format` |
| `tlv/field/variable.h` | 4/4 | 0 | `VariableIdentifier`, `VariableLength`, `IdentifierPolicy`, `LengthPolicy` | `composed_format` |
| `tlv/format.h` | 8/8 | 0 | `FormatRef` operations, `ConstructedBit`, `Decoded::preserve()` | `composed_format`, `writer`, `stream` |
| `tlv/formats/compose.h` | 3/3 | 12 | `ComposedFormat::fields()`, `FieldComposition` | `composed_format`, `compose_abi` |
| `tlv/formats/escaped.h` | 1/1 | 3 | Escaped `FieldComposition` | `composed_format` |
| `tlv/formats/fixed.h` | 1/1 | 0 | `FixedFormat`, `OwnedFixedFormat`, `FixedFormatConfig` | `composed_format`, `fixed_abi` |
| `tlv/formats/packed.h` | 4/4 | 0 | `ComposedFormat::packed()`, `PackedLayout` | `composed_format` |
| `tlv/formats/variable.h` | 2/2 | 4 | Variable `FieldComposition` | `composed_format` |
| `tlv/generator.h` | 2/2 | 0 | `Generator`, `GeneratorCandidate`, `GeneratorLimits` | `generator`, `codec_abi` |
| `tlv/query/adapters.h` | 3/3 | 0 | `QueryProvider` builtin hooks, codec decoding and Definition resolution | `query_extensions`, `query_reuse` |
| `tlv/query/plan.h` | 2/2 | 0 | `QueryProgram::open_plan()`, `plan_info()` | `query_reuse` |
| `tlv/query/program.h` | 31/31 | 0 | `QueryProgram`, `QueryExecution`, `ProgramError` | `program`, `query_extensions`, `query_reuse` |
| `tlv/query/query.h` | 9/9 | 1 | `Query`, `QueryMatcher`, `Query::visit_buffer()` | `query_reuse`, `tree_reader`, `document` |
| `tlv/reader/diagnostic.h` | 1/1 | 0 | `ReaderOperation` | `diagnostic_vocabulary` |
| `tlv/reader/reader.h` | 11/11 | 2 | `Reader`, `read()`, `read_element()`, `ReaderError` | `reader`, `stream`, `incremental_native` |
| `tlv/reader/tree.h` | 9/9 | 2 | `TreeReader`, `TreeEvent`, `TreeItem` | `tree_reader` |
| `tlv/reader/visitor.h` | 2/2 | 1 | `Reader` and Tree Reader `visit` with `Visit` | `tree_reader` |
| `tlv/result.h` | 1/1 | 0 | `Error` and its `Display` text | `opentlv` unit tests |
| `tlv/schema/constraint.h` | 3/3 | 0 | `ValueConstraint` | `checked_schema` |
| `tlv/schema/detail.h` | 2/2 | 0 | `SchemaIssue`, `SchemaDefinitionKind` | `schema`, `diagnostic_vocabulary` |
| `tlv/schema/number.h` | 2/2 | 0 | `SchemaNumberCodec` | `value_codecs` |
| `tlv/schema/query.h` | 3/3 | 0 | `QuerySchema`, `QueryRule` | `query_schema` |
| `tlv/schema/schema.h` | 10/10 | 0 | `LengthSchema`, `StructureSchema`, `CheckedStructureSchema`, `SchemaDiagnostic` | `schema`, `checked_schema`, `validation` |
| `tlv/writer/tree.h` | 10/10 | 4 | `TreeWriter`, `TreeWriteItem`, `WriterDiagnostic` | `tree_writer`, `tree_reader` |
| `tlv/writer/writer.h` | 13/13 | 8 | `Writer`, `write_element()`, measurement functions | `writer`, `stream` |
| `tlv/copy.h`, `tlv/endian.h`, `tlv/size.h`, `tlv/tag.h`, `tlv/value.h` | - | 34 | Native Rust slices, byte order, comparison and size arithmetic | - |

The reason for every exclusion is in
[`ci/api-coverage.json`](../../ci/api-coverage.json), and the
[quality gates](../quality-gates.md#core-api-coverage) summarize the groups:
primitives, diagnostic producers, callbacks that bound descriptor initializers
install, NULL-diagnostic wrappers, the Tag/Value write forms,
`tlv_writer_copy_element`, `tlv_query_parse` and `tlv_location_translate`.

`opentlv-sys` declares a few Core symbols that the safe crate does not use:
`tlv_codec_diagnostic_init`, `tlv_query_parse`, `tlv_reader_next`,
`tlv_write_element`, `tlv_writer_copy_encoded`, `tlv_writer_preserve`,
`tlv_writer_write` and `tlv_writer_write_element`. Each is a reviewed
exclusion. The raw crate also declares the Builtins functions
`tlv_asn1_is_constructed` and `tlv_emv_schema`, which the safe crate does not
use either.

## Limitations

- Builtins-specific parity, including DOL, the ASN.1 value codecs, DER Schema,
  Bluetooth, DHCP and LLDP Schema APIs, is tracked outside this epic. Official
  builds still compile every supported Core component and builtin.
- The gate counts names, not behavior. A reviewer still checks that a new
  reference is a usable public path with binding tests. Core behavior is tested
  in Core CI.
- Core's inline constructors `tlv_text_codec`, `tlv_digits_codec` and
  `tlv_schema_number_codec` are not `TLV_API` symbols and do not appear in the
  report. The Rust codecs build the same descriptors.
- Rust has no safe API for raw C codec or Format callbacks. Applications
  convert through Rust closures (`StructureCodec`, Query providers) or compose
  Formats from Core's field encodings.
- In Core 0.7.1 only Schema producers track enclosing paths. The path fields and
  `path_text()` of Reader, Writer and non-Schema Query failures are therefore
  `None`.

## Updating the Core pin

This inventory is the baseline that a Core pin update is checked against:

1. Change the lock in a pull request. `Core API coverage` lists the symbols
   that were added and removed, each with its status.
2. For each added generic symbol, either bind it in `opentlv/src` with binding
   tests and an entry in this table, or add it to `ci/api-coverage.json` with a
   reason. Until one of these is done, the gate fails.
3. Remove exclusions that no longer match a symbol; the gate reports them as
   stale. The crate stops compiling when a referenced symbol is removed.
4. Update the pin, counts and changed rows above in the same pull request.

Regenerate the complete report locally:

```sh
python -B scripts/api_coverage.py --core ../opentlv --output build/api-coverage.json
python -B scripts/api_coverage.py --core ../opentlv --commit <new-sha> --base-commit <old-sha>
```

## Validation scope

`opentlv/tests/diagnostic_path.rs` covers the binding contract: owned paths
formatted after their input and Schema are dropped, top-level empty paths,
truncated paths of nested Schema and Query Schema failures, absent paths, and
paths that Core rejects. `tests/test_api_coverage.py` covers the completeness
configuration, failure after a complete report and the workflow wiring.
`tests/test_ci_contract.py` covers the gate prerequisite.

On 2026-10-24, the local PR and Main `linux-stable` configurations passed in WSL
Ubuntu with Rust 1.99.1 and the pinned Git Core dependency. PR ran 299 tests
with default features and 276 without features, each matching the inventory in
[`ci/matrix.json`](../../ci/matrix.json), along with rustfmt, Clippy, Rustdoc, the
runnable examples and the shared Query corpus (171 fixtures, 2,133 native
executions). Main ran the build and `quick_start` smoke. All 80 Python
infrastructure tests, source quality and the core completeness report also
passed. These are local working-tree checks, not hosted CI evidence. Full
configurations were not run.
