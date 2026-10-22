// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Sequential writer over a borrowed output buffer.

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::slice;

use opentlv_sys as native;

use crate::element::Element;
use crate::error::{Error, Result};
use crate::fixed_format::FixedFormat;
use crate::format::Format;
use crate::tag::Tag;
use crate::{Decoded, WriterDiagnostic};

/// A single-element write or measurement failure, owning its C diagnostic.
#[derive(Clone, Debug)]
pub struct WriterError {
    /// Canonical error code.
    pub error: Error,
    /// Copied failure detail, independent of temporary input lifetimes.
    pub diagnostic: Box<WriterDiagnostic>,
}
impl std::fmt::Display for WriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for WriterError {}

fn diagnosed_size(
    call: impl FnOnce(*mut usize, *mut native::tlv_writer_diagnostic_t) -> i32,
) -> std::result::Result<usize, WriterError> {
    let mut size = 0;
    let mut diagnostic = MaybeUninit::uninit();
    // SAFETY: initialize writable diagnostic before the synchronous operation.
    unsafe { native::tlv_writer_diagnostic_init(diagnostic.as_mut_ptr()) };
    let code = call(&mut size, diagnostic.as_mut_ptr());
    if let Some(error) = Error::from_code(code) {
        // SAFETY: initialized diagnostic, caller retains all inputs until we return.
        return Err(WriterError {
            error,
            diagnostic: Box::new(unsafe { WriterDiagnostic::from_raw(&diagnostic.assume_init()) }),
        });
    }
    Ok(size)
}

/// Measure exact encoding through C, preserving structured errors.
pub fn measure_element(
    element: &Element<'_>,
    format: Format,
) -> std::result::Result<usize, WriterError> {
    let element = raw_element(element);
    // SAFETY: live immutable content/Format and writable outputs.
    diagnosed_size(|size, diagnostic| unsafe {
        native::tlv_element_encoded_size_diag(&element, format.raw(), size, diagnostic)
    })
}

/// Measure using a borrowed Fixed Format, preserving structured errors.
pub fn measure_element_fixed(
    element: &Element<'_>,
    format: &FixedFormat<'_>,
) -> std::result::Result<usize, WriterError> {
    let element = raw_element(element);
    // SAFETY: live immutable content/Format and writable outputs.
    diagnosed_size(|size, diagnostic| unsafe {
        native::tlv_element_encoded_size_diag(&element, format.raw(), size, diagnostic)
    })
}

/// Encode a single Element into caller storage, preserving C diagnostics.
/// Capacity shortage reports diagnostic.required and leaves output unchanged.
pub fn write_element(
    output: &mut [u8],
    element: &Element<'_>,
    format: Format,
) -> std::result::Result<usize, WriterError> {
    let element = raw_element(element);
    // SAFETY: output is exclusive and disjoint from all readable inputs.
    diagnosed_size(|written, diagnostic| unsafe {
        native::tlv_write_element_diag(
            output.as_mut_ptr(),
            output.len(),
            format.raw(),
            &element,
            written,
            diagnostic,
        )
    })
}

/// Encode a single Element with a borrowed Fixed Format and structured errors.
pub fn write_element_fixed(
    output: &mut [u8],
    element: &Element<'_>,
    format: &FixedFormat<'_>,
) -> std::result::Result<usize, WriterError> {
    let element = raw_element(element);
    // SAFETY: output is exclusive and disjoint from all readable inputs.
    diagnosed_size(|written, diagnostic| unsafe {
        native::tlv_write_element_diag(
            output.as_mut_ptr(),
            output.len(),
            format.raw(),
            &element,
            written,
            diagnostic,
        )
    })
}

fn raw_element(element: &Element<'_>) -> native::tlv_element_t {
    native::tlv_element_t {
        tag: element.tag().raw(),
        value: native::tlv_value_t {
            data: element.value().as_ptr(),
            size: element.value().len() as u64,
        },
    }
}

/// Measures exact destination storage including Header and Trailer without allocation.
/// Readable Value content is supplied, so content-dependent sizing is supported.
///
/// # Errors
/// Returns the C measurement error, including native size overflow.
pub fn element_encoded_size(element: &Element<'_>, format: Format) -> Result<usize> {
    let raw = raw_element(element);
    let mut size = 0;
    // SAFETY: input and format remain valid for the call; size is writable.
    Error::check(unsafe { native::tlv_element_encoded_size(&raw, format.raw(), &mut size) })?;
    Ok(size)
}

/// Measures an Element in a configurable fixed format without allocation.
///
/// # Errors
/// Returns the C measurement error, including native size overflow.
pub fn element_encoded_size_fixed(
    element: &Element<'_>,
    format: &FixedFormat<'_>,
) -> Result<usize> {
    let raw = raw_element(element);
    let mut size = 0;
    // SAFETY: input and borrowed format remain valid for the call; size is writable.
    Error::check(unsafe { native::tlv_element_encoded_size(&raw, format.raw(), &mut size) })?;
    Ok(size)
}

