// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! The DER and CER validators: strict, limit-bounded validation and canonical
//! writing of ASN.1 encodings.
//!
//! Bounded operations on [`Format::Der`] and [`Format::Cer`] enforce canonical
//! encoding rules and resource [`Limits`]. Other formats return
//! [`Error::InvalidArg`] with unknown location. All checks run in the C library.

use std::error;
use std::fmt;
use std::mem::MaybeUninit;
use std::ptr;

use opentlv_sys as native;

use crate::element::Element;
use crate::error::Error;
use crate::format::Format;
use crate::tag::Tag;

/// How much of the content a [`Format`] checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Strictness {
    /// Check framing, tags and lengths against the format's canonical rules.
    #[default]
    Canonical,
    /// Additionally validate the content of every UNIVERSAL-class element
    /// against the ASN.1 canonical rules for its type.
    Strict,
}

/// Inclusive resource limits for a [`Format`]. Zero is a real limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Limits {
    /// Maximum number of constructed ancestors, at most 64.
    pub max_depth: usize,
    /// Bounds the supplied input, or the complete encoded output when writing.
    pub max_input_size: usize,
    /// Bounds each value.
    pub max_value_size: usize,
    /// Bounds the total visited elements.
    pub max_elements: usize,
}

/// A failed validation operation: the error and where it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationError {
    /// The error reported by the C library.
    pub error: Error,
    /// Primary evidence in input or would-be output coordinates.
    /// Argument/configuration failures have unknown location.
    pub location: crate::Location,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location.offset() {
            Some(offset) => write!(
                f,
                "{} at offset {} ({:?} {:?})",
                self.error, offset, self.location.domain, self.location.kind
            ),
            None => write!(f, "{}", self.error),
        }
    }
}

impl error::Error for ValidationError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<ValidationError> for Error {
    fn from(err: ValidationError) -> Error {
        err.error
    }
}

impl Limits {
    fn der(&self) -> native::tlv_der_limits_t {
        native::tlv_der_limits_t {
            max_depth: self.max_depth,
            max_input_size: self.max_input_size,
            max_value_size: self.max_value_size,
            max_elements: self.max_elements,
        }
    }

    fn cer(&self) -> native::tlv_cer_limits_t {
        native::tlv_cer_limits_t {
            max_depth: self.max_depth,
            max_input_size: self.max_input_size,
            max_value_size: self.max_value_size,
            max_elements: self.max_elements,
        }
    }
}

fn outcome(
    code: native::tlv_result_t,
    diagnostic: native::tlv_diagnostic_t,
) -> Result<(), ValidationError> {
    match Error::from_code(code) {
        None => Ok(()),
        Some(error) => Err(ValidationError {
            error,
            location: crate::Location::from_raw(diagnostic.location),
        }),
    }
}

impl Format {
    /// Returns the library's default limits for DER or CER.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArg`] at offset zero for other formats.
    pub fn default_limits(self) -> Result<Limits, ValidationError> {
        // SAFETY: reads an immutable static of plain integers.
        unsafe {
            Ok(match self {
                Format::Der => {
                    let l = ptr::addr_of!(native::tlv_der_default_limits).read();
                    Limits {
                        max_depth: l.max_depth,
                        max_input_size: l.max_input_size,
                        max_value_size: l.max_value_size,
                        max_elements: l.max_elements,
                    }
                }
                Format::Cer => {
                    let l = ptr::addr_of!(native::tlv_cer_default_limits).read();
                    Limits {
                        max_depth: l.max_depth,
                        max_input_size: l.max_input_size,
                        max_value_size: l.max_value_size,
                        max_elements: l.max_elements,
                    }
                }
                _ => {
                    return Err(ValidationError {
                        error: Error::InvalidArg,
                        location: crate::Location::default(),
                    })
                }
            })
        }
    }

