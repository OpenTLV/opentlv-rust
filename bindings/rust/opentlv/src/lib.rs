// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Safe Rust bindings for OpenTLV.
//!
//! Raw C declarations live in `opentlv-sys`; this crate wraps those calls
//! with safe ownership and lifetime contracts. It provides [`Tag`], [`Element`],
//! [`Error`] and [`Result`], the [`Reader`] that parses TLV buffers, the
//! [`Writer`] that encodes them, and reports the library version. Higher-level
//! layers wrap the C library's implementation instead of reimplementing it:
//! [`LengthSchema`] and [`StructureSchema`] validate data, [`Codec`] converts
//! values to and from typed [`Value`]s, [`Format`] enforces the DER and CER
//! canonical rules, and the [`emv`] module exposes the EMV dictionary. No raw
//! pointers appear in the public API.
//!
//! # Setup
//!
//! Add the crate by path from an OpenTLV checkout; the build script builds and
//! links the C library with CMake (see `docs/guides/rust.md`).
//!
//! ```toml
//! [dependencies]
//! opentlv = { path = "OpenTLV/bindings/rust/opentlv" }
//! ```
//!
//! # Reading and writing
//!
//! ```
//! use opentlv::{Format, Reader, Tag, Writer};
//!
//! # fn main() -> opentlv::Result<()> {
//! let mut buf = [0u8; 16];
//! let mut writer = Writer::with_format(&mut buf, Format::Ber);
//! writer.write(&Tag::from_bytes(&[0x50]), b"VISA")?;
//!
//! for element in Reader::with_format(writer.written(), Format::Ber) {
//!     let element = element?;
//!     assert_eq!(element.tag().as_bytes(), [0x50]);
//!     assert_eq!(element.value(), b"VISA");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Complete programs are in the crate's `examples/` directory
//! (`cargo run --example reader`, `cargo run --example writer`).
//!
//! # Errors
//!
//! Fallible operations return [`Result`], whose error is [`Error`]: one variant
//! per C `TLV_ERR_*` code, [`Display`](std::fmt::Display)ed with the C
//! description. [`SchemaError`] and [`ValidationError`] add the failing offset and
//! [`CodecFailure`] retains shared result codes and delegated codec evidence. Malformed input never
//! panics. A [`Reader`] ends after its first error; a [`Writer`] that reports
//! [`Error::BufferTooShort`] keeps its position.
//!
//! # Ownership and lifetimes
//!
//! [`Tag`] is an owned value of any length. [`Reader<'a>`](Reader) borrows its input
//! and yields [`Element<'a>`](Element) values that are zero-copy slices of it, so
//! elements outlive the reader but not the input. [`Writer<'a>`](Writer)
//! exclusively borrows a caller-owned output buffer and never allocates.
//! Schemas own their C tables and free them on drop.
//!
//! # Relationship to the C API
//!
//! Every operation calls into the OpenTLV C library through `opentlv-sys`, so
//! behavior and error codes match the C API. The safe layer replaces pointer and
//! length pairs with slices and lifetimes. [`TreeReader`], resumable Readers,
//! Visitors and [`QueryMatcher`] delegate to the canonical C engines. Structure
//! codecs and the DOL component are not bound yet.

#![warn(missing_docs)]

mod codec;
mod diagnostic_kind;
pub use diagnostic_kind::*;
mod definition;
pub use definition::{Definition, DefinitionRegistry};
#[cfg(feature = "document")]
mod document;
mod element;
mod error;
mod fixed_format;
mod format;
mod query;
mod reader;
mod reader_diagnostic;
mod schema;
mod source;
mod tag;
mod tree_reader;
mod tree_writer;
#[cfg(feature = "document")]
pub use document::{Document, DocumentBuilder, DocumentError, Node, NodeMut};
mod validation;
mod visitor;
mod writer;

pub mod emv;

pub use codec::{
    AccountType, AflEntry, BiometricType, Codec, CodecDiagnostic, CodecFailure, CodecResult,
    CryptogramInfo, CryptogramType, CvmResult, Date, NumberCodec, NumberEncoding, Time, Track2,
    Value, ValueKind,
};
pub use element::Element;
pub use error::{Error, Result};
pub use fixed_format::{
    ByteOrder, ElementOrder, FixedFormat, FixedFormatConfig, LengthScope, OwnedFixedFormat,
};
pub use format::Format;
pub use query::{Query, QueryError, QueryMatcher};
pub use reader::{read, read_fixed, Reader, ReaderError};
pub use reader_diagnostic::{DiagnosticContext, DiagnosticMetadata, ReaderDiagnostic};
pub use schema::{
    Kind, LengthRule, LengthSchema, SchemaBounds, SchemaDiagnostic, SchemaDiagnosticReport,
    SchemaError, SchemaOrder, StructureGroup, StructureRule, StructureSchema, UnknownPolicy,
    ValidationLimits,
};
pub use source::{decode, decode_fixed, Decoded, Layout};
pub use tag::Tag;
pub use tree_reader::{TreeEvent, TreeItem, TreeReader};
pub use tree_writer::{TreeWriteItem, TreeWriter, WriterDiagnostic};
pub use validation::{Limits, Strictness, ValidationError};
pub use visitor::Visit;
pub use writer::{
    element_encoded_size, element_encoded_size_fixed, encoded_size, encoded_size_fixed,
    measure_element, measure_element_fixed, write_element, write_element_fixed, Writer,
    WriterError,
};

use std::ffi::CStr;
mod program;
pub use program::{
    ProgramError, ProgramOptions, ProgramResult, QueryBinding, QueryConversion, QueryDecoded,
    QueryDefinitionScope, QueryEvent, QueryExecution, QueryLimit, QueryMatch, QueryMetadata,
    QueryProgram, QueryProvider, QueryResolver, QueryRule, QuerySchema, QuerySchemaDetail,
    QuerySchemaError, QuerySchemaLimits, QueryTagAdapter, QueryType, QueryValue,
};
#[cfg(feature = "document")]
pub use program::{QueryEdit, QueryEditError, QueryEditOptions};

/// Returns the version of the linked OpenTLV C library, for example `"0.6.0"`.
pub fn version() -> &'static str {
    // SAFETY: `tlv_version_string` returns a pointer to a static,
    // NUL-terminated ASCII string that is valid for the program's lifetime.
    unsafe { CStr::from_ptr(opentlv_sys::tlv_version_string()) }
        .to_str()
        .expect("OpenTLV version string is ASCII")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!super::version().is_empty());
    }
}

mod location;
pub use location::{Location, LocationDomain, LocationKind};