/// Returns the encoded size of an element with the given tag and value length
/// in `format`, without writing anything.
///
/// # Errors
///
/// [`Error::InvalidTagSize`] for an empty tag, [`Error::InvalidLength`] if the
/// total does not fit in `usize`, or any error of the format.
///
/// ```
/// use opentlv::{encoded_size, Format, Tag};
///
/// let tag = Tag::from_bytes(&[0x01]);
/// assert_eq!(encoded_size(&tag, 3, Format::Ber).unwrap(), 5);
/// ```
pub fn encoded_size(tag: &Tag, value_len: usize, format: Format) -> Result<usize> {
    let mut size = 0usize;
    // SAFETY: `format.raw()` points to a static format and `size` is
    // a valid, writable `usize`.
    let code = unsafe { native::tlv_encoded_size(tag.raw(), value_len, format.raw(), &mut size) };
    Error::check(code)?;
    Ok(size)
}

/// Returns the encoded size of an element with the given tag and value length
/// in a [`FixedFormat`], without writing anything.
///
/// # Errors
///
/// Same as [`encoded_size`].
pub fn encoded_size_fixed(tag: &Tag, value_len: usize, format: &FixedFormat<'_>) -> Result<usize> {
    let mut size = 0usize;
    // SAFETY: `format.raw()` points at storage owned by `format`,
    // borrowed for this call only, and `size` is a valid, writable `usize`.
    let code = unsafe { native::tlv_encoded_size(tag.raw(), value_len, format.raw(), &mut size) };
    Error::check(code)?;
    Ok(size)
}

/// A sequential writer that encodes TLV entries into a byte slice.
///
/// Output storage never grows: it fills the caller's buffer and reports
/// [`Error::BufferTooShort`] when an element does not fit. A failed write leaves
/// the position unchanged. Failure diagnostics own copies of borrowed details.
/// Values are supplied as `&[u8]`.
///
/// ```
/// use opentlv::{Tag, Writer};
///
/// let mut buf = [0u8; 16];
/// let mut writer = Writer::new(&mut buf);
/// writer.write(&Tag::from_bytes(&[0x01]), &[0xAA, 0xBB]).unwrap();
/// writer.write(&Tag::from_bytes(&[0x02]), &[]).unwrap();
/// assert_eq!(writer.written(), &[0x01, 0x02, 0xAA, 0xBB, 0x02, 0x00]);
/// ```
#[derive(Debug)]
pub struct Writer<'a> {
    raw: native::tlv_writer_t,
    _buf: PhantomData<&'a mut [u8]>,
    diagnostic: Option<WriterDiagnostic>,
}

impl<'a> Writer<'a> {
    /// Creates a writer for BER with its tag encoding and definite lengths.
    pub fn new(buf: &'a mut [u8]) -> Writer<'a> {
        Writer::with_format(buf, Format::Ber)
    }

    /// Creates a writer for the given wire format.
    pub fn with_format(buf: &'a mut [u8], format: Format) -> Writer<'a> {
        let mut raw = MaybeUninit::<native::tlv_writer_t>::uninit();
        // SAFETY: `raw` is writable; `buf` is a valid slice (a non-null
        // pointer even when empty); `format.raw()` points to a static
        // format.
        let code = unsafe {
            native::tlv_writer_init(raw.as_mut_ptr(), buf.as_mut_ptr(), buf.len(), format.raw())
        };
        // Every argument is non-null and the built-in formats are complete, so
        // initialization cannot fail.
        assert_eq!(code, native::TLV_OK, "tlv_writer_init failed");
        Writer {
            // SAFETY: `tlv_writer_init` succeeded and initialized every field.
            raw: unsafe { raw.assume_init() },
            _buf: PhantomData,
            diagnostic: None,
        }
    }

    /// Creates a writer for a [`FixedFormat`]; `format` must outlive the writer.
    pub fn with_fixed_format<'f>(buf: &'a mut [u8], format: &'a FixedFormat<'f>) -> Writer<'a> {
        let mut raw = MaybeUninit::<native::tlv_writer_t>::uninit();
        // SAFETY: `raw` is writable; `buf` is a valid slice (a non-null
        // pointer even when empty); `format.raw()` points at storage
        // owned by `format`, which the borrow checker keeps alive for `'a`.
        let code = unsafe {
            native::tlv_writer_init(raw.as_mut_ptr(), buf.as_mut_ptr(), buf.len(), format.raw())
        };
        assert_eq!(code, native::TLV_OK, "tlv_writer_init failed");
        Writer {
            // SAFETY: `tlv_writer_init` succeeded and initialized every field.
            raw: unsafe { raw.assume_init() },
            _buf: PhantomData,
            diagnostic: None,
        }
    }

