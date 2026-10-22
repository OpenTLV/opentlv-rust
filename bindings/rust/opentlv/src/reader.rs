// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Sequential reader over a borrowed TLV buffer.

use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

use opentlv_sys as native;

use crate::element::Element;
use crate::error::{Error, Result};
use crate::fixed_format::FixedFormat;
use crate::format::Format;

/// Single-element read failure with owned C diagnostic fields.
#[derive(Clone, Debug)]
pub struct ReaderError {
    /// Canonical error code.
    pub error: Error,
    /// Failure detail independent of the input lifetime.
    pub diagnostic: Option<Box<crate::ReaderDiagnostic>>,
}
impl std::fmt::Display for ReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ReaderError {}

/// Read one complete element and its original Layout through the C Reader.
/// The consumed byte count is `decoded.encoded().len()`; trailing input is unread.
pub fn read(data: &[u8], format: Format) -> std::result::Result<crate::Decoded<'_>, ReaderError> {
    read_once(Reader::with_format(data, format))
}

/// Read one complete element borrowing input and a Fixed Format for `'a`.
pub fn read_fixed<'a>(
    data: &'a [u8],
    format: &'a FixedFormat<'_>,
) -> std::result::Result<crate::Decoded<'a>, ReaderError> {
    read_once(Reader::with_fixed_format(data, format))
}

fn read_once(mut reader: Reader<'_>) -> std::result::Result<crate::Decoded<'_>, ReaderError> {
    reader.read_source().map_err(|error| ReaderError {
        error,
        diagnostic: reader.diagnostic.take().map(Box::new),
    })
}

/// A sequential reader that parses TLV elements from a byte slice.
///
/// `Reader` is an [`Iterator`] over `Result<Element<'a>>`. Elements borrow the
/// input, so the input must outlive them; the compiler enforces this. After
/// the first error the iterator yields nothing more, because the C reader does
/// not advance past malformed input.
///
/// ```
/// let data = [0x01, 0x02, 0xAA, 0xBB, 0x02, 0x00];
/// let mut found = Vec::new();
/// for element in opentlv::Reader::new(&data) {
///     let element = element.unwrap();
///     found.push((element.tag().as_bytes()[0], element.value().len()));
/// }
/// assert_eq!(found, [(0x01, 2), (0x02, 0)]);
/// ```
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    raw: native::tlv_reader_t,
    failed: bool,
    diagnostic: Option<crate::ReaderDiagnostic>,
    input: &'a [u8],
    _data: PhantomData<&'a [u8]>,
}

