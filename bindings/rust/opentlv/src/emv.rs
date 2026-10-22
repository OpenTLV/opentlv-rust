// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! The EMV module: data dictionary, contexts and per-tag codecs.
//!
//! Wraps the C library's EMV Contact Book 3 dictionary. Look a tag up with
//! [`find`] to get its [`Definition`]: name, expected [`ValueKind`], length
//! rules and, where available, a [`Codec`].

use std::ffi::CStr;
use std::fmt;

use opentlv_sys as native;

use crate::codec::{Codec, ValueKind};
use crate::error::{Error, Result};
use crate::tag::Tag;

/// The dictionary context a tag is interpreted under.
///
/// Contexts are explicit and never fall back to the base dictionary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Context {
    /// Ordinary application data.
    #[default]
    Base,
    /// Inside 7F60.
    Bit,
    /// Inside A1 within 7F60.
    Bht,
    /// Inside level-2 A1/A2 within BHT.
    BhtFormat,
    /// Inside BF4A/BF4B, or a terminal group.
    BitGroup,
    /// Inside BF4C.
    BiometricCounters,
    /// Inside BF4D.
    BiometricAttempts,
    /// Inside BF4E.
    BiometricVerification,
}

impl Context {
    pub(crate) fn raw(self) -> native::tlv_emv_context_t {
        match self {
            Context::Base => native::TLV_EMV_CONTEXT_BASE,
            Context::Bit => native::TLV_EMV_CONTEXT_BIT,
            Context::Bht => native::TLV_EMV_CONTEXT_BHT,
            Context::BhtFormat => native::TLV_EMV_CONTEXT_BHT_FORMAT,
            Context::BitGroup => native::TLV_EMV_CONTEXT_BIT_GROUP,
            Context::BiometricCounters => native::TLV_EMV_CONTEXT_BIOMETRIC_COUNTERS,
            Context::BiometricAttempts => native::TLV_EMV_CONTEXT_BIOMETRIC_ATTEMPTS,
            Context::BiometricVerification => native::TLV_EMV_CONTEXT_BIOMETRIC_VERIFICATION,
        }
    }

    fn from_raw(raw: native::tlv_emv_context_t) -> Option<Context> {
        Some(match raw {
            native::TLV_EMV_CONTEXT_BASE => Context::Base,
            native::TLV_EMV_CONTEXT_BIT => Context::Bit,
            native::TLV_EMV_CONTEXT_BHT => Context::Bht,
            native::TLV_EMV_CONTEXT_BHT_FORMAT => Context::BhtFormat,
            native::TLV_EMV_CONTEXT_BIT_GROUP => Context::BitGroup,
            native::TLV_EMV_CONTEXT_BIOMETRIC_COUNTERS => Context::BiometricCounters,
            native::TLV_EMV_CONTEXT_BIOMETRIC_ATTEMPTS => Context::BiometricAttempts,
            native::TLV_EMV_CONTEXT_BIOMETRIC_VERIFICATION => Context::BiometricVerification,
            _ => return None,
        })
    }

    /// Returns the context the children of `tag` are interpreted under, or
    /// `None` if `tag` is not a known template in `self`, in which case the
    /// library does not guess a context for its children.
    pub fn child(self, tag: &Tag) -> Option<Context> {
        let tag = tag.raw();
        // SAFETY: `tag` is a valid tag for the call.
        Context::from_raw(unsafe { native::tlv_emv_child_context(self.raw(), &tag) })
    }
}

/// One entry of the EMV data dictionary.
///
/// A cheap, `Copy` handle to an immutable static table entry.
#[derive(Clone, Copy)]
pub struct Definition {
    raw: &'static native::tlv_emv_definition_t,
}

// SAFETY: the definition points into immutable static tables of the C library.
unsafe impl Send for Definition {}
unsafe impl Sync for Definition {}

impl fmt::Debug for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Definition")
            .field("name", &self.name())
            .field("tag", &self.tag())
            .field("kind", &self.kind())
            .finish()
    }
}

