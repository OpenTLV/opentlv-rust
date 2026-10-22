// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Runtime-configurable fixed-width TLV format.

use std::mem::MaybeUninit;
use std::sync::Arc;

use opentlv_sys as native;

use crate::error::{Error, Result};

/// Byte order of a [`FixedFormat`]'s length field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    /// Most significant byte first.
    Big,
    /// Least significant byte first.
    Little,
}
/// Wire ordering of configurable Fixed fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementOrder {
    /// Tag, Length, Value.
    Tlv,
    /// Length, Tag, Value.
    Ltv,
}
/// Logical content counted by the Fixed length field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthScope {
    /// Value bytes only.
    Value,
    /// Tag and Value bytes together.
    TagAndValue,
}

impl From<ByteOrder> for native::tlv_byte_order_t {
    fn from(order: ByteOrder) -> native::tlv_byte_order_t {
        match order {
            ByteOrder::Big => native::TLV_BYTE_ORDER_BIG_ENDIAN,
            ByteOrder::Little => native::TLV_BYTE_ORDER_LITTLE_ENDIAN,
        }
    }
}

/// A runtime-configurable fixed-width TLV format's state: independent tag
/// width, length width (1 to 8 bytes) and length byte order.
///
/// Equivalent to the C `tlv_fixed_format_t`. A caller-owned, `Copy` value: the
/// caller holds it (on the stack, in a struct, wherever suits it) for as long
/// as any [`FixedFormat`] borrows it, mirroring the C API's
/// `tlv_fixed_format_t`/`tlv_fixed_format_init()` split directly instead of
/// hiding the address-stability requirement behind a heap allocation. See
/// [format context ownership and
/// lifetime](https://github.com/marekcingel/OpenTLV/blob/main/docs/guides/memory.md#format-context-ownership-and-lifetime).
#[derive(Clone, Copy, Debug)]
pub struct FixedFormatConfig {
    inner: native::tlv_fixed_format_t,
}

impl FixedFormatConfig {
    /// Select field order, leaving width and byte-order settings unchanged.
    pub fn with_order(mut self, order: ElementOrder) -> Self {
        self.inner.element_order = match order {
            ElementOrder::Tlv => native::TLV_ELEMENT_ORDER_TLV,
            ElementOrder::Ltv => native::TLV_ELEMENT_ORDER_LTV,
        };
        self
    }
    /// Select what the length field counts; C validates the resulting descriptor.
    pub fn with_length_scope(mut self, scope: LengthScope) -> Self {
        self.inner.length_scope = match scope {
            LengthScope::Value => native::TLV_LENGTH_SCOPE_VALUE,
            LengthScope::TagAndValue => native::TLV_LENGTH_SCOPE_TAG_AND_VALUE,
        };
        self
    }
    /// Creates a fixed-width format configuration. A one-byte tag and a
    /// one-byte big-endian length is `FixedFormatConfig::new(1, 1,
    /// ByteOrder::Big)`.
    ///
    /// Validity of `tag_size`/`length_size` is checked when a [`FixedFormat`]
    /// is built from this configuration, not here.
    pub fn new(tag_size: usize, length_size: usize, order: ByteOrder) -> FixedFormatConfig {
        FixedFormatConfig {
            inner: native::tlv_fixed_format_t {
                identifier: native::tlv_fixed_identifier_t { size: tag_size },
                length: native::tlv_fixed_length_t {
                    size: length_size,
                    byte_order: order.into(),
                },
                element_order: native::TLV_ELEMENT_ORDER_TLV,
                length_scope: native::TLV_LENGTH_SCOPE_VALUE,
            },
        }
    }

    /// Tag width in bytes.
    pub fn tag_size(&self) -> usize {
        self.inner.identifier.size
    }

    /// Length field width in bytes.
    pub fn length_size(&self) -> usize {
        self.inner.length.size
    }
}

/// A runtime-configurable fixed-width TLV format, borrowed from a
/// [`FixedFormatConfig`] the caller owns.
///
/// Equivalent to the C `tlv_fixed_format_t` and `tlv_fixed_format_init()`.
///
/// ```
/// use opentlv::{ByteOrder, FixedFormat, FixedFormatConfig, Reader, Tag, Writer};
///
/// let config = FixedFormatConfig::new(2, 1, ByteOrder::Big);
/// let format = FixedFormat::new(&config).unwrap();
/// let mut buf = [0u8; 16];
/// let mut writer = Writer::with_fixed_format(&mut buf, &format);
/// writer.write(&Tag::from_bytes(&[0x01, 0x02]), &[0xAA, 0xBB, 0xCC]).unwrap();
///
/// let element = Reader::with_fixed_format(writer.written(), &format)
///     .next().unwrap().unwrap();
/// assert_eq!(element.value(), &[0xAA, 0xBB, 0xCC]);
/// ```
#[derive(Debug)]
pub struct FixedFormat<'a> {
    config: &'a FixedFormatConfig,
    format: native::tlv_format_t,
}

