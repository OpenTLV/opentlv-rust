// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Field encodings (`tlv/field/*.h`), Format composition (`tlv/formats/*.h`)
//! and the generic descriptor operations of `tlv/format.h`.

use super::*;
use std::os::raw::c_uint;

/// Header or unnamed header field (`TLV_REGION_HEADER`).
pub const TLV_REGION_HEADER: c_int = 0;
/// Identifier field (`TLV_REGION_TAG`).
pub const TLV_REGION_TAG: c_int = 1;
/// Encoded length field (`TLV_REGION_LENGTH`).
pub const TLV_REGION_LENGTH: c_int = 2;
/// Semantic value bytes (`TLV_REGION_VALUE`).
pub const TLV_REGION_VALUE: c_int = 3;
/// Trailing framing (`TLV_REGION_TRAILER`).
pub const TLV_REGION_TRAILER: c_int = 4;

/// Constructed classification by an exact masked identifier byte (`tlv_constructed_bit_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_constructed_bit_t {
    /// Zero-based canonical identifier byte index.
    pub byte_index: usize,
    /// Nonzero mask of tested bits.
    pub mask: u8,
    /// Required masked value.
    pub value: u8,
}

/// Optional constraints on a variable identifier (`tlv_identifier_policy_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_identifier_policy_t {
    /// Forbidden first bytes; null only with a zero count.
    pub forbidden_leading: *const u8,
    /// Number of bytes in `forbidden_leading`.
    pub forbidden_leading_count: usize,
    /// Boolean: reject a zero first escaped payload.
    pub reject_zero_first_payload: c_uint,
    /// Boolean: require the shortest identifier number representation.
    pub require_minimal: c_uint,
}

/// Allowed definite count forms and limits (`tlv_length_policy_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_length_policy_t {
    /// Boolean: permit inline counts.
    pub allow_short: c_uint,
    /// Boolean: permit long-form counts.
    pub allow_long: c_uint,
    /// Boolean: reject padding and unnecessary long form.
    pub require_minimal: c_uint,
    /// Maximum following octets.
    pub max_long_octets: usize,
    /// Inclusive count limit.
    pub max_value: tlv_size_t,
}

/// Inline identifier bits with an escape into continuation octets (`tlv_variable_identifier_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_variable_identifier_t {
    /// Nonzero mask selecting inline bits in the first octet.
    pub inline_mask: u8,
    /// Escape pattern within `inline_mask`.
    pub escape: u8,
    /// Exactly one bit; set means another octet follows.
    pub continuation_bit: u8,
    /// Nonzero payload mask, disjoint from `continuation_bit`.
    pub payload_mask: u8,
    /// Maximum complete identifier width in bytes.
    pub max_size: usize,
    /// Optional borrowed wire constraints.
    pub policy: *const tlv_identifier_policy_t,
}

/// Short count or a prefix giving the following octet count (`tlv_variable_length_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_variable_length_t {
    /// Exactly one bit selecting long form.
    pub long_form_bit: u8,
    /// Nonzero mask, disjoint from `long_form_bit`.
    pub payload_mask: u8,
    /// Order of the long-form count octets.
    pub byte_order: tlv_byte_order_t,
    /// Optional borrowed definite count policy.
    pub policy: *const tlv_length_policy_t,
}

/// One-byte inline count or an escape and a fixed-width count (`tlv_escaped_length_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_escaped_length_t {
    /// Nonzero escape marker and exclusive inline limit.
    pub escape: u8,
    /// Extended count width, one through eight bytes.
    pub extended_size: usize,
    /// Explicit wire byte order.
    pub byte_order: tlv_byte_order_t,
    /// Inclusive minimum extended count.
    pub min_extended: tlv_size_t,
    /// Inclusive maximum count.
    pub max_length: tlv_size_t,
}

/// One unsigned bit field within a fixed-size wire integer (`tlv_packed_field_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_packed_field_t {
    /// Backing integer width in bytes, 1..8.
    pub storage_size: usize,
    /// Least significant field bit, 0..63.
    pub bit_offset: c_uint,
    /// Field width, 1..64.
    pub bit_width: c_uint,
    /// Explicit wire byte order.
    pub byte_order: tlv_byte_order_t,
}

