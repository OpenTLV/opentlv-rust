// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Generic Value codecs: Core's fundamental integer, byte, IPv4, text and
//! packed digit codecs and its Schema number composition.
//!
//! Every conversion runs in a public C codec through `tlv_codec_decode()` and
//! `tlv_codec_encode()`. Codecs never select themselves by tag; choose one per
//! Value. Configured codecs build the same `tlv_codec_t` descriptor as Core's
//! inline initializers (`tlv_text_codec()`, `tlv_digits_codec()`,
//! `tlv_schema_number_codec()`) for the duration of one call, so C validates
//! their configuration on every use.

use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::net::Ipv4Addr;
use std::ptr;

use opentlv_sys as native;

use crate::codec::{
    codec_buffer, decode_digits, decode_object, encode_with, CodecFailure, CodecResult,
};
use crate::{Error, LengthRule, NumberCodec};

/// Owned bytes of a successful encoding: measured first, then written.
fn encode_owned(
    measure: impl Fn() -> CodecResult<usize>,
    write: impl Fn(&mut [u8]) -> CodecResult<usize>,
) -> CodecResult<Vec<u8>> {
    let mut out = codec_buffer(measure()?)?;
    let written = write(&mut out)?;
    out.truncate(written);
    Ok(out)
}

/// The borrowed bytes of a decoded `tlv_value_t`.
///
/// # Safety
///
/// `value` must have been decoded by a codec that borrows `data`.
unsafe fn borrowed<'a>(data: &'a [u8], value: &native::tlv_value_t) -> CodecResult<&'a [u8]> {
    let size = usize::try_from(value.size).map_err(|_| CodecFailure::from(Error::Callback))?;
    if size == 0 {
        return Ok(&[]);
    }
    let start = (value.data as usize).wrapping_sub(data.as_ptr() as usize);
    // A view outside the input would break the codec's borrowing contract.
    if value.data.is_null() || start > data.len() || size > data.len() - start {
        return Err(CodecFailure::from(Error::Callback));
    }
    Ok(&data[start..start + size])
}

/// A `tlv_value_t` viewing `bytes` for the duration of an encode call.
fn value_view(bytes: &[u8]) -> native::tlv_value_t {
    native::tlv_value_t {
        data: bytes.as_ptr(),
        size: bytes.len() as u64,
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for i64 {}
}

/// An integer type an [`IntegerCodec`] converts: `u8`, `u16`, `u32` or `i64`.
///
/// The trait is sealed; each type is the exact C representation of its codecs.
pub trait CodecInteger: Copy + Default + sealed::Sealed {}
impl CodecInteger for u8 {}
impl CodecInteger for u16 {}
impl CodecInteger for u32 {}
impl CodecInteger for i64 {}

/// One of Core's fundamental integer codecs (`tlv/codec/values.h`).
///
/// `T` is the exact C representation, so a value outside it cannot be
/// passed. The unsigned codecs require their exact wire width; the signed
/// [`IntegerCodec::int64_minimal_be`] codec uses the shortest big-endian
/// two's-complement form and rejects redundant sign octets. Wrong wire
/// lengths fail with [`Error::InvalidValue`].
///
/// ```
/// use opentlv::IntegerCodec;
///
/// assert_eq!(IntegerCodec::uint16_be().decode(&[0x01, 0x02])?, 0x0102);
/// assert_eq!(IntegerCodec::uint32_le().encode(1)?, [1, 0, 0, 0]);
/// assert_eq!(IntegerCodec::int64_minimal_be().encode(-129)?, [0xff, 0x7f]);
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
pub struct IntegerCodec<T> {
    raw: &'static native::tlv_codec_t,
    name: &'static str,
    representation: PhantomData<fn(T) -> T>,
}

impl<T> Clone for IntegerCodec<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for IntegerCodec<T> {}

// SAFETY: the descriptor is immutable static data; its callbacks are pure.
unsafe impl<T> Send for IntegerCodec<T> {}
unsafe impl<T> Sync for IntegerCodec<T> {}

impl<T> fmt::Debug for IntegerCodec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntegerCodec").field(&self.name).finish()
    }
}