/// Shared owning Fixed descriptor and immutable configuration. Clones preserve
/// stable native context addresses across programs, Readers and Documents.
/// Share clones of one owner when combining Query with a Reader or Document;
/// independently created equal configurations have different native identities.
#[derive(Clone, Debug)]
pub struct OwnedFixedFormat {
    owner: Arc<FixedOwner>,
}
#[derive(Debug)]
struct FixedOwner {
    config: Box<FixedFormatConfig>,
    format: native::tlv_format_t,
}
// SAFETY: the descriptor references only its immutable owned configuration and
// static C callbacks. Its context allocation never moves after construction.
unsafe impl Send for FixedOwner {}
unsafe impl Sync for FixedOwner {}
impl OwnedFixedFormat {
    /// Validate and own a configuration, retaining stable native storage.
    pub fn new(config: FixedFormatConfig) -> Result<Self> {
        let config = Box::new(config);
        let format = FixedFormat::new(&config)?.format;
        Ok(Self {
            owner: Arc::new(FixedOwner { config, format }),
        })
    }
    /// Borrow a descriptor for existing Fixed Reader, Writer and Document APIs.
    pub fn as_borrowed(&self) -> FixedFormat<'_> {
        FixedFormat {
            config: &self.owner.config,
            format: self.owner.format,
        }
    }
    pub(crate) fn raw(&self) -> *const native::tlv_format_t {
        &self.owner.format
    }
}

impl<'a> FixedFormat<'a> {
    /// Copy this validated configuration into an independent shared owner.
    pub fn owned(&self) -> Result<OwnedFixedFormat> {
        OwnedFixedFormat::new(*self.config)
    }
    /// Creates a configurable fixed-width format borrowing `config`; `config`
    /// must outlive this `FixedFormat`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArg`] if `config`'s `tag_size` is 0 or `length_size` is
    /// 0 or greater than 8. An unknown byte order cannot occur since
    /// `config` is always built with a valid [`ByteOrder`].
    pub fn new(config: &'a FixedFormatConfig) -> Result<FixedFormat<'a>> {
        let mut format = MaybeUninit::<native::tlv_format_t>::uninit();
        // SAFETY: `config`'s address is stable for `'a` (the borrow checker
        // enforces it); `format` is writable.
        let code = unsafe { native::tlv_fixed_format_init(format.as_mut_ptr(), &config.inner) };
        Error::check(code)?;
        Ok(FixedFormat {
            config,
            // SAFETY: tlv_fixed_format_init() above succeeded, so the
            // descriptor is fully initialized.
            format: unsafe { format.assume_init() },
        })
    }

    /// Tag width in bytes.
    pub fn tag_size(&self) -> usize {
        self.config.tag_size()
    }

    /// Length field width in bytes.
    pub fn length_size(&self) -> usize {
        self.config.length_size()
    }

    pub(crate) fn raw(&self) -> *const native::tlv_format_t {
        &self.format
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_configurations() {
        assert_eq!(
            FixedFormat::new(&FixedFormatConfig::new(0, 1, ByteOrder::Big)).unwrap_err(),
            Error::InvalidArg
        );
        assert_eq!(
            FixedFormat::new(&FixedFormatConfig::new(1, 0, ByteOrder::Big)).unwrap_err(),
            Error::InvalidArg
        );
        assert_eq!(
            FixedFormat::new(&FixedFormatConfig::new(1, 9, ByteOrder::Big)).unwrap_err(),
            Error::InvalidArg
        );
    }

    #[test]
    fn one_byte_configuration_matches_the_wire_bytes() {
        let config = FixedFormatConfig::new(1, 1, ByteOrder::Big);
        let format = FixedFormat::new(&config).unwrap();
        assert_eq!(format.tag_size(), 1);
        assert_eq!(format.length_size(), 1);
    }

    #[test]
    fn one_config_shared_by_reader_and_writer() {
        let config = FixedFormatConfig::new(2, 1, ByteOrder::Big);
        let format = FixedFormat::new(&config).unwrap();

        let mut buf = [0u8; 16];
        let mut writer = crate::Writer::with_fixed_format(&mut buf, &format);
        writer
            .write(&crate::Tag::from_bytes(&[0x01, 0x02]), &[0xAA, 0xBB])
            .unwrap();
        let written = writer.finish();

        let element = crate::Reader::with_fixed_format(written, &format)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(element.value(), &[0xAA, 0xBB]);
    }

    #[test]
    fn two_formats_share_one_config() {
        let config = FixedFormatConfig::new(1, 1, ByteOrder::Big);
        let a = FixedFormat::new(&config).unwrap();
        let b = FixedFormat::new(&config).unwrap();
        assert_eq!(a.tag_size(), b.tag_size());
        assert_eq!(a.length_size(), b.length_size());
    }
}
