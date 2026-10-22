// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Raw, unsafe FFI bindings to the OpenTLV C API.
//!
//! This crate is the only place in the Rust bindings that declares `extern "C"`
//! items. Prefer the safe `opentlv` crate unless you need the C API directly.
//! Only the part of the C API needed by the safe crate is declared so far.

#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int, c_void};

mod codec_diagnostic;
mod diagnostic_strings;
pub use diagnostic_strings::*;
#[cfg(feature = "document")]
mod document;
mod program;
mod schema_report;
pub use codec_diagnostic::*;
#[cfg(feature = "document")]
pub use document::*;
pub use program::*;
pub use schema_report::*;

/// Logical TLV value length (`tlv_size_t`), always 64 bits wide.
pub type tlv_size_t = u64;

/// Result code returned by fallible OpenTLV functions (`tlv_result_t`).
/// Zero is success.
pub type tlv_result_t = c_int;

/// The operation succeeded (`TLV_OK`).
pub const TLV_OK: tlv_result_t = 0;
/// Normal end of iteration, or empty single-read input; not a failure (`TLV_END`).
pub const TLV_END: tlv_result_t = 1;
/// Incremental Reader requires more bytes or explicit EOF (`TLV_NEED_MORE_DATA`).
pub const TLV_NEED_MORE_DATA: tlv_result_t = 2;
/// Final input ends inside an element (`TLV_ERR_TRUNCATED`).
pub const TLV_ERR_TRUNCATED: tlv_result_t = 3;
/// A tag is malformed or invalid (`TLV_ERR_INVALID_TAG`).
pub const TLV_ERR_INVALID_TAG: tlv_result_t = 4;
/// Tag size is outside the supported range (`TLV_ERR_INVALID_TAG_SIZE`).
pub const TLV_ERR_INVALID_TAG_SIZE: tlv_result_t = 5;
/// A length is malformed or out of range (`TLV_ERR_INVALID_LENGTH`).
pub const TLV_ERR_INVALID_LENGTH: tlv_result_t = 6;
/// Primitive content is malformed (`TLV_ERR_INVALID_VALUE`).
pub const TLV_ERR_INVALID_VALUE: tlv_result_t = 7;
/// Text does not match the requested grammar (`TLV_ERR_SYNTAX`).
pub const TLV_ERR_SYNTAX: tlv_result_t = 8;
/// Input violates a schema rule (`TLV_ERR_SCHEMA`).
pub const TLV_ERR_SCHEMA: tlv_result_t = 9;
/// A required pointer argument is `NULL` (`TLV_ERR_NULL_ARG`).
pub const TLV_ERR_NULL_ARG: tlv_result_t = 10;
/// An argument has an invalid value (`TLV_ERR_INVALID_ARG`).
pub const TLV_ERR_INVALID_ARG: tlv_result_t = 11;
/// Operation forbidden by the current lifecycle state (`TLV_ERR_INVALID_STATE`).
pub const TLV_ERR_INVALID_STATE: tlv_result_t = 12;
/// A schema definition is invalid (`TLV_ERR_INVALID_SCHEMA`).
pub const TLV_ERR_INVALID_SCHEMA: tlv_result_t = 13;
/// A destination or workspace is too small (`TLV_ERR_BUFFER_TOO_SHORT`).
pub const TLV_ERR_BUFFER_TOO_SHORT: tlv_result_t = 14;
/// A configured limit was exceeded (`TLV_ERR_LIMIT`).
pub const TLV_ERR_LIMIT: tlv_result_t = 15;
/// A value does not fit the requested width (`TLV_ERR_OVERFLOW`).
pub const TLV_ERR_OVERFLOW: tlv_result_t = 16;
/// Logical size cannot fit the native address space (`TLV_ERR_NATIVE_SIZE`).
pub const TLV_ERR_NATIVE_SIZE: tlv_result_t = 17;
/// An allocation failed (`TLV_ERR_OUT_OF_MEMORY`).
pub const TLV_ERR_OUT_OF_MEMORY: tlv_result_t = 18;
/// A requested type or capability is unsupported (`TLV_ERR_UNSUPPORTED`).
pub const TLV_ERR_UNSUPPORTED: tlv_result_t = 19;
/// A visitor callback requested an error stop (`TLV_ERR_VISITOR`).
pub const TLV_ERR_VISITOR: tlv_result_t = 20;
/// A callback violated its contract (`TLV_ERR_CALLBACK`).
pub const TLV_ERR_CALLBACK: tlv_result_t = 21;

/// Byte order of a multi-byte integer (`tlv_byte_order_t`).
pub type tlv_byte_order_t = c_int;
/// Unknown byte order, rejected by the library (`TLV_BYTE_ORDER_UNKNOWN`).
pub const TLV_BYTE_ORDER_UNKNOWN: tlv_byte_order_t = 0;
/// Most significant byte first (`TLV_BYTE_ORDER_BIG_ENDIAN`).
pub const TLV_BYTE_ORDER_BIG_ENDIAN: tlv_byte_order_t = 1;
/// Least significant byte first (`TLV_BYTE_ORDER_LITTLE_ENDIAN`).
pub const TLV_BYTE_ORDER_LITTLE_ENDIAN: tlv_byte_order_t = 2;

/// Element order of a configurable fixed-width format (`tlv_element_order_t`).
pub type tlv_element_order_t = c_int;
/// Tag, then length, then value; the conventional wire layout (`TLV_ELEMENT_ORDER_TLV`).
pub const TLV_ELEMENT_ORDER_TLV: tlv_element_order_t = 0;
/// Length, then tag, then value; for example Bluetooth LTV (`TLV_ELEMENT_ORDER_LTV`).
pub const TLV_ELEMENT_ORDER_LTV: tlv_element_order_t = 1;

/// What a configurable fixed-width format's length field counts (`tlv_length_scope_t`).
pub type tlv_length_scope_t = c_int;
/// The length field counts only the value (`TLV_LENGTH_SCOPE_VALUE`).
pub const TLV_LENGTH_SCOPE_VALUE: tlv_length_scope_t = 0;
/// The length field counts the tag and the value (`TLV_LENGTH_SCOPE_TAG_AND_VALUE`).
pub const TLV_LENGTH_SCOPE_TAG_AND_VALUE: tlv_length_scope_t = 1;

/// A borrowed raw TLV tag (`tlv_tag_t`): a pointer to bytes and their count.
///
/// The type does not own the bytes and has no length limit. The layout does
/// not depend on any C build configuration.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tag_t {
    /// Tag bytes in wire order; may be null only when `size` is zero.
    pub data: *const u8,
    /// Number of bytes in `data`.
    pub size: usize,
}

/// A non-owning element of a TLV value (`tlv_value_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_value_t {
    /// Borrowed value bytes; may be null only when size is zero.
    pub data: *const u8,
    /// Decoded logical value byte count.
    pub size: tlv_size_t,
}

/// Borrowed original length-field bytes (`tlv_length_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_length_t {
    /// Original bytes in wire order.
    pub data: *const u8,
    /// Native encoded field byte count.
    pub size: usize,
}

/// Canonical borrowed TLV element (`tlv_element_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_element_t {
    /// Raw tag identity.
    pub tag: tlv_tag_t,
    /// Value bytes and decoded size.
    pub value: tlv_value_t,
}

