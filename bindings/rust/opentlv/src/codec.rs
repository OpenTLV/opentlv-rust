// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Value codecs: conversion between raw TLV values and typed Rust values.
//!
//! The conversions run in the C library's codecs. This module owns the
//! correctly sized and aligned C objects the codecs read and write and maps
//! them to Rust types. Codecs are obtained from the EMV dictionary, see
//! [`Definition::codec`](crate::emv::Definition::codec), or from
//! [`Codec::amount`].

use std::error;
use std::ffi::{c_void, CStr};
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::os::raw::c_char;
use std::ptr;

use opentlv_sys as native;

/// A conversion failure in the shared OpenTLV result domain.
#[derive(Clone, Debug)]
pub struct CodecFailure {
    /// Original common result.
    pub error: crate::Error,
    /// Owned conversion and delegated evidence when supplied by C.
    pub diagnostic: Option<Box<CodecDiagnostic>>,
}
/// Conversion result with optional typed failure evidence.
pub type CodecResult<T> = std::result::Result<T, CodecFailure>;
impl From<crate::Error> for CodecFailure {
    fn from(error: crate::Error) -> Self {
        Self {
            error,
            diagnostic: None,
        }
    }
}
impl CodecFailure {
    /// Original shared result number.
    pub fn code(&self) -> i32 {
        self.error.code()
    }
    fn check(code: i32, diagnostic: &native::tlv_codec_diagnostic_t) -> CodecResult<()> {
        crate::Error::check(code).map_err(|error| Self {
            error,
            diagnostic: Some(Box::new(unsafe { CodecDiagnostic::from_raw(diagnostic) })),
        })
    }
}
impl PartialEq for CodecFailure {
    fn eq(&self, other: &Self) -> bool {
        self.error == other.error
    }
}
impl Eq for CodecFailure {}
impl fmt::Display for CodecFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl error::Error for CodecFailure {}

/// Owned conversion evidence; no native borrowed pointer escapes.
#[derive(Clone, Debug)]
pub struct CodecDiagnostic {
    /// Shared native result.
    pub code: i32,
    /// Native informational, warning or error severity.
    pub severity: crate::Severity,
    /// Expected representation when provided.
    pub expected: Option<String>,
    /// Actual representation when provided.
    pub actual: Option<String>,
    /// Owned enclosing Tags; None means no path was tracked.
    pub path: Option<Vec<Vec<u8>>>,
    /// Number of innermost scopes omitted from the bounded native path.
    pub path_omitted: usize,
    /// Owned (layer, key, value) context entries in native order.
    pub contexts: Vec<(String, String, String)>,
    /// Native decode, encode or measure operation.
    pub operation: crate::CodecOperation,
    /// Delegated evidence category.
    pub cause: crate::CodecCause,
    /// Original provider result, including success before a contract breach.
    pub reported: i32,
    /// Native callback contract violation discriminator.
    pub violation: crate::CodecViolation,
    /// Why the conversion rejected its Value or object.
    pub reason: crate::CodecReason,
    /// Optional provider representation name.
    pub representation: Option<String>,
    /// Shared primary failure location.
    pub location: crate::Location,
    /// Owned delegated Reader cause.
    pub reader: Option<Box<crate::ReaderDiagnostic>>,
    /// Owned delegated Schema cause.
    pub schema: Option<Box<crate::SchemaDiagnostic>>,
}
impl CodecDiagnostic {
    pub(crate) unsafe fn from_raw(raw: &native::tlv_codec_diagnostic_t) -> Self {
        let common =
            unsafe { crate::ReaderDiagnostic::from_parts(&raw.diagnostic, &mem::zeroed()) };
        let mut contexts = Vec::new();
        let mut entry = raw.diagnostic.contexts;
        while !entry.is_null() {
            let value = unsafe { &*entry };
            let text = |p: *const c_char| {
                if p.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
                }
            };
            contexts.push((text(value.layer), text(value.key), text(value.value)));
            entry = value.next;
        }
        let reader = if raw.codec.cause == 1 {
            let cause = native::tlv_reader_diagnostic_t {
                diagnostic: raw.diagnostic,
                detail: unsafe { raw.codec.detail.reader },
            };
            Some(Box::new(unsafe {
                crate::ReaderDiagnostic::from_raw(&cause)
            }))
        } else {
            None
        };
        let schema = if raw.codec.cause == 2 {
            let cause = native::tlv_schema_diagnostic_t {
                diagnostic: raw.diagnostic,
                cause: native::TLV_SCHEMA_CAUSE_SCHEMA,
                detail: native::tlv_schema_cause_detail_t {
                    schema: unsafe { raw.codec.detail.schema },
                },
            };
            unsafe { crate::SchemaDiagnostic::from_raw(&cause) }
                .ok()
                .map(Box::new)
        } else {
            None
        };
        Self {
            code: raw.diagnostic.code,
            severity: crate::Severity::from_raw(raw.diagnostic.severity),
            expected: common.expected,
            actual: common.actual,
            path: common.path,
            path_omitted: common.path_omitted,
            contexts,
            operation: crate::CodecOperation::from_raw(raw.codec.operation),
            cause: crate::CodecCause::from_raw(raw.codec.cause),
            reported: raw.codec.reported,
            violation: crate::CodecViolation::from_raw(raw.codec.violation),
            reason: crate::CodecReason::from_raw(raw.codec.reason),
            representation: if raw.codec.representation.is_null() {
                None
            } else {
                Some(
                    unsafe { CStr::from_ptr(raw.codec.representation) }
                        .to_string_lossy()
                        .into_owned(),
                )
            },
            location: crate::Location::from_raw(raw.diagnostic.location),
            reader,
            schema,
        }
    }
}

