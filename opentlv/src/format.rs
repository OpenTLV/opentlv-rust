// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Wire formats shared by the reader and the writer.

use std::fmt;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::ptr;
use std::str::FromStr;

use opentlv_sys as native;

use crate::error::Error;
use crate::{ComposedFormat, Decoded, Element, FixedFormat, OwnedFixedFormat};

/// The wire format a [`Reader`](crate::Reader), [`Writer`](crate::Writer) or
/// [`StructureSchema`](crate::StructureSchema) uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// BER-TLV.
    #[default]
    Ber,
    /// Definite EMV Contact Book 3 BER-TLV element framing.
    Emv,
    /// Canonical Encoding Rules.
    Cer,
    /// Distinguished Encoding Rules.
    Der,
    /// LLDP packed Type/Length framing; no LLDPDU semantic validation.
    #[cfg(feature = "lldp")]
    Lldp,
    /// Contiguous NFC Type 2 TLV stream; termination is controlled by the caller.
    #[cfg(feature = "nfc")]
    NfcType2,
}

impl Format {
    /// Every supported format.
    pub const ALL: [Format; 4 + cfg!(feature = "lldp") as usize + cfg!(feature = "nfc") as usize] = [
        Format::Ber,
        Format::Emv,
        Format::Cer,
        Format::Der,
        #[cfg(feature = "lldp")]
        Format::Lldp,
        #[cfg(feature = "nfc")]
        Format::NfcType2,
    ];

    /// Returns the lowercase name of the format, accepted by [`FromStr`].
    pub fn name(self) -> &'static str {
        match self {
            Format::Ber => "ber",
            Format::Emv => "emv",
            Format::Cer => "cer",
            Format::Der => "der",
            #[cfg(feature = "lldp")]
            Format::Lldp => "lldp",
            #[cfg(feature = "nfc")]
            Format::NfcType2 => "nfc-type2",
        }
    }

    pub(crate) fn raw(self) -> *const native::tlv_format_t {
        // SAFETY: only the address of an immutable static is taken; the
        // static lives for the whole program and is never written. Newer
        // compilers treat this as safe, but the MSRV (1.70) needs `unsafe`.
        #[allow(unused_unsafe)]
        unsafe {
            match self {
                Format::Ber => ptr::addr_of!(native::tlv_format_ber),
                Format::Emv => ptr::addr_of!(native::tlv_format_emv),
                Format::Cer => ptr::addr_of!(native::tlv_format_cer),
                Format::Der => ptr::addr_of!(native::tlv_format_der),
                #[cfg(feature = "lldp")]
                Format::Lldp => ptr::addr_of!(native::tlv_format_lldp),
                #[cfg(feature = "nfc")]
                Format::NfcType2 => ptr::addr_of!(native::tlv_format_nfc_type2),
            }
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A borrowed wire-format descriptor: a builtin [`Format`], a
/// [`FixedFormat`], an [`OwnedFixedFormat`] or a [`ComposedFormat`].
///
/// Every entry point that takes a format, such as [`Reader::with_format`],
/// [`Writer::with_format`], [`TreeReader::new`](crate::TreeReader::new),
/// [`Document::parse`](crate::Document::parse) or
/// [`StructureSchema::validate`](crate::StructureSchema::validate), accepts
/// anything that converts into a `FormatRef<'a>`, and keeps the descriptor and
/// its configuration borrowed for `'a`. It also exposes the operations of the
/// C `tlv_format_t` contract, independent of Reader and Writer.
///
/// ```
/// use opentlv::{Element, Format, FormatRef, Tag};
///
/// let ber = FormatRef::from(Format::Ber);
/// assert!(ber.can_read() && ber.can_write());
/// let element = Element::new(Tag::from_bytes(&[0x50]), b"VISA");
/// let encoded = ber.encode(&element).unwrap();
/// assert_eq!(encoded, [0x50, 4, b'V', b'I', b'S', b'A']);
/// assert_eq!(ber.decode(&encoded).unwrap().element, element);
/// ```
///
/// [`Reader::with_format`]: crate::Reader::with_format
/// [`Writer::with_format`]: crate::Writer::with_format
#[derive(Clone, Copy)]
pub struct FormatRef<'a> {
    raw: *const native::tlv_format_t,
    lifetime: PhantomData<&'a native::tlv_format_t>,
}

// SAFETY: every descriptor behind a FormatRef is immutable while borrowed:
// builtin statics, or Fixed and composed descriptors whose owners are Sync.
unsafe impl Send for FormatRef<'_> {}
unsafe impl Sync for FormatRef<'_> {}

impl fmt::Debug for FormatRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FormatRef").field(&self.raw).finish()
    }
}