/// Identifier decoder (`tlv_read_tag_fn`).
pub type tlv_read_tag_fn = unsafe extern "C" fn(
    context: *const c_void,
    data: *const u8,
    size: usize,
    tag: *mut tlv_tag_t,
    consumed: *mut usize,
) -> tlv_result_t;
/// Count decoder (`tlv_read_length_fn`).
pub type tlv_read_length_fn = unsafe extern "C" fn(
    context: *const c_void,
    data: *const u8,
    size: usize,
    length: *mut tlv_size_t,
    consumed: *mut usize,
) -> tlv_result_t;
/// Identifier encoder and width query (`tlv_write_tag_fn`).
pub type tlv_write_tag_fn = unsafe extern "C" fn(
    context: *const c_void,
    tag: *const tlv_tag_t,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> tlv_result_t;
/// Count encoder and width query (`tlv_write_length_fn`).
pub type tlv_write_length_fn = unsafe extern "C" fn(
    context: *const c_void,
    length: tlv_size_t,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> tlv_result_t;
/// Terminated framing resolver (`tlv_resolve_bounds_fn`).
pub type tlv_resolve_bounds_fn = unsafe extern "C" fn(
    context: *const c_void,
    tag: *const tlv_tag_t,
    data: *const u8,
    size: usize,
    length_size: *mut usize,
    value_size: *mut tlv_size_t,
    trailer_size: *mut usize,
    error: *mut tlv_format_error_t,
) -> tlv_result_t;

/// Borrowed field encoding composition (`tlv_field_composition_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_field_composition_t {
    /// Immutable encoding context.
    pub context: *const c_void,
    /// Identifier decoder.
    pub read_tag: Option<tlv_read_tag_fn>,
    /// Count decoder.
    pub read_length: Option<tlv_read_length_fn>,
    /// Optional terminated framing resolver.
    pub resolve: Option<tlv_resolve_bounds_fn>,
    /// Identifier encoder and width query.
    pub write_tag: Option<tlv_write_tag_fn>,
    /// Count encoder and width query.
    pub write_length: Option<tlv_write_length_fn>,
    /// Wire field order.
    pub element_order: tlv_element_order_t,
    /// Wire count meaning.
    pub length_scope: tlv_length_scope_t,
}

/// Fixed-width binary fields (`tlv_binary_composition_t`, also `tlv_fixed_format_t`).
pub type tlv_binary_composition_t = tlv_fixed_format_t;

/// Fixed binary framing with identifier-selected tag-only elements
/// (`tlv_tagged_binary_composition_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tagged_binary_composition_t {
    /// Default binary TLV field configuration.
    pub fields: tlv_binary_composition_t,
    /// Identifier table; null only when `count` is zero.
    pub tag_only: *const tlv_tag_t,
    /// Number of table entries.
    pub count: usize,
}

/// Field composition with identifier-selected tag-only elements
/// (`tlv_tagged_fields_composition_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_tagged_fields_composition_t {
    /// Default field encoding composition.
    pub fields: tlv_field_composition_t,
    /// Identifier table; null only when `count` is zero.
    pub tag_only: *const tlv_tag_t,
    /// Number of table entries.
    pub count: usize,
}

/// Fixed identifiers with escape-prefixed lengths (`tlv_escaped_format_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_escaped_format_t {
    /// Nonzero fixed identifier width.
    pub identifier: tlv_fixed_identifier_t,
    /// Count encoding and accepted range.
    pub length: tlv_escaped_length_t,
    /// TLV or LTV ordering.
    pub element_order: tlv_element_order_t,
    /// Value or identifier-and-value count.
    pub length_scope: tlv_length_scope_t,
    /// Tag-only identifier table; null only when `count` is zero.
    pub tag_only: *const tlv_tag_t,
    /// Number of table entries.
    pub count: usize,
}

/// Definite packed Tag/Length header (`tlv_packed_layout_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_packed_layout_t {
    /// Header size in bytes, 1..8.
    pub header_size: usize,
    /// Packed unsigned identifier.
    pub tag: tlv_packed_field_t,
    /// Packed unsigned count.
    pub length: tlv_packed_field_t,
    /// Value or wire Tag envelope plus Value.
    pub length_scope: tlv_length_scope_t,
    /// Immutable canonical identifier table.
    pub tag_storage: *const u8,
    /// Canonical identifier width.
    pub tag_size: usize,
    /// Available table capacity in bytes.
    pub tag_storage_size: usize,
}

/// Definite variable-width field configuration (`tlv_variable_format_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_variable_format_t {
    /// Identifier encoding.
    pub identifier: tlv_variable_identifier_t,
    /// Count encoding.
    pub length: tlv_variable_length_t,
    /// Identifier/count field order.
    pub element_order: tlv_element_order_t,
    /// What the encoded count covers.
    pub length_scope: tlv_length_scope_t,
    /// Optional borrowed Tag bit predicate.
    pub constructed: *const tlv_constructed_bit_t,
}