/// The representation a semantic codec converts a raw value to and from.
///
/// [`ValueKind::Bytes`], [`ValueKind::Text`] and [`ValueKind::Template`] have
/// no codec: use the borrowed value of the reader directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ValueKind {
    /// Opaque bytes; no codec.
    Bytes,
    /// Text bytes, not necessarily UTF-8; no codec.
    Text,
    /// Constructed template of nested data objects; no codec.
    Template,
    /// [`Value::Number`]: a binary or decimal BCD number.
    Number,
    /// [`Value::Flags`]: bit flags, every wire bit preserved.
    Flags,
    /// [`Value::Digits`]: decimal digits.
    Digits,
    /// [`Value::Date`].
    Date,
    /// [`Value::Time`].
    Time,
    /// [`Value::Account`].
    Account,
    /// [`Value::Cryptogram`].
    Cryptogram,
    /// [`Value::Biometric`].
    Biometric,
    /// [`Value::NumberList`].
    NumberList,
    /// [`Value::Afl`].
    Afl,
    /// [`Value::CvmResult`].
    CvmResult,
    /// [`Value::Track2`].
    Track2,
}

impl ValueKind {
    pub(crate) fn from_raw(raw: native::tlv_emv_value_kind_t) -> Option<ValueKind> {
        Some(match raw {
            native::TLV_EMV_VALUE_BYTES => ValueKind::Bytes,
            native::TLV_EMV_VALUE_TEXT => ValueKind::Text,
            native::TLV_EMV_VALUE_TEMPLATE => ValueKind::Template,
            native::TLV_EMV_VALUE_NUMBER => ValueKind::Number,
            native::TLV_EMV_VALUE_FLAGS => ValueKind::Flags,
            native::TLV_EMV_VALUE_DIGITS => ValueKind::Digits,
            native::TLV_EMV_VALUE_DATE => ValueKind::Date,
            native::TLV_EMV_VALUE_TIME => ValueKind::Time,
            native::TLV_EMV_VALUE_ACCOUNT => ValueKind::Account,
            native::TLV_EMV_VALUE_CRYPTOGRAM => ValueKind::Cryptogram,
            native::TLV_EMV_VALUE_BIOMETRIC => ValueKind::Biometric,
            native::TLV_EMV_VALUE_NUMBER_LIST => ValueKind::NumberList,
            native::TLV_EMV_VALUE_AFL => ValueKind::Afl,
            native::TLV_EMV_VALUE_CVM_RESULT => ValueKind::CvmResult,
            native::TLV_EMV_VALUE_TRACK2 => ValueKind::Track2,
            _ => return None,
        })
    }