impl<T> PartialEq for IntegerCodec<T> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.raw, other.raw)
    }
}
impl<T> Eq for IntegerCodec<T> {}

impl<T> IntegerCodec<T> {
    fn new(raw: *const native::tlv_codec_t, name: &'static str) -> Self {
        IntegerCodec {
            // SAFETY: `raw` is the address of an immutable C static that
            // lives for the whole program.
            raw: unsafe { &*raw },
            name,
            representation: PhantomData,
        }
    }

    /// The C descriptor name, for example `"tlv_codec_uint16_be"`.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl IntegerCodec<u8> {
    /// Exactly one byte (`tlv_codec_uint8`).
    pub fn uint8() -> Self {
        Self::new(ptr::addr_of!(native::tlv_codec_uint8), "tlv_codec_uint8")
    }
}

impl IntegerCodec<u16> {
    /// Exactly two big-endian bytes (`tlv_codec_uint16_be`).
    pub fn uint16_be() -> Self {
        Self::new(
            ptr::addr_of!(native::tlv_codec_uint16_be),
            "tlv_codec_uint16_be",
        )
    }

    /// Exactly two little-endian bytes (`tlv_codec_uint16_le`).
    pub fn uint16_le() -> Self {
        Self::new(
            ptr::addr_of!(native::tlv_codec_uint16_le),
            "tlv_codec_uint16_le",
        )
    }
}

impl IntegerCodec<u32> {
    /// Exactly four big-endian bytes (`tlv_codec_uint32_be`).
    pub fn uint32_be() -> Self {
        Self::new(
            ptr::addr_of!(native::tlv_codec_uint32_be),
            "tlv_codec_uint32_be",
        )
    }

    /// Exactly four little-endian bytes (`tlv_codec_uint32_le`).
    pub fn uint32_le() -> Self {
        Self::new(
            ptr::addr_of!(native::tlv_codec_uint32_le),
            "tlv_codec_uint32_le",
        )
    }
}

impl IntegerCodec<i64> {
    /// One to eight bytes of minimal big-endian two's complement
    /// (`tlv_codec_int64_minimal_be`); zero encodes as one byte. This is
    /// Value encoding only, without an ASN.1 tag or type policy.
    pub fn int64_minimal_be() -> Self {
        Self::new(
            ptr::addr_of!(native::tlv_codec_int64_minimal_be),
            "tlv_codec_int64_minimal_be",
        )
    }
}

impl<T: CodecInteger> IntegerCodec<T> {
    /// Decodes Value bytes.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] for a wrong length or a non-minimal signed form.
    pub fn decode(&self, data: &[u8]) -> CodecResult<T> {
        let mut value = T::default();
        // SAFETY: `T` is this descriptor's C representation; the descriptor is static.
        unsafe { decode_object(self.raw, data, &mut value) }?;
        Ok(value)
    }

    /// Returns the encoded size of `value` without writing.
    pub fn encoded_size(&self, value: T) -> CodecResult<usize> {
        self.encode_raw(value, ptr::null_mut(), 0)
    }

    /// Encodes `value` into `out` and returns the bytes written.
    ///
    /// # Errors
    ///
    /// [`Error::BufferTooShort`] if `out` is too small; C leaves it unchanged.
    pub fn encode_into(&self, value: T, out: &mut [u8]) -> CodecResult<usize> {
        self.encode_raw(value, out.as_mut_ptr(), out.len())
    }

    /// Encodes `value` into a new vector.
    pub fn encode(&self, value: T) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(value),
            |out| self.encode_into(value, out),
        )
    }

    fn encode_raw(&self, value: T, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        // SAFETY: `value` is the descriptor's C representation; `data` is
        // null with zero capacity or a writable slice.
        unsafe {
            encode_with(
                self.raw,
                (&value as *const T).cast::<c_void>(),
                mem::size_of::<T>(),
                data,
                capacity,
            )
        }
    }
}