/// Native optional source range.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_range_t {
    /// Offset; follows the corresponding C field contract.
    pub offset: usize,
    /// Size; follows the corresponding C field contract.
    pub size: usize,
    /// Present; follows the corresponding C field contract.
    pub present: c_int,
}
/// Format-produced failure information.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_format_error_t {
    /// Region; follows the corresponding C field contract.
    pub region: c_int,
    /// Offset; follows the corresponding C field contract.
    pub offset: usize,
    /// Has offset; follows the corresponding C field contract.
    pub has_offset: c_int,
    /// Required; follows the corresponding C field contract.
    pub required: tlv_size_t,
    /// Has required; follows the corresponding C field contract.
    pub has_required: c_int,
    /// Tag; follows the corresponding C field contract.
    pub tag: tlv_range_t,
    /// Length; follows the corresponding C field contract.
    pub length: tlv_range_t,
    /// Value; follows the corresponding C field contract.
    pub value: tlv_range_t,
}
/// Logical encoding sizes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_encoding_t {
    /// Header; follows the corresponding C field contract.
    pub header: tlv_size_t,
    /// Value; follows the corresponding C field contract.
    pub value: tlv_size_t,
    /// Trailer; follows the corresponding C field contract.
    pub trailer: tlv_size_t,
    /// Total; follows the corresponding C field contract.
    pub total: tlv_size_t,
}
/// Decoded Tag borrows exactly its source range (the default).
pub const TLV_TAG_BINDING_SOURCE: c_int = 0;
/// Decoded Tag borrows immutable format-supplied storage that outlives all results.
pub const TLV_TAG_BINDING_FORMAT: c_int = 1;
/// C enum describing the storage binding of a decoded identifier.
pub type tlv_tag_binding_t = c_int;

/// Immutable borrowed source and framing information.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_source_t {
    /// Data; follows the corresponding C field contract.
    pub data: *const u8,
    /// Size; follows the corresponding C field contract.
    pub size: usize,
    /// Header; follows the corresponding C field contract.
    pub header: tlv_range_t,
    /// Tag; follows the corresponding C field contract.
    pub tag: tlv_range_t,
    /// Length; follows the corresponding C field contract.
    pub length: tlv_range_t,
    /// Value; follows the corresponding C field contract.
    pub value: tlv_range_t,
    /// Trailer; follows the corresponding C field contract.
    pub trailer: tlv_range_t,
    /// Element; follows the corresponding C field contract.
    pub element: tlv_element_t,
    /// Format; follows the corresponding C field contract.
    pub format: *const tlv_format_t,
    /// Semantic identifier storage; the wire Tag range is only an envelope for FORMAT.
    pub tag_binding: tlv_tag_binding_t,
}
/// Semantic element plus its original wire representation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_decoded_t {
    /// Element; follows the corresponding C field contract.
    pub element: tlv_element_t,
    /// Source; follows the corresponding C field contract.
    pub source: tlv_source_t,
}
/// Complete framing decoder.
pub type tlv_decode_fn = unsafe extern "C" fn(
    *const c_void,
    *const u8,
    usize,
    *mut tlv_decoded_t,
    *mut tlv_format_error_t,
) -> tlv_result_t;
/// Logical sizing callback.
pub type tlv_measure_fn = unsafe extern "C" fn(
    *const c_void,
    *const tlv_element_t,
    *mut tlv_encoding_t,
    *mut tlv_format_error_t,
) -> tlv_result_t;
/// Complete framing encoder.
pub type tlv_encode_fn = unsafe extern "C" fn(
    *const c_void,
    *const tlv_element_t,
    *mut u8,
    usize,
    *mut usize,
    *mut tlv_format_error_t,
) -> tlv_result_t;
/// Canonical wire-format descriptor with borrowed immutable configuration.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_format_t {
    /// Context; follows the corresponding C field contract.
    pub context: *const c_void,
    /// Decode; follows the corresponding C field contract.
    pub decode: Option<tlv_decode_fn>,
    /// Measure; follows the corresponding C field contract.
    pub measure: Option<tlv_measure_fn>,
    /// Encode; follows the corresponding C field contract.
    pub encode: Option<tlv_encode_fn>,
    /// Is constructed; follows the corresponding C field contract.
    pub is_constructed: Option<tlv_is_constructed_fn>,
}

/// Sequential writer over a caller-owned buffer (`tlv_writer_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_writer_t {
    /// Borrowed format.
    pub format: *const tlv_format_t,
    /// Borrowed output buffer.
    pub buf: *mut u8,
    /// Capacity of `buf` in bytes.
    pub capacity: usize,
    /// Number of bytes written so far.
    pub pos: usize,
}

/// Sequential reader over a caller-owned buffer (`tlv_reader_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_reader_t {
    /// Borrowed format.
    pub format: *const tlv_format_t,
    /// Borrowed input buffer.
    pub data: *const u8,
    /// Input size in bytes.
    pub size: usize,
    /// Offset of the next element to read.
    pub pos: usize,
    /// Absolute logical offset of the current input window.
    pub base_offset: usize,
    /// Whether the input window ends at EOF.
    pub final_input: c_int,
}

/// Fixed-width raw identifier encoding.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_fixed_identifier_t {
    /// Nonzero identifier width in bytes.
    pub size: usize,
}

/// Fixed-width unsigned count encoding.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_fixed_length_t {
    /// Count width in bytes, from one through eight.
    pub size: usize,
    /// Explicit wire byte order.
    pub byte_order: tlv_byte_order_t,
}

/// State describing a configurable fixed-width format (`tlv_fixed_format_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_fixed_format_t {
    /// Raw identifier encoding.
    pub identifier: tlv_fixed_identifier_t,
    /// Unsigned count encoding.
    pub length: tlv_fixed_length_t,
    /// Where the length field falls relative to the tag.
    pub element_order: tlv_element_order_t,
    /// What the length field counts.
    pub length_scope: tlv_length_scope_t,
}