    fn raw(self) -> native::tlv_emv_value_kind_t {
        match self {
            ValueKind::Bytes => native::TLV_EMV_VALUE_BYTES,
            ValueKind::Text => native::TLV_EMV_VALUE_TEXT,
            ValueKind::Template => native::TLV_EMV_VALUE_TEMPLATE,
            ValueKind::Number => native::TLV_EMV_VALUE_NUMBER,
            ValueKind::Flags => native::TLV_EMV_VALUE_FLAGS,
            ValueKind::Digits => native::TLV_EMV_VALUE_DIGITS,
            ValueKind::Date => native::TLV_EMV_VALUE_DATE,
            ValueKind::Time => native::TLV_EMV_VALUE_TIME,
            ValueKind::Account => native::TLV_EMV_VALUE_ACCOUNT,
            ValueKind::Cryptogram => native::TLV_EMV_VALUE_CRYPTOGRAM,
            ValueKind::Biometric => native::TLV_EMV_VALUE_BIOMETRIC,
            ValueKind::NumberList => native::TLV_EMV_VALUE_NUMBER_LIST,
            ValueKind::Afl => native::TLV_EMV_VALUE_AFL,
            ValueKind::CvmResult => native::TLV_EMV_VALUE_CVM_RESULT,
            ValueKind::Track2 => native::TLV_EMV_VALUE_TRACK2,
        }
    }

    /// Returns `true` if a codec exists for this kind.
    pub fn has_codec(self) -> bool {
        !matches!(
            self,
            ValueKind::Bytes | ValueKind::Text | ValueKind::Template
        )
    }

    /// Describes the kind's representation and wire meaning, for example
    /// `"Bit flags"` for [`ValueKind::Flags`].
    pub fn description(self) -> &'static str {
        // SAFETY: the function returns a static NUL-terminated ASCII string.
        unsafe { CStr::from_ptr(native::tlv_emv_value_kind_description(self.raw())) }
            .to_str()
            .expect("description is ASCII")
    }
}

/// A decoded EMV date; `year` is YY (0..99) with no century inferred.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Date {
    /// Two-digit year, 0..99.
    pub year: u8,
    /// Month, 1..12.
    pub month: u8,
    /// Day of month.
    pub day: u8,
}

/// A decoded EMV time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Time {
    /// Hour.
    pub hour: u8,
    /// Minute.
    pub minute: u8,
    /// Second.
    pub second: u8,
}

/// EMV account type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccountType {
    /// Default (unspecified) account.
    Default,
    /// Savings account.
    Savings,
    /// Cheque or debit account.
    ChequeDebit,
    /// Credit account.
    Credit,
}

/// Cryptogram type carried in the two most significant bits of the CID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CryptogramType {
    /// Application Authentication Cryptogram (AAC).
    Aac,
    /// Transaction Certificate (TC).
    Tc,
    /// Authorisation Request Cryptogram (ARQC).
    Arqc,
    /// Reserved for future use.
    Rfu,
}

/// A decoded Cryptogram Information Data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CryptogramInfo {
    /// Cryptogram type, from wire bits b8-b7.
    pub kind: CryptogramType,
    /// Remaining six bits, preserved without interpretation.
    pub flags: u8,
}

/// EMV biometric type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BiometricType {
    /// Facial biometric.
    Facial,
    /// Voice biometric.
    Voice,
    /// Fingerprint biometric.
    Finger,
    /// Iris biometric.
    Iris,
    /// Palm biometric.
    Palm,
}

/// One Application File Locator entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AflEntry {
    /// Short file identifier, 1-30.
    pub sfi: u8,
    /// First record of the inclusive range; nonzero.
    pub first_record: u8,
    /// Last record of the inclusive range; not below `first_record`.
    pub last_record: u8,
    /// Leading records of the range used for offline data authentication.
    pub offline_auth_record_count: u8,
}

/// CVM Results, preserved raw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CvmResult {
    /// CVM method code.
    pub method: u8,
    /// CVM condition code.
    pub condition: u8,
    /// CVM outcome: 0 unknown, 1 failed, 2 successful.
    pub result: u8,
}

/// Decoded Track 2 Equivalent Data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Track2 {
    /// Primary account number: 1-19 decimal digits.
    pub pan: String,
    /// Expiration year, YY, 0-99.
    pub expiration_year: u8,
    /// Expiration month, 1-12.
    pub expiration_month: u8,
    /// Three-digit service code, 0-999.
    pub service_code: u16,
    /// Discretionary data: up to 30 decimal digits; may be empty.
    pub discretionary_data: String,
}

