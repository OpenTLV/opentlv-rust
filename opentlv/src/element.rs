// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! The decoded TLV element type.

use std::slice;

use opentlv_sys as native;

use crate::error::{Error, Result};
use crate::tag::Tag;

/// A canonical TLV element: an identifier and a borrowed value.
///
/// Value borrows the input, which must outlive the element. The tag is owned. Cloning an
/// element clones the tag but not the borrowed bytes. Equality compares tag
/// and value, ignoring differences in raw length encoding.
#[derive(Clone, Debug)]
pub struct Element<'a> {
    tag: Tag,
    value: &'a [u8],
}

// Semantic equality compares tag and value; raw length encodings can differ.
impl PartialEq for Element<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag && self.value == other.value
    }
}
impl Eq for Element<'_> {}

impl<'a> Element<'a> {
    /// Creates an element from a tag and a value.
    pub fn new(tag: Tag, value: &'a [u8]) -> Element<'a> {
        Element { tag, value }
    }

    /// Returns the element tag.
    pub fn tag(&self) -> &Tag {
        &self.tag
    }

    /// Returns the element value bytes.
    pub fn value(&self) -> &'a [u8] {
        self.value
    }

    /// Converts a raw C element into an element, validating every field.
    ///
    /// # Safety
    ///
    /// If `raw.value.data` is non-null, it must point to `raw.value.size`
    /// readable bytes that stay valid and unmodified for `'a`. The same
    /// also applies to the identifier bytes.
    pub(crate) unsafe fn from_raw(raw: &native::tlv_element_t) -> Result<Element<'a>> {
        // SAFETY: the caller guarantees the tag bytes are readable, as for the value.
        let tag = unsafe { Tag::from_raw(&raw.tag) }?;

        let mut length = 0usize;
        // SAFETY: `length` is a valid, writable `usize`.
        Error::check(unsafe { native::tlv_size_to_native(raw.value.size, &mut length) })?;

        if length > isize::MAX as usize {
            return Err(Error::NativeSize);
        }
        let value: &'a [u8] = if length == 0 {
            &[]
        } else if raw.value.data.is_null() {
            return Err(Error::NullArg);
        } else {
            // SAFETY: non-null, and the caller guarantees `length` readable
            // bytes valid for `'a`.
            unsafe { slice::from_raw_parts(raw.value.data, length) }
        };
        Ok(Element { tag, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_element(tag: &[u8], data: *const u8, length: u64) -> native::tlv_element_t {
        let raw_tag = native::tlv_tag_t {
            data: if tag.is_empty() {
                std::ptr::null()
            } else {
                tag.as_ptr()
            },
            size: tag.len(),
        };
        native::tlv_element_t {
            tag: raw_tag,
            value: native::tlv_value_t { data, size: length },
        }
    }

    #[test]
    fn exposes_tag_and_value() {
        let tag = Tag::from_bytes(&[0x5A]);
        let element = Element::new(tag.clone(), &[1, 2, 3]);
        assert_eq!(element.tag(), &tag);
        assert_eq!(element.value(), &[1, 2, 3]);
    }

    #[test]
    fn from_raw_borrows_the_value() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF];
        let raw = raw_element(&[0x9F, 0x02], bytes.as_ptr(), bytes.len() as u64);
        // SAFETY: `bytes` outlives `element` and matches the declared length.
        let element = unsafe { Element::from_raw(&raw) }.unwrap();
        assert_eq!(element.tag().as_bytes(), &[0x9F, 0x02]);
        assert_eq!(element.value(), &bytes);
        assert_eq!(element.value().as_ptr(), bytes.as_ptr());
    }

    #[test]
    fn from_raw_accepts_empty_value_with_null_data() {
        let raw = raw_element(&[0x01], std::ptr::null(), 0);
        // SAFETY: a null pointer with zero length is valid.
        let element = unsafe { Element::from_raw(&raw) }.unwrap();
        assert!(element.value().is_empty());
    }

    #[test]
    fn from_raw_rejects_null_data_with_length() {
        let raw = raw_element(&[0x01], std::ptr::null(), 4);
        // SAFETY: the null pointer is rejected before any read.
        assert_eq!(unsafe { Element::from_raw(&raw) }, Err(Error::NullArg));
    }

    #[test]
    fn from_raw_rejects_a_tag_size_without_bytes() {
        let mut raw = raw_element(&[0x01], std::ptr::null(), 0);
        raw.tag.data = std::ptr::null();
        // SAFETY: the tag is rejected before the value is touched.
        assert_eq!(unsafe { Element::from_raw(&raw) }, Err(Error::NullArg));
    }

    #[test]
    fn from_raw_accepts_tags_of_any_length() {
        let tag = [0x5A; 300];
        let raw = raw_element(&tag, std::ptr::null(), 0);
        // SAFETY: `tag` outlives the call and matches the declared size.
        let element = unsafe { Element::from_raw(&raw) }.unwrap();
        assert_eq!(element.tag().as_bytes(), &tag[..]);
    }

    #[cfg(target_pointer_width = "32")]
    #[test]
    fn from_raw_rejects_length_beyond_usize() {
        let bytes = [0u8; 1];
        let raw = raw_element(&[0x01], bytes.as_ptr(), u64::MAX);
        // SAFETY: the length is rejected before any read.
        assert_eq!(unsafe { Element::from_raw(&raw) }, Err(Error::NativeSize));
    }
}