extern "C" {
    /// BER-TLV format.
    pub static tlv_format_ber: tlv_format_t;
    /// Definite EMV Contact Book 3 BER-TLV element framing.
    pub static tlv_format_emv: tlv_format_t;
    /// LLDP framing descriptor; requires a C library built with OPENTLV_LLDP=ON.
    #[cfg(feature = "lldp")]
    pub static tlv_format_lldp: tlv_format_t;
    /// Reports whether the linked C library includes LLDP framing.
    pub fn tlv_config_lldp() -> c_int;
    #[cfg(feature = "nfc")]
    pub static tlv_format_nfc_type2: tlv_format_t;
    pub fn tlv_config_nfc() -> c_int;
    /// CER format.
    pub static tlv_format_cer: tlv_format_t;
    /// DER format.
    pub static tlv_format_der: tlv_format_t;

    /// Computes the encoded size of an element without accessing value bytes.
    pub fn tlv_encoded_size(
        tag: tlv_tag_t,
        length: usize,
        format: *const tlv_format_t,
        size: *mut usize,
    ) -> tlv_result_t;
    /// Initializes a sequential writer over `buf`; both `buf` and `format` are borrowed.
    pub fn tlv_writer_init(
        writer: *mut tlv_writer_t,
        buf: *mut u8,
        capacity: usize,
        format: *const tlv_format_t,
    ) -> tlv_result_t;
    /// Writes one element at the writer's current position.
    pub fn tlv_writer_write(
        writer: *mut tlv_writer_t,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
    ) -> tlv_result_t;
    /// Returns the number of bytes written so far.
    pub fn tlv_writer_size(writer: *const tlv_writer_t) -> usize;
    /// Measures exact native storage using readable semantic content.
    pub fn tlv_element_encoded_size(
        element: *const tlv_element_t,
        format: *const tlv_format_t,
        size: *mut usize,
    ) -> tlv_result_t;
    /// Encodes an Element into caller-owned storage.
    pub fn tlv_write_element(
        data: *mut u8,
        capacity: usize,
        format: *const tlv_format_t,
        element: *const tlv_element_t,
        written: *mut usize,
    ) -> tlv_result_t;
    /// Appends an Element; advances only on success.
    pub fn tlv_writer_write_element(
        writer: *mut tlv_writer_t,
        element: *const tlv_element_t,
    ) -> tlv_result_t;
    /// Appends unvalidated raw bytes; overlap is supported.
    pub fn tlv_writer_copy_encoded(
        writer: *mut tlv_writer_t,
        data: *const u8,
        size: usize,
    ) -> tlv_result_t;
    /// Preserves immutable Source bytes after checking semantic equality.
    pub fn tlv_writer_preserve(
        writer: *mut tlv_writer_t,
        source: *const tlv_source_t,
        element: *const tlv_element_t,
    ) -> tlv_result_t;
    /// Returns remaining destination bytes.
    pub fn tlv_writer_remaining(writer: *const tlv_writer_t) -> usize;

    /// Initializes a sequential reader over `data`; both `data` and `format` are borrowed.
    pub fn tlv_reader_init(
        reader: *mut tlv_reader_t,
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
    ) -> tlv_result_t;
    /// Initializes non-final borrowed input at logical offset zero.
    pub fn tlv_reader_init_incremental(
        reader: *mut tlv_reader_t,
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
    ) -> tlv_result_t;

    /// Replaces input, preserving the unconsumed prefix and borrowed lifetimes.
    pub fn tlv_reader_set_input(
        reader: *mut tlv_reader_t,
        data: *const u8,
        size: usize,
        discard: usize,
        final_input: c_int,
    ) -> tlv_result_t;

    /// Returns the consumed prefix of the current window.
    pub fn tlv_reader_consumed(reader: *const tlv_reader_t) -> usize;
    /// Returns the absolute logical cursor offset.
    pub fn tlv_reader_offset(reader: *const tlv_reader_t) -> usize;

    /// Returns 1 only when final input has been consumed.
    pub fn tlv_reader_at_end(reader: *const tlv_reader_t) -> c_int;
    /// Reads the next element and advances the reader.
    pub fn tlv_reader_next(
        reader: *mut tlv_reader_t,
        out_element: *mut tlv_element_t,
    ) -> tlv_result_t;

    /// Initializes a format for the configurable fixed-width encoding, with
    /// both read and write capability; stores `config`'s address as the
    /// format's context.
    pub fn tlv_fixed_format_init(
        format: *mut tlv_format_t,
        config: *const tlv_fixed_format_t,
    ) -> tlv_result_t;

    /// Returns the library version as a NUL-terminated static string, for example `"0.6.0"`.
    pub fn tlv_version_string() -> *const c_char;
    /// Returns the major version number.
    pub fn tlv_version_major() -> u32;
    /// Returns the minor version number.
    pub fn tlv_version_minor() -> u32;
    /// Returns the patch version number.
    pub fn tlv_version_patch() -> u32;
    /// Returns a NUL-terminated static description of a result code.
    pub fn tlv_result_string(result: tlv_result_t) -> *const c_char;
    /// Narrows a length to the native `size_t`.
    pub fn tlv_size_to_native(length: tlv_size_t, size: *mut usize) -> tlv_result_t;
    /// Tests whether two tags have the same size and bytes.
    pub fn tlv_tag_equal(lhs: tlv_tag_t, rhs: tlv_tag_t) -> bool;
    /// Orders two tags lexicographically by their bytes.
    pub fn tlv_tag_compare(lhs: tlv_tag_t, rhs: tlv_tag_t) -> c_int;
}

/// Optional nesting predicate carried by `tlv_format_t::is_constructed` (`tlv_is_constructed_fn`).
pub type tlv_is_constructed_fn =
    unsafe extern "C" fn(context: *const c_void, tag: *const tlv_tag_t) -> c_int;

/// Permit only the minimum or maximum length; length_multiple still applies.
pub const TLV_SCHEMA_LENGTH_ENDPOINTS: u32 = 1;

/// Length rule for one tag (`tlv_schema_entry_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_schema_entry_t {
    /// Tag this entry describes.
    pub tag: tlv_tag_t,
    /// Minimum permitted value length, inclusive.
    pub min_length: usize,
    /// Maximum permitted value length, inclusive; `usize::MAX` is unrestricted.
    pub max_length: usize,
    /// Length policy bits; unknown bits are ignored.
    pub flags: u32,
    /// Borrowed name of the field this entry describes, or null if unnamed.
    pub name: *const c_char,
    /// Required value-length multiple in bytes; zero disables the constraint.
    pub length_multiple: usize,
}

/// Borrowed table of per-tag length rules (`tlv_schema_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_schema_t {
    /// Borrowed entries; may be null only when `count` is zero.
    pub entries: *const tlv_schema_entry_t,
    /// Number of entries.
    pub count: usize,
}

/// Expected form of a value (`tlv_schema_kind_t`).
pub type tlv_schema_kind_t = c_int;
/// The element may be primitive or constructed (`TLV_SCHEMA_ANY`).
pub const TLV_SCHEMA_ANY: tlv_schema_kind_t = 0;
/// The element must be primitive (`TLV_SCHEMA_PRIMITIVE`).
pub const TLV_SCHEMA_PRIMITIVE: tlv_schema_kind_t = 1;
/// The element must be constructed (`TLV_SCHEMA_CONSTRUCTED`).
pub const TLV_SCHEMA_CONSTRUCTED: tlv_schema_kind_t = 2;

/// Structural rule for one tag within a single parent (`tlv_structure_rule_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_structure_rule_t {
    /// Required borrowed field Schema; immutable storage must outlive the rule.
    pub entry: *const tlv_schema_entry_t,
    /// Minimum occurrences within the parent. Must be 0 when `group` is nonzero.
    pub min_occurs: usize,
    /// Maximum occurrences within the parent; `usize::MAX` is unrestricted.
    pub max_occurs: usize,
    /// Required form of the value.
    pub kind: tlv_schema_kind_t,
    /// Schema for the value's children, or null.
    pub children: *const tlv_structure_schema_t,
    /// Group this rule belongs to, or 0 if it stands alone (`tlv_structure_rule_t::group`).
    pub group: u32,
}

/// Ordering required among a scope's matched elements (`tlv_schema_order_t`).
pub type tlv_schema_order_t = c_int;
/// No relative order is required among matched elements (`TLV_SCHEMA_ORDER_ANY`).
pub const TLV_SCHEMA_ORDER_ANY: tlv_schema_order_t = 0;
/// Matched elements must appear in rule-table order (`TLV_SCHEMA_ORDER_SEQUENCE`).
pub const TLV_SCHEMA_ORDER_SEQUENCE: tlv_schema_order_t = 1;