/// Arbitrary Value bytes (`tlv_codec_bytes`), without text or identifier
/// semantics. Decoding borrows the input; encoding copies it unchanged.
///
/// ```
/// use opentlv::BytesCodec;
///
/// let raw = [0xde, 0xad];
/// assert_eq!(BytesCodec.decode(&raw)?, &raw);
/// assert_eq!(BytesCodec.encode(&raw)?, raw);
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BytesCodec;

impl BytesCodec {
    fn raw() -> *const native::tlv_codec_t {
        ptr::addr_of!(native::tlv_codec_bytes)
    }

    /// Returns the Value bytes, borrowed from `data`.
    pub fn decode<'a>(&self, data: &'a [u8]) -> CodecResult<&'a [u8]> {
        let mut value = value_view(&[]);
        // SAFETY: the bytes codec decodes into a `tlv_value_t` borrowing `data`.
        unsafe {
            decode_object(Self::raw(), data, &mut value)?;
            borrowed(data, &value)
        }
    }

    /// Returns the encoded size of `value` without writing.
    pub fn encoded_size(&self, value: &[u8]) -> CodecResult<usize> {
        Self::encode_raw(value, ptr::null_mut(), 0)
    }

    /// Copies `value` into `out` and returns the bytes written.
    pub fn encode_into(&self, value: &[u8], out: &mut [u8]) -> CodecResult<usize> {
        Self::encode_raw(value, out.as_mut_ptr(), out.len())
    }

    /// Encodes `value` into a new vector.
    pub fn encode(&self, value: &[u8]) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(value),
            |out| self.encode_into(value, out),
        )
    }

    fn encode_raw(value: &[u8], data: *mut u8, capacity: usize) -> CodecResult<usize> {
        let view = value_view(value);
        // SAFETY: the view borrows `value` for the call; `data` is null with
        // zero capacity or a writable slice disjoint from it.
        unsafe { encode_view(Self::raw(), &view, data, capacity) }
    }
}

/// Encodes a `tlv_value_t` with a codec whose representation it is.
///
/// # Safety
///
/// As for [`encode_with`].
unsafe fn encode_view(
    codec: *const native::tlv_codec_t,
    view: &native::tlv_value_t,
    data: *mut u8,
    capacity: usize,
) -> CodecResult<usize> {
    unsafe {
        encode_with(
            codec,
            (view as *const native::tlv_value_t).cast::<c_void>(),
            mem::size_of::<native::tlv_value_t>(),
            data,
            capacity,
        )
    }
}

/// One IPv4 address of exactly four bytes in network order (`tlv_codec_ipv4`),
/// without address policy checks.
///
/// ```
/// use opentlv::Ipv4Codec;
/// use std::net::Ipv4Addr;
///
/// assert_eq!(Ipv4Codec.decode(&[192, 0, 2, 1])?, Ipv4Addr::new(192, 0, 2, 1));
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ipv4Codec;

impl Ipv4Codec {
    fn raw() -> *const native::tlv_codec_t {
        ptr::addr_of!(native::tlv_codec_ipv4)
    }

    /// Decodes four Value bytes.
    pub fn decode(&self, data: &[u8]) -> CodecResult<Ipv4Addr> {
        let mut address = native::tlv_ipv4_t::default();
        // SAFETY: the IPv4 codec decodes into a `tlv_ipv4_t`.
        unsafe { decode_object(Self::raw(), data, &mut address) }?;
        Ok(Ipv4Addr::from(address.bytes))
    }

    /// Returns the encoded size of `address` without writing.
    pub fn encoded_size(&self, address: Ipv4Addr) -> CodecResult<usize> {
        Self::encode_raw(address, ptr::null_mut(), 0)
    }

    /// Encodes `address` into `out` and returns the bytes written.
    pub fn encode_into(&self, address: Ipv4Addr, out: &mut [u8]) -> CodecResult<usize> {
        Self::encode_raw(address, out.as_mut_ptr(), out.len())
    }

    /// Encodes `address` into a new vector.
    pub fn encode(&self, address: Ipv4Addr) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(address),
            |out| self.encode_into(address, out),
        )
    }

    fn encode_raw(address: Ipv4Addr, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        let raw = native::tlv_ipv4_t {
            bytes: address.octets(),
        };
        // SAFETY: `raw` is the codec's C representation.
        unsafe {
            encode_with(
                Self::raw(),
                (&raw as *const native::tlv_ipv4_t).cast::<c_void>(),
                mem::size_of::<native::tlv_ipv4_t>(),
                data,
                capacity,
            )
        }
    }
}

