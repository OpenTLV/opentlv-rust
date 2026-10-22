// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Error type mapped from OpenTLV result codes.

use std::error;
use std::ffi::CStr;
use std::fmt;

use opentlv_sys as native;

/// An OpenTLV error, mapped from a non-zero `tlv_result_t`.
///
/// Variants mirror the C non-success codes, including `TLV_NEED_MORE_DATA`. A code this crate does not know
/// (for example one added by a newer C library) is preserved as
/// [`Error::Unknown`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// A caller-supplied destination or workspace is too small.
    BufferTooShort,
    /// Final input ends inside an element; supply complete input.
    Truncated,
    /// A length is malformed, out of range, or not representable as `usize`.
    InvalidLength,
    /// A required pointer argument is `NULL`.
    NullArg,
    /// An allocation failed.
    OutOfMemory,
    /// Normal end of iteration, or empty single-read input; not a failure.
    End,
    /// A tag is malformed or invalid for the format or standard.
    InvalidTag,
    /// A visitor callback requested an error stop.
    Visitor,
    /// A configured depth, size, or element-count limit was exceeded.
    Limit,
    /// Input violates a schema rule.
    Schema,
    /// An argument has an invalid value that no more specific code describes.
    InvalidArg,
    /// Tag size violates the range supported by the operation.
    InvalidTagSize,
    /// Text does not match the requested grammar, such as Query syntax.
    Syntax,
    /// An unsigned value cannot fit the requested numeric width.
    Overflow,
    /// Data or its application representation is invalid for the requested interpretation.
    InvalidValue,
    /// A valid requested capability or representation is not implemented.
    Unsupported,
    /// A schema definition is invalid independently of input.
    InvalidSchema,
    /// A logical size exceeds the host address space.
    NativeSize,
    /// Non-final input is exhausted or incomplete; this condition is resumable.
    NeedMoreData,
    /// The operation is forbidden by the current lifecycle state.
    InvalidState,
    /// A provider violated its callback contract.
    Callback,
    /// A result code not known to this crate; carries the raw code.
    Unknown(i32),
}

/// Result type used by all fallible OpenTLV operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Maps a raw `tlv_result_t` to an error, or `None` for success (`TLV_OK`).
    pub fn from_code(code: i32) -> Option<Error> {
        Some(match code {
            native::TLV_OK => return None,
            native::TLV_ERR_BUFFER_TOO_SHORT => Error::BufferTooShort,
            native::TLV_ERR_TRUNCATED => Error::Truncated,
            native::TLV_ERR_INVALID_LENGTH => Error::InvalidLength,
            native::TLV_ERR_NULL_ARG => Error::NullArg,
            native::TLV_ERR_OUT_OF_MEMORY => Error::OutOfMemory,
            native::TLV_END => Error::End,
            native::TLV_ERR_INVALID_TAG => Error::InvalidTag,
            native::TLV_ERR_VISITOR => Error::Visitor,
            native::TLV_ERR_LIMIT => Error::Limit,
            native::TLV_ERR_SCHEMA => Error::Schema,
            native::TLV_ERR_INVALID_ARG => Error::InvalidArg,
            native::TLV_ERR_INVALID_TAG_SIZE => Error::InvalidTagSize,
            native::TLV_ERR_SYNTAX => Error::Syntax,
            native::TLV_ERR_OVERFLOW => Error::Overflow,
            native::TLV_ERR_INVALID_VALUE => Error::InvalidValue,
            native::TLV_ERR_UNSUPPORTED => Error::Unsupported,
            native::TLV_ERR_INVALID_SCHEMA => Error::InvalidSchema,
            native::TLV_ERR_NATIVE_SIZE => Error::NativeSize,
            native::TLV_NEED_MORE_DATA => Error::NeedMoreData,
            native::TLV_ERR_INVALID_STATE => Error::InvalidState,
            native::TLV_ERR_CALLBACK => Error::Callback,
            other => Error::Unknown(other),
        })
    }

    /// Returns the raw `tlv_result_t` code of this error.
    pub fn code(self) -> i32 {
        match self {
            Error::BufferTooShort => native::TLV_ERR_BUFFER_TOO_SHORT,
            Error::Truncated => native::TLV_ERR_TRUNCATED,
            Error::InvalidLength => native::TLV_ERR_INVALID_LENGTH,
            Error::NullArg => native::TLV_ERR_NULL_ARG,
            Error::OutOfMemory => native::TLV_ERR_OUT_OF_MEMORY,
            Error::End => native::TLV_END,
            Error::InvalidTag => native::TLV_ERR_INVALID_TAG,
            Error::Visitor => native::TLV_ERR_VISITOR,
            Error::Limit => native::TLV_ERR_LIMIT,
            Error::Schema => native::TLV_ERR_SCHEMA,
            Error::InvalidArg => native::TLV_ERR_INVALID_ARG,
            Error::InvalidTagSize => native::TLV_ERR_INVALID_TAG_SIZE,
            Error::Syntax => native::TLV_ERR_SYNTAX,
            Error::Overflow => native::TLV_ERR_OVERFLOW,
            Error::InvalidValue => native::TLV_ERR_INVALID_VALUE,
            Error::Unsupported => native::TLV_ERR_UNSUPPORTED,
            Error::InvalidSchema => native::TLV_ERR_INVALID_SCHEMA,
            Error::NativeSize => native::TLV_ERR_NATIVE_SIZE,
            Error::NeedMoreData => native::TLV_NEED_MORE_DATA,
            Error::InvalidState => native::TLV_ERR_INVALID_STATE,
            Error::Callback => native::TLV_ERR_CALLBACK,
            Error::Unknown(code) => code,
        }
    }

    /// Converts a raw result code into `Ok(())` or the matching error.
    pub(crate) fn check(code: native::tlv_result_t) -> Result<()> {
        match Error::from_code(code) {
            None => Ok(()),
            Some(error) => Err(error),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: `tlv_result_string` never returns NULL; it returns a static,
        // NUL-terminated string for every input.
        let message = unsafe { CStr::from_ptr(native::tlv_result_string(self.code())) };
        f.write_str(&message.to_string_lossy())
    }
}