impl<'a> From<Format> for FormatRef<'a> {
    fn from(format: Format) -> FormatRef<'a> {
        // SAFETY: builtin descriptors are immutable statics.
        unsafe { FormatRef::from_raw(format.raw()) }
    }
}
impl<'a> From<&'a FixedFormat<'_>> for FormatRef<'a> {
    fn from(format: &'a FixedFormat<'_>) -> FormatRef<'a> {
        // SAFETY: the borrow keeps the descriptor and its configuration alive.
        unsafe { FormatRef::from_raw(format.raw()) }
    }
}
impl<'a> From<&'a OwnedFixedFormat> for FormatRef<'a> {
    fn from(format: &'a OwnedFixedFormat) -> FormatRef<'a> {
        // SAFETY: the borrow keeps the shared owner alive.
        unsafe { FormatRef::from_raw(format.raw()) }
    }
}
impl<'a> From<&'a ComposedFormat> for FormatRef<'a> {
    fn from(format: &'a ComposedFormat) -> FormatRef<'a> {
        // SAFETY: the borrow keeps the shared owner and its tables alive.
        unsafe { FormatRef::from_raw(format.raw()) }
    }
}

/// A Format retained by an owner that outlives every borrow, such as a
/// compiled Query program or a Query schema.
#[derive(Clone, Debug)]
pub(crate) enum FormatOwner {
    Builtin(Format),
    Fixed(OwnedFixedFormat),
    Composed(ComposedFormat),
}

impl FormatOwner {
    pub(crate) fn raw(&self) -> *const native::tlv_format_t {
        match self {
            FormatOwner::Builtin(format) => format.raw(),
            FormatOwner::Fixed(format) => format.raw(),
            FormatOwner::Composed(format) => format.raw(),
        }
    }
}

/// Logical framing sizes of one element (`tlv_encoding_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Encoding {
    /// Bytes preceding the Value.
    pub header: u64,
    /// Semantic Value bytes.
    pub value: u64,
    /// Bytes following the Value.
    pub trailer: u64,
    /// Sum of header, value and trailer.
    pub total: u64,
}

/// The [`FormatRef`] operation that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatOperation {
    /// [`FormatRef::decode`].
    Decode,
    /// [`FormatRef::measure`].
    Measure,
    /// [`FormatRef::encode`] or [`FormatRef::encode_into`].
    Encode,
}

/// Wire region a Format was interpreting when it failed (`tlv_region_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Region {
    /// The header, or a header field without a name.
    Header,
    /// The identifier field.
    Tag,
    /// The encoded length field.
    Length,
    /// The Value bytes.
    Value,
    /// Trailing framing.
    Trailer,
}

impl Region {
    fn from_raw(raw: i32) -> Region {
        match raw {
            native::TLV_REGION_TAG => Region::Tag,
            native::TLV_REGION_LENGTH => Region::Length,
            native::TLV_REGION_VALUE => Region::Value,
            native::TLV_REGION_TRAILER => Region::Trailer,
            _ => Region::Header,
        }
    }
}

/// A [`FormatRef`] operation failure with the Format's own detail
/// (`tlv_format_error_t`).
///
/// Decode offsets are relative to the input; measure and encode offsets to the
/// output the element would have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    /// Canonical error code.
    pub error: Error,
    /// The failed operation.
    pub operation: FormatOperation,
    /// The region being interpreted.
    pub region: Region,
    /// Known failure position.
    pub offset: Option<usize>,
    /// Expected logical extent, when known; for a short destination, its required size.
    pub required: Option<u64>,
    /// Available identifier field.
    pub tag: Option<Range<usize>>,
    /// Available length bytes, including a truncated prefix.
    pub length: Option<Range<usize>>,
    /// Known available Value range.
    pub value: Option<Range<usize>>,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for FormatError {}

impl From<FormatError> for Error {
    fn from(error: FormatError) -> Error {
        error.error
    }
}

impl FormatError {
    fn new(code: i32, operation: FormatOperation, raw: &native::tlv_format_error_t) -> Self {
        let range =
            |r: native::tlv_range_t| (r.present != 0).then_some(r.offset..r.offset + r.size);
        FormatError {
            error: Error::from_code(code).unwrap_or(Error::Unknown(code)),
            operation,
            region: Region::from_raw(raw.region),
            offset: (raw.has_offset != 0).then_some(raw.offset),
            required: (raw.has_required != 0).then_some(raw.required),
            tag: range(raw.tag),
            length: range(raw.length),
            value: range(raw.value),
        }
    }
}