/// A CHOICE-like group of mutually related alternative rules (`tlv_structure_group_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_structure_group_t {
    /// Nonzero identifier, referenced by `tlv_structure_rule_t::group`.
    pub id: u32,
    /// Minimum total occurrences across all member rules, inclusive.
    pub min_occurs: usize,
    /// Maximum total occurrences across all member rules, inclusive; `usize::MAX` is unrestricted.
    pub max_occurs: usize,
    /// Borrowed name of the group, for diagnostics, or null if unnamed.
    pub name: *const c_char,
}

/// Borrowed set of structural rules for one parent scope (`tlv_structure_schema_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_structure_schema_t {
    /// Borrowed rules; tags must be unique.
    pub rules: *const tlv_structure_rule_t,
    /// Number of rules.
    pub count: usize,
    /// Nonzero accepts tags that match no rule.
    pub allow_unknown: c_int,
    /// Borrowed table of alternative groups referenced by `rules[*].group`; may be null only
    /// when `group_count` is zero.
    pub groups: *const tlv_structure_group_t,
    /// Number of entries in `groups`.
    pub group_count: usize,
    /// Ordering required among this scope's matched elements.
    pub order: tlv_schema_order_t,
}

/// Value decoder callback of a codec.
pub type tlv_codec_decode_fn = unsafe extern "C" fn(
    context: *const c_void,
    data: *const u8,
    size: usize,
    value: *mut c_void,
    capacity: usize,
    diagnostic: *mut tlv_codec_diagnostic_t,
) -> tlv_result_t;

/// Value encoder callback of a codec.
pub type tlv_codec_encode_fn = unsafe extern "C" fn(
    context: *const c_void,
    value: *const c_void,
    size: usize,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
    diagnostic: *mut tlv_codec_diagnostic_t,
) -> tlv_result_t;

/// Borrowed codec descriptor (`tlv_codec_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_codec_t {
    /// Immutable context passed to both callbacks; may be null.
    pub context: *const c_void,
    /// Decoder; `None` if unsupported.
    pub decode: Option<tlv_codec_decode_fn>,
    /// Encoder; `None` if unsupported.
    pub encode: Option<tlv_codec_encode_fn>,
}

/// Dictionary context of an EMV tag (`tlv_emv_context_t`).
pub type tlv_emv_context_t = c_int;
/// Ordinary application data (`TLV_EMV_CONTEXT_BASE`).
pub const TLV_EMV_CONTEXT_BASE: tlv_emv_context_t = 0;
/// Inside 7F60 (`TLV_EMV_CONTEXT_BIT`).
pub const TLV_EMV_CONTEXT_BIT: tlv_emv_context_t = 1;
/// Inside A1 within 7F60 (`TLV_EMV_CONTEXT_BHT`).
pub const TLV_EMV_CONTEXT_BHT: tlv_emv_context_t = 2;
/// Inside level-2 A1/A2 within BHT (`TLV_EMV_CONTEXT_BHT_FORMAT`).
pub const TLV_EMV_CONTEXT_BHT_FORMAT: tlv_emv_context_t = 3;
/// Inside BF4A/BF4B or a terminal group (`TLV_EMV_CONTEXT_BIT_GROUP`).
pub const TLV_EMV_CONTEXT_BIT_GROUP: tlv_emv_context_t = 4;
/// Inside BF4C (`TLV_EMV_CONTEXT_BIOMETRIC_COUNTERS`).
pub const TLV_EMV_CONTEXT_BIOMETRIC_COUNTERS: tlv_emv_context_t = 5;
/// Inside BF4D (`TLV_EMV_CONTEXT_BIOMETRIC_ATTEMPTS`).
pub const TLV_EMV_CONTEXT_BIOMETRIC_ATTEMPTS: tlv_emv_context_t = 6;
/// Inside BF4E (`TLV_EMV_CONTEXT_BIOMETRIC_VERIFICATION`).
pub const TLV_EMV_CONTEXT_BIOMETRIC_VERIFICATION: tlv_emv_context_t = 7;
/// Number of contexts, and the "no new context" result (`TLV_EMV_CONTEXT_COUNT`).
pub const TLV_EMV_CONTEXT_COUNT: tlv_emv_context_t = 8;

/// Optional builtin presentation category (`tlv_emv_value_kind_t` in presentation.h).
pub type tlv_emv_value_kind_t = c_int;
/// Opaque bytes (`TLV_EMV_VALUE_BYTES`).
pub const TLV_EMV_VALUE_BYTES: tlv_emv_value_kind_t = 0;
/// Text bytes (`TLV_EMV_VALUE_TEXT`).
pub const TLV_EMV_VALUE_TEXT: tlv_emv_value_kind_t = 1;
/// Constructed template (`TLV_EMV_VALUE_TEMPLATE`).
pub const TLV_EMV_VALUE_TEMPLATE: tlv_emv_value_kind_t = 2;
/// `u64` number (`TLV_EMV_VALUE_NUMBER`).
pub const TLV_EMV_VALUE_NUMBER: tlv_emv_value_kind_t = 3;
/// `u64` bit flags (`TLV_EMV_VALUE_FLAGS`).
pub const TLV_EMV_VALUE_FLAGS: tlv_emv_value_kind_t = 4;
/// NUL-terminated decimal digits (`TLV_EMV_VALUE_DIGITS`).
pub const TLV_EMV_VALUE_DIGITS: tlv_emv_value_kind_t = 5;
/// [`tlv_emv_date_t`] (`TLV_EMV_VALUE_DATE`).
pub const TLV_EMV_VALUE_DATE: tlv_emv_value_kind_t = 6;
/// [`tlv_emv_time_t`] (`TLV_EMV_VALUE_TIME`).
pub const TLV_EMV_VALUE_TIME: tlv_emv_value_kind_t = 7;
/// Account type (`TLV_EMV_VALUE_ACCOUNT`).
pub const TLV_EMV_VALUE_ACCOUNT: tlv_emv_value_kind_t = 8;
/// [`tlv_emv_cryptogram_info_t`] (`TLV_EMV_VALUE_CRYPTOGRAM`).
pub const TLV_EMV_VALUE_CRYPTOGRAM: tlv_emv_value_kind_t = 9;
/// Biometric type (`TLV_EMV_VALUE_BIOMETRIC`).
pub const TLV_EMV_VALUE_BIOMETRIC: tlv_emv_value_kind_t = 10;
/// [`tlv_emv_number_list_t`] (`TLV_EMV_VALUE_NUMBER_LIST`).
pub const TLV_EMV_VALUE_NUMBER_LIST: tlv_emv_value_kind_t = 11;
/// [`tlv_emv_afl_t`] (`TLV_EMV_VALUE_AFL`).
pub const TLV_EMV_VALUE_AFL: tlv_emv_value_kind_t = 12;
/// [`tlv_emv_cvm_result_t`] (`TLV_EMV_VALUE_CVM_RESULT`).
pub const TLV_EMV_VALUE_CVM_RESULT: tlv_emv_value_kind_t = 13;
/// [`tlv_emv_track2_t`] (`TLV_EMV_VALUE_TRACK2`).
pub const TLV_EMV_VALUE_TRACK2: tlv_emv_value_kind_t = 14;
/// Not an entry in the builtin presentation profile (`TLV_EMV_VALUE_UNKNOWN`).
pub const TLV_EMV_VALUE_UNKNOWN: tlv_emv_value_kind_t = 15;

/// Decoded EMV date (`tlv_emv_date_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_emv_date_t {
    /// Two-digit year.
    pub year: u8,
    /// Month, 1..12.
    pub month: u8,
    /// Day of month.
    pub day: u8,
}