/// A typed value a [`Codec`] converts a raw TLV value to and from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Value {
    /// A binary or BCD number.
    Number(u64),
    /// Bit flags, every wire bit preserved.
    Flags(u64),
    /// Decimal digits, for example a PAN without padding.
    Digits(String),
    /// A date.
    Date(Date),
    /// A time.
    Time(Time),
    /// An account type.
    Account(AccountType),
    /// Cryptogram Information Data.
    Cryptogram(CryptogramInfo),
    /// A biometric type.
    Biometric(BiometricType),
    /// A list of up to four numbers.
    NumberList(Vec<u64>),
    /// An Application File Locator of up to 63 entries.
    Afl(Vec<AflEntry>),
    /// CVM Results.
    CvmResult(CvmResult),
    /// Track 2 Equivalent Data.
    Track2(Track2),
}

impl Value {
    /// Returns the [`ValueKind`] this value belongs to.
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Number(_) => ValueKind::Number,
            Value::Flags(_) => ValueKind::Flags,
            Value::Digits(_) => ValueKind::Digits,
            Value::Date(_) => ValueKind::Date,
            Value::Time(_) => ValueKind::Time,
            Value::Account(_) => ValueKind::Account,
            Value::Cryptogram(_) => ValueKind::Cryptogram,
            Value::Biometric(_) => ValueKind::Biometric,
            Value::NumberList(_) => ValueKind::NumberList,
            Value::Afl(_) => ValueKind::Afl,
            Value::CvmResult(_) => ValueKind::CvmResult,
            Value::Track2(_) => ValueKind::Track2,
        }
    }
}

/// A value codec: converts a raw TLV value to a typed [`Value`] and back.
///
/// All validation (lengths, BCD digits, calendar ranges, enum membership) is
/// done by the C library. A codec is a cheap, `Copy` handle to an immutable
/// C descriptor.
///
/// ```
/// use opentlv::{Codec, Value};
///
/// let amount = Codec::amount();
/// let raw = [0x00, 0x00, 0x00, 0x00, 0x12, 0x34];
/// assert_eq!(amount.decode(&raw).unwrap(), Value::Number(1234));
/// assert_eq!(amount.encode(&Value::Number(1234)).unwrap(), raw);
/// ```
#[derive(Clone, Copy)]
pub struct Codec {
    raw: &'static native::tlv_codec_t,
    kind: ValueKind,
}

// SAFETY: the descriptor is immutable static data; its callbacks are pure.
unsafe impl Send for Codec {}
unsafe impl Sync for Codec {}

impl fmt::Debug for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Codec").field("kind", &self.kind).finish()
    }
}

/// Copies the bytes of `text` into a zeroed, NUL-terminated C array, or fails
/// if it does not fit.
fn fill_cstr<const N: usize>(text: &str) -> CodecResult<[c_char; N]> {
    let bytes = text.as_bytes();
    if bytes.len() >= N {
        return Err(CodecFailure::from(crate::Error::InvalidValue));
    }
    let mut out = [0 as c_char; N];
    for (dst, src) in out.iter_mut().zip(bytes) {
        *dst = *src as c_char;
    }
    Ok(out)
}

/// Reads a NUL-terminated C array; an unterminated array is read in full.
fn read_cstr(chars: &[c_char]) -> CodecResult<String> {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8(bytes).map_err(|_| CodecFailure::from(crate::Error::InvalidValue))
}

impl Codec {
    /// # Safety
    ///
    /// `raw` must point to a valid codec descriptor with static lifetime whose
    /// representation is described by `kind`.
    pub(crate) unsafe fn from_raw(
        raw: *const native::tlv_codec_t,
        kind: ValueKind,
    ) -> Option<Codec> {
        Some(Codec {
            raw: raw.as_ref()?,
            kind,
        })
    }

