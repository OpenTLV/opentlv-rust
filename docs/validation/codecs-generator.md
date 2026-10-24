# Generic Codec and wire Generator coverage

Issue [#22](https://github.com/OpenTLV/opentlv-rust/issues/22) completes the
generic Codec and Generator gaps against Core `0.7.1`, commit
`476f1fbecde6b056419e43c9a6cea4d1ca8df1cf`, as pinned by
[`core-artifact.json`](../../opentlv-sys/core/core-artifact.json).
The comparison baseline is Rust commit
`5cb5cc1159ac65d7c5b75636ea50b760c66af35e` (before this issue's working changes).
The baseline was reconstructed from Git while completing the existing work;
it does not assume that uncommitted APIs were already available.

## Per-header inventory

Counts are `TLV_API` symbols referenced by the safe `opentlv/src` implementation,
excluding the documented ignores. Raw `opentlv-sys` declarations alone do not
count. References establish API reachability, not complete functional parity.

| Pinned Core header | Before | After | Public Rust mapping |
| --- | ---: | ---: | --- |
| `tlv/codec/codec.h` | 2/2 | 2/2 | Shared `tlv_codec_decode`/`encode` dispatch |
| `tlv/codec/diagnostic.h` | 4/5 | 4/4, 1 ignored | Existing owned `CodecDiagnostic`; initializer excluded below |
| `tlv/codec/number.h` | 2/2 | 2/2 | Existing `NumberCodec` |
| `tlv/codec/values.h` | 0/7 | 7/7 | `IntegerCodec<u8/u16/u32/i64>`, `BytesCodec` |
| `tlv/codec/ipv4.h` | 0/3 | 3/3 | `Ipv4Codec`, `Ipv4ListCodec`, `Ipv4List::get` |
| `tlv/codec/text.h` | 0/2 | 2/2 | `TextCodec`, `TextAlphabet` |
| `tlv/codec/digits.h` | 0/2 | 2/2 | `DigitsCodec` |
| `tlv/codec/structure.h` | 0/2 | 2/2 | `StructureCodec` with Rust conversion closures |
| `tlv/schema/number.h` | 0/2 | 2/2 | `SchemaNumberCodec` owning its `LengthRule` and number configuration |
| `tlv/generator.h` | 0/2 | 2/2 | `Generator`, `GeneratorCandidate`, `GeneratorLimits` |

Core coverage changes from **194/217 (89.4%)**, with 80 ignored symbols, to
**214/216 (99.1%)**, with 81 ignored. Builtins stay at **34/291 (11.7%)**.
The two remaining generic symbols, `tlv_diagnostic_path_string` and
`tlv_location_translate`, are outside this issue's Codec/Generator scope and
remain visible as missing. The Core lock, Cargo features and official native
build component selection are unchanged.

Regenerate the complete report, including every header and missing symbol:

```sh
python -B scripts/api_coverage.py --core ../opentlv --output build/api-coverage.json
```

## Intentional exclusions and ownership

- `tlv_codec_diagnostic_init` joins the existing diagnostic-producer ignore
  list: the C conversion entry points initialize their own output diagnostic.
  Rust owns the converted evidence after the call and does not expose a second
  public diagnostic initialization API.
- The inline `tlv_text_codec`, `tlv_digits_codec` and `tlv_schema_number_codec`
  constructors are not exported `TLV_API` symbols. Rust builds their descriptor
  layouts with the same native callbacks and a context that lives through each
  call. Configuration validation and conversions remain in Core.
- Integer codecs accept their exact C integer type. Bytes, text and IPv4-list
  decodes borrow input slices; string digit output is owned. Schema-number
  descriptors borrow the codec's owned rule only for a call.
- Structure codecs borrow the Format and Schema. Application closures run
  outside C; decode follows Core validation and encode precedes it. No
  application callback or temporary object is retained by C.
- Generator clones rebuild the C candidate table over their own tag bytes.
  Format lifetimes are preserved, calls share immutable state, and all random
  generation and wire behavior remain in Core. It generates wire-valid streams,
  not Schema- or Codec-valid application content.
- Protocol-specific Builtins parity and custom raw codec callbacks remain
  outside this public safe API expansion.

## Validation scope

The binding tests cover conversions and error evidence, borrowed results,
caller capacities, allocation bounds, deterministic seed/index replay, moved
and cloned candidate storage, concurrent immutable generation, composed
Formats, and StructureCodec validation order. Compile-fail Rustdoc examples
reject objects that outlive their input. A native C ABI probe compares the
sizes, alignments, every field offset and relevant constants for all eight new
FFI structures. Core remains responsible for exhaustive codec and generator
behavior tests.

The current test and example inventories are in
[`ci/matrix.json`](../../ci/matrix.json). On 2026-10-24, local Windows x86-64/MSVC
validation with Rust 1.99.1 and the pinned Git Core dependency passed:

| Cargo features | Passed tests, including Rustdoc and C ABI probes |
| --- | ---: |
| Default | 295 |
| None | 272 |
| LLDP only | 273 |
| NFC only | 272 |
| Document only | 294 |

Every per-target result matched the CI inventory. Rustfmt, Clippy with warnings
denied for both default and no-default-features, and Rustdoc with warnings
denied also passed, as did all ten runnable examples, the shared Query corpus
(171 fixtures, 2,133 native executions, no unavailable-provider fixtures), all
78 Python infrastructure tests, source quality, local documentation links and
whitespace checks. These are local working-tree checks, not hosted Full CI,
release or publication evidence. Linux, macOS, MSRV, sanitizers, packaging and
prebuilt linking were not rerun for this change locally.
