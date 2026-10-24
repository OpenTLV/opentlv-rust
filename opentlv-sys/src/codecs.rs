// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Generic Value codecs (`tlv/codec/values.h`, `text.h`, `digits.h`,
//! `ipv4.h`), structure codecs (`tlv/codec/structure.h`) and the Schema
//! number composition (`tlv/schema/number.h`).
use super::*;

extern "C" {
    /// Exactly one byte to/from `uint8_t`.
    pub static tlv_codec_uint8: tlv_codec_t;
    /// Exactly two big-endian bytes to/from `uint16_t`.
    pub static tlv_codec_uint16_be: tlv_codec_t;
    /// Exactly four big-endian bytes to/from `uint32_t`.
    pub static tlv_codec_uint32_be: tlv_codec_t;
    /// Exactly two little-endian bytes to/from `uint16_t`.
    pub static tlv_codec_uint16_le: tlv_codec_t;
    /// Exactly four little-endian bytes to/from `uint32_t`.
    pub static tlv_codec_uint32_le: tlv_codec_t;
    /// Minimal big-endian two's-complement bytes to/from `int64_t`.
    pub static tlv_codec_int64_minimal_be: tlv_codec_t;
    /// Arbitrary bytes to/from a borrowed `tlv_value_t`.
    pub static tlv_codec_bytes: tlv_codec_t;
}

/// Allowed logical text characters (`tlv_text_alphabet_t`).
pub type tlv_text_alphabet_t = c_int;
/// Printable ASCII, including space (`TLV_TEXT_ASCII_PRINTABLE`).
pub const TLV_TEXT_ASCII_PRINTABLE: tlv_text_alphabet_t = 0;
/// ASCII letters and digits only (`TLV_TEXT_ASCII_ALNUM`).
pub const TLV_TEXT_ASCII_ALNUM: tlv_text_alphabet_t = 1;

/// Borrowed text-codec configuration (`tlv_text_codec_config_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_text_codec_config_t {
    /// Explicit character repertoire.
    pub alphabet: tlv_text_alphabet_t,
    /// Fixed wire width, or zero for variable-width text.
    pub width: usize,
    /// Zero disables padding; one enables trailing zero padding.
    pub zero_padding: c_int,
}

/// Borrowed packed digit-string configuration (`tlv_digits_codec_config_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_digits_codec_config_t {
    /// Fixed byte width, or zero for minimal encoding.
    pub width: usize,
}

/// IPv4 address in network octet order (`tlv_ipv4_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct tlv_ipv4_t {
    /// Four address octets in dotted-decimal order.
    pub bytes: [u8; 4],
}

/// Borrowed sequence of IPv4 addresses (`tlv_ipv4_list_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_ipv4_list_t {
    /// Borrowed bytes; the size is a multiple of four.
    pub raw: tlv_value_t,
}

extern "C" {
    /// Exactly four bytes to/from `tlv_ipv4_t`.
    pub static tlv_codec_ipv4: tlv_codec_t;
    /// A multiple of four bytes to/from `tlv_ipv4_list_t`.
    pub static tlv_codec_ipv4_list: tlv_codec_t;

    /// Copies one address from a validated borrowed IPv4 list.
    pub fn tlv_ipv4_list_at(
        list: *const tlv_ipv4_list_t,
        index: usize,
        value: *mut tlv_ipv4_t,
    ) -> tlv_result_t;

    /// Validates text and returns a borrowed `tlv_value_t` of unpadded characters.
    pub fn tlv_text_decode(
        context: *const c_void,
        data: *const u8,
        size: usize,
        value: *mut c_void,
        capacity: usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    /// Encodes a `tlv_value_t` character span, optionally zero padded.
    pub fn tlv_text_encode(
        context: *const c_void,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;

    /// Decodes packed digits to a NUL-terminated ASCII string.
    pub fn tlv_digits_decode(
        context: *const c_void,
        data: *const u8,
        size: usize,
        value: *mut c_void,
        capacity: usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    /// Encodes ASCII decimal digits, retaining leading zeros.
    pub fn tlv_digits_encode(
        context: *const c_void,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
}

/// Field Schema composed with a `uint64_t` representation (`tlv_schema_number_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_schema_number_t {
    /// Required authoritative field constraints.
    pub schema: *const tlv_schema_entry_t,
    /// Numeric representation.
    pub representation: tlv_number_codec_config_t,
}

extern "C" {
    /// Validates the field length, then delegates numeric decoding.
    pub fn tlv_schema_number_decode(
        context: *const c_void,
        data: *const u8,
        size: usize,
        value: *mut c_void,
        capacity: usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    /// Selects a fitting Schema-permitted width, then delegates numeric encoding.
    pub fn tlv_schema_number_encode(
        context: *const c_void,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
}

/// Frame capacity of structure codecs without a Schema (`TLV_STRUCTURE_MAX_DEPTH`).
pub const TLV_STRUCTURE_MAX_DEPTH: usize = 64;

/// Converts a validated sequence to an application object.
pub type tlv_structure_decode_fn = unsafe extern "C" fn(
    context: *const c_void,
    format: *const tlv_format_t,
    data: *const u8,
    size: usize,
    value: *mut c_void,
    capacity: usize,
    diagnostic: *mut tlv_codec_diagnostic_t,
) -> tlv_result_t;

/// Converts an application object to a sequence.
pub type tlv_structure_encode_fn = unsafe extern "C" fn(
    context: *const c_void,
    format: *const tlv_format_t,
    value: *const c_void,
    size: usize,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
    diagnostic: *mut tlv_codec_diagnostic_t,
) -> tlv_result_t;

/// Descriptor mapping a complete TLV sequence to one object (`tlv_structure_codec_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_structure_codec_t {
    /// Borrowed context passed to the object callbacks; may be null.
    pub context: *const c_void,
    /// Borrowed Format.
    pub format: *const tlv_format_t,
    /// Optional borrowed Schema; null validates framing and nesting only.
    pub schema: *const tlv_structure_schema_t,
    /// Maximum nesting depth; zero is a real limit.
    pub max_depth: usize,
    /// Maximum element count; zero is a real limit.
    pub max_elements: usize,
    /// Decoder; `None` if unsupported.
    pub decode: Option<tlv_structure_decode_fn>,
    /// Encoder; `None` if unsupported.
    pub encode: Option<tlv_structure_encode_fn>,
}

extern "C" {
    /// Validates and decodes a complete sequence into an application object.
    pub fn tlv_structure_decode(
        codec: *const tlv_structure_codec_t,
        data: *const u8,
        size: usize,
        value: *mut c_void,
        capacity: usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    /// Encodes an application object into a complete, validated sequence.
    pub fn tlv_structure_encode(
        codec: *const tlv_structure_codec_t,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
}