    /// Returns the codec for amounts (format n12): six bytes of BCD to and
    /// from a [`Value::Number`] in unscaled minor units.
    pub fn amount() -> Codec {
        // SAFETY: only the address of an immutable static is taken; it lives
        // for the whole program and its representation is a `u64`.
        unsafe {
            Codec::from_raw(
                ptr::addr_of!(native::tlv_emv_codec_amount),
                ValueKind::Number,
            )
        }
        .expect("the address of a static is not null")
    }

    /// Returns the representation this codec converts to and from.
    pub fn kind(&self) -> ValueKind {
        self.kind
    }

    /// Calls the C decoder into `out`, a zero-initialized `T`.
    fn decode_into<T>(&self, data: &[u8], out: &mut T) -> CodecResult<()> {
        // SAFETY: `data` is a valid slice; `out` is a valid, writable object of
        // the size passed as capacity, and it has the C type this codec kind
        // decodes into. The codec descriptor is immutable and static.
        let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
        CodecFailure::check(
            unsafe {
                native::tlv_codec_decode(
                    self.raw,
                    data.as_ptr(),
                    data.len(),
                    (out as *mut T).cast::<c_void>(),
                    mem::size_of::<T>(),
                    &mut diagnostic,
                )
            },
            &diagnostic,
        )
    }

    /// Decodes a raw value, for example the value of an [`Element`](crate::Element).
    ///
    /// # Errors
    ///
    /// [`CodecFailure::from(crate::Error::InvalidValue)`](crate::Error::InvalidValue) if the bytes are not a valid value of this
    /// codec's kind (wrong length, bad BCD digit, out-of-range field, …).
    pub fn decode(&self, data: &[u8]) -> CodecResult<Value> {
        // SAFETY (all `zeroed` uses below): the C types are plain integers,
        // arrays and structs of them, for which all-zero bytes are valid.
        Ok(match self.kind {
            ValueKind::Number | ValueKind::Flags => {
                let mut number = 0u64;
                self.decode_into(data, &mut number)?;
                if self.kind == ValueKind::Number {
                    Value::Number(number)
                } else {
                    Value::Flags(number)
                }
            }
            ValueKind::Digits => {
                // At most two digits per byte, plus the terminating NUL.
                let mut digits = vec![0u8; data.len() * 2 + 1];
                // SAFETY: `digits` is a valid writable buffer of its length;
                // the digits codec writes a NUL-terminated string into it.
                let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
                CodecFailure::check(
                    unsafe {
                        native::tlv_codec_decode(
                            self.raw,
                            data.as_ptr(),
                            data.len(),
                            digits.as_mut_ptr().cast::<c_void>(),
                            digits.len(),
                            &mut diagnostic,
                        )
                    },
                    &diagnostic,
                )?;
                let end = digits.iter().position(|b| *b == 0).unwrap_or(digits.len());
                digits.truncate(end);
                Value::Digits(
                    String::from_utf8(digits)
                        .map_err(|_| CodecFailure::from(crate::Error::InvalidValue))?,
                )
            }
            ValueKind::Date => {
                let mut date = native::tlv_emv_date_t::default();
                self.decode_into(data, &mut date)?;
                Value::Date(Date {
                    year: date.year,
                    month: date.month,
                    day: date.day,
                })
            }
            ValueKind::Time => {
                let mut time = native::tlv_emv_time_t::default();
                self.decode_into(data, &mut time)?;
                Value::Time(Time {
                    hour: time.hour,
                    minute: time.minute,
                    second: time.second,
                })
            }
            ValueKind::Account => {
                let mut account: native::tlv_emv_account_type_t = 0;
                self.decode_into(data, &mut account)?;
                Value::Account(match account {
                    native::TLV_EMV_ACCOUNT_DEFAULT => AccountType::Default,
                    native::TLV_EMV_ACCOUNT_SAVINGS => AccountType::Savings,
                    native::TLV_EMV_ACCOUNT_CHEQUE_DEBIT => AccountType::ChequeDebit,
                    native::TLV_EMV_ACCOUNT_CREDIT => AccountType::Credit,
                    _ => return Err(CodecFailure::from(crate::Error::InvalidValue)),
                })
            }
            ValueKind::Cryptogram => {
                let mut info = native::tlv_emv_cryptogram_info_t::default();
                self.decode_into(data, &mut info)?;
                Value::Cryptogram(CryptogramInfo {
                    kind: match info.type_ {
                        0 => CryptogramType::Aac,
                        1 => CryptogramType::Tc,
                        2 => CryptogramType::Arqc,
                        3 => CryptogramType::Rfu,
                        _ => return Err(CodecFailure::from(crate::Error::InvalidValue)),
                    },
                    flags: info.flags,
                })
            }
            ValueKind::Biometric => {
                let mut biometric: native::tlv_emv_biometric_type_t = 0;
                self.decode_into(data, &mut biometric)?;
                Value::Biometric(match biometric {
                    native::TLV_EMV_BIOMETRIC_FACIAL => BiometricType::Facial,
                    native::TLV_EMV_BIOMETRIC_VOICE => BiometricType::Voice,
                    native::TLV_EMV_BIOMETRIC_FINGER => BiometricType::Finger,
                    native::TLV_EMV_BIOMETRIC_IRIS => BiometricType::Iris,
                    native::TLV_EMV_BIOMETRIC_PALM => BiometricType::Palm,
                    _ => return Err(CodecFailure::from(crate::Error::InvalidValue)),
                })
            }
            ValueKind::NumberList => {
                let mut list = native::tlv_emv_number_list_t::default();
                self.decode_into(data, &mut list)?;
                let count = list.count.min(list.values.len());
                Value::NumberList(list.values[..count].to_vec())
            }
            ValueKind::Afl => {
                let mut afl = MaybeUninit::<native::tlv_emv_afl_t>::zeroed();
                // SAFETY: zeroed is a valid `tlv_emv_afl_t`.
                let afl = unsafe { &mut *afl.as_mut_ptr() };
                self.decode_into(data, afl)?;
                let count = afl.count.min(afl.entries.len());
                Value::Afl(
                    afl.entries[..count]
                        .iter()
                        .map(|e| AflEntry {
                            sfi: e.sfi,
                            first_record: e.first_record,
                            last_record: e.last_record,
                            offline_auth_record_count: e.offline_auth_record_count,
                        })
                        .collect(),
                )
            }
            ValueKind::CvmResult => {
                let mut result = native::tlv_emv_cvm_result_t::default();
                self.decode_into(data, &mut result)?;
                Value::CvmResult(CvmResult {
                    method: result.method,
                    condition: result.condition,
                    result: result.result,
                })
            }
            ValueKind::Track2 => {
                let mut track2 = MaybeUninit::<native::tlv_emv_track2_t>::zeroed();
                // SAFETY: zeroed is a valid `tlv_emv_track2_t`.
                let track2 = unsafe { &mut *track2.as_mut_ptr() };
                self.decode_into(data, track2)?;
                Value::Track2(Track2 {
                    pan: read_cstr(&track2.pan)?,
                    expiration_year: track2.expiration_year,
                    expiration_month: track2.expiration_month,
                    service_code: track2.service_code,
                    discretionary_data: read_cstr(&track2.discretionary_data)?,
                })
            }
            ValueKind::Bytes | ValueKind::Text | ValueKind::Template => {
                return Err(CodecFailure::from(crate::Error::Unsupported))
            }
        })
    }

