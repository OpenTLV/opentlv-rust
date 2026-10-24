// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Field encodings that compose a [`ComposedFormat`](crate::ComposedFormat),
//! mirroring `tlv/field/*.h`.
//!
//! Each type is an immutable configuration. Core validates masks, widths,
//! policies and their combination when a Format is composed from them, and
//! [`PackedField`] reads and writes its bits through Core directly.

use std::ptr;

use opentlv_sys as native;

use crate::error::{Error, Result};
use crate::fixed_format::ByteOrder;

/// An identifier of exactly `size` raw bytes (`tlv_fixed_identifier_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FixedIdentifier {
    /// Identifier width in bytes; nonzero.
    pub size: usize,
}

impl FixedIdentifier {
    /// An identifier of `size` bytes.
    pub fn new(size: usize) -> Self {
        FixedIdentifier { size }
    }

    pub(crate) fn raw(&self) -> native::tlv_fixed_identifier_t {
        native::tlv_fixed_identifier_t { size: self.size }
    }
}

/// Optional wire constraints of a [`VariableIdentifier`] (`tlv_identifier_policy_t`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IdentifierPolicy {
    /// Rejected first bytes.
    pub forbidden_leading: Vec<u8>,
    /// Reject a zero first escaped payload.
    pub reject_zero_first_payload: bool,
    /// Reject redundant leading zero digits and escaped numbers that could be inline.
    pub require_minimal: bool,
}

/// Inline identifier bits with an escape into continuation octets
/// (`tlv_variable_identifier_t`).
///
/// One octet suffices unless `first & inline_mask == escape`; continuation
/// octets follow while `continuation_bit` is set and carry `payload_mask`
/// bits. The complete identifier, including its first octet, is at most
/// `max_size` bytes. BER identifiers of up to two bytes, for example:
///
/// ```
/// use opentlv::{IdentifierPolicy, VariableIdentifier};
///
/// let tag = VariableIdentifier::new(0x1F, 0x1F, 0x80, 0x7F, 2).with_policy(IdentifierPolicy {
///     reject_zero_first_payload: true,
///     ..IdentifierPolicy::default()
/// });
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VariableIdentifier {
    /// Nonzero mask selecting inline bits in the first octet.
    pub inline_mask: u8,
    /// Escape pattern, containing only `inline_mask` bits.
    pub escape: u8,
    /// Exactly one bit; set means another octet follows.
    pub continuation_bit: u8,
    /// Nonzero payload mask, disjoint from `continuation_bit`.
    pub payload_mask: u8,
    /// Maximum complete identifier width in bytes; at least one.
    pub max_size: usize,
    /// Optional wire constraints.
    pub policy: Option<IdentifierPolicy>,
}

impl VariableIdentifier {
    /// A variable identifier without a policy.
    pub fn new(
        inline_mask: u8,
        escape: u8,
        continuation_bit: u8,
        payload_mask: u8,
        max_size: usize,
    ) -> Self {
        VariableIdentifier {
            inline_mask,
            escape,
            continuation_bit,
            payload_mask,
            max_size,
            policy: None,
        }
    }

    /// Constrain the wire representation with `policy`.
    pub fn with_policy(mut self, policy: IdentifierPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// The native configuration; the policy pointer is set by its owner.
    pub(crate) fn raw(&self) -> native::tlv_variable_identifier_t {
        native::tlv_variable_identifier_t {
            inline_mask: self.inline_mask,
            escape: self.escape,
            continuation_bit: self.continuation_bit,
            payload_mask: self.payload_mask,
            max_size: self.max_size,
            policy: ptr::null(),
        }
    }
}

/// An unsigned count of `size` bytes, 1 to 8, in `byte_order` (`tlv_fixed_length_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FixedLength {
    /// Count width in bytes, one through eight.
    pub size: usize,
    /// Byte order of the count.
    pub byte_order: ByteOrder,
}

impl FixedLength {
    /// A `size`-byte count in `byte_order`.
    pub fn new(size: usize, byte_order: ByteOrder) -> Self {
        FixedLength { size, byte_order }
    }

    pub(crate) fn raw(&self) -> native::tlv_fixed_length_t {
        native::tlv_fixed_length_t {
            size: self.size,
            byte_order: self.byte_order.into(),
        }
    }
}

/// Allowed definite count forms and limits of a [`VariableLength`]
/// (`tlv_length_policy_t`).
///
/// At least one form must be allowed. The default allows both forms, up to
/// eight long-form octets and any count, without requiring minimal encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LengthPolicy {
    /// Permit inline counts.
    pub allow_short: bool,
    /// Permit long-form counts.
    pub allow_long: bool,
    /// Reject padding and unnecessary long forms.
    pub require_minimal: bool,
    /// Maximum long-form octets; positive when `allow_long` is set.
    pub max_long_octets: usize,
    /// Inclusive count limit.
    pub max_value: u64,
}