    /// Validates all concatenated elements of `data`, recursively. Empty input
    /// is valid.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArg`] at offset zero for formats other than DER/CER.
    ///
    /// A [`ValidationError`]: [`Error::Limit`] if a limit is exceeded,
    /// [`Error::InvalidValue`] or [`Error::Unsupported`] for content that
    /// fails [`Strictness::Strict`], or another error for malformed or
    /// noncanonical input.
    ///
    /// ```
    /// use opentlv::{Limits, Format, Strictness};
    ///
    /// let limits = Format::Der.default_limits().unwrap();
    /// // INTEGER 5, then the same element with a non-minimal length.
    /// assert!(Format::Der.validate(&[0x02, 0x01, 0x05], &limits, Strictness::Canonical).is_ok());
    /// assert!(Format::Der.validate(&[0x02, 0x81, 0x01, 0x05], &limits, Strictness::Canonical).is_err());
    /// ```
    pub fn validate(
        self,
        data: &[u8],
        limits: &Limits,
        strictness: Strictness,
    ) -> Result<(), ValidationError> {
        // SAFETY: zero diagnostic has no active borrows.
        let mut diagnostic: native::tlv_diagnostic_t = unsafe { std::mem::zeroed() };
        let (ptr, len) = (data.as_ptr(), data.len());
        let no_context = ptr::null_mut();
        // SAFETY: `data` is a valid slice, the limits are valid for the call,
        // there is no visitor, and `offset` is a writable `usize`.
        let code = unsafe {
            match (self, strictness) {
                (Format::Der, Strictness::Canonical) => native::tlv_der_visit(
                    ptr,
                    len,
                    &limits.der(),
                    None,
                    no_context,
                    &mut diagnostic,
                ),
                (Format::Der, Strictness::Strict) => native::tlv_der_visit_strict(
                    ptr,
                    len,
                    &limits.der(),
                    None,
                    no_context,
                    &mut diagnostic,
                ),
                (Format::Cer, Strictness::Canonical) => native::tlv_cer_visit(
                    ptr,
                    len,
                    &limits.cer(),
                    None,
                    no_context,
                    &mut diagnostic,
                ),
                (Format::Cer, Strictness::Strict) => native::tlv_cer_visit_strict(
                    ptr,
                    len,
                    &limits.cer(),
                    None,
                    no_context,
                    &mut diagnostic,
                ),
                _ => {
                    return Err(ValidationError {
                        error: Error::InvalidArg,
                        location: crate::Location::default(),
                    })
                }
            }
        };
        outcome(code, diagnostic)
    }