    /// Calls the C encoder for the object `value`. With a null `data` and
    /// zero `capacity` this is a size query.
    fn encode_object<T>(&self, value: &T, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        self.encode_raw(
            (value as *const T).cast::<c_void>(),
            mem::size_of::<T>(),
            data,
            capacity,
        )
    }

    fn encode_raw(
        &self,
        value: *const c_void,
        size: usize,
        data: *mut u8,
        capacity: usize,
    ) -> CodecResult<usize> {
        let mut written = 0usize;
        // SAFETY: `value` points to `size` readable bytes of the C type of this
        // codec kind; `data` is either null with zero capacity or a writable
        // buffer of `capacity` bytes; `written` is a writable `usize` that
        // aliases neither.
        let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
        CodecFailure::check(
            unsafe {
                native::tlv_codec_encode(
                    self.raw,
                    value,
                    size,
                    data,
                    capacity,
                    &mut written,
                    &mut diagnostic,
                )
            },
            &diagnostic,
        )?;
        Ok(written)
    }

    fn encode_value(&self, value: &Value, data: *mut u8, capacity: usize) -> CodecResult<usize> {
        if value.kind() != self.kind {
            return Err(CodecFailure::from(crate::Error::InvalidValue));
        }
        match value {
            Value::Number(n) | Value::Flags(n) => self.encode_object(n, data, capacity),
            Value::Digits(digits) => {
                self.encode_raw(digits.as_ptr().cast(), digits.len(), data, capacity)
            }
            Value::Date(d) => self.encode_object(
                &native::tlv_emv_date_t {
                    year: d.year,
                    month: d.month,
                    day: d.day,
                },
                data,
                capacity,
            ),
            Value::Time(t) => self.encode_object(
                &native::tlv_emv_time_t {
                    hour: t.hour,
                    minute: t.minute,
                    second: t.second,
                },
                data,
                capacity,
            ),
            Value::Account(account) => {
                let raw: native::tlv_emv_account_type_t = match account {
                    AccountType::Default => native::TLV_EMV_ACCOUNT_DEFAULT,
                    AccountType::Savings => native::TLV_EMV_ACCOUNT_SAVINGS,
                    AccountType::ChequeDebit => native::TLV_EMV_ACCOUNT_CHEQUE_DEBIT,
                    AccountType::Credit => native::TLV_EMV_ACCOUNT_CREDIT,
                };
                self.encode_object(&raw, data, capacity)
            }
            Value::Cryptogram(info) => self.encode_object(
                &native::tlv_emv_cryptogram_info_t {
                    type_: match info.kind {
                        CryptogramType::Aac => 0,
                        CryptogramType::Tc => 1,
                        CryptogramType::Arqc => 2,
                        CryptogramType::Rfu => 3,
                    },
                    flags: info.flags,
                },
                data,
                capacity,
            ),
            Value::Biometric(biometric) => {
                let raw: native::tlv_emv_biometric_type_t = match biometric {
                    BiometricType::Facial => native::TLV_EMV_BIOMETRIC_FACIAL,
                    BiometricType::Voice => native::TLV_EMV_BIOMETRIC_VOICE,
                    BiometricType::Finger => native::TLV_EMV_BIOMETRIC_FINGER,
                    BiometricType::Iris => native::TLV_EMV_BIOMETRIC_IRIS,
                    BiometricType::Palm => native::TLV_EMV_BIOMETRIC_PALM,
                };
                self.encode_object(&raw, data, capacity)
            }
            Value::NumberList(numbers) => {
                let mut list = native::tlv_emv_number_list_t::default();
                if numbers.len() > list.values.len() {
                    return Err(CodecFailure::from(crate::Error::InvalidValue));
                }
                list.values[..numbers.len()].copy_from_slice(numbers);
                list.count = numbers.len();
                self.encode_object(&list, data, capacity)
            }
            Value::Afl(entries) => {
                if entries.len() > native::TLV_EMV_AFL_MAX_ENTRIES {
                    return Err(CodecFailure::from(crate::Error::InvalidValue));
                }
                let mut afl = native::tlv_emv_afl_t {
                    entries: [native::tlv_emv_afl_entry_t::default();
                        native::TLV_EMV_AFL_MAX_ENTRIES],
                    count: entries.len(),
                };
                for (dst, src) in afl.entries.iter_mut().zip(entries) {
                    *dst = native::tlv_emv_afl_entry_t {
                        sfi: src.sfi,
                        first_record: src.first_record,
                        last_record: src.last_record,
                        offline_auth_record_count: src.offline_auth_record_count,
                    };
                }
                self.encode_object(&afl, data, capacity)
            }
            Value::CvmResult(result) => self.encode_object(
                &native::tlv_emv_cvm_result_t {
                    method: result.method,
                    condition: result.condition,
                    result: result.result,
                },
                data,
                capacity,
            ),
            Value::Track2(track2) => self.encode_object(
                &native::tlv_emv_track2_t {
                    pan: fill_cstr(&track2.pan)?,
                    expiration_year: track2.expiration_year,
                    expiration_month: track2.expiration_month,
                    service_code: track2.service_code,
                    discretionary_data: fill_cstr(&track2.discretionary_data)?,
                },
                data,
                capacity,
            ),
        }
    }

