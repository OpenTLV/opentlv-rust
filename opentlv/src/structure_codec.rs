// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Structure codecs: one complete TLV sequence to and from one application
//! object, validated by Core's `tlv_structure_decode()` and
//! `tlv_structure_encode()`.

use std::ffi::c_void;
use std::fmt;
use std::mem;
use std::ptr;

use opentlv_sys as native;

use crate::codec::{codec_buffer, CodecFailure, CodecResult};
use crate::{FormatRef, StructureSchema, ValidationLimits};

/// Maps one complete TLV sequence to one application object
/// (`tlv_structure_codec_t`).
///
/// Core validates framing, nesting, the limits and, when one is set, the
/// [`StructureSchema`]; the application conversion is a Rust closure passed
/// to each call. [`StructureCodec::decode`] validates the whole sequence
/// before its closure sees it. [`StructureCodec::encode`] runs its closure
/// once and has Core validate the produced bytes before returning them, with
/// failure locations in output coordinates. No tag is prescribed for the
/// complete object, and the codec never selects itself by tag.
///
/// Failures are [`CodecFailure`]s with the Reader or Schema cause; a
/// closure's own failure is returned unchanged. Encoding requires a Format
/// that can write.
///
/// ```
/// use opentlv::{Format, StructureCodec, StructureRule, StructureSchema, Tag};
///
/// let name = Tag::from_bytes(&[0x50]);
/// let schema = StructureSchema::new([StructureRule::new(name.clone()).required_once()], false);
/// let codec = StructureCodec::new(Format::Ber).with_schema(&schema);
///
/// let label = codec.decode(&[0x50, 0x02, b'O', b'K'], |data| Ok(data[2..].to_vec()))?;
/// assert_eq!(label, b"OK");
/// assert_eq!(codec.encode(|| Ok([0x50, 0x01, b'A']))?, [0x50, 0x01, b'A']);
/// // The produced bytes are validated: the required 50 is missing.
/// assert!(codec.encode(|| Ok([0x51, 0x00])).is_err());
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
///
/// An application object borrowing the sequence cannot outlive it:
///
/// ```compile_fail
/// use opentlv::{Format, StructureCodec};
/// let object;
/// {
///     let input = vec![0x50, 0x01, b'A'];
///     object = StructureCodec::new(Format::Ber)
///         .decode(&input, |bytes| Ok(&bytes[2..])).unwrap();
/// }
/// println!("{object:?}");
/// ```
#[derive(Clone, Copy)]
pub struct StructureCodec<'a> {
    format: FormatRef<'a>,
    schema: Option<&'a StructureSchema>,
    limits: ValidationLimits,
}

impl fmt::Debug for StructureCodec<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StructureCodec")
            .field("format", &self.format)
            .field("schema", &self.schema)
            .field("limits", &self.limits)
            .finish()
    }
}

/// The produced bytes the encode callback copies.
struct Produced<'b> {
    bytes: &'b [u8],
}

/// Accepts the sequence Core validated; the closure runs after Core returns.
unsafe extern "C" fn accept(
    _context: *const c_void,
    _format: *const native::tlv_format_t,
    _data: *const u8,
    _size: usize,
    _value: *mut c_void,
    _capacity: usize,
    _diagnostic: *mut native::tlv_codec_diagnostic_t,
) -> native::tlv_result_t {
    native::TLV_OK
}