/// A list of IPv4 addresses in network order, a multiple of four bytes
/// (`tlv_codec_ipv4_list`). Order and duplicates are preserved and empty
/// lists are valid.
///
/// ```
/// use opentlv::Ipv4ListCodec;
/// use std::net::Ipv4Addr;
///
/// let raw = [192, 0, 2, 1, 198, 51, 100, 1];
/// let list = Ipv4ListCodec.decode(&raw)?;
/// assert_eq!(list.len(), 2);
/// assert_eq!(list.get(1), Some(Ipv4Addr::new(198, 51, 100, 1)));
/// let addresses: Vec<Ipv4Addr> = list.iter().collect();
/// assert_eq!(Ipv4ListCodec.encode(&addresses)?, raw);
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ipv4ListCodec;

impl Ipv4ListCodec {
    fn raw() -> *const native::tlv_codec_t {
        ptr::addr_of!(native::tlv_codec_ipv4_list)
    }

    /// Validates the Value bytes and returns a list borrowing them.
    pub fn decode<'a>(&self, data: &'a [u8]) -> CodecResult<Ipv4List<'a>> {
        let mut list = native::tlv_ipv4_list_t {
            raw: value_view(&[]),
        };
        // SAFETY: the list codec decodes into a `tlv_ipv4_list_t` borrowing `data`.
        let bytes = unsafe {
            decode_object(Self::raw(), data, &mut list)?;
            borrowed(data, &list.raw)
        }?;
        Ok(Ipv4List { bytes })
    }

    /// Returns the encoded size of `addresses` without writing.
    pub fn encoded_size(&self, addresses: &[Ipv4Addr]) -> CodecResult<usize> {
        Self::encode_raw(addresses, ptr::null_mut(), 0)
    }

    /// Encodes `addresses` into `out` and returns the bytes written.
    pub fn encode_into(&self, addresses: &[Ipv4Addr], out: &mut [u8]) -> CodecResult<usize> {
        Self::encode_raw(addresses, out.as_mut_ptr(), out.len())
    }

    /// Encodes `addresses` into a new vector.
    pub fn encode(&self, addresses: &[Ipv4Addr]) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(addresses),
            |out| self.encode_into(addresses, out),
        )
    }

    fn encode_raw(addresses: &[Ipv4Addr], data: *mut u8, capacity: usize) -> CodecResult<usize> {
        let bytes: Vec<u8> = addresses.iter().flat_map(|a| a.octets()).collect();
        let list = native::tlv_ipv4_list_t {
            raw: value_view(&bytes),
        };
        // SAFETY: the list views `bytes` for the call; it is the codec's C representation.
        unsafe {
            encode_with(
                Self::raw(),
                (&list as *const native::tlv_ipv4_list_t).cast::<c_void>(),
                mem::size_of::<native::tlv_ipv4_list_t>(),
                data,
                capacity,
            )
        }
    }
}

/// A validated IPv4 address list borrowing decoded Value bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ipv4List<'a> {
    bytes: &'a [u8],
}

impl<'a> Ipv4List<'a> {
    /// Number of addresses.
    pub fn len(&self) -> usize {
        self.bytes.len() / 4
    }

    /// Returns `true` if the list has no addresses.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The address at `index`, copied by `tlv_ipv4_list_at()`, or `None`
    /// past the end.
    pub fn get(&self, index: usize) -> Option<Ipv4Addr> {
        let list = native::tlv_ipv4_list_t {
            raw: value_view(self.bytes),
        };
        let mut address = native::tlv_ipv4_t::default();
        // SAFETY: the list views live immutable bytes; `address` is disjoint.
        let code = unsafe { native::tlv_ipv4_list_at(&list, index, &mut address) };
        (code == native::TLV_OK).then(|| Ipv4Addr::from(address.bytes))
    }

