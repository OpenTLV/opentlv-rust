// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
//! Native full-language Query ABI, versioned together with the C headers.
use super::*;

/// Query lifecycle or callback reentrancy failure category.
pub const TLV_QUERY_ERROR_STATE: c_int = 12;
pub const TLV_QUERY_ERROR_CALLBACK: c_int = 13;
pub const TLV_QUERY_ERROR_TYPE: c_int = 14;
pub const TLV_QUERY_ERROR_IMAGE: c_int = 15;
pub const TLV_QUERY_ERROR_SCHEMA: c_int = 16;
/// Query expression text coordinate domain.
pub const TLV_LOCATION_EXPRESSION: c_int = 3;
/// Query-owned failure without lower-layer detail.
pub const TLV_QUERY_CAUSE_NONE: c_int = 0;
/// Reader detail is active.
pub const TLV_QUERY_CAUSE_READER: c_int = 1;
/// Conversion detail is active.
pub const TLV_QUERY_CAUSE_CODEC: c_int = 2;
/// Schema detail is active.
pub const TLV_QUERY_CAUSE_SCHEMA: c_int = 3;

macro_rules! record {
    ($name:ident { $($field:ident: $type:ty),* $(,)? }) => {
        #[doc = concat!("Native ", stringify!($name), " layout.")]
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct $name { $(#[doc = concat!("C field ", stringify!($field), ".")] pub $field: $type),* }
    };
}
/// Opaque immutable same-release program.
#[repr(C)]
pub struct tlv_query_program_t {
    _private: [u8; 0],
}
/// Opaque exclusive execution workspace.
#[repr(C)]
pub struct tlv_query_exec_t {
    _private: [u8; 0],
}
record!(tlv_query_diagnostic_t {
    diagnostic: tlv_diagnostic_t,
    kind: c_int, expression: tlv_location_t,
    limit: *const c_char, configured: usize,
    cause: c_int, detail: tlv_query_cause_detail_t,
});
#[repr(C)]
#[derive(Clone, Copy)]
/// Only the member selected by `cause` is active.
pub union tlv_query_cause_detail_t {
    pub reader: tlv_reader_detail_t,
    pub codec: tlv_codec_detail_t,
    pub schema: tlv_schema_detail_t,
}
impl std::fmt::Debug for tlv_query_cause_detail_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QueryCause")
    }
}
record!(tlv_query_result_t { kind: c_int, boolean: c_int, integer: i64, data: *const u8, size: usize });
/// Closed conversion provider callback; spans borrow input or exclusive scratch.
pub type tlv_query_decode_t = Option<
    unsafe extern "C" fn(
        *const c_void,
        *const tlv_tree_event_t,
        *const u8,
        usize,
        *mut c_void,
        usize,
        *mut tlv_query_result_t,
        *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t,
>;
record!(tlv_query_hook_t { id: u32, function: c_int, scratch_size: usize, scratch_alignment: usize,
    context: *const c_void, decode: tlv_query_decode_t });
/// Signed `int64_t` codec representation.
pub const TLV_QUERY_CODEC_SIGNED: c_int = 0;
/// Unsigned `uint64_t` codec representation, checked before int64 narrowing.
pub const TLV_QUERY_CODEC_UNSIGNED: c_int = 1;
/// Borrowed validated UTF-8 codec representation.
pub const TLV_QUERY_CODEC_UTF8: c_int = 2;
record!(tlv_query_codec_adapter_t { codec: *const tlv_codec_t, representation: c_int });
record!(tlv_query_tag_adapter_t { id: u32, context: *const c_void,
    class_of: Option<unsafe extern "C" fn(*const c_void, *const tlv_tag_t, *mut i64) -> tlv_result_t>,
    number_of: Option<unsafe extern "C" fn(*const c_void, *const tlv_tag_t, *mut i64) -> tlv_result_t> });
record!(tlv_query_environment_t { format: *const tlv_format_t, tags: *const tlv_query_tag_adapter_t,
    hooks: *const tlv_query_hook_t, hook_count: usize });
record!(tlv_query_definition_scope_t { namespace_name: *const c_char, definitions: *const tlv_definition_registry_t });
record!(tlv_query_definition_resolver_t { scopes: *const tlv_query_definition_scope_t, count: usize });
record!(tlv_schema_query_rule_t { context: *const tlv_query_program_t,
    assertion: *const tlv_query_program_t, environment: *const tlv_query_environment_t, name: *const c_char });
record!(tlv_schema_query_context_t { ordinal: usize, event: tlv_tree_event_t, node: *mut c_void });
record!(tlv_schema_query_workspace_t { selector: *mut c_void, selector_size: usize,
    assertion: *mut c_void, assertion_size: usize, contexts: *mut tlv_schema_query_context_t,
    context_capacity: usize, frames: *mut tlv_tree_frame_t, frame_capacity: usize });
record!(tlv_schema_query_diagnostic_t {
    rule: usize,
    query: tlv_query_diagnostic_t
});
record!(tlv_query_variable_t { name: *const c_char, type_: c_int });
/// Compile-only resolver; C copies its returned Tag.
pub type tlv_query_resolve_t = Option<
    unsafe extern "C" fn(
        *const c_void,
        *const c_char,
        usize,
        *const c_char,
        usize,
        *mut tlv_tag_t,
    ) -> tlv_result_t,
>;
record!(tlv_query_compile_options_t {
    struct_size: usize, language_version: u32, max_text: usize, max_tokens: usize,
    max_nesting: usize, max_states: usize, variables: *const tlv_query_variable_t,
    variable_count: usize, resolve: tlv_query_resolve_t, resolve_context: *const c_void,
    environment: *const tlv_query_environment_t, max_resolved_tag: usize, max_pattern: usize,
    optimize: c_int,
});
record!(tlv_query_program_info_t {
    struct_size: usize,
    program_size: usize,
    program_alignment: usize,
    scratch_size: usize,
    scratch_alignment: usize,
    states: usize,
    language_version: u32,
    level: c_int,
    result_kind: c_int,
    expression_values: usize,
    instructions: usize,
    variable_slots: usize,
    codec_scratch: usize,
    pattern_bytes: usize,
    optimized_states: usize,
    expression_stack: usize,
    candidate_size: usize,
    candidate_alignment: usize,
    frame_states: usize,
    decision_timing: c_int,
    stable_input_required: c_int,
    constructed_values_required: c_int,
});
record!(tlv_query_variable_info_t { name: *const c_char, name_size: usize, type_: c_int });
record!(tlv_query_exec_info_t {
    struct_size: usize,
    elements: usize,
    work: usize,
    skipped_subtrees: usize,
    finished: c_int,
    full_validation: c_int,
    invalid: c_int
});
/// Visitor status matches the canonical C visitor enum.
pub type tlv_query_event_visitor_t =
    Option<unsafe extern "C" fn(*const tlv_tree_event_t, *mut c_void) -> c_int>;

extern "C" {
    /// Bound preparation storage without invoking scoped name resolution.
    pub fn tlv_query_compile_prepare_size(
        text: *const c_char,
        size: usize,
        options: *const tlv_query_compile_options_t,
        bytes: *mut usize,
        alignment: *mut usize,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Compile one immutable preparation image retained inside caller workspace.
    pub fn tlv_query_compile_prepare(
        text: *const c_char,
        size: usize,
        options: *const tlv_query_compile_options_t,
        workspace: *mut c_void,
        capacity: usize,
        prepared: *mut *const tlv_query_program_t,
        info: *mut tlv_query_program_info_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Recheck scoped resolution and atomically publish identical program bytes.
    pub fn tlv_query_compile_commit(
        prepared: *const c_void,
        prepared_size: usize,
        options: *const tlv_query_compile_options_t,
        scratch: *mut c_void,
        scratch_capacity: usize,
        storage: *mut c_void,
        storage_capacity: usize,
        info: *mut tlv_query_program_info_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Resolve a scoped Definition name with canonical unknown/ambiguity semantics.
    pub fn tlv_query_definition_resolve(
        context: *const c_void,
        namespace_name: *const c_char,
        namespace_size: usize,
        name: *const c_char,
        name_size: usize,
        tag: *mut tlv_tag_t,
    ) -> tlv_result_t;
    /// Discover contextual selector/assertion workspace requirements.
    pub fn tlv_schema_query_size(
        rules: *const tlv_schema_query_rule_t,
        count: usize,
        depth: usize,
        nodes: usize,
        selector: *mut usize,
        assertion: *mut usize,
        alignment: *mut usize,
    ) -> tlv_result_t;
    /// Validate contextual assertions over immutable complete input.
    pub fn tlv_schema_query_validate_buffer(
        data: *const u8,
        size: usize,
        format: *const tlv_format_t,
        rules: *const tlv_schema_query_rule_t,
        count: usize,
        depth: usize,
        nodes: usize,
        work: usize,
        workspace: *mut tlv_schema_query_workspace_t,
        diagnostic: *mut tlv_schema_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Validate contextual assertions over an immutable Document revision.
    #[cfg(feature = "document")]
    pub fn tlv_schema_query_validate_document(
        document: *const tlv_document_t,
        rules: *const tlv_schema_query_rule_t,
        count: usize,
        depth: usize,
        nodes: usize,
        work: usize,
        workspace: *mut tlv_schema_query_workspace_t,
        values: *mut u8,
        capacity: usize,
        staging: *mut tlv_tree_writer_workspace_t,
        diagnostic: *mut tlv_schema_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_compile_options_init(options: *mut tlv_query_compile_options_t);
    pub fn tlv_query_compile_scratch(
        text: *const c_char,
        size: usize,
        options: *const tlv_query_compile_options_t,
        bytes: *mut usize,
        alignment: *mut usize,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_compile(
        text: *const c_char,
        size: usize,
        options: *const tlv_query_compile_options_t,
        scratch: *mut c_void,
        scratch_capacity: usize,
        storage: *mut c_void,
        capacity: usize,
        info: *mut tlv_query_program_info_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_program_load_scratch(
        image: *const c_void,
        size: usize,
        options: *const tlv_query_compile_options_t,
        bytes: *mut usize,
        alignment: *mut usize,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_program_load(
        image: *const c_void,
        size: usize,
        options: *const tlv_query_compile_options_t,
        scratch: *mut c_void,
        capacity: usize,
        program: *mut *const tlv_query_program_t,
        info: *mut tlv_query_program_info_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_program_format(
        program: *const tlv_query_program_t,
        output: *mut c_char,
        capacity: usize,
        required: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_query_program_explain(
        program: *const tlv_query_program_t,
        output: *mut c_char,
        capacity: usize,
        required: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_query_program_variable_count(program: *const tlv_query_program_t) -> usize;
    pub fn tlv_query_program_variable(
        program: *const tlv_query_program_t,
        index: usize,
        info: *mut tlv_query_variable_info_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_size(
        program: *const tlv_query_program_t,
        depth: usize,
        bytes: *mut usize,
        alignment: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_query_eval_size(
        program: *const tlv_query_program_t,
        depth: usize,
        nodes: usize,
        bytes: *mut usize,
        alignment: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_init(
        program: *const tlv_query_program_t,
        storage: *mut c_void,
        capacity: usize,
        depth: usize,
        elements: usize,
        work: usize,
        exec: *mut *mut tlv_query_exec_t,
    ) -> tlv_result_t;
    pub fn tlv_query_eval_init(
        program: *const tlv_query_program_t,
        environment: *const tlv_query_environment_t,
        storage: *mut c_void,
        capacity: usize,
        depth: usize,
        nodes: usize,
        work: usize,
        exec: *mut *mut tlv_query_exec_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_bind(
        exec: *mut tlv_query_exec_t,
        name: *const c_char,
        type_: c_int,
        integer: i64,
        data: *const u8,
        size: usize,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Validate and borrow a bounded native plan without the Query frontend.
    pub fn tlv_query_plan_open(
        image: *const c_void,
        size: usize,
        program: *mut *const tlv_query_program_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Read the requirements of a validated plan.
    pub fn tlv_query_plan_info(
        program: *const tlv_query_program_t,
        info: *mut tlv_query_program_info_t,
    ) -> tlv_result_t;
    /// Reset an execution in place, keeping its program, providers and budgets.
    pub fn tlv_query_exec_reset(exec: *mut tlv_query_exec_t) -> tlv_result_t;
    /// Generic provider decoding a Value with a `tlv_query_codec_adapter_t` context.
    pub fn tlv_query_codec_decode(
        context: *const c_void,
        event: *const tlv_tree_event_t,
        data: *const u8,
        size: usize,
        scratch: *mut c_void,
        capacity: usize,
        result: *mut tlv_query_result_t,
        diagnostic: *mut tlv_codec_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_context(exec: *mut tlv_query_exec_t, ordinal: usize) -> tlv_result_t;
    pub fn tlv_query_exec_pruning(exec: *mut tlv_query_exec_t, enabled: c_int) -> tlv_result_t;
    pub fn tlv_query_exec_info(
        exec: *const tlv_query_exec_t,
        info: *mut tlv_query_exec_info_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_feed(
        exec: *mut tlv_query_exec_t,
        event: *const tlv_tree_event_t,
        matched: *mut c_int,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_selected(
        exec: *const tlv_query_exec_t,
        event: *mut tlv_tree_event_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_finish(
        exec: *mut tlv_query_exec_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_program_visit(
        reader: *mut tlv_tree_reader_t,
        exec: *mut tlv_query_exec_t,
        visitor: tlv_query_event_visitor_t,
        context: *mut c_void,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_program_exists(
        reader: *mut tlv_tree_reader_t,
        exec: *mut tlv_query_exec_t,
        early: c_int,
        found: *mut c_int,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    pub fn tlv_query_exec_result(
        exec: *const tlv_query_exec_t,
        result: *mut tlv_query_result_t,
    ) -> tlv_result_t;
    pub fn tlv_query_result_next(
        exec: *mut tlv_query_exec_t,
        event: *mut tlv_tree_event_t,
    ) -> tlv_result_t;
    pub fn tlv_query_result_next_ordinal(
        exec: *mut tlv_query_exec_t,
        event: *mut tlv_tree_event_t,
        ordinal: *mut usize,
    ) -> tlv_result_t;
    pub fn tlv_query_builtin_hooks(count: *mut usize) -> *const tlv_query_hook_t;
    pub fn tlv_emv_query_resolve(
        context: *const c_void,
        namespace_name: *const c_char,
        namespace_size: usize,
        name: *const c_char,
        name_size: usize,
        tag: *mut tlv_tag_t,
    ) -> tlv_result_t;
    pub static tlv_asn1_query_tags: tlv_query_tag_adapter_t;
    pub static tlv_asn1_query_date: tlv_query_hook_t;
    #[cfg(feature = "document")]
    pub fn tlv_document_query_evaluate(
        document: *const tlv_document_t,
        exec: *mut tlv_query_exec_t,
        context: *const tlv_node_t,
        values: *mut c_void,
        capacity: usize,
        staging: *mut tlv_tree_writer_workspace_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    #[cfg(feature = "document")]
    pub fn tlv_document_query_next(
        exec: *mut tlv_query_exec_t,
        node: *mut *mut tlv_node_t,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Visit the remaining finalized Document results of an evaluated execution.
    #[cfg(feature = "document")]
    pub fn tlv_document_query_program_visit(
        exec: *mut tlv_query_exec_t,
        visitor: tlv_document_query_visitor_t,
        context: *mut c_void,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Visit every path match of a parsed Query in Document order.
    #[cfg(feature = "document")]
    pub fn tlv_document_query_visit(
        document: *const tlv_document_t,
        query: *const tlv_query_t,
        visitor: tlv_document_query_visitor_t,
        context: *mut c_void,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
    /// Discover the complete encoded snapshot size for Document Query Values.
    #[cfg(feature = "document")]
    pub fn tlv_document_query_value_size(
        document: *const tlv_document_t,
        staging: *mut tlv_tree_writer_workspace_t,
        bytes: *mut usize,
        diagnostic: *mut tlv_query_diagnostic_t,
    ) -> tlv_result_t;
}