/// Decoded EMV time (`tlv_emv_time_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_emv_time_t {
    /// Hour.
    pub hour: u8,
    /// Minute.
    pub minute: u8,
    /// Second.
    pub second: u8,
}

/// Account type (`tlv_emv_account_type_t`), a C `enum`.
pub type tlv_emv_account_type_t = c_int;
/// Default account (`TLV_EMV_ACCOUNT_DEFAULT`).
pub const TLV_EMV_ACCOUNT_DEFAULT: tlv_emv_account_type_t = 0;
/// Savings account (`TLV_EMV_ACCOUNT_SAVINGS`).
pub const TLV_EMV_ACCOUNT_SAVINGS: tlv_emv_account_type_t = 10;
/// Cheque or debit account (`TLV_EMV_ACCOUNT_CHEQUE_DEBIT`).
pub const TLV_EMV_ACCOUNT_CHEQUE_DEBIT: tlv_emv_account_type_t = 20;
/// Credit account (`TLV_EMV_ACCOUNT_CREDIT`).
pub const TLV_EMV_ACCOUNT_CREDIT: tlv_emv_account_type_t = 30;

/// Cryptogram type (`tlv_emv_cryptogram_type_t`), a C `enum`; values 0..=3.
pub type tlv_emv_cryptogram_type_t = c_int;

/// Decoded Cryptogram Information Data (`tlv_emv_cryptogram_info_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_emv_cryptogram_info_t {
    /// Cryptogram type, from wire bits b8-b7.
    pub type_: tlv_emv_cryptogram_type_t,
    /// Remaining six bits.
    pub flags: u8,
}

/// Biometric type (`tlv_emv_biometric_type_t`), a C `enum`.
pub type tlv_emv_biometric_type_t = c_int;
/// Facial biometric (`TLV_EMV_BIOMETRIC_FACIAL`).
pub const TLV_EMV_BIOMETRIC_FACIAL: tlv_emv_biometric_type_t = 0x02;
/// Voice biometric (`TLV_EMV_BIOMETRIC_VOICE`).
pub const TLV_EMV_BIOMETRIC_VOICE: tlv_emv_biometric_type_t = 0x04;
/// Fingerprint biometric (`TLV_EMV_BIOMETRIC_FINGER`).
pub const TLV_EMV_BIOMETRIC_FINGER: tlv_emv_biometric_type_t = 0x08;
/// Iris biometric (`TLV_EMV_BIOMETRIC_IRIS`).
pub const TLV_EMV_BIOMETRIC_IRIS: tlv_emv_biometric_type_t = 0x10;
/// Palm biometric (`TLV_EMV_BIOMETRIC_PALM`).
pub const TLV_EMV_BIOMETRIC_PALM: tlv_emv_biometric_type_t = 0x020000;

/// Decoded list of up to four numbers (`tlv_emv_number_list_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_emv_number_list_t {
    /// List values; only the first `count` are populated.
    pub values: [u64; 4],
    /// Number of populated entries.
    pub count: usize,
}

/// One Application File Locator entry (`tlv_emv_afl_entry_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_emv_afl_entry_t {
    /// Short file identifier, 1-30.
    pub sfi: u8,
    /// First record, nonzero.
    pub first_record: u8,
    /// Last record, not below `first_record`.
    pub last_record: u8,
    /// Leading records used for offline data authentication.
    pub offline_auth_record_count: u8,
}

/// Maximum number of AFL entries (`TLV_EMV_AFL_MAX_ENTRIES`).
pub const TLV_EMV_AFL_MAX_ENTRIES: usize = 63;

/// Decoded Application File Locator (`tlv_emv_afl_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_emv_afl_t {
    /// Entries in wire order; only the first `count` are populated.
    pub entries: [tlv_emv_afl_entry_t; TLV_EMV_AFL_MAX_ENTRIES],
    /// Number of populated entries.
    pub count: usize,
}

/// CVM Results, preserved raw (`tlv_emv_cvm_result_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_emv_cvm_result_t {
    /// CVM method code.
    pub method: u8,
    /// CVM condition code.
    pub condition: u8,
    /// CVM outcome.
    pub result: u8,
}

/// Maximum PAN digits in Track 2 data (`TLV_EMV_TRACK2_PAN_MAX_DIGITS`).
pub const TLV_EMV_TRACK2_PAN_MAX_DIGITS: usize = 19;
/// Maximum discretionary digits (`TLV_EMV_TRACK2_DISCRETIONARY_MAX_DIGITS`).
pub const TLV_EMV_TRACK2_DISCRETIONARY_MAX_DIGITS: usize = 30;

/// Decoded Track 2 Equivalent Data (`tlv_emv_track2_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_emv_track2_t {
    /// PAN digits, NUL-terminated.
    pub pan: [c_char; TLV_EMV_TRACK2_PAN_MAX_DIGITS + 1],
    /// Expiration year, YY.
    pub expiration_year: u8,
    /// Expiration month, 1-12.
    pub expiration_month: u8,
    /// Three-digit service code.
    pub service_code: u16,
    /// Discretionary data digits, NUL-terminated.
    pub discretionary_data: [c_char; TLV_EMV_TRACK2_DISCRETIONARY_MAX_DIGITS + 1],
}

/// Generic borrowed identifier and descriptive name.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_definition_t {
    /// Canonical identifier bytes.
    pub tag: tlv_tag_t,
    /// Borrowed descriptive name.
    pub name: *const c_char,
}

/// Borrowed composition of Definition, Schema and Codec.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_emv_definition_t {
    /// Canonical identifier and descriptive name.
    pub definition: *const tlv_definition_t,
    /// Authoritative field schema and diagnostic symbol.
    pub schema: *const tlv_schema_entry_t,
    /// Selected semantic codec, or null for opaque input.
    pub codec: *const tlv_codec_t,
}

/// Inclusive resource limits for DER validation and writing (`tlv_der_limits_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_der_limits_t {
    /// Maximum number of constructed ancestors.
    pub max_depth: usize,
    /// Bounds the input, or the complete output when writing.
    pub max_input_size: usize,
    /// Bounds each value.
    pub max_value_size: usize,
    /// Bounds the total visited elements.
    pub max_elements: usize,
}

/// Inclusive resource limits for CER validation and writing (`tlv_cer_limits_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_cer_limits_t {
    /// Maximum number of constructed ancestors.
    pub max_depth: usize,
    /// Bounds the input, or the complete output when writing.
    pub max_input_size: usize,
    /// Bounds each element's content.
    pub max_value_size: usize,
    /// Bounds the total visited elements.
    pub max_elements: usize,
}

/// Preorder DER traversal callback (`tlv_der_visitor_t`); the safe crate passes `None`.
pub type tlv_der_visitor_t = unsafe extern "C" fn(
    element: *const tlv_element_t,
    depth: usize,
    offset: usize,
    context: *mut c_void,
) -> c_int;

/// Preorder CER traversal callback (`tlv_cer_visitor_t`); the safe crate passes `None`.
pub type tlv_cer_visitor_t = tlv_der_visitor_t;