    /// Validates `value` and returns the size of its encoding without writing.
    ///
    /// # Errors
    ///
    /// [`CodecFailure::from(crate::Error::InvalidValue)`](crate::Error::InvalidValue) if `value` is of another kind than the
    /// codec's or invalid for it.
    pub fn encoded_size(&self, value: &Value) -> CodecResult<usize> {
        self.encode_value(value, ptr::null_mut(), 0)
    }

    /// Encodes `value` into `out` and returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// [`CodecFailure::from(crate::Error::BufferTooShort)`](crate::Error::BufferTooShort) if `out` is too small, or the errors of
    /// [`Codec::encoded_size`]. On error `out` may have been modified.
    pub fn encode_into(&self, value: &Value, out: &mut [u8]) -> CodecResult<usize> {
        self.encode_value(value, out.as_mut_ptr(), out.len())
    }

    /// Encodes `value` into a new vector.
    ///
    /// # Errors
    ///
    /// Same as [`Codec::encoded_size`].
    pub fn encode(&self, value: &Value) -> CodecResult<Vec<u8>> {
        let mut out = vec![0u8; self.encoded_size(value)?];
        let written = self.encode_into(value, &mut out)?;
        out.truncate(written);
        Ok(out)
    }
}

/// Numeric Value representation selected explicitly, independent of a protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum NumberEncoding {
    /// Most-significant byte first.
    BigEndian = 0,
    /// Least-significant byte first.
    LittleEndian = 1,
    /// Packed decimal, most-significant digit first.
    Bcd = 2,
}

