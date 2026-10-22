// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Optional owning Document API declarations.
use super::*;
extern "C" {
    /// Edit a finalized compiled selection using exclusive explicit target storage.
    pub fn tlv_document_query_edit(
        document: *mut tlv_document_t,
        exec: *mut tlv_query_exec_t,
        kind: c_int,
        tag: tlv_tag_t,
        value: *const u8,
        size: usize,
        targets: *mut *mut tlv_node_t,
        capacity: usize,
        applied: *mut usize,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Stable native node identity, scoped to its owning Document.
    pub fn tlv_node_identity(node: *const tlv_node_t) -> u64;
}

/// Opaque resumable Document builder.
#[repr(C)]
pub struct tlv_document_builder_t {
    _private: [u8; 0],
}

/// Opaque owning document.
#[repr(C)]
pub struct tlv_document_t {
    _private: [u8; 0],
}
/// Opaque document-owned node.
#[repr(C)]
pub struct tlv_node_t {
    _private: [u8; 0],
}
/// Native document configuration, initialized through C.
#[repr(C)]
pub struct tlv_document_options_t {
    /// Borrowed Format descriptor.
    pub format: *const tlv_format_t,
    /// Root-relative depth limit.
    pub max_depth: usize,
    /// Maximum document node count.
    pub max_elements: usize,
    /// Optional borrowed allocator descriptor.
    pub allocator: *const c_void,
    /// Retain original parse coordinates without borrowing input bytes (default zero).
    pub retain_source_locations: c_int,
}
extern "C" {
    pub fn tlv_document_builder_create(
        options: *const tlv_document_options_t,
        reader: *mut tlv_tree_reader_t,
        root: *const tlv_tree_item_t,
        builder: *mut *mut tlv_document_builder_t,
    ) -> tlv_result_t;
    pub fn tlv_document_builder_consume(
        builder: *mut tlv_document_builder_t,
        document: *mut *mut tlv_document_t,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_document_builder_free(builder: *mut tlv_document_builder_t);
    pub fn tlv_document_options_init(
        options: *mut tlv_document_options_t,
        format: *const tlv_format_t,
    ) -> tlv_result_t;
    pub fn tlv_document_create(
        options: *const tlv_document_options_t,
        document: *mut *mut tlv_document_t,
    ) -> tlv_result_t;
    pub fn tlv_document_parse(
        data: *const u8,
        size: usize,
        options: *const tlv_document_options_t,
        document: *mut *mut tlv_document_t,
        diagnostic: *mut tlv_reader_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_document_free(document: *mut tlv_document_t);
    pub fn tlv_document_count(document: *const tlv_document_t) -> usize;
    pub fn tlv_document_first(document: *const tlv_document_t) -> *mut tlv_node_t;
    pub fn tlv_document_find(
        document: *const tlv_document_t,
        parent: *const tlv_node_t,
        tag: tlv_tag_t,
    ) -> *mut tlv_node_t;
    pub fn tlv_document_find_path(
        document: *const tlv_document_t,
        query: *const tlv_query_t,
        node: *mut *mut tlv_node_t,
    ) -> tlv_result_t;
    pub fn tlv_document_insert(
        document: *mut tlv_document_t,
        parent: *mut tlv_node_t,
        before: *const tlv_node_t,
        tag: tlv_tag_t,
        value: *const u8,
        length: usize,
        node: *mut *mut tlv_node_t,
    ) -> tlv_result_t;
    pub fn tlv_document_encoded_size(
        document: *const tlv_document_t,
        size: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_document_encoded_size_as(
        document: *const tlv_document_t,
        format: *const tlv_format_t,
        size: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_document_encode(
        document: *const tlv_document_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_document_encode_as(
        document: *const tlv_document_t,
        format: *const tlv_format_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_node_first_child(node: *const tlv_node_t) -> *mut tlv_node_t;
    pub fn tlv_node_next(node: *const tlv_node_t) -> *mut tlv_node_t;
    pub fn tlv_node_next_same_tag(node: *const tlv_node_t) -> *mut tlv_node_t;
    pub fn tlv_node_parent(node: *const tlv_node_t) -> *mut tlv_node_t;
    pub fn tlv_node_tag(node: *const tlv_node_t) -> tlv_tag_t;
    pub fn tlv_node_is_constructed(node: *const tlv_node_t) -> c_int;
    pub fn tlv_node_value_data(node: *const tlv_node_t) -> *const u8;
    pub fn tlv_node_value_size(node: *const tlv_node_t) -> usize;
    pub fn tlv_node_set_value(
        node: *mut tlv_node_t,
        value: *const u8,
        length: usize,
    ) -> tlv_result_t;
    pub fn tlv_node_erase(node: *mut tlv_node_t);
    pub fn tlv_node_encoded_size(node: *const tlv_node_t, size: *mut usize) -> tlv_result_t;
    pub fn tlv_node_encoded_size_as(
        node: *const tlv_node_t,
        format: *const tlv_format_t,
        size: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_node_encode(
        node: *const tlv_node_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_node_encode_as(
        node: *const tlv_node_t,
        format: *const tlv_format_t,
        data: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> tlv_result_t;
}