extern "C" {
    /// Shared ASN.1 nesting predicate for BER, DER and CER.
    pub fn tlv_asn1_is_constructed(context: *const c_void, tag: *const tlv_tag_t) -> c_int;

    /// Looks up a tag's entry by linear search.
    pub fn tlv_schema_find(
        schema: *const tlv_schema_t,
        tag: *const tlv_tag_t,
    ) -> *const tlv_schema_entry_t;
    /// Validates a value length against an entry's bounds.
    pub fn tlv_schema_validate_length(
        entry: *const tlv_schema_entry_t,
        length: usize,
    ) -> tlv_result_t;
    /// Validates framing, nesting, lengths, occurrences and membership.
    pub fn tlv_schema_validate(
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
        schema: *const tlv_structure_schema_t,
        max_depth: usize,
        max_elements: usize,
        diagnostic: *mut tlv_schema_diagnostic_t,
    ) -> tlv_result_t;

    /// Structural schema for common EMV top-level data objects.
    pub static tlv_emv_structure_schema: tlv_structure_schema_t;
    /// EMV dictionary length schema for the base context.
    pub static tlv_emv_schema: tlv_schema_t;
    /// Returns the EMV dictionary schema for a context, or null.
    pub fn tlv_emv_schema_for(context: tlv_emv_context_t) -> *const tlv_schema_t;
    /// Looks up a tag's definition in a context, or null.
    pub fn tlv_emv_find(
        context: tlv_emv_context_t,
        tag: *const tlv_tag_t,
    ) -> *const tlv_emv_definition_t;
    /// Returns the context for a tag's children.
    pub fn tlv_emv_child_context(
        context: tlv_emv_context_t,
        tag: *const tlv_tag_t,
    ) -> tlv_emv_context_t;
    /// Builtin-only presentation profile; does not inspect callbacks or custom codecs.
    pub fn tlv_emv_builtin_value_kind(
        definition: *const tlv_emv_definition_t,
    ) -> tlv_emv_value_kind_t;
    /// Borrows the symbol from the entry's Schema.
    pub fn tlv_emv_symbol(definition: *const tlv_emv_definition_t) -> *const c_char;
    /// Derives length spacing from Schema.
    pub fn tlv_emv_length_step(definition: *const tlv_emv_definition_t) -> usize;
    pub fn tlv_emv_validate_length(
        definition: *const tlv_emv_definition_t,
        length: usize,
    ) -> tlv_result_t;
    /// Returns a curated label for a dictionary symbol, or null.
    pub fn tlv_emv_display_label(name: *const c_char) -> *const c_char;
    /// Describes an EMV value kind.
    pub fn tlv_emv_value_kind_description(kind: tlv_emv_value_kind_t) -> *const c_char;
    /// Codec for amounts (format n12).
    pub static tlv_emv_codec_amount: tlv_codec_t;

    /// Decodes a raw value with a codec.
    pub fn tlv_codec_decode(
        codec: *const tlv_codec_t,
        data: *const u8,
        size: usize,
        value: *mut c_void,
        capacity: usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    /// Encodes a C representation into raw value bytes with a codec.
    pub fn tlv_codec_encode(
        codec: *const tlv_codec_t,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    /// Describes a codec result.

    /// Default DER limits.
    pub static tlv_der_default_limits: tlv_der_limits_t;
    /// Validates one complete DER element.
    pub fn tlv_der_read(
        data: *const u8,
        size: usize,
        limits: *const tlv_der_limits_t,
        element: *mut tlv_element_t,
        consumed: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Strict counterpart of [`tlv_der_read`].
    pub fn tlv_der_read_strict(
        data: *const u8,
        size: usize,
        limits: *const tlv_der_limits_t,
        element: *mut tlv_element_t,
        consumed: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Validates all concatenated DER elements recursively.
    pub fn tlv_der_visit(
        data: *const u8,
        size: usize,
        limits: *const tlv_der_limits_t,
        visitor: Option<tlv_der_visitor_t>,
        context: *mut c_void,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Strict counterpart of [`tlv_der_visit`].
    pub fn tlv_der_visit_strict(
        data: *const u8,
        size: usize,
        limits: *const tlv_der_limits_t,
        visitor: Option<tlv_der_visitor_t>,
        context: *mut c_void,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Writes a canonical DER element.
    pub fn tlv_der_write(
        data: *mut u8,
        capacity: usize,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
        limits: *const tlv_der_limits_t,
        written: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Strict counterpart of [`tlv_der_write`].
    pub fn tlv_der_write_strict(
        data: *mut u8,
        capacity: usize,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
        limits: *const tlv_der_limits_t,
        written: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;

    /// Default CER limits.
    pub static tlv_cer_default_limits: tlv_cer_limits_t;
    /// Validates one complete CER element.
    pub fn tlv_cer_read(
        data: *const u8,
        size: usize,
        limits: *const tlv_cer_limits_t,
        element: *mut tlv_element_t,
        consumed: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Strict counterpart of [`tlv_cer_read`].
    pub fn tlv_cer_read_strict(
        data: *const u8,
        size: usize,
        limits: *const tlv_cer_limits_t,
        element: *mut tlv_element_t,
        consumed: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Validates all concatenated CER elements recursively.
    pub fn tlv_cer_visit(
        data: *const u8,
        size: usize,
        limits: *const tlv_cer_limits_t,
        visitor: Option<tlv_cer_visitor_t>,
        context: *mut c_void,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Strict counterpart of [`tlv_cer_visit`].
    pub fn tlv_cer_visit_strict(
        data: *const u8,
        size: usize,
        limits: *const tlv_cer_limits_t,
        visitor: Option<tlv_cer_visitor_t>,
        context: *mut c_void,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Writes a canonical CER element.
    pub fn tlv_cer_write(
        data: *mut u8,
        capacity: usize,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
        limits: *const tlv_cer_limits_t,
        written: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    /// Strict counterpart of [`tlv_cer_write`].
    pub fn tlv_cer_write_strict(
        data: *mut u8,
        capacity: usize,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
        limits: *const tlv_cer_limits_t,
        written: *mut usize,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
}

extern "C" {
    /// Decode semantic content and original source information.
    pub fn tlv_format_decode(
        format: *const tlv_format_t,
        data: *const u8,
        size: usize,
        decoded: *mut tlv_decoded_t,
        error: *mut tlv_format_error_t,
    ) -> tlv_result_t;
    /// Compute exact logical framing sizes.
    pub fn tlv_format_measure(
        format: *const tlv_format_t,
        element: *const tlv_element_t,
        encoding: *mut tlv_encoding_t,
        error: *mut tlv_format_error_t,
    ) -> tlv_result_t;
    /// Encode semantic content with complete framing.
    pub fn tlv_format_encode(
        format: *const tlv_format_t,
        element: *const tlv_element_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        error: *mut tlv_format_error_t,
    ) -> tlv_result_t;
    /// Reproduce immutable original bytes after checking semantic equality.
    pub fn tlv_source_preserve(
        source: *const tlv_source_t,
        element: *const tlv_element_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn links_and_reports_version() {
        let version = unsafe { CStr::from_ptr(tlv_version_string()) }
            .to_str()
            .unwrap();
        let expected = unsafe {
            format!(
                "{}.{}.{}",
                tlv_version_major(),
                tlv_version_minor(),
                tlv_version_patch()
            )
        };
        assert!(version.starts_with(&expected), "{version} vs {expected}");
    }

    #[test]
    fn result_string_describes_ok() {
        let message = unsafe { CStr::from_ptr(tlv_result_string(TLV_OK)) };
        assert!(!message.to_bytes().is_empty());
    }
}

extern "C" {
    pub fn tlv_reader_diagnostic_init(diagnostic: *mut tlv_reader_diagnostic_t);
    pub fn tlv_reader_next_source_diag(
        reader: *mut tlv_reader_t,
        element: *mut tlv_element_t,
        source: *mut tlv_source_t,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_reader_init(
        reader: *mut tlv_tree_reader_t,
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
        frames: *mut tlv_tree_frame_t,
        capacity: usize,
        max_depth: usize,
        max_elements: usize,
    ) -> tlv_result_t;
    pub fn tlv_tree_reader_init_incremental(
        reader: *mut tlv_tree_reader_t,
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
        frames: *mut tlv_tree_frame_t,
        capacity: usize,
        max_depth: usize,
        max_elements: usize,
    ) -> tlv_result_t;
    pub fn tlv_tree_reader_next_diag(
        reader: *mut tlv_tree_reader_t,
        item: *mut tlv_tree_item_t,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_reader_set_input(
        reader: *mut tlv_tree_reader_t,
        data: *const u8,
        size: usize,
        discard: usize,
        final_input: c_int,
    ) -> tlv_result_t;
    pub fn tlv_tree_reader_skip_subtree(reader: *mut tlv_tree_reader_t) -> tlv_result_t;
    pub fn tlv_tree_reader_consumed(reader: *const tlv_tree_reader_t) -> usize;
    pub fn tlv_tree_reader_offset(reader: *const tlv_tree_reader_t) -> usize;
    pub fn tlv_tree_reader_at_end(reader: *const tlv_tree_reader_t) -> c_int;
}

// Canonical Reader state and diagnostics. Keep repr(C) fields in header order.
/// Self-contained canonical query storage.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_query_t {
    opaque: [u64; 81],
}
/// Native opaque matcher borrowing stable query storage.
#[repr(C)]
pub union tlv_query_matcher_t {
    opaque: [u8; 16],
    alignment: *const c_void,
    size_alignment: usize,
}

extern "C" {
    pub fn tlv_query_parse(
        text: *const c_char,
        query: *mut tlv_query_t,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_parse_n(
        text: *const c_char,
        size: usize,
        query: *mut tlv_query_t,
        diagnostic: *mut tlv_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_count(query: *const tlv_query_t) -> usize;
    pub fn tlv_query_format(
        query: *const tlv_query_t,
        output: *mut c_char,
        capacity: usize,
        required: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_query_matcher_rebind(
        matcher: *mut tlv_query_matcher_t,
        query: *const tlv_query_t,
    ) -> tlv_result_t;
    pub fn tlv_query_step(query: *const tlv_query_t, index: usize) -> tlv_tag_t;
    pub fn tlv_query_matcher_init(
        matcher: *mut tlv_query_matcher_t,
        query: *const tlv_query_t,
    ) -> tlv_result_t;
    pub fn tlv_query_matcher_visit(
        matcher: *mut tlv_query_matcher_t,
        tag: *const tlv_tag_t,
        depth: usize,
    ) -> c_int;
    pub fn tlv_query_visit(
        reader: *mut tlv_tree_reader_t,
        matcher: *mut tlv_query_matcher_t,
        visitor: Option<
            unsafe extern "C" fn(*const tlv_element_t, usize, usize, *mut c_void) -> c_int,
        >,
        context: *mut c_void,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
}

extern "C" {
    /// Visit the remaining tree with optional structured failure detail.
    pub fn tlv_tree_reader_visit(
        reader: *mut tlv_tree_reader_t,
        visitor: Option<
            unsafe extern "C" fn(*const tlv_element_t, usize, usize, *mut c_void) -> c_int,
        >,
        context: *mut c_void,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_reader_visit_diag(
        reader: *mut tlv_reader_t,
        visitor: Option<unsafe extern "C" fn(*const tlv_element_t, *mut c_void) -> c_int>,
        context: *mut c_void,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
}

/// Native tlv_diagnostic state.
/// Borrowed native context chain. Strings and nodes must outlive inspection.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_diagnostic_context_t {
    pub layer: *const c_char,
    pub key: *const c_char,
    pub value: *const c_char,
    pub next: *const tlv_diagnostic_context_t,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_diagnostic_t {
    /// Stable result.
    pub code: tlv_result_t,
    /// Severity.
    pub severity: c_int,
    /// Primary evidence location.
    pub location: tlv_location_t,
    /// Borrowed expected text.
    pub expected: *const c_char,
    /// Borrowed actual text.
    pub actual: *const c_char,
    /// Borrowed contexts.
    pub contexts: *const tlv_diagnostic_context_t,
    /// Whether the enclosing path was tracked.
    pub has_path: c_int,
    /// Path copied by value; identifier bytes remain borrowed.
    pub path: tlv_diagnostic_path_t,
}
/// Native coordinate domain and checked half-open bounds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_location_t {
    /// Coordinate domain.
    pub domain: c_int,
    /// Anchor kind, zero for unknown.
    pub kind: c_int,
    /// Inclusive start.
    pub begin: usize,
    /// Exclusive end; equal to begin for points and boundaries.
    pub end: usize,
}
/// Native tlv_reader_diagnostic state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_reader_diagnostic_t {
    /// Common result, location and path.
    pub diagnostic: tlv_diagnostic_t,
    /// Reader-specific evidence.
    pub detail: tlv_reader_detail_t,
}
/// Native Reader-specific evidence without common diagnostic metadata.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_reader_detail_t {
    /// Corresponding C field.
    pub operation: c_int,
    /// Corresponding C field.
    pub has_tag: c_int,
    /// Corresponding C field.
    pub tag: tlv_tag_t,
    /// Corresponding C field.
    pub has_tag_offset: c_int,
    /// Corresponding C field.
    pub tag_offset: usize,
    /// Corresponding C field.
    pub has_length_offset: c_int,
    /// Corresponding C field.
    pub length_offset: usize,
    /// Corresponding C field.
    pub has_value_offset: c_int,
    /// Corresponding C field.
    pub value_offset: usize,
    /// Corresponding C field.
    pub has_declared_length: c_int,
    /// Corresponding C field.
    pub declared_length: tlv_size_t,
    /// Corresponding C field.
    pub has_raw_length: c_int,
    /// Corresponding C field.
    pub raw_length: tlv_length_t,
    /// Corresponding C field.
    pub has_available: c_int,
    /// Corresponding C field.
    pub available: usize,
    /// Corresponding C field.
    pub has_enclosing_end: c_int,
    /// Corresponding C field.
    pub enclosing_end: usize,
    /// Corresponding C field.
    pub has_required: c_int,
    /// Corresponding C field.
    pub required: tlv_size_t,
}
/// Native tlv_tree_frame state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tree_frame_t {
    /// Corresponding C field.
    pub end: usize,
    /// Corresponding C field.
    pub resume: usize,
}
/// Native tlv_tree_item state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tree_item_t {
    /// Corresponding C field.
    pub element: tlv_element_t,
    /// Corresponding C field.
    pub source: tlv_source_t,
    /// Corresponding C field.
    pub depth: usize,
    /// Corresponding C field.
    pub offset: usize,
    /// Corresponding C field.
    pub constructed: c_int,
}
/// Native tlv_tree_reader state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tree_reader_t {
    /// Corresponding C field.
    pub input: tlv_reader_t,
    /// Corresponding C field.
    pub frames: *mut tlv_tree_frame_t,
    /// Corresponding C field.
    pub capacity: usize,
    /// Corresponding C field.
    pub max_depth: usize,
    /// Corresponding C field.
    pub max_elements: usize,
    /// Corresponding C field.
    pub count: usize,
    /// Corresponding C field.
    pub depth: usize,
    /// Corresponding C field.
    pub pending: tlv_tree_frame_t,
    /// Corresponding C field.
    pub descend_pending: c_int,
    /// Pending empty or skipped END.
    pub end_pending: c_int,
    /// Omitted descendants on pending END.
    pub skipped: c_int,
    /// Node-only projection selected.
    pub item_projection: c_int,
}

// Canonical bounded Tree Writer state.
extern "C" {
    pub fn tlv_tree_writer_init(
        writer: *mut tlv_tree_writer_t,
        data: *mut u8,
        size: usize,
        format: *const tlv_format_t,
        frames: *mut tlv_tree_writer_frame_t,
        capacity: usize,
        scratch: *mut u8,
        scratch_capacity: usize,
        max_depth: usize,
        max_elements: usize,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_begin_diag(
        writer: *mut tlv_tree_writer_t,
        tag: tlv_tag_t,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_write_element_diag(
        writer: *mut tlv_tree_writer_t,
        element: *const tlv_element_t,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_end_diag(
        writer: *mut tlv_tree_writer_t,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_finish(writer: *const tlv_tree_writer_t) -> tlv_result_t;
    pub fn tlv_tree_writer_size(writer: *const tlv_tree_writer_t) -> usize;
    pub fn tlv_writer_diagnostic_init(diagnostic: *mut tlv_writer_diagnostic_t);
    pub fn tlv_element_encoded_size_diag(
        element: *const tlv_element_t,
        format: *const tlv_format_t,
        size: *mut usize,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_write_element_diag(
        data: *mut u8,
        capacity: usize,
        format: *const tlv_format_t,
        element: *const tlv_element_t,
        written: *mut usize,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_writer_write_diag(
        writer: *mut tlv_writer_t,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_writer_write_element_diag(
        writer: *mut tlv_writer_t,
        element: *const tlv_element_t,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_writer_copy_encoded_diag(
        writer: *mut tlv_writer_t,
        data: *const u8,
        size: usize,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_writer_preserve_diag(
        writer: *mut tlv_writer_t,
        source: *const tlv_source_t,
        element: *const tlv_element_t,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_measure(
        format: *const tlv_format_t,
        next: Option<
            unsafe extern "C" fn(
                *mut c_void,
                *mut tlv_element_t,
                *mut usize,
                *mut c_int,
            ) -> tlv_result_t,
        >,
        context: *mut c_void,
        workspace: *mut tlv_tree_writer_workspace_t,
        max_depth: usize,
        max_elements: usize,
        size: *mut usize,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
}

/// Caller-owned native tree measurement storage.
#[repr(C)]
pub struct tlv_tree_writer_workspace_t {
    /// Structural stack storage.
    pub frames: *mut tlv_tree_writer_frame_t,
    /// Stack capacity.
    pub frame_capacity: usize,
    /// Staged encoding storage.
    pub data: *mut u8,
    /// Staged capacity.
    pub data_capacity: usize,
    /// Parent closure scratch storage.
    pub scratch: *mut u8,
    /// Scratch capacity.
    pub scratch_capacity: usize,
    /// Next required output capacity on exhaustion.
    pub required_data: usize,
    /// Next required scratch capacity on exhaustion.
    pub required_scratch: usize,
}

/// Native tlv_writer_diagnostic state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_writer_diagnostic_t {
    /// Corresponding C field.
    pub diagnostic: tlv_diagnostic_t,
    /// Corresponding C field.
    pub operation: c_int,
    /// Corresponding C field.
    pub has_tag: c_int,
    /// Corresponding C field.
    pub tag: tlv_tag_t,
    /// Corresponding C field.
    pub has_length: c_int,
    /// Corresponding C field.
    pub length: usize,
    /// Corresponding C field.
    pub has_required: c_int,
    /// Corresponding C field.
    pub required: usize,
    /// Corresponding C field.
    pub has_available: c_int,
    /// Corresponding C field.
    pub available: usize,
}
/// Native tlv_tree_writer_frame state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tree_writer_frame_t {
    /// Corresponding C field.
    pub tag: tlv_tag_t,
    /// Corresponding C field.
    pub start: usize,
}
/// Native tlv_tree_writer state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tree_writer_t {
    /// Corresponding C field.
    pub output: tlv_writer_t,
    /// Corresponding C field.
    pub frames: *mut tlv_tree_writer_frame_t,
    /// Corresponding C field.
    pub capacity: usize,
    /// Corresponding C field.
    pub depth: usize,
    /// Corresponding C field.
    pub max_depth: usize,
    /// Corresponding C field.
    pub max_elements: usize,
    /// Corresponding C field.
    pub count: usize,
    /// Corresponding C field.
    pub scratch: *mut u8,
    /// Corresponding C field.
    pub scratch_capacity: usize,
    /// Optional owned-by-caller Tag arena.
    pub tags: *mut u8,
    /// Tag arena capacity.
    pub tags_capacity: usize,
    /// Tag arena used bytes.
    pub tags_used: usize,
}

/// Configuration of the canonical uint64 numeric codec.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_number_codec_config_t {
    /// Binary BE, binary LE or packed BCD.
    pub encoding: c_int,
    /// Fixed width, or zero for minimal encoding.
    pub width: usize,
    /// BCD precision, zero for binary encoding.
    pub digits: u32,
}
extern "C" {
    pub fn tlv_number_decode(
        context: *const c_void,
        data: *const u8,
        size: usize,
        value: *mut c_void,
        capacity: usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_number_encode(
        context: *const c_void,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
}

/// Borrowed table of generic Definitions.
#[repr(C)]
pub struct tlv_definition_registry_t {
    /// Immutable entries.
    pub entries: *const tlv_definition_t,
    /// Entry count.
    pub count: usize,
}
extern "C" {
    pub fn tlv_definition_find(
        registry: *const tlv_definition_registry_t,
        tag: *const tlv_tag_t,
    ) -> *const tlv_definition_t;
}

/// Canonical structural event shared by Reader and Writer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tree_event_t {
    pub kind: c_int,
    pub element: tlv_element_t,
    pub source: tlv_source_t,
    pub depth: usize,
    pub offset: usize,
    pub skipped: c_int,
}
pub const TLV_TREE_BEGIN: c_int = 0;
pub const TLV_TREE_ELEMENT: c_int = 1;
pub const TLV_TREE_END: c_int = 2;
extern "C" {
    pub fn tlv_tree_writer_measure_events(
        format: *const tlv_format_t,
        next: Option<unsafe extern "C" fn(*mut c_void, *mut tlv_tree_event_t) -> tlv_result_t>,
        context: *mut c_void,
        workspace: *mut tlv_tree_writer_workspace_t,
        max_depth: usize,
        max_elements: usize,
        size: *mut usize,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;

    pub fn tlv_tree_reader_next_event_diag(
        reader: *mut tlv_tree_reader_t,
        event: *mut tlv_tree_event_t,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_write_event_diag(
        writer: *mut tlv_tree_writer_t,
        event: *const tlv_tree_event_t,
        diagnostic: *mut tlv_writer_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_tree_writer_set_tag_storage(
        writer: *mut tlv_tree_writer_t,
        data: *mut u8,
        capacity: usize,
    ) -> tlv_result_t;
}