extern "C" {
    /// Tests a canonical Tag against a borrowed bit predicate.
    pub fn tlv_constructed_bit_predicate(context: *const c_void, tag: *const tlv_tag_t) -> c_int;
    /// Initializes a descriptor from callbacks without allocation.
    pub fn tlv_format_init(
        format: *mut tlv_format_t,
        context: *const c_void,
        decode: Option<tlv_decode_fn>,
        measure: Option<tlv_measure_fn>,
        encode: Option<tlv_encode_fn>,
    ) -> tlv_result_t;
    /// Returns nonzero if the descriptor can decode.
    pub fn tlv_format_can_read(format: *const tlv_format_t) -> c_int;
    /// Returns nonzero if the descriptor can measure and encode.
    pub fn tlv_format_can_write(format: *const tlv_format_t) -> c_int;

    /// Borrows exactly the configured number of identifier bytes.
    pub fn tlv_fixed_identifier_read(
        config: *const tlv_fixed_identifier_t,
        data: *const u8,
        size: usize,
        tag: *mut tlv_tag_t,
        consumed: *mut usize,
    ) -> tlv_result_t;
    /// Copies a fixed-width identifier, or measures it with null output.
    pub fn tlv_fixed_identifier_write(
        config: *const tlv_fixed_identifier_t,
        tag: *const tlv_tag_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    /// Decodes a fixed-width unsigned count.
    pub fn tlv_fixed_length_read(
        config: *const tlv_fixed_length_t,
        data: *const u8,
        size: usize,
        length: *mut tlv_size_t,
        consumed: *mut usize,
    ) -> tlv_result_t;
    /// Encodes a fixed-width unsigned count, or measures it with null output.
    pub fn tlv_fixed_length_write(
        config: *const tlv_fixed_length_t,
        length: tlv_size_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    /// Reads a complete variable identifier without copying its bytes.
    pub fn tlv_variable_identifier_read(
        config: *const tlv_variable_identifier_t,
        data: *const u8,
        size: usize,
        tag: *mut tlv_tag_t,
        consumed: *mut usize,
    ) -> tlv_result_t;
    /// Validates and copies a variable identifier, or measures it with null output.
    pub fn tlv_variable_identifier_write(
        config: *const tlv_variable_identifier_t,
        tag: *const tlv_tag_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    /// Decodes a definite variable count.
    pub fn tlv_variable_length_read(
        config: *const tlv_variable_length_t,
        data: *const u8,
        size: usize,
        length: *mut tlv_size_t,
        consumed: *mut usize,
    ) -> tlv_result_t;
    /// Encodes a definite variable count, or measures it with null output.
    pub fn tlv_variable_length_write(
        config: *const tlv_variable_length_t,
        length: tlv_size_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    /// Decodes an inline or escape-prefixed count.
    pub fn tlv_escaped_length_read(
        config: *const tlv_escaped_length_t,
        data: *const u8,
        size: usize,
        length: *mut tlv_size_t,
        consumed: *mut usize,
    ) -> tlv_result_t;
    /// Encodes an escaped count, or measures it with null output.
    pub fn tlv_escaped_length_write(
        config: *const tlv_escaped_length_t,
        length: tlv_size_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    /// Extracts an unsigned field from its backing wire integer.
    pub fn tlv_packed_field_read(
        field: *const tlv_packed_field_t,
        data: *const u8,
        size: usize,
        value: *mut u64,
    ) -> tlv_result_t;
    /// Inserts an unsigned field, preserving every bit outside it.
    pub fn tlv_packed_field_write(
        field: *const tlv_packed_field_t,
        data: *mut u8,
        capacity: usize,
        value: u64,
    ) -> tlv_result_t;

    /// Initializes a descriptor borrowing a field composition.
    pub fn tlv_fields_format_init(
        format: *mut tlv_format_t,
        composition: *const tlv_field_composition_t,
    ) -> tlv_result_t;
    /// Initializes a field format with identifier-selected tag-only elements.
    pub fn tlv_tagged_fields_format_init(
        format: *mut tlv_format_t,
        composition: *const tlv_tagged_fields_composition_t,
    ) -> tlv_result_t;
    /// Initializes a binary format with identifier-selected tag-only elements.
    pub fn tlv_tagged_binary_format_init(
        format: *mut tlv_format_t,
        composition: *const tlv_tagged_binary_composition_t,
    ) -> tlv_result_t;
    /// Initializes an escape-length format.
    pub fn tlv_escaped_format_init(
        format: *mut tlv_format_t,
        config: *const tlv_escaped_format_t,
    ) -> tlv_result_t;
    /// Initializes variable field callbacks for a field composition.
    pub fn tlv_variable_fields_init(
        fields: *mut tlv_field_composition_t,
        config: *const tlv_variable_format_t,
    ) -> tlv_result_t;
    /// Initializes a definite variable-width format.
    pub fn tlv_variable_format_init(
        format: *mut tlv_format_t,
        config: *const tlv_variable_format_t,
    ) -> tlv_result_t;
    /// Validates a packed layout without reading its identifier table.
    pub fn tlv_packed_layout_validate(layout: *const tlv_packed_layout_t) -> tlv_result_t;
    /// Decodes one packed element; context is a `tlv_packed_layout_t`.
    pub fn tlv_packed_decode(
        context: *const c_void,
        data: *const u8,
        size: usize,
        result: *mut tlv_decoded_t,
        error: *mut tlv_format_error_t,
    ) -> tlv_result_t;
    /// Measures one packed element; context is a `tlv_packed_layout_t`.
    pub fn tlv_packed_measure(
        context: *const c_void,
        element: *const tlv_element_t,
        result: *mut tlv_encoding_t,
        error: *mut tlv_format_error_t,
    ) -> tlv_result_t;
    /// Encodes one packed element; context is a `tlv_packed_layout_t`.
    pub fn tlv_packed_encode(
        context: *const c_void,
        element: *const tlv_element_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        error: *mut tlv_format_error_t,
    ) -> tlv_result_t;
}
