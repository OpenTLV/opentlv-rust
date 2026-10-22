// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Owned Reader diagnostics: no native borrowed pointer escapes.
use opentlv_sys as native;
use std::{ffi::CStr, slice};

/// Structured detail from the canonical Reader, safe to retain after input replacement.
#[derive(Clone, Debug, Default)]
pub struct ReaderDiagnostic {
    /// Canonical diagnostic severity.
    pub severity: crate::Severity,
    /// Owned subsystem context entries.
    pub contexts: Vec<crate::DiagnosticContext>,

    /// Primary evidence coordinates.
    pub location: crate::Location,
    /// Retained enclosing Tag bytes; None means no path was tracked.
    pub path: Option<Vec<Vec<u8>>>,
    /// Number of omitted innermost scopes.
    pub path_omitted: usize,
    /// Absolute failing field offset.
    pub offset: Option<usize>,
    /// C Reader operation code (tag, length, value, trailer or header).
    pub operation: crate::ReaderOperation,
    /// Raw wire identifier, when reported.
    pub tag: Option<Vec<u8>>,
    /// Original available length-field bytes.
    pub raw_length: Option<Vec<u8>>,
    /// Absolute tag field offset.
    pub tag_offset: Option<usize>,
    /// Absolute length field offset.
    pub length_offset: Option<usize>,
    /// Absolute value field offset.
    pub value_offset: Option<usize>,
    /// Declared logical length.
    pub declared_length: Option<u64>,
    /// Required failing region extent.
    pub required: Option<u64>,
    /// Available bytes at the failure.
    pub available: Option<usize>,
    /// Absolute enclosing boundary.
    pub enclosing_end: Option<usize>,
    /// Expected description, when supplied by C.
    pub expected: Option<String>,
    /// Actual description, when supplied by C.
    pub actual: Option<String>,
}

impl ReaderDiagnostic {
    pub(crate) unsafe fn from_raw(raw: &native::tlv_reader_diagnostic_t) -> Self {
        // SAFETY: the caller retains all borrowed diagnostic storage.
        unsafe { Self::from_parts(&raw.diagnostic, &raw.detail) }
    }
    pub(crate) unsafe fn from_parts(
        common: &native::tlv_diagnostic_t,
        raw: &native::tlv_reader_detail_t,
    ) -> Self {
        unsafe fn bytes(ptr: *const u8, size: usize) -> Vec<u8> {
            if size == 0 {
                Vec::new()
            } else {
                // SAFETY: the caller retains the input backing the C diagnostic.
                unsafe { slice::from_raw_parts(ptr, size) }.to_vec()
            }
        }
        unsafe fn text(ptr: *const std::os::raw::c_char) -> Option<String> {
            if ptr.is_null() {
                None
            } else {
                // SAFETY: C diagnostic descriptions are live NUL-terminated strings.
                Some(
                    unsafe { CStr::from_ptr(ptr) }
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        }
        Self {
            location: crate::Location::from_raw(common.location),
            severity: crate::Severity::from_raw(common.severity),
            contexts: unsafe { contexts(common) },
            path: (common.has_path != 0).then(|| {
                common.path.tags[..common.path.length]
                    .iter()
                    .map(|tag| unsafe { bytes(tag.data, tag.size) })
                    .collect()
            }),
            path_omitted: common.path.omitted,
            offset: (common.location.kind != 0).then_some(common.location.begin),
            operation: crate::ReaderOperation::from_raw(raw.operation),
            // SAFETY: the caller keeps diagnostic buffers alive for these copies.
            tag: (raw.has_tag != 0).then(|| unsafe { bytes(raw.tag.data, raw.tag.size) }),
            raw_length: (raw.has_raw_length != 0)
                .then(|| unsafe { bytes(raw.raw_length.data, raw.raw_length.size) }),
            tag_offset: (raw.has_tag_offset != 0).then_some(raw.tag_offset),
            length_offset: (raw.has_length_offset != 0).then_some(raw.length_offset),
            value_offset: (raw.has_value_offset != 0).then_some(raw.value_offset),
            declared_length: (raw.has_declared_length != 0).then_some(raw.declared_length),
            required: (raw.has_required != 0).then_some(raw.required),
            available: (raw.has_available != 0).then_some(raw.available),
            enclosing_end: (raw.has_enclosing_end != 0).then_some(raw.enclosing_end),
            // SAFETY: descriptions remain live until after this conversion.
            expected: unsafe { text(common.expected) },
            actual: unsafe { text(common.actual) },
        }
    }
}

/// Owned subsystem context, independent of native pointer lifetimes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticContext {
    /// Producing layer.
    pub layer: String,
    /// Context key.
    pub key: String,
    /// Context value.
    pub value: String,
}

pub(crate) unsafe fn contexts(raw: &native::tlv_diagnostic_t) -> Vec<DiagnosticContext> {
    let text = |p: *const std::os::raw::c_char| {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: caller retains the native context list and its strings.
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        }
    };
    let mut result = Vec::new();
    let mut current = raw.contexts;
    while !current.is_null() {
        // SAFETY: caller retains the acyclic native context list.
        let entry = unsafe { &*current };
        result.push(DiagnosticContext {
            layer: text(entry.layer),
            key: text(entry.key),
            value: text(entry.value),
        });
        current = entry.next;
    }
    result
}

pub(crate) unsafe fn path(raw: &native::tlv_diagnostic_t) -> Option<Vec<Vec<u8>>> {
    (raw.has_path != 0).then(|| {
        raw.path.tags[..raw.path.length]
            .iter()
            .map(|tag| {
                if tag.size == 0 {
                    Vec::new()
                } else {
                    // SAFETY: caller retains the borrowed tag spans until copied.
                    unsafe { slice::from_raw_parts(tag.data, tag.size) }.to_vec()
                }
            })
            .collect()
    })
}

/// Owned common diagnostic metadata; absence of a path is distinct from an empty path.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticMetadata {
    /// Common expected condition, independent of related Query expression detail.
    pub expected: Option<Box<str>>,
    /// Common observed condition, when reported.
    pub actual: Option<Box<str>>,
    /// Canonical diagnostic severity.
    pub severity: crate::Severity,
    /// Owned subsystem context entries.
    pub contexts: Vec<crate::DiagnosticContext>,
    /// Retained enclosing tags; None means no path evidence.
    pub path: Option<Vec<Vec<u8>>>,
    /// Number of omitted innermost scopes.
    pub path_omitted: usize,
}