impl Definition {
    fn entry(&self) -> &'static native::tlv_schema_entry_t {
        // SAFETY: every dictionary definition points to a schema entry in the
        // library's static tables.
        unsafe { &*self.raw.schema }
    }

    /// Returns the stable symbolic name, for example `"application_label"`.
    pub fn name(&self) -> &'static str {
        // SAFETY: names are static NUL-terminated ASCII strings.
        unsafe { CStr::from_ptr(native::tlv_emv_symbol(self.raw)) }
            .to_str()
            .expect("dictionary names are ASCII")
    }

    /// Returns a curated human-readable label, for example
    /// `"Application File Locator (AFL)"`, or `None` if the name has none.
    pub fn display_label(&self) -> Option<&'static str> {
        // SAFETY: `name` is a valid C string; a non-null result is a static
        // NUL-terminated string.
        let label = unsafe { native::tlv_emv_display_label(native::tlv_emv_symbol(self.raw)) };
        if label.is_null() {
            return None;
        }
        // SAFETY: non-null, static, NUL-terminated.
        unsafe { CStr::from_ptr(label) }.to_str().ok()
    }

    /// Returns the tag the definition describes.
    pub fn tag(&self) -> Tag {
        // SAFETY: dictionary tags borrow immutable static bytes.
        unsafe { Tag::from_raw(&self.entry().tag) }.expect("dictionary tags are valid")
    }

    /// Returns this immutable builtin entry's presentation category.
    /// Uses the builtin profile, never codec callback introspection.
    pub fn kind(&self) -> ValueKind {
        ValueKind::from_raw(unsafe { native::tlv_emv_builtin_value_kind(self.raw) })
            .expect("dictionary kinds are known")
    }

    /// Returns the minimum value length in bytes.
    pub fn min_length(&self) -> usize {
        self.entry().min_length
    }

    /// Returns the maximum value length in bytes, or `None` if the dictionary
    /// gives no context-independent upper bound.
    pub fn max_length(&self) -> Option<usize> {
        Some(self.entry().max_length).filter(|max| *max != usize::MAX)
    }

    /// Returns the step between permitted lengths: `min + n * step`.
    pub fn length_step(&self) -> usize {
        // SAFETY: the borrowed builtin definition has a valid static schema.
        unsafe { native::tlv_emv_length_step(self.raw) }
    }

    /// Checks that a value of `length` bytes is permitted, including the
    /// length step.
    ///
    /// # Errors
    ///
    /// [`Error::Schema`] if the length is not permitted.
    pub fn validate_length(&self, length: usize) -> Result<()> {
        // SAFETY: `self.raw` is a valid definition.
        Error::check(unsafe { native::tlv_emv_validate_length(self.raw, length) })
    }

    /// Returns the semantic codec of the value, or `None` for opaque bytes,
    /// text and templates.
    pub fn codec(&self) -> Option<Codec> {
        // SAFETY: the codec, if any, is a static descriptor whose
        // representation is the definition's value kind.
        unsafe { Codec::from_raw(self.raw.codec, self.kind()) }
    }
}

/// Looks up the definition of `tag` in `context`, or `None` if unknown.
///
/// ```
/// use opentlv::emv::{self, Context};
/// use opentlv::{Tag, Value};
///
/// let aip = emv::find(Context::Base, &Tag::from_bytes(&[0x82])).unwrap();
/// assert_eq!(aip.name(), "aip");
/// assert_eq!(aip.codec().unwrap().decode(&[0x20, 0x00]).unwrap(), Value::Flags(0x2000));
/// ```
pub fn find(context: Context, tag: &Tag) -> Option<Definition> {
    let tag = tag.raw();
    // SAFETY: `tag` is valid for the call; a non-null result points into the
    // library's immutable static tables.
    let raw = unsafe { native::tlv_emv_find(context.raw(), &tag).as_ref()? };
    Some(Definition { raw })
}