    /// Appends one element with the given tag and value.
    ///
    /// # Errors
    ///
    /// [`Error::BufferTooShort`] if the element does not fit in the remaining
    /// space, or any error of the format (for example an invalid tag). On error
    /// the position is unchanged, though bytes past it may have been modified.
    pub fn write(&mut self, tag: &Tag, value: &[u8]) -> Result<()> {
        // SAFETY: `self.raw` is initialized and its buffer is exclusively
        // borrowed for `'a`; `value` is a valid slice of `value.len()` bytes.
        self.operation(|writer, diagnostic| unsafe {
            native::tlv_writer_write_diag(
                writer,
                tag.raw(),
                value.as_ptr(),
                value.len(),
                diagnostic,
            )
        })
    }

    /// Appends a decoded [`Element`], for example one produced by a `Reader`.
    ///
    /// # Errors
    ///
    /// Same as [`Writer::write`].
    pub fn write_element(&mut self, element: &Element<'_>) -> Result<()> {
        let raw = raw_element(element);
        // SAFETY: input is readable, output is exclusively borrowed and disjoint.
        self.operation(|writer, diagnostic| unsafe {
            native::tlv_writer_write_element_diag(writer, &raw, diagnostic)
        })
    }

    /// Appends bytes without framing validation or format conversion.
    ///
    /// # Errors
    /// Insufficient capacity leaves both output and position unchanged.
    pub fn copy_encoded(&mut self, encoded: &[u8]) -> Result<()> {
        // SAFETY: input is readable and the cursor owns an exclusive output borrow.
        self.operation(|writer, diagnostic| unsafe {
            native::tlv_writer_copy_encoded_diag(
                writer,
                encoded.as_ptr(),
                encoded.len(),
                diagnostic,
            )
        })
    }

    /// Preserve a decoded source exactly, rejecting semantic changes through C.
    /// The destination Format does not reinterpret the copied source bytes.
    pub fn preserve(&mut self, decoded: &Decoded<'_>) -> Result<()> {
        let element = raw_element(&decoded.element);
        // SAFETY: immutable source and content remain live and disjoint from output.
        self.operation(|writer, diagnostic| unsafe {
            native::tlv_writer_preserve_diag(writer, &decoded.source, &element, diagnostic)
        })
    }

    /// Owned detail for the latest failed write/copy/preservation operation.
    pub fn diagnostic(&self) -> Option<&WriterDiagnostic> {
        self.diagnostic.as_ref()
    }

    fn operation(
        &mut self,
        call: impl FnOnce(*mut native::tlv_writer_t, *mut native::tlv_writer_diagnostic_t) -> i32,
    ) -> Result<()> {
        let mut diagnostic = MaybeUninit::uninit();
        // SAFETY: writable diagnostic storage initialized before every operation.
        unsafe { native::tlv_writer_diagnostic_init(diagnostic.as_mut_ptr()) };
        let code = call(&mut self.raw, diagnostic.as_mut_ptr());
        // SAFETY: initialized diagnostic and caller-owned operation inputs remain live.
        self.diagnostic = (code != native::TLV_OK)
            .then(|| unsafe { WriterDiagnostic::from_raw(&diagnostic.assume_init()) });
        Error::check(code)
    }

    /// Returns the number of bytes written so far.
    pub fn position(&self) -> usize {
        // SAFETY: `self.raw` was initialized by `tlv_writer_init`.
        unsafe { native::tlv_writer_size(&self.raw) }
    }

    /// Returns the total capacity of the output buffer in bytes.
    pub fn capacity(&self) -> usize {
        self.raw.capacity
    }

    /// Returns the number of bytes that can still be written.
    pub fn remaining(&self) -> usize {
        // SAFETY: the cursor is initialized and borrowed for this call.
        unsafe { native::tlv_writer_remaining(&self.raw) }
    }

    /// Returns the bytes written so far.
    pub fn written(&self) -> &[u8] {
        // SAFETY: the first `position()` bytes of the buffer were written by
        // the C library and are valid; the buffer is not mutated while `self`
        // is borrowed shared.
        unsafe { slice::from_raw_parts(self.raw.buf, self.position()) }
    }

    /// Ends writing and returns the written bytes with the buffer's lifetime.
    pub fn finish(self) -> &'a mut [u8] {
        // SAFETY: as in `written`; consuming `self` guarantees exclusive access
        // to the buffer, which was exclusively borrowed for `'a`.
        unsafe { slice::from_raw_parts_mut(self.raw.buf, self.position()) }
    }
}