    /// The addresses in wire order.
    pub fn iter(&self) -> impl Iterator<Item = Ipv4Addr> + 'a {
        let list = *self;
        (0..list.len()).map_while(move |index| list.get(index))
    }

    /// The borrowed Value bytes.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Characters a [`TextCodec`] accepts (`tlv_text_alphabet_t`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlphabet {
    /// Printable ASCII including space, 0x20..0x7E.
    #[default]
    AsciiPrintable,
    /// ASCII letters and decimal digits only.
    AsciiAlnum,
}

impl TextAlphabet {
    fn raw(self) -> native::tlv_text_alphabet_t {
        match self {
            TextAlphabet::AsciiPrintable => native::TLV_TEXT_ASCII_PRINTABLE,
            TextAlphabet::AsciiAlnum => native::TLV_TEXT_ASCII_ALNUM,
        }
    }
}

/// ASCII text with an explicit alphabet (`tlv_text_decode()`/`tlv_text_encode()`).
///
/// Width zero selects variable-width text. A nonzero width is fixed: with
/// zero padding, decoding strips trailing zero bytes and encoding pads to the
/// width, otherwise the text must fill it. Embedded zeros are rejected, and
/// field-length limits belong to Schema. The default is variable-width
/// printable ASCII without padding.
///
/// ```
/// use opentlv::{TextAlphabet, TextCodec};
///
/// let padded = TextCodec::new(TextAlphabet::AsciiPrintable, 4, true);
/// assert_eq!(padded.encode("ab")?, b"ab\0\0");
/// assert_eq!(padded.decode(b"ab\0\0")?, "ab");
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
///
/// A decoded view cannot outlive its input:
///
/// ```compile_fail
/// use opentlv::TextCodec;
/// let text;
/// {
///     let input = b"OK".to_vec();
///     text = TextCodec::default().decode(&input).unwrap();
/// }
/// println!("{text}");
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextCodec {
    alphabet: TextAlphabet,
    width: usize,
    zero_padding: bool,
}

impl TextCodec {
    /// A codec for `alphabet` with a fixed `width` (zero for variable width)
    /// and optional trailing zero padding.
    pub fn new(alphabet: TextAlphabet, width: usize, zero_padding: bool) -> Self {
        TextCodec {
            alphabet,
            width,
            zero_padding,
        }
    }

    /// The accepted characters.
    pub fn alphabet(&self) -> TextAlphabet {
        self.alphabet
    }

    /// The fixed wire width, or zero for variable-width text.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Whether trailing zero bytes pad a fixed width.
    pub fn zero_padding(&self) -> bool {
        self.zero_padding
    }

    fn config(&self) -> native::tlv_text_codec_config_t {
        native::tlv_text_codec_config_t {
            alphabet: self.alphabet.raw(),
            width: self.width,
            zero_padding: self.zero_padding as i32,
        }
    }

    /// Validates the Value bytes and returns the unpadded text, borrowed from `data`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] for a wrong width or a character outside the alphabet.
    pub fn decode<'a>(&self, data: &'a [u8]) -> CodecResult<&'a str> {
        let config = self.config();
        let codec = text_codec(&config);
        let mut value = value_view(&[]);
        // SAFETY: the configured descriptor decodes into a `tlv_value_t`
        // borrowing `data`; `config` outlives the call.
        let text = unsafe {
            decode_object(&codec, data, &mut value)?;
            borrowed(data, &value)
        }?;
        // C validated an ASCII alphabet.
        std::str::from_utf8(text).map_err(|_| CodecFailure::from(Error::Callback))
    }

    /// Returns the encoded size of `text` without writing.
    pub fn encoded_size(&self, text: &str) -> CodecResult<usize> {
        self.encode_raw(text, ptr::null_mut(), 0)
    }

    /// Encodes `text` into `out` and returns the bytes written.
    pub fn encode_into(&self, text: &str, out: &mut [u8]) -> CodecResult<usize> {
        self.encode_raw(text, out.as_mut_ptr(), out.len())
    }

    /// Encodes `text` into a new vector.
    pub fn encode(&self, text: &str) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(text),
            |out| self.encode_into(text, out),
        )
    }

    fn encode_raw(&self, text: &str, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        let config = self.config();
        let view = value_view(text.as_bytes());
        // SAFETY: `config` and `text` outlive the call.
        unsafe { encode_view(&text_codec(&config), &view, data, capacity) }
    }
}