fn format_result<T>(
    code: i32,
    operation: FormatOperation,
    error: &native::tlv_format_error_t,
    value: T,
) -> std::result::Result<T, FormatError> {
    if code == native::TLV_OK {
        Ok(value)
    } else {
        Err(FormatError::new(code, operation, error))
    }
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

impl<'a> FormatRef<'a> {
    /// Wraps a descriptor that stays valid and immutable for `'a`.
    ///
    /// # Safety
    ///
    /// `raw` and everything its context borrows must outlive `'a`.
    pub(crate) unsafe fn from_raw(raw: *const native::tlv_format_t) -> Self {
        FormatRef {
            raw,
            lifetime: PhantomData,
        }
    }

    pub(crate) fn raw(self) -> *const native::tlv_format_t {
        self.raw
    }

    /// Whether this Format can decode (`tlv_format_can_read()`).
    pub fn can_read(self) -> bool {
        // SAFETY: the descriptor is live for 'a.
        unsafe { native::tlv_format_can_read(self.raw) != 0 }
    }

    /// Whether this Format can measure and encode (`tlv_format_can_write()`).
    pub fn can_write(self) -> bool {
        // SAFETY: the descriptor is live for 'a.
        unsafe { native::tlv_format_can_write(self.raw) != 0 }
    }

    /// Decodes one complete element at the start of `data` (`tlv_format_decode()`).
    ///
    /// `decoded.encoded().len()` is the consumed byte count; trailing bytes are
    /// not read. Empty input fails with [`Error::End`].
    pub fn decode<'d>(self, data: &'d [u8]) -> std::result::Result<Decoded<'d>, FormatError>
    where
        'a: 'd,
    {
        let mut decoded = MaybeUninit::<native::tlv_decoded_t>::uninit();
        // SAFETY: zero is a valid empty detail; C initializes it on failure.
        let mut error: native::tlv_format_error_t = unsafe { std::mem::zeroed() };
        // SAFETY: descriptor and input are live; outputs are writable.
        let code = unsafe {
            native::tlv_format_decode(
                self.raw,
                data.as_ptr(),
                data.len(),
                decoded.as_mut_ptr(),
                &mut error,
            )
        };
        format_result(code, FormatOperation::Decode, &error, ())?;
        // SAFETY: a successful decode initialized the result, which borrows
        // `data` and, for format-bound Tags, the descriptor's tables for 'd.
        unsafe { Decoded::from_raw(decoded.assume_init()) }.map_err(|error| FormatError {
            error,
            operation: FormatOperation::Decode,
            region: Region::Header,
            offset: None,
            required: None,
            tag: None,
            length: None,
            value: None,
        })
    }

    /// Measures the header, value and trailer of `element` (`tlv_format_measure()`).
    pub fn measure(self, element: &Element<'_>) -> std::result::Result<Encoding, FormatError> {
        let element = raw_element(element);
        let mut encoding = MaybeUninit::<native::tlv_encoding_t>::uninit();
        // SAFETY: zero is a valid empty detail.
        let mut error: native::tlv_format_error_t = unsafe { std::mem::zeroed() };
        // SAFETY: descriptor and element are live; outputs are writable.
        let code = unsafe {
            native::tlv_format_measure(self.raw, &element, encoding.as_mut_ptr(), &mut error)
        };
        format_result(code, FormatOperation::Measure, &error, ())?;
        // SAFETY: successful measurement initialized every field.
        let encoding = unsafe { encoding.assume_init() };
        Ok(Encoding {
            header: encoding.header,
            value: encoding.value,
            trailer: encoding.trailer,
            total: encoding.total,
        })
    }

    /// Encodes `element` into `output` and returns the bytes written
    /// (`tlv_format_encode()`).
    ///
    /// A short `output` fails with [`Error::BufferTooShort`], `required` set to
    /// the encoded size and `output` unchanged.
    pub fn encode_into(
        self,
        element: &Element<'_>,
        output: &mut [u8],
    ) -> std::result::Result<usize, FormatError> {
        let element = raw_element(element);
        let mut written = 0;
        // SAFETY: zero is a valid empty detail.
        let mut error: native::tlv_format_error_t = unsafe { std::mem::zeroed() };
        // SAFETY: output is exclusive and disjoint from the borrowed element.
        let code = unsafe {
            native::tlv_format_encode(
                self.raw,
                &element,
                output.as_mut_ptr(),
                output.len(),
                &mut written,
                &mut error,
            )
        };
        if code == native::TLV_ERR_BUFFER_TOO_SHORT && error.has_required == 0 {
            // C reports the required capacity through `written`.
            error.required = written as u64;
            error.has_required = 1;
        }
        format_result(code, FormatOperation::Encode, &error, written)
    }

    /// Encodes `element` into new storage of its measured size (`tlv_format_measure()`
    /// and `tlv_format_encode()`).
    pub fn encode(self, element: &Element<'_>) -> std::result::Result<Vec<u8>, FormatError> {
        let total = self.measure(element)?.total;
        let size = usize::try_from(total).map_err(|_| FormatError {
            error: Error::NativeSize,
            operation: FormatOperation::Encode,
            region: Region::Header,
            offset: None,
            required: Some(total),
            tag: None,
            length: None,
            value: None,
        })?;
        let mut output = vec![0; size];
        let written = self.encode_into(element, &mut output)?;
        output.truncate(written);
        Ok(output)
    }
}

impl FromStr for Format {
    type Err = Error;

    /// Parses a format name as returned by [`Format::name`], ignoring case.
    ///
    /// Fails with [`Error::InvalidArg`] for an unknown name.
    fn from_str(s: &str) -> Result<Format, Error> {
        Format::ALL
            .into_iter()
            .find(|format| format.name().eq_ignore_ascii_case(s))
            .ok_or(Error::InvalidArg)
    }
}