/// Copies the produced bytes, or reports their size for a size query.
unsafe extern "C" fn copy_produced(
    _context: *const c_void,
    _format: *const native::tlv_format_t,
    value: *const c_void,
    _size: usize,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
    _diagnostic: *mut native::tlv_codec_diagnostic_t,
) -> native::tlv_result_t {
    // SAFETY: `encode_raw` passes a live `Produced` as the value object and a
    // writable `written`; `data` is null or `capacity` writable bytes
    // disjoint from the produced bytes.
    unsafe {
        let bytes = (*value.cast::<Produced<'_>>()).bytes;
        if !data.is_null() {
            if bytes.len() > capacity {
                return native::TLV_ERR_BUFFER_TOO_SHORT;
            }
            ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        }
        *written = bytes.len();
    }
    native::TLV_OK
}

impl<'a> StructureCodec<'a> {
    /// A codec over `format` that validates framing and nesting only, with
    /// the default [`ValidationLimits`].
    pub fn new(format: impl Into<FormatRef<'a>>) -> Self {
        StructureCodec {
            format: format.into(),
            schema: None,
            limits: ValidationLimits::default(),
        }
    }

    /// Also validates against `schema`; a checked schema's
    /// [`schema()`](crate::CheckedStructureSchema::schema) works too.
    pub fn with_schema(mut self, schema: &'a StructureSchema) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Sets the nesting depth and element limits; zero is a real limit.
    /// Without a Schema, traversal is bounded by 64 frames
    /// (`TLV_STRUCTURE_MAX_DEPTH`).
    pub fn with_limits(mut self, limits: ValidationLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The Format.
    pub fn format(&self) -> FormatRef<'a> {
        self.format
    }

    /// The Schema, if any.
    pub fn schema(&self) -> Option<&'a StructureSchema> {
        self.schema
    }

    /// The validation limits.
    pub fn limits(&self) -> ValidationLimits {
        self.limits
    }

    fn raw(&self) -> native::tlv_structure_codec_t {
        native::tlv_structure_codec_t {
            context: ptr::null(),
            format: self.format.raw(),
            schema: self
                .schema
                .map_or(ptr::null(), |schema| schema.as_raw() as *const _),
            max_depth: self.limits.max_depth,
            max_elements: self.limits.max_elements,
            decode: Some(accept),
            encode: Some(copy_produced),
        }
    }

    /// Validates the complete sequence `data`, then returns what `decode`
    /// makes of it. The object may borrow `data`.
    ///
    /// # Errors
    ///
    /// The Reader or Schema failure, with its cause, before `decode` runs;
    /// [`Error::Unsupported`](crate::Error::Unsupported) for a Format that
    /// cannot read; otherwise `decode`'s own failure.
    pub fn decode<'d, T>(
        &self,
        data: &'d [u8],
        decode: impl FnOnce(&'d [u8]) -> CodecResult<T>,
    ) -> CodecResult<T> {
        let codec = self.raw();
        let mut slot = 0u8;
        let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
        // SAFETY: the descriptor, its Format and Schema and `data` are live
        // for the call; `slot` is a writable object of the size passed.
        let code = unsafe {
            native::tlv_structure_decode(
                &codec,
                data.as_ptr(),
                data.len(),
                (&mut slot as *mut u8).cast(),
                mem::size_of::<u8>(),
                &mut diagnostic,
            )
        };
        CodecFailure::check(code, &diagnostic)?;
        decode(data)
    }

    /// Returns the size of the sequence `encode` produces. Like Core's size
    /// query, the bytes are not validated; [`StructureCodec::encode`] does.
    pub fn encoded_size<B: AsRef<[u8]>>(
        &self,
        encode: impl FnOnce() -> CodecResult<B>,
    ) -> CodecResult<usize> {
        self.encode_raw(encode()?.as_ref(), ptr::null_mut(), 0)
    }

    /// Validates the sequence `encode` produces and copies it into `out`;
    /// returns the bytes written.
    ///
    /// # Errors
    ///
    /// `encode`'s own failure, [`Error::BufferTooShort`](crate::Error::BufferTooShort)
    /// if `out` is too small, or the Reader or Schema failure of the produced
    /// bytes. On a validation failure `out` holds the rejected bytes.
    pub fn encode_into<B: AsRef<[u8]>>(
        &self,
        out: &mut [u8],
        encode: impl FnOnce() -> CodecResult<B>,
    ) -> CodecResult<usize> {
        self.encode_raw(encode()?.as_ref(), out.as_mut_ptr(), out.len())
    }

    /// Returns the validated sequence `encode` produces.
    ///
    /// # Errors
    ///
    /// As for [`StructureCodec::encode_into`], without a capacity failure.
    pub fn encode<B: AsRef<[u8]>>(
        &self,
        encode: impl FnOnce() -> CodecResult<B>,
    ) -> CodecResult<Vec<u8>> {
        let produced = encode()?;
        let bytes = produced.as_ref();
        let mut out = codec_buffer(bytes.len())?;
        let written = self.encode_raw(bytes, out.as_mut_ptr(), out.len())?;
        out.truncate(written);
        Ok(out)
    }

    fn encode_raw(&self, bytes: &[u8], data: *mut u8, capacity: usize) -> CodecResult<usize> {
        let codec = self.raw();
        let produced = Produced { bytes };
        let mut written = 0usize;
        let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
        // SAFETY: the descriptor, its Format and Schema and the produced
        // bytes are live for the call; `data` is null with zero capacity or a
        // writable slice disjoint from them.
        let code = unsafe {
            native::tlv_structure_encode(
                &codec,
                (&produced as *const Produced<'_>).cast(),
                mem::size_of::<Produced<'_>>(),
                data,
                capacity,
                &mut written,
                &mut diagnostic,
            )
        };
        CodecFailure::check(code, &diagnostic)?;
        Ok(written)
    }
}