impl Default for LengthPolicy {
    fn default() -> Self {
        LengthPolicy {
            allow_short: true,
            allow_long: true,
            require_minimal: false,
            max_long_octets: 8,
            max_value: u64::MAX,
        }
    }
}

impl LengthPolicy {
    pub(crate) fn raw(&self) -> native::tlv_length_policy_t {
        native::tlv_length_policy_t {
            allow_short: self.allow_short.into(),
            allow_long: self.allow_long.into(),
            require_minimal: self.require_minimal.into(),
            max_long_octets: self.max_long_octets,
            max_value: self.max_value,
        }
    }
}

/// A short count, or a prefix giving the number of following count octets
/// (`tlv_variable_length_t`).
///
/// With `long_form_bit` clear, `payload_mask` holds the count; otherwise it
/// holds the number of following octets, read in `byte_order`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VariableLength {
    /// Exactly one bit selecting long form.
    pub long_form_bit: u8,
    /// Nonzero mask, disjoint from `long_form_bit`.
    pub payload_mask: u8,
    /// Byte order of the long-form count octets.
    pub byte_order: ByteOrder,
    /// Optional definite count policy.
    pub policy: Option<LengthPolicy>,
}

impl VariableLength {
    /// A variable count without a policy.
    pub fn new(long_form_bit: u8, payload_mask: u8, byte_order: ByteOrder) -> Self {
        VariableLength {
            long_form_bit,
            payload_mask,
            byte_order,
            policy: None,
        }
    }

    /// Constrain the accepted forms and counts with `policy`.
    pub fn with_policy(mut self, policy: LengthPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// The native configuration; the policy pointer is set by its owner.
    pub(crate) fn raw(&self) -> native::tlv_variable_length_t {
        native::tlv_variable_length_t {
            long_form_bit: self.long_form_bit,
            payload_mask: self.payload_mask,
            byte_order: self.byte_order.into(),
            policy: ptr::null(),
        }
    }
}

/// A one-byte inline count, or `escape` followed by an `extended_size`-byte
/// count (`tlv_escaped_length_t`).
///
/// Prefixes below `escape` are the count itself; prefixes above it are
/// invalid. Extended counts are at least `min_extended`. Every count is at
/// most `max_length`, by default the largest `extended_size`-byte count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EscapedLength {
    /// Nonzero escape marker and exclusive inline limit.
    pub escape: u8,
    /// Extended count width, one through eight bytes.
    pub extended_size: usize,
    /// Byte order of the extended count.
    pub byte_order: ByteOrder,
    /// Inclusive minimum extended count, at most `escape`.
    pub min_extended: u64,
    /// Inclusive maximum count; `None` is the largest `extended_size`-byte count.
    pub max_length: Option<u64>,
}

impl EscapedLength {
    /// An escaped count accepting every extended count.
    pub fn new(escape: u8, extended_size: usize, byte_order: ByteOrder) -> Self {
        EscapedLength {
            escape,
            extended_size,
            byte_order,
            min_extended: 0,
            max_length: None,
        }
    }

    /// Reject extended counts below `min_extended`.
    pub fn with_min_extended(mut self, min_extended: u64) -> Self {
        self.min_extended = min_extended;
        self
    }

    /// Reject counts above `max_length`.
    pub fn with_max_length(mut self, max_length: u64) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub(crate) fn raw(&self) -> native::tlv_escaped_length_t {
        let max_length = self.max_length.unwrap_or(match self.extended_size {
            // Core rejects other widths; any maximum preserves that error.
            size @ 1..=7 => (1 << (8 * size)) - 1,
            _ => u64::MAX,
        });
        native::tlv_escaped_length_t {
            escape: self.escape,
            extended_size: self.extended_size,
            byte_order: self.byte_order.into(),
            min_extended: self.min_extended,
            max_length,
        }
    }
}

/// An unsigned bit field of a packed header integer (`tlv_packed_field_t`).
///
/// `bit_offset` counts from the least significant header bit; the header
/// width and byte order are those of the enclosing
/// [`PackedLayout`](crate::PackedLayout), or the ones given to
/// [`read`](Self::read) and [`write`](Self::write).
///
/// ```
/// use opentlv::{ByteOrder, PackedField};
///
/// let flags = PackedField::new(5, 3);
/// assert_eq!(flags.read(&[0xA4], 1, ByteOrder::Big), Ok(5));
/// let mut header = [0xA4];
/// flags.write(&mut header, 2, 1, ByteOrder::Big).unwrap();
/// assert_eq!(header, [0x44]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackedField {
    /// Least significant field bit, 0 to 63.
    pub bit_offset: u32,
    /// Field width, 1 to 64 bits; must fit the header.
    pub bit_width: u32,
}