    /// Validates the first complete element of `data` and returns it with the
    /// number of bytes it occupies. Trailing bytes are ignored.
    ///
    /// The returned element's value borrows `data`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArg`] at offset zero for formats other than DER/CER.
    ///
    /// Same as [`Format::validate`], plus [`Error::End`] for empty
    /// input.
    pub fn read<'a>(
        self,
        data: &'a [u8],
        limits: &Limits,
        strictness: Strictness,
    ) -> Result<(Element<'a>, usize), ValidationError> {
        let mut element = MaybeUninit::<native::tlv_element_t>::uninit();
        let mut consumed = 0usize;
        // SAFETY: zero diagnostic has no active borrows.
        let mut diagnostic: native::tlv_diagnostic_t = unsafe { std::mem::zeroed() };
        let (ptr, len) = (data.as_ptr(), data.len());
        let out = element.as_mut_ptr();
        // SAFETY: `data` is a valid slice, the limits are valid for the call,
        // and `out`, `consumed` and `offset` are writable.
        let code = unsafe {
            match (self, strictness) {
                (Format::Der, Strictness::Canonical) => native::tlv_der_read(
                    ptr,
                    len,
                    &limits.der(),
                    out,
                    &mut consumed,
                    &mut diagnostic,
                ),
                (Format::Der, Strictness::Strict) => native::tlv_der_read_strict(
                    ptr,
                    len,
                    &limits.der(),
                    out,
                    &mut consumed,
                    &mut diagnostic,
                ),
                (Format::Cer, Strictness::Canonical) => native::tlv_cer_read(
                    ptr,
                    len,
                    &limits.cer(),
                    out,
                    &mut consumed,
                    &mut diagnostic,
                ),
                (Format::Cer, Strictness::Strict) => native::tlv_cer_read_strict(
                    ptr,
                    len,
                    &limits.cer(),
                    out,
                    &mut consumed,
                    &mut diagnostic,
                ),
                _ => {
                    return Err(ValidationError {
                        error: Error::InvalidArg,
                        location: crate::Location::default(),
                    })
                }
            }
        };
        outcome(code, diagnostic)?;
        // SAFETY: the read succeeded, so `element` is initialized and its value
        // borrows `data`, which lives for `'a`.
        let element = unsafe { Element::from_raw(&element.assume_init()) }.map_err(|error| {
            ValidationError {
                error,
                location: crate::Location::default(),
            }
        })?;
        Ok((element, consumed))
    }

    fn write_raw(
        self,
        tag: &Tag,
        value: &[u8],
        limits: &Limits,
        strictness: Strictness,
        data: *mut u8,
        capacity: usize,
    ) -> Result<usize, ValidationError> {
        let mut written = 0usize;
        // SAFETY: zero diagnostic has no active borrows.
        let mut diagnostic: native::tlv_diagnostic_t = unsafe { std::mem::zeroed() };
        let (vptr, vlen) = (value.as_ptr(), value.len());
        // SAFETY: `data` is null with zero capacity (a size query) or a
        // writable buffer of `capacity` bytes; `value` is a valid slice that
        // does not overlap it; the limits are valid for the call; `written`
        // and `offset` are writable.
        let code = unsafe {
            match (self, strictness) {
                (Format::Der, Strictness::Canonical) => native::tlv_der_write(
                    data,
                    capacity,
                    tag.raw(),
                    vptr,
                    vlen,
                    &limits.der(),
                    &mut written,
                    &mut diagnostic,
                ),
                (Format::Der, Strictness::Strict) => native::tlv_der_write_strict(
                    data,
                    capacity,
                    tag.raw(),
                    vptr,
                    vlen,
                    &limits.der(),
                    &mut written,
                    &mut diagnostic,
                ),
                (Format::Cer, Strictness::Canonical) => native::tlv_cer_write(
                    data,
                    capacity,
                    tag.raw(),
                    vptr,
                    vlen,
                    &limits.cer(),
                    &mut written,
                    &mut diagnostic,
                ),
                (Format::Cer, Strictness::Strict) => native::tlv_cer_write_strict(
                    data,
                    capacity,
                    tag.raw(),
                    vptr,
                    vlen,
                    &limits.cer(),
                    &mut written,
                    &mut diagnostic,
                ),
                _ => {
                    return Err(ValidationError {
                        error: Error::InvalidArg,
                        location: crate::Location::default(),
                    })
                }
            }
        };
        outcome(code, diagnostic)?;
        Ok(written)
    }

    /// Validates an element and returns the size of its canonical encoding
    /// without writing it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArg`] at offset zero for formats other than DER/CER.
    ///
    /// A [`ValidationError`] for an invalid tag, value or child.
    pub fn encoded_size(
        self,
        tag: &Tag,
        value: &[u8],
        limits: &Limits,
        strictness: Strictness,
    ) -> Result<usize, ValidationError> {
        self.write_raw(tag, value, limits, strictness, ptr::null_mut(), 0)
    }

    /// Writes one canonical element into `out` and returns the number of bytes
    /// written.
    ///
    /// The value is copied verbatim; the value of a constructed element must
    /// already hold canonical children, which are validated first.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArg`] at offset zero for formats other than DER/CER.
    ///
    /// [`Error::BufferTooShort`] if `out` is too small, or another
    /// [`ValidationError`] for an invalid tag, value or child.
    ///
    /// ```
    /// use opentlv::{Format, Strictness, Tag};
    ///
    /// let mut out = [0u8; 8];
    /// let tag = Tag::from_bytes(&[0x04]); // OCTET STRING
    /// let limits = Format::Der.default_limits().unwrap();
    /// let n = Format::Der.write(&tag, &[0xAB], &limits, Strictness::Strict, &mut out).unwrap();
    /// assert_eq!(&out[..n], &[0x04, 0x01, 0xAB]);
    /// ```
    pub fn write(
        self,
        tag: &Tag,
        value: &[u8],
        limits: &Limits,
        strictness: Strictness,
        out: &mut [u8],
    ) -> Result<usize, ValidationError> {
        self.write_raw(tag, value, limits, strictness, out.as_mut_ptr(), out.len())
    }
}
