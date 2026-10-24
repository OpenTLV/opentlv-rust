// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Deterministic wire generation (`tlv/generator.h`).
use super::*;

/// Wire generator algorithm version (`TLV_GENERATOR_VERSION`).
pub const TLV_GENERATOR_VERSION: u32 = 2;

/// A borrowed candidate identifier and Value size interval (`tlv_generator_candidate_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_generator_candidate_t {
    /// Borrowed identifier.
    pub tag: tlv_tag_t,
    /// Inclusive minimum Value size in bytes.
    pub min_value_size: usize,
    /// Inclusive maximum Value size in bytes.
    pub max_value_size: usize,
}

/// Immutable configuration of one reproducible case (`tlv_generator_options_t`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct tlv_generator_options_t {
    /// Explicit seed; zero is valid.
    pub seed: u64,
    /// Random-access case index; zero is valid.
    pub case_index: u64,
    /// Positive total element bound, including parents.
    pub max_elements: usize,
    /// Maximum child depth; roots have depth zero; at most 64.
    pub max_depth: usize,
    /// Maximum Value size for every element in bytes.
    pub max_value_size: usize,
    /// Positive maximum complete stream size in bytes.
    pub max_case_size: usize,
    /// Borrowed ordered candidate table.
    pub candidates: *const tlv_generator_candidate_t,
    /// Positive table extent.
    pub candidate_count: usize,
}

extern "C" {
    /// Computes the required caller-owned workspace without allocation.
    pub fn tlv_generator_workspace_size(
        options: *const tlv_generator_options_t,
        size: *mut usize,
    ) -> tlv_result_t;
    /// Generates one nonempty complete stream through the canonical Writer.
    pub fn tlv_generate(
        format: *const tlv_format_t,
        options: *const tlv_generator_options_t,
        data: *mut u8,
        capacity: usize,
        workspace: *mut u8,
        workspace_size: usize,
        written: *mut usize,
    ) -> tlv_result_t;
}