impl PackedField {
    /// A `bit_width`-bit field starting at `bit_offset`.
    pub fn new(bit_offset: u32, bit_width: u32) -> Self {
        PackedField {
            bit_offset,
            bit_width,
        }
    }

    pub(crate) fn raw(
        &self,
        header_size: usize,
        byte_order: ByteOrder,
    ) -> native::tlv_packed_field_t {
        native::tlv_packed_field_t {
            storage_size: header_size,
            bit_offset: self.bit_offset,
            bit_width: self.bit_width,
            byte_order: byte_order.into(),
        }
    }

    /// Extracts the field from the `header_size`-byte integer at the start of
    /// `data` (`tlv_packed_field_read()`).
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] when `data` is shorter than the header and
    /// [`Error::InvalidArg`] for a field that does not fit it.
    pub fn read(&self, data: &[u8], header_size: usize, byte_order: ByteOrder) -> Result<u64> {
        let field = self.raw(header_size, byte_order);
        let mut value = 0;
        // SAFETY: the configuration and input are live for the call.
        Error::check(unsafe {
            native::tlv_packed_field_read(&field, data.as_ptr(), data.len(), &mut value)
        })?;
        Ok(value)
    }

    /// Inserts `value` into `data` in place, keeping every bit outside the
    /// field (`tlv_packed_field_write()`).
    ///
    /// # Errors
    ///
    /// [`Error::Overflow`] when `value` does not fit `bit_width` and
    /// [`Error::BufferTooShort`] when `data` is shorter than the header; `data`
    /// is unchanged on failure.
    pub fn write(
        &self,
        data: &mut [u8],
        value: u64,
        header_size: usize,
        byte_order: ByteOrder,
    ) -> Result<()> {
        let field = self.raw(header_size, byte_order);
        // SAFETY: the configuration is live and `data` is exclusively borrowed.
        Error::check(unsafe {
            native::tlv_packed_field_write(&field, data.as_mut_ptr(), data.len(), value)
        })
    }
}

/// Constructed classification by a masked identifier byte (`tlv_constructed_bit_t`).
///
/// An element is constructed when `tag[byte_index] & mask == value`. `mask`
/// must be nonzero and `value` must contain only masked bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConstructedBit {
    /// Zero-based canonical identifier byte index.
    pub byte_index: usize,
    /// Nonzero mask of tested bits.
    pub mask: u8,
    /// Required masked value.
    pub value: u8,
}

impl ConstructedBit {
    /// Constructed when `tag[byte_index] & mask == value`.
    pub fn new(byte_index: usize, mask: u8, value: u8) -> Self {
        ConstructedBit {
            byte_index,
            mask,
            value,
        }
    }

    pub(crate) fn raw(&self) -> native::tlv_constructed_bit_t {
        native::tlv_constructed_bit_t {
            byte_index: self.byte_index,
            mask: self.mask,
            value: self.value,
        }
    }
}

/// The identifier encoding of a [`FieldComposition`](crate::FieldComposition).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IdentifierEncoding {
    /// Fixed-width raw identifier.
    Fixed(FixedIdentifier),
    /// Variable-width identifier.
    Variable(VariableIdentifier),
}

impl From<FixedIdentifier> for IdentifierEncoding {
    fn from(encoding: FixedIdentifier) -> Self {
        IdentifierEncoding::Fixed(encoding)
    }
}
impl From<VariableIdentifier> for IdentifierEncoding {
    fn from(encoding: VariableIdentifier) -> Self {
        IdentifierEncoding::Variable(encoding)
    }
}

/// The length encoding of a [`FieldComposition`](crate::FieldComposition).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LengthEncoding {
    /// Fixed-width count.
    Fixed(FixedLength),
    /// Short or long-form count.
    Variable(VariableLength),
    /// Inline or escape-prefixed count.
    Escaped(EscapedLength),
}

impl From<FixedLength> for LengthEncoding {
    fn from(encoding: FixedLength) -> Self {
        LengthEncoding::Fixed(encoding)
    }
}
impl From<VariableLength> for LengthEncoding {
    fn from(encoding: VariableLength) -> Self {
        LengthEncoding::Variable(encoding)
    }
}
impl From<EscapedLength> for LengthEncoding {
    fn from(encoding: EscapedLength) -> Self {
        LengthEncoding::Escaped(encoding)
    }
}
