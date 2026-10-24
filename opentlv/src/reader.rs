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
use crate::format::{Format, FormatRef};

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

/// Read one complete element and its original Layout through C `tlv_read_source_diag()`.
/// The consumed byte count is `decoded.encoded().len()`; trailing input is unread.
/// Empty input fails with [`Error::End`].
pub fn read<'a>(
    data: &'a [u8],
    format: impl Into<FormatRef<'a>>,
) -> std::result::Result<crate::Decoded<'a>, ReaderError> {
    let format = format.into();
    let mut decoded = MaybeUninit::<native::tlv_decoded_t>::uninit();
    let mut consumed = 0;
    // SAFETY: input and Format are borrowed for 'a; outputs are writable.
    read_once(|diagnostic| unsafe {
        let raw = decoded.as_mut_ptr();
        native::tlv_read_source_diag(
            data.as_ptr(),
            data.len(),
            format.raw(),
            std::ptr::addr_of_mut!((*raw).element),
            &mut consumed,
            std::ptr::addr_of_mut!((*raw).source),
            diagnostic,
        )
    })?;
    // SAFETY: success initializes decoded; backing input/Format remains live for 'a.
    unsafe { crate::Decoded::from_raw(decoded.assume_init()) }.map_err(ReaderError::from)
}

/// Read one complete element borrowing input and a Fixed Format for `'a`.
pub fn read_fixed<'a>(
    data: &'a [u8],
    format: &'a FixedFormat<'_>,
) -> std::result::Result<crate::Decoded<'a>, ReaderError> {
    read(data, format)
}

/// Read one Element without source metadata through C `tlv_read_diag()`.
/// Returns the Element and the consumed byte count; trailing input is unread.
/// The Tag is copied, so the Format is borrowed only for the call.
/// Empty input fails with [`Error::End`].
///
/// ```
/// use opentlv::{read_element, Format};
///
/// let (element, consumed) = read_element(&[0x50, 1, 0xAA, 0x51, 0], Format::Ber).unwrap();
/// assert_eq!((element.value(), consumed), (&[0xAA][..], 3));
/// ```
pub fn read_element<'a, 'f>(
    data: &'a [u8],
    format: impl Into<FormatRef<'f>>,
) -> std::result::Result<(Element<'a>, usize), ReaderError> {
    let format = format.into();
    let mut element = MaybeUninit::<native::tlv_element_t>::uninit();
    let mut consumed = 0;
    // SAFETY: input and Format are live for the call; outputs are writable.
    read_once(|diagnostic| unsafe {
        native::tlv_read_diag(
            data.as_ptr(),
            data.len(),
            format.raw(),
            element.as_mut_ptr(),
            &mut consumed,
            diagnostic,
        )
    })?;
    // SAFETY: success initializes element; its Value borrows the input for 'a,
    // and the Tag is copied before the Format borrow ends.
    let element = unsafe { Element::from_raw(&element.assume_init()) }?;
    Ok((element, consumed))
}

fn read_once(
    call: impl FnOnce(*mut native::tlv_reader_diagnostic_t) -> i32,
) -> std::result::Result<(), ReaderError> {
    let mut diagnostic = MaybeUninit::uninit();
    // SAFETY: C initializes all diagnostic fields before the synchronous operation.
    let (code, diagnostic) = unsafe {
        native::tlv_reader_diagnostic_init(diagnostic.as_mut_ptr());
        let code = call(diagnostic.as_mut_ptr());
        (code, diagnostic.assume_init())
    };
    match Error::from_code(code) {
        None => Ok(()),
        Some(error) => Err(ReaderError {
            error,
            // SAFETY: diagnostic pointers still borrow the caller's live input or Format.
            diagnostic: Some(Box::new(unsafe {
                crate::ReaderDiagnostic::from_raw(&diagnostic)
            })),
        }),
    }
}

impl From<Error> for ReaderError {
    fn from(error: Error) -> Self {
        ReaderError {
            error,
            diagnostic: None,
        }
    }
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

    /// Creates a reader for the given wire format: a builtin [`Format`], or a
    /// `&`[`FixedFormat`], `&`[`OwnedFixedFormat`](crate::OwnedFixedFormat) or
    /// `&`[`ComposedFormat`](crate::ComposedFormat) that outlives the reader.
    pub fn with_format(data: &'a [u8], format: impl Into<FormatRef<'a>>) -> Reader<'a> {
        let format = format.into();
        let mut raw = MaybeUninit::<native::tlv_reader_t>::uninit();
        // SAFETY: `raw` is writable; `data` is a valid slice (a non-null
        // pointer even when empty); `format` is borrowed for `'a`.
        let code = unsafe {
            native::tlv_reader_init(raw.as_mut_ptr(), data.as_ptr(), data.len(), format.raw())
        };
        // Every argument is non-null and every FormatRef descriptor can
        // decode, so initialization cannot fail.
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
    /// Equivalent to [`Reader::with_format`].
    pub fn with_fixed_format<'f>(data: &'a [u8], format: &'a FixedFormat<'f>) -> Reader<'a> {
        Reader::with_format(data, format)
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

    /// Pulls the next Element through C `tlv_reader_next_diag()`, without
    /// source metadata. Like [`Reader::read_source`], it remains callable after
    /// any error; final exhaustion is [`Error::End`].
    pub fn read_element(&mut self) -> Result<Element<'a>> {
        let mut element = MaybeUninit::<native::tlv_element_t>::uninit();
        let mut diagnostic = MaybeUninit::<native::tlv_reader_diagnostic_t>::uninit();
        // SAFETY: C initializes all diagnostic fields, and success initializes the element.
        let (code, diagnostic) = unsafe {
            native::tlv_reader_diagnostic_init(diagnostic.as_mut_ptr());
            let code = native::tlv_reader_next_diag(
                &mut self.raw,
                element.as_mut_ptr(),
                diagnostic.as_mut_ptr(),
            );
            (code, diagnostic.assume_init())
        };
        // SAFETY: diagnostic pointers still borrow live input or Format storage.
        self.diagnostic = (code != native::TLV_OK)
            .then(|| unsafe { crate::ReaderDiagnostic::from_raw(&diagnostic) });
        Error::check(code)?;
        // SAFETY: success initialized element; backing input/Format remains live for 'a.
        unsafe { Element::from_raw(&element.assume_init()) }
    }

    /// Creates a non-final reader. `NeedMoreData` permits `set_input` and retry.
    pub fn incremental(data: &'a [u8], format: impl Into<FormatRef<'a>>) -> Self {
        let format = format.into();
        let mut reader = Self::with_format(data, format);
        // SAFETY: same live input and borrowed Format as with_format.
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
    /// Equivalent to [`Reader::incremental`].
    pub fn incremental_fixed(data: &'a [u8], format: &'a FixedFormat<'_>) -> Self {
        Self::incremental(data, format)
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
        let result = self.read_element();
        self.failed = matches!(&result, Err(error) if *error != Error::NeedMoreData);
        Some(result)
    }
}

impl FusedIterator for Reader<'_> {}