/// The descriptor `tlv_text_codec()` builds.
fn text_codec(config: &native::tlv_text_codec_config_t) -> native::tlv_codec_t {
    native::tlv_codec_t {
        context: (config as *const native::tlv_text_codec_config_t).cast(),
        decode: Some(native::tlv_text_decode),
        encode: Some(native::tlv_text_encode),
    }
}

/// Decimal digit strings packed high nibble first with trailing `F` padding
/// (`tlv_digits_decode()`/`tlv_digits_encode()`).
///
/// Unlike BCD numbers, leading zero digits are kept. Width zero selects
/// minimal encoding; a nonzero width pads to that many bytes.
///
/// ```
/// use opentlv::DigitsCodec;
///
/// assert_eq!(DigitsCodec::default().decode(&[0x01, 0x23, 0x4f])?, "01234");
/// assert_eq!(DigitsCodec::new(3).encode("0123")?, [0x01, 0x23, 0xff]);
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DigitsCodec {
    width: usize,
}

impl DigitsCodec {
    /// A codec with a fixed byte `width`, or zero for minimal encoding.
    pub fn new(width: usize) -> Self {
        DigitsCodec { width }
    }

    /// The fixed byte width, or zero for minimal encoding.
    pub fn width(&self) -> usize {
        self.width
    }

    fn config(&self) -> native::tlv_digits_codec_config_t {
        native::tlv_digits_codec_config_t { width: self.width }
    }

    /// Decodes the Value bytes into decimal digits.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidValue`] for a wrong width, a non-decimal nibble or
    /// misplaced padding.
    pub fn decode(&self, data: &[u8]) -> CodecResult<String> {
        let config = self.config();
        // SAFETY: the configured descriptor writes a NUL-terminated string;
        // `config` outlives the call.
        unsafe { decode_digits(&digits_codec(&config), data) }
    }

    /// Returns the encoded size of `digits` without writing.
    pub fn encoded_size(&self, digits: &str) -> CodecResult<usize> {
        self.encode_raw(digits, ptr::null_mut(), 0)
    }

    /// Encodes `digits` into `out` and returns the bytes written.
    pub fn encode_into(&self, digits: &str, out: &mut [u8]) -> CodecResult<usize> {
        self.encode_raw(digits, out.as_mut_ptr(), out.len())
    }

    /// Encodes `digits` into a new vector.
    pub fn encode(&self, digits: &str) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(digits),
            |out| self.encode_into(digits, out),
        )
    }

    fn encode_raw(&self, digits: &str, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        let config = self.config();
        // SAFETY: the digits codec reads `digits.len()` characters; `config`
        // and `digits` outlive the call.
        unsafe {
            encode_with(
                &digits_codec(&config),
                digits.as_ptr().cast::<c_void>(),
                digits.len(),
                data,
                capacity,
            )
        }
    }
}

/// The descriptor `tlv_digits_codec()` builds.
fn digits_codec(config: &native::tlv_digits_codec_config_t) -> native::tlv_codec_t {
    native::tlv_codec_t {
        context: (config as *const native::tlv_digits_codec_config_t).cast(),
        decode: Some(native::tlv_digits_decode),
        encode: Some(native::tlv_digits_encode),
    }
}

