// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Immutable source representation, separate from semantic element content.
use crate::{Element, Error, FixedFormat, Format, Result};
use opentlv_sys as native;
use std::{marker::PhantomData, mem::MaybeUninit, slice};

/// A decoded element and its immutable original encoding.
/// Source bytes and any borrowed fixed-format configuration outlive this value.
#[derive(Clone, Debug)]
pub struct Decoded<'a> {
    /// Canonical content; replacing it does not change the original source.
    pub element: Element<'a>,
    pub(crate) source: native::tlv_source_t,
    lifetime: PhantomData<&'a [u8]>,
}
impl<'a> Decoded<'a> {
    /// Convert a successful native decode whose storage remains valid for `'a`.
    pub(crate) unsafe fn from_raw(raw: native::tlv_decoded_t) -> Result<Self> {
        Ok(Self {
            // SAFETY: caller guarantees the successful decode and storage lifetime.
            element: unsafe { Element::from_raw(&raw.element) }?,
            source: raw.source,
            lifetime: PhantomData,
        })
    }

    /// Original framing ranges, relative to `encoded()`. Absent fields are `None`.
    pub fn layout(&self) -> Layout {
        let range =
            |r: native::tlv_range_t| (r.present != 0).then_some(r.offset..r.offset + r.size);
        Layout {
            header: range(self.source.header),
            tag: range(self.source.tag),
            length: range(self.source.length),
            value: range(self.source.value),
            trailer: range(self.source.trailer),
        }
    }
    /// Returns the complete original encoded element, including framing.
    pub fn encoded(&self) -> &'a [u8] {
        // SAFETY: successful C decode returns a range within the borrowed input.
        unsafe { slice::from_raw_parts(self.source.data, self.source.size) }
    }
    /// Returns the original length field, or an empty slice if absent.
    pub fn raw_length(&self) -> &'a [u8] {
        let r = self.source.length;
        if r.present == 0 {
            return &[];
        }
        &self.encoded()[r.offset..r.offset + r.size]
    }
    /// Copies the original encoding only if `element` still has the original content.
    /// Returns `InvalidArg` after semantic mutation, or `BufferTooShort` without writing.
    pub fn preserve(&self, output: &mut [u8]) -> Result<usize> {
        let raw = native::tlv_element_t {
            tag: self.element.tag().raw(),
            value: native::tlv_value_t {
                data: self.element.value().as_ptr(),
                size: self.element.value().len() as u64,
            },
        };
        let mut written = 0;
        // SAFETY: all input/source borrows are live; output is exclusively borrowed.
        Error::check(unsafe {
            native::tlv_source_preserve(
                &self.source,
                &raw,
                output.as_mut_ptr(),
                output.len(),
                &mut written,
            )
        })?;
        Ok(written)
    }
}

/// Source-relative byte ranges for the original encoded element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Complete header.
    pub header: Option<std::ops::Range<usize>>,
    /// Optional wire tag.
    pub tag: Option<std::ops::Range<usize>>,
    /// Optional length field.
    pub length: Option<std::ops::Range<usize>>,
    /// Logical value.
    pub value: Option<std::ops::Range<usize>>,
    /// Optional trailer.
    pub trailer: Option<std::ops::Range<usize>>,
}
unsafe fn decode_raw<'a>(
    data: &'a [u8],
    format: *const native::tlv_format_t,
) -> Result<Decoded<'a>> {
    let mut result = MaybeUninit::<native::tlv_decoded_t>::uninit();
    // SAFETY: caller guarantees descriptor lifetime; data and writable result are valid.
    Error::check(unsafe {
        native::tlv_format_decode(
            format,
            data.as_ptr(),
            data.len(),
            result.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    })?;
    // SAFETY: successful decode initialized the complete result.
    let result = unsafe { result.assume_init() };
    // SAFETY: decoded value borrows the immutable input for 'a.
    let element = unsafe { Element::from_raw(&result.element) }?;
    Ok(Decoded {
        element,
        source: result.source,
        lifetime: PhantomData,
    })
}
/// Decodes one element, retaining immutable source bytes and raw framing.
pub fn decode(data: &[u8], format: Format) -> Result<Decoded<'_>> {
    // SAFETY: builtin descriptors and contexts have static lifetime.
    unsafe { decode_raw(data, format.raw()) }
}
/// Decodes using a fixed-format descriptor borrowed for the result's lifetime.
pub fn decode_fixed<'a>(data: &'a [u8], format: &'a FixedFormat<'_>) -> Result<Decoded<'a>> {
    // SAFETY: the signature keeps both configuration and descriptor borrowed.
    unsafe { decode_raw(data, format.raw()) }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tag;
    #[test]
    fn preserves_nonminimal_length_and_rejects_mutation() {
        let bytes = [4, 0x81, 1, 0xAA];
        let mut decoded = decode(&bytes, Format::Ber).unwrap();
        assert_eq!(decoded.source.tag_binding, native::TLV_TAG_BINDING_SOURCE);
        assert_eq!(decoded.raw_length(), &[0x81, 1]);
        let mut output = [0; 4];
        assert_eq!(decoded.preserve(&mut output), Ok(4));
        assert_eq!(output, bytes);
        decoded.element = Element::new(Tag::from_bytes(&[4]), &[0xBB]);
        assert_eq!(decoded.preserve(&mut output), Err(Error::InvalidArg));
    }
}
