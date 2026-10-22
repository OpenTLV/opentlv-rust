// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! The TLV tag type.

use std::fmt;
use std::ptr;
use std::slice;

use opentlv_sys as native;

use crate::error::{Error, Result};

/// A TLV tag: an arbitrary sequence of raw bytes in wire order.
///
/// A tag owns its bytes and has no length limit. Whether the bytes form a valid
/// tag, and how long a tag may be, is decided by the [`Format`](crate::Format)
/// or standard a tag is used with, and which tags are allowed is decided by a
/// schema. Tags compare equal when their bytes are equal and are ordered
/// lexicographically by their bytes.
///
/// The C library's `tlv_tag_t` only borrows its bytes. This type owns them, so
/// a tag can be stored in a schema or kept after the input it came from is
/// gone; it is cloned, not copied.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tag {
    bytes: Vec<u8>,
}

impl Tag {
    /// Creates a tag from raw bytes in wire order.
    ///
    /// There is no size limit; the bytes are not validated against any format.
    pub fn from_bytes(bytes: &[u8]) -> Tag {
        Tag {
            bytes: bytes.to_vec(),
        }
    }

    /// Copies a tag received from the C library.
    ///
    /// # Safety
    ///
    /// If `raw.data` is non-null, it must point to `raw.size` readable bytes.
    pub(crate) unsafe fn from_raw(raw: &native::tlv_tag_t) -> Result<Tag> {
        if raw.size == 0 {
            return Ok(Tag::default());
        }
        if raw.data.is_null() {
            return Err(Error::NullArg);
        }
        // SAFETY: non-null, and the caller guarantees `raw.size` readable bytes.
        Ok(Tag::from_bytes(unsafe {
            slice::from_raw_parts(raw.data, raw.size)
        }))
    }

    /// Returns a C tag that borrows this tag's bytes.
    ///
    /// The result must not outlive `self` or be used after `self` changes.
    pub(crate) fn raw(&self) -> native::tlv_tag_t {
        native::tlv_tag_t {
            data: if self.bytes.is_empty() {
                ptr::null()
            } else {
                self.bytes.as_ptr()
            },
            size: self.bytes.len(),
        }
    }

    /// Returns the tag bytes in wire order.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the number of bytes in the tag.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` if the tag has no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<&[u8]> for Tag {
    fn from(bytes: &[u8]) -> Tag {
        Tag::from_bytes(bytes)
    }
}

impl From<Vec<u8>> for Tag {
    fn from(bytes: Vec<u8>) -> Tag {
        Tag { bytes }
    }
}

/// Formats the tag as uppercase hexadecimal, for example `9F02`.
impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_bytes()
            .iter()
            .try_for_each(|byte| write!(f, "{byte:02X}"))
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn from_bytes_keeps_wire_order() {
        let tag = Tag::from_bytes(&[0x9F, 0x02]);
        assert_eq!(tag.as_bytes(), &[0x9F, 0x02]);
        assert_eq!(tag.len(), 2);
        assert!(!tag.is_empty());
    }

    #[test]
    fn empty_tag_is_allowed() {
        let tag = Tag::from_bytes(&[]);
        assert!(tag.is_empty());
        assert_eq!(tag.as_bytes(), &[] as &[u8]);
        assert_eq!(tag, Tag::default());
    }

    #[test]
    fn tags_have_no_length_limit() {
        for size in [1, 8, 9, 12, 255, 256, 1000] {
            let bytes = vec![0xAB; size];
            let tag = Tag::from_bytes(&bytes);
            assert_eq!(tag.len(), size);
            assert_eq!(tag.as_bytes(), bytes.as_slice());
        }
    }

    #[test]
    fn equality_ordering_and_hash_use_bytes_only() {
        let a = Tag::from_bytes(&[0x5F, 0x2A]);
        let b = Tag::from(&[0x5F, 0x2A][..]);
        let c = Tag::from_bytes(&[0x5F]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        // Lexicographic: a prefix orders first, and bytes are unsigned.
        assert!(c < a);
        assert!(Tag::from_bytes(&[0x7F]) < Tag::from_bytes(&[0x80]));
        let set: HashSet<Tag> = [a, b, c].into_iter().collect();
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn ordering_matches_the_c_comparison() {
        let cases: [(&[u8], &[u8]); 5] = [
            (&[0x9F, 0x02], &[0x9F, 0x03]),
            (&[0x9F], &[0x9F, 0x00]),
            (&[], &[0x00]),
            (&[0x7F], &[0x80]),
            (
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13],
            ),
        ];
        for (lhs, rhs) in cases {
            let (l, r) = (Tag::from_bytes(lhs), Tag::from_bytes(rhs));
            // SAFETY: both tags borrow live `Tag`s for the calls.
            let (c_order, c_equal) = unsafe {
                (
                    native::tlv_tag_compare(l.raw(), r.raw()),
                    native::tlv_tag_equal(l.raw(), r.raw()),
                )
            };
            assert_eq!(l.cmp(&r), c_order.cmp(&0), "{lhs:?} vs {rhs:?}");
            assert_eq!(l == r, c_equal);
        }
    }

    #[test]
    fn formats_as_hex() {
        let tag = Tag::from_bytes(&[0x9F, 0x02]);
        assert_eq!(tag.to_string(), "9F02");
        assert_eq!(format!("{tag:?}"), "Tag(9F02)");
    }

    #[test]
    fn raw_borrows_the_bytes_and_from_raw_copies_them() {
        let tag = Tag::from_bytes(&[0x11, 0x22, 0x33]);
        let raw = tag.raw();
        assert_eq!(raw.size, 3);
        assert_eq!(raw.data, tag.as_bytes().as_ptr());
        // SAFETY: `raw` borrows `tag`, which is alive.
        let copy = unsafe { Tag::from_raw(&raw) }.unwrap();
        assert_eq!(copy, tag);
        assert_ne!(copy.as_bytes().as_ptr(), tag.as_bytes().as_ptr());
    }

    #[test]
    fn empty_tags_use_a_null_pointer() {
        let raw = Tag::default().raw();
        assert!(raw.data.is_null());
        assert_eq!(raw.size, 0);
        // SAFETY: a null pointer with zero size is valid.
        assert_eq!(unsafe { Tag::from_raw(&raw) }, Ok(Tag::default()));
    }

    #[test]
    fn from_raw_rejects_null_data_with_a_size() {
        let raw = native::tlv_tag_t {
            data: ptr::null(),
            size: 2,
        };
        // SAFETY: the null pointer is rejected before any read.
        assert_eq!(unsafe { Tag::from_raw(&raw) }, Err(Error::NullArg));
    }
}