impl<'a> Reader<'a> {
    /// Creates a reader for BER with its tag encoding and definite lengths.
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader::with_format(data, Format::Ber)
    }

    /// Creates a reader for the given wire format.
    pub fn with_format(data: &'a [u8], format: Format) -> Reader<'a> {
        let mut raw = MaybeUninit::<native::tlv_reader_t>::uninit();
        // SAFETY: `raw` is writable; `data` is a valid slice (a non-null
        // pointer even when empty); `format.raw()` points to a static format.
        let code = unsafe {
            native::tlv_reader_init(raw.as_mut_ptr(), data.as_ptr(), data.len(), format.raw())
        };
        // Every argument is non-null and the built-in formats are complete, so
        // initialization cannot fail.
        assert_eq!(code, native::TLV_OK, "tlv_reader_init failed");
        Reader {
            // SAFETY: `tlv_reader_init` succeeded and initialized every field.
            raw: unsafe { raw.assume_init() },
            failed: false,
            diagnostic: None,
            input: data,
            _data: PhantomData,
        }
    }

    /// Creates a reader for a [`FixedFormat`]; `format` must outlive the reader.
    pub fn with_fixed_format<'f>(data: &'a [u8], format: &'a FixedFormat<'f>) -> Reader<'a> {
        let mut raw = MaybeUninit::<native::tlv_reader_t>::uninit();
        // SAFETY: `raw` is writable; `data` is a valid slice (a non-null
        // pointer even when empty); `format.raw()` points at storage
        // owned by `format`, which the borrow checker keeps alive for `'a`.
        let code = unsafe {
            native::tlv_reader_init(raw.as_mut_ptr(), data.as_ptr(), data.len(), format.raw())
        };
        assert_eq!(code, native::TLV_OK, "tlv_reader_init failed");
        Reader {
            // SAFETY: `tlv_reader_init` succeeded and initialized every field.
            raw: unsafe { raw.assume_init() },
            failed: false,
            diagnostic: None,
            input: data,
            _data: PhantomData,
        }
    }

    /// Returns the offset in the input of the next element to read.
    pub fn position(&self) -> usize {
        self.raw.pos
    }

    /// Returns `true` if all input has been consumed.
    pub fn is_at_end(&self) -> bool {
        // SAFETY: `self.raw` was initialized by `tlv_reader_init`.
        unsafe { native::tlv_reader_at_end(&self.raw) != 0 }
    }

    /// Reads the next element, or returns `None` once the input is consumed or
    /// after an error.
    pub fn next_element(&mut self) -> Option<Result<Element<'a>>> {
        self.next()
    }

    /// Creates a non-final reader. `NeedMoreData` permits `set_input` and retry.
    pub fn incremental(data: &'a [u8], format: Format) -> Self {
        let mut reader = Self::with_format(data, format);
        // SAFETY: same live input and static Format as with_format.
        let code = unsafe {
            native::tlv_reader_init_incremental(
                &mut reader.raw,
                data.as_ptr(),
                data.len(),
                format.raw(),
            )
        };
        assert_eq!(code, native::TLV_OK);
        reader
    }

    /// Creates a non-final reader borrowing a fixed descriptor and its configuration.
    pub fn incremental_fixed(data: &'a [u8], format: &'a FixedFormat<'_>) -> Self {
        let mut reader = Self::with_fixed_format(data, format);
        // SAFETY: input and format are borrowed for 'a by this signature.
        let code = unsafe {
            native::tlv_reader_init_incremental(
                &mut reader.raw,
                data.as_ptr(),
                data.len(),
                format.raw(),
            )
        };
        assert_eq!(code, native::TLV_OK);
        reader
    }

    /// Replaces the input window; undiscarded bytes must remain identical.
    /// All old views retain their original `'a` borrow. Failure preserves the cursor.
    ///
    /// ```compile_fail
    /// use opentlv::{Reader, Format};
    /// let mut old = vec![4, 1, 42];
    /// let replacement = [4, 0];
    /// let mut reader = Reader::incremental(&old, Format::Ber);
    /// let retained = reader.read_source().unwrap();
    /// reader.set_input(&replacement, 3, true).unwrap();
    /// old.clear(); // retained still borrows the old window
    /// assert_eq!(retained.element.value(), &[42]);
    /// ```
    pub fn set_input(&mut self, data: &'a [u8], discard: usize, final_input: bool) -> Result<()> {
        if discard > self.input.len() || !data.starts_with(&self.input[discard..]) {
            return Err(Error::InvalidArg);
        }
        // SAFETY: both old and new input are immutable and live for 'a.
        Error::check(unsafe {
            native::tlv_reader_set_input(
                &mut self.raw,
                data.as_ptr(),
                data.len(),
                discard,
                final_input.into(),
            )
        })?;
        self.input = data;
        Ok(())
    }

    /// Bytes of the current window that the parser no longer needs.
    pub fn consumed(&self) -> usize {
        // SAFETY: initialized cursor.
        unsafe { native::tlv_reader_consumed(&self.raw) }
    }

    /// Absolute next-element offset, including discarded windows.
    pub fn offset(&self) -> usize {
        // SAFETY: initialized cursor.
        unsafe { native::tlv_reader_offset(&self.raw) }
    }

    /// Owned detail for the most recent failed pull; cleared by a successful pull.
    pub fn diagnostic(&self) -> Option<&crate::ReaderDiagnostic> {
        self.diagnostic.as_ref()
    }

    /// Visit remaining elements through C. STOP consumes the current element.
    /// Panics resume after returning from C; prior callback effects are not rolled back.
    pub fn visit(&mut self, mut callback: impl FnMut(Element<'a>) -> crate::Visit) -> Result<()> {
        // SAFETY: exclusive cursor, input and Format borrowed for 'a.
        let (result, diagnostic) = unsafe {
            crate::visitor::run(
                &mut self.raw,
                std::ptr::null_mut(),
                |element, _, _| callback(element),
                std::ptr::null_mut(),
            )
        };
        self.diagnostic = diagnostic;
        result
    }

    /// Pulls complete content and source ranges with original C continuation semantics.
    /// Unlike the convenience iterator, this method remains callable after any error.
    pub fn read_source(&mut self) -> Result<crate::Decoded<'a>> {
        let mut decoded = MaybeUninit::<native::tlv_decoded_t>::uninit();
        let mut diagnostic = MaybeUninit::<native::tlv_reader_diagnostic_t>::uninit();
        // SAFETY: C initializes all diagnostic fields, and success initializes both outputs.
        let (code, diagnostic) = unsafe {
            native::tlv_reader_diagnostic_init(diagnostic.as_mut_ptr());
            let raw = decoded.as_mut_ptr();
            let code = native::tlv_reader_next_source_diag(
                &mut self.raw,
                std::ptr::addr_of_mut!((*raw).element),
                std::ptr::addr_of_mut!((*raw).source),
                diagnostic.as_mut_ptr(),
            );
            (code, diagnostic.assume_init())
        };
        // SAFETY: diagnostic pointers still borrow live input or Format storage.
        self.diagnostic = (code != native::TLV_OK)
            .then(|| unsafe { crate::ReaderDiagnostic::from_raw(&diagnostic) });
        Error::check(code)?;
        // SAFETY: success initializes decoded; backing input/Format remains live for 'a.
        unsafe { crate::Decoded::from_raw(decoded.assume_init()) }
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = Result<Element<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.is_at_end() {
            return None;
        }
        let result = self.read_source().map(|decoded| decoded.element);
        self.failed = matches!(&result, Err(error) if *error != Error::NeedMoreData);
        Some(result)
    }
}

impl FusedIterator for Reader<'_> {}
