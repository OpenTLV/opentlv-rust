// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Canonical Schema report ABI.
use super::*;
/// Native enclosing scope path.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_diagnostic_path_t {
    /// Borrowed scope Tags.
    pub tags: [tlv_tag_t; 32],
    /// Valid prefix length.
    pub length: usize,
    /// Number of innermost scope Tags omitted after the retained outer prefix.
    pub omitted: usize,
}
/// Schema-specific violation evidence, shared by Schema, Codec and Query diagnostics.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_schema_detail_t {
    /// Corresponding C field.
    pub kind: c_int,
    /// Corresponding C field.
    pub tag: tlv_tag_t,
    /// Corresponding C field.
    pub definition: tlv_schema_definition_location_t,
    /// Corresponding C field.
    pub field: *const c_char,
    /// Corresponding C field.
    pub is_group: c_int,
    /// Corresponding C field.
    pub has_occurs: c_int,
    /// Corresponding C field.
    pub min_occurs: usize,
    /// Corresponding C field.
    pub max_occurs: usize,
    /// Corresponding C field.
    pub occurs: usize,
    /// Corresponding C field.
    pub has_length: c_int,
    /// Corresponding C field.
    pub min_length: usize,
    /// Corresponding C field.
    pub max_length: usize,
    /// Corresponding C field.
    pub actual_length: usize,
    /// Corresponding C field.
    pub has_form: c_int,
    /// Corresponding C field.
    pub expected_form: c_int,
    /// Corresponding C field.
    pub actual_constructed: c_int,
    /// Corresponding C field.
    pub length_multiple: usize,
    /// Corresponding C field.
    pub length_flags: u32,
}
/// Schema finding or definition failure.
pub const TLV_SCHEMA_CAUSE_SCHEMA: c_int = 1;
/// Reader rejected the input.
pub const TLV_SCHEMA_CAUSE_READER: c_int = 2;
/// A Value conversion rejected the input.
pub const TLV_SCHEMA_CAUSE_CODEC: c_int = 3;
/// Only the member selected by `cause` is active.
#[repr(C)]
#[derive(Clone, Copy)]
pub union tlv_schema_cause_detail_t {
    /// Schema finding.
    pub schema: tlv_schema_detail_t,
    /// Original Reader detail.
    pub reader: tlv_reader_detail_t,
    /// Value conversion detail.
    pub codec: tlv_codec_detail_t,
}
impl std::fmt::Debug for tlv_schema_cause_detail_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SchemaCause")
    }
}
/// Detailed native Schema violation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_schema_diagnostic_t {
    /// Corresponding C field.
    pub diagnostic: tlv_diagnostic_t,
    /// Active member of `detail`.
    pub cause: c_int,
    /// Corresponding C field.
    pub detail: tlv_schema_cause_detail_t,
}
/// Caller-owned detailed report storage.
#[repr(C)]
pub struct tlv_schema_diagnostic_report_t {
    /// Diagnostic buffer.
    pub diagnostics: *mut tlv_schema_diagnostic_t,
    /// Buffer capacity.
    pub capacity: usize,
    /// Total violation count.
    pub count: usize,
}
extern "C" {
    pub fn tlv_schema_issue_kind_string(kind: c_int) -> *const c_char;
    pub fn tlv_schema_validate_all_diag(
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
        schema: *const tlv_structure_schema_t,
        max_depth: usize,
        max_elements: usize,
        unknown: c_int,
        report: *mut tlv_schema_diagnostic_report_t,
        diagnostic: *mut tlv_schema_diagnostic_t,
    ) -> tlv_result_t;
}

/// Native definition owner and member index.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_schema_definition_location_t {
    /// Table/rule/group/type/component discriminator, zero for unknown.
    pub kind: c_int,
    /// Borrowed owner table or type.
    pub owner: *const c_void,
    /// Index within the owner for indexed kinds.
    pub index: usize,
}