/// A [`NumberCodec`] whose Value lengths a field [`LengthRule`] limits
/// (`tlv_schema_number_t`).
///
/// The rule owns the permitted lengths and the number codec the
/// representation. Decoding checks the length against the rule first.
/// Width zero encodes in the shortest width the rule permits; a fixed width
/// the rule rejects fails instead of changing width. Length violations fail
/// with [`Error::Schema`] and the Schema cause.
///
/// ```
/// use opentlv::{LengthRule, NumberCodec, NumberEncoding, SchemaNumberCodec, Tag};
///
/// // Two or four bytes only: three would fit 0x123456 but are not permitted.
/// let mut rule = LengthRule::new(Tag::from_bytes(&[0x9f, 0x02]), 2, 4);
/// rule.flags = 1;
/// let codec = SchemaNumberCodec::new(rule, NumberCodec::new(NumberEncoding::BigEndian, 0, 0));
/// assert_eq!(codec.encode(5)?, [0x00, 0x05]);
/// assert_eq!(codec.encode(0x123456)?, [0x00, 0x12, 0x34, 0x56]);
/// # Ok::<(), opentlv::CodecFailure>(())
/// ```
#[derive(Clone, Debug)]
pub struct SchemaNumberCodec {
    rule: LengthRule,
    number: NumberCodec,
}

impl SchemaNumberCodec {
    /// Composes `rule` with the representation of `number`.
    pub fn new(rule: LengthRule, number: NumberCodec) -> Self {
        SchemaNumberCodec { rule, number }
    }

    /// The field rule.
    pub fn rule(&self) -> &LengthRule {
        &self.rule
    }

    /// The numeric representation.
    pub fn number(&self) -> NumberCodec {
        self.number
    }

    /// Runs `call` with the descriptor `tlv_schema_number_codec()` builds,
    /// valid only for the call.
    fn with_codec<R>(&self, call: impl FnOnce(&native::tlv_codec_t) -> R) -> R {
        let entry = self.rule.raw();
        let config = native::tlv_schema_number_t {
            schema: &entry,
            representation: self.number.config(),
        };
        call(&native::tlv_codec_t {
            context: (&config as *const native::tlv_schema_number_t).cast(),
            decode: Some(native::tlv_schema_number_decode),
            encode: Some(native::tlv_schema_number_encode),
        })
    }

    /// Decodes Value bytes whose length the rule permits.
    pub fn decode(&self, data: &[u8]) -> CodecResult<u64> {
        self.with_codec(|codec| {
            let mut value = 0u64;
            // SAFETY: the descriptor decodes into a `u64`; its configuration,
            // rule and tag bytes outlive the call.
            unsafe { decode_object(codec, data, &mut value) }?;
            Ok(value)
        })
    }

    /// Returns the encoded size of `value` without writing.
    pub fn encoded_size(&self, value: u64) -> CodecResult<usize> {
        self.encode_raw(value, ptr::null_mut(), 0)
    }

    /// Encodes `value` into `out` and returns the bytes written.
    pub fn encode_into(&self, value: u64, out: &mut [u8]) -> CodecResult<usize> {
        self.encode_raw(value, out.as_mut_ptr(), out.len())
    }

    /// Encodes `value` into a new vector.
    pub fn encode(&self, value: u64) -> CodecResult<Vec<u8>> {
        encode_owned(
            || self.encoded_size(value),
            |out| self.encode_into(value, out),
        )
    }

    fn encode_raw(&self, value: u64, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        self.with_codec(|codec| {
            // SAFETY: the descriptor encodes a `u64`; its configuration
            // outlives the call; `data` is null with zero capacity or writable.
            unsafe {
                encode_with(
                    codec,
                    (&value as *const u64).cast::<c_void>(),
                    mem::size_of::<u64>(),
                    data,
                    capacity,
                )
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_views_stay_inside_the_input() {
        let data = [1u8, 2, 3, 4];
        let inside = native::tlv_value_t {
            data: data[1..].as_ptr(),
            size: 2,
        };
        // SAFETY: the views are only compared against `data`.
        assert_eq!(unsafe { borrowed(&data, &inside) }.unwrap(), &[2, 3]);
        let outside = native::tlv_value_t {
            data: data[2..].as_ptr(),
            size: 3,
        };
        assert_eq!(
            unsafe { borrowed(&data, &outside) },
            Err(CodecFailure::from(Error::Callback))
        );
        let other = [0u8; 4];
        let foreign = value_view(&other);
        assert!(unsafe { borrowed(&data, &foreign) }.is_err());
        let empty = native::tlv_value_t {
            data: ptr::null(),
            size: 0,
        };
        assert_eq!(unsafe { borrowed(&data, &empty) }.unwrap(), &[] as &[u8]);
    }
}