/// Configurable uint64 Value codec delegated to C.
/// Width zero selects minimal encoding. C validates configuration on use;
/// Schema remains responsible for contextual field-length constraints.
#[derive(Clone, Copy, Debug)]
pub struct NumberCodec {
    config: native::tlv_number_codec_config_t,
}
impl NumberCodec {
    /// Set encoding, fixed byte width (or zero), and BCD precision (zero for binary).
    pub fn new(encoding: NumberEncoding, width: usize, digits: u32) -> Self {
        Self {
            config: native::tlv_number_codec_config_t {
                encoding: encoding as i32,
                width,
                digits,
            },
        }
    }
    /// Decode Value bytes through the canonical C numeric codec.
    pub fn decode(&self, data: &[u8]) -> CodecResult<u64> {
        let mut value = 0u64;
        // SAFETY: correctly aligned configuration and u64 destination; disjoint live input.
        let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
        CodecFailure::check(
            unsafe {
                native::tlv_number_decode(
                    (&self.config as *const native::tlv_number_codec_config_t).cast(),
                    data.as_ptr(),
                    data.len(),
                    (&mut value as *mut u64).cast(),
                    mem::size_of::<u64>(),
                    &mut diagnostic,
                )
            },
            &diagnostic,
        )?;
        Ok(value)
    }
    /// Measure a validated encoding through the C size-query operation.
    pub fn encoded_size(&self, value: u64) -> CodecResult<usize> {
        self.encode_raw(value, ptr::null_mut(), 0)
    }
    /// Encode into caller storage; C leaves output unchanged on failure.
    pub fn encode_into(&self, value: u64, output: &mut [u8]) -> CodecResult<usize> {
        self.encode_raw(value, output.as_mut_ptr(), output.len())
    }
    /// Return owned encoded Value bytes.
    pub fn encode(&self, value: u64) -> CodecResult<Vec<u8>> {
        let mut bytes = vec![0; self.encoded_size(value)?];
        let written = self.encode_into(value, &mut bytes)?;
        bytes.truncate(written);
        Ok(bytes)
    }
    fn encode_raw(&self, value: u64, output: *mut u8, capacity: usize) -> CodecResult<usize> {
        let mut written = 0;
        // SAFETY: private callers supply either NULL/zero or a writable exclusive slice.
        // Configuration, scalar input and count are aligned live independent objects.
        let mut diagnostic: native::tlv_codec_diagnostic_t = unsafe { mem::zeroed() };
        CodecFailure::check(
            unsafe {
                native::tlv_number_encode(
                    (&self.config as *const native::tlv_number_codec_config_t).cast(),
                    (&value as *const u64).cast(),
                    mem::size_of::<u64>(),
                    output,
                    capacity,
                    &mut written,
                    &mut diagnostic,
                )
            },
            &diagnostic,
        )?;
        Ok(written)
    }
}
