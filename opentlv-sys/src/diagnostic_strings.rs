// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use std::os::raw::{c_char, c_int};

extern "C" {
    /// Canonical location domain label.
    pub fn tlv_location_domain_string(value: c_int) -> *const c_char;
    /// Canonical location anchor label.
    pub fn tlv_location_kind_string(value: c_int) -> *const c_char;
    /// Canonical static ReaderOperation label, including unknown values.
    pub fn tlv_reader_operation_string(value: c_int) -> *const c_char;
    /// Canonical static WriterOperation label, including unknown values.
    pub fn tlv_writer_operation_string(value: c_int) -> *const c_char;
    /// Canonical static QueryErrorKind label, including unknown values.
    pub fn tlv_query_error_kind_string(value: c_int) -> *const c_char;
    /// Canonical static QueryCause label, including unknown values.
    pub fn tlv_query_cause_string(value: c_int) -> *const c_char;
    /// Canonical static SchemaDefinitionKind label, including unknown values.
    pub fn tlv_schema_definition_kind_string(value: c_int) -> *const c_char;
    /// Canonical static CodecOperation label, including unknown values.
    pub fn tlv_codec_operation_string(value: c_int) -> *const c_char;
    /// Canonical static CodecCause label, including unknown values.
    pub fn tlv_codec_cause_string(value: c_int) -> *const c_char;
    /// Canonical static CodecViolation label, including unknown values.
    pub fn tlv_codec_violation_string(value: c_int) -> *const c_char;
    /// Canonical static CodecReason label, including unknown values.
    pub fn tlv_codec_reason_string(value: c_int) -> *const c_char;
    /// Canonical static SchemaCause label, including unknown values.
    pub fn tlv_schema_cause_string(value: c_int) -> *const c_char;
    /// Canonical static Severity label, including unknown values.
    pub fn tlv_diagnostic_severity_string(value: c_int) -> *const c_char;
}