impl error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: [(i32, Error, &str); 21] = [
        (1, Error::End, "end of iteration"),
        (2, Error::NeedMoreData, "need more data"),
        (3, Error::Truncated, "truncated input"),
        (4, Error::InvalidTag, "invalid tag"),
        (5, Error::InvalidTagSize, "invalid tag size"),
        (6, Error::InvalidLength, "invalid length encoding"),
        (
            7,
            Error::InvalidValue,
            "invalid data or application representation",
        ),
        (8, Error::Syntax, "syntax error"),
        (9, Error::Schema, "schema constraint violated"),
        (10, Error::NullArg, "null argument"),
        (11, Error::InvalidArg, "invalid argument"),
        (12, Error::InvalidState, "invalid state"),
        (13, Error::InvalidSchema, "invalid schema definition"),
        (14, Error::BufferTooShort, "buffer too short"),
        (15, Error::Limit, "resource limit exceeded"),
        (16, Error::Overflow, "numeric overflow"),
        (17, Error::NativeSize, "native address space exceeded"),
        (18, Error::OutOfMemory, "out of memory"),
        (19, Error::Unsupported, "unsupported capability"),
        (20, Error::Visitor, "visitor error"),
        (21, Error::Callback, "callback contract violated"),
    ];

    #[test]
    fn success_maps_to_none() {
        assert_eq!(Error::from_code(0), None);
        assert_eq!(Error::check(0), Ok(()));
    }

    #[test]
    fn every_known_code_round_trips() {
        for (code, error, _) in KNOWN {
            assert_eq!(Error::from_code(code), Some(error));
            assert_eq!(error.code(), code);
            assert_eq!(Error::check(code), Err(error));
        }
    }

    #[test]
    fn unknown_code_is_preserved() {
        assert_eq!(Error::from_code(999), Some(Error::Unknown(999)));
        assert_eq!(Error::Unknown(-5).code(), -5);
    }

    #[test]
    fn display_uses_the_c_description() {
        // Exact C texts catch a sys constant that drifted to another result.
        for (_, error, text) in KNOWN {
            assert_eq!(error.to_string(), text, "{error:?}");
        }
        assert_eq!(Error::Unknown(999).to_string(), "unknown error");
    }

    #[test]
    fn implements_std_error() {
        fn assert_error<E: std::error::Error + Send + Sync + 'static>() {}
        assert_error::<Error>();
    }
}
