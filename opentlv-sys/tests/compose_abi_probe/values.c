// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

#include "tlv/format.h"
#include "tlv/formats/escaped.h"
#include "tlv/formats/packed.h"
#include "tlv/formats/variable.h"
#include <stddef.h>
#include <stdint.h>

/* C99 alignment probe: a leading byte exposes the next member's alignment. */
struct align_tlv_format_error_t {
    char               prefix;
    tlv_format_error_t value;
};
struct align_tlv_encoding_t {
    char           prefix;
    tlv_encoding_t value;
};
struct align_tlv_constructed_bit_t {
    char                  prefix;
    tlv_constructed_bit_t value;
};
struct align_tlv_identifier_policy_t {
    char                    prefix;
    tlv_identifier_policy_t value;
};
struct align_tlv_length_policy_t {
    char                prefix;
    tlv_length_policy_t value;
};
struct align_tlv_variable_identifier_t {
    char                      prefix;
    tlv_variable_identifier_t value;
};
struct align_tlv_variable_length_t {
    char                  prefix;
    tlv_variable_length_t value;
};
struct align_tlv_escaped_length_t {
    char                 prefix;
    tlv_escaped_length_t value;
};
struct align_tlv_packed_field_t {
    char               prefix;
    tlv_packed_field_t value;
};
struct align_tlv_field_composition_t {
    char                    prefix;
    tlv_field_composition_t value;
};
struct align_tlv_tagged_binary_composition_t {
    char                            prefix;
    tlv_tagged_binary_composition_t value;
};
struct align_tlv_tagged_fields_composition_t {
    char                            prefix;
    tlv_tagged_fields_composition_t value;
};
struct align_tlv_escaped_format_t {
    char                 prefix;
    tlv_escaped_format_t value;
};
struct align_tlv_packed_layout_t {
    char                prefix;
    tlv_packed_layout_t value;
};
struct align_tlv_variable_format_t {
    char                  prefix;
    tlv_variable_format_t value;
};

size_t opentlv_test_compose_abi(size_t index) {
    const size_t values[] = {sizeof(tlv_format_error_t),
                             offsetof(struct align_tlv_format_error_t, value),
                             offsetof(tlv_format_error_t, region),
                             offsetof(tlv_format_error_t, offset),
                             offsetof(tlv_format_error_t, has_offset),
                             offsetof(tlv_format_error_t, required),
                             offsetof(tlv_format_error_t, has_required),
                             offsetof(tlv_format_error_t, tag),
                             offsetof(tlv_format_error_t, length),
                             offsetof(tlv_format_error_t, value),
                             sizeof(tlv_encoding_t),
                             offsetof(struct align_tlv_encoding_t, value),
                             offsetof(tlv_encoding_t, header),
                             offsetof(tlv_encoding_t, value),
                             offsetof(tlv_encoding_t, trailer),
                             offsetof(tlv_encoding_t, total),
                             sizeof(tlv_constructed_bit_t),
                             offsetof(struct align_tlv_constructed_bit_t, value),
                             offsetof(tlv_constructed_bit_t, byte_index),
                             offsetof(tlv_constructed_bit_t, mask),
                             offsetof(tlv_constructed_bit_t, value),
                             sizeof(tlv_identifier_policy_t),
                             offsetof(struct align_tlv_identifier_policy_t, value),
                             offsetof(tlv_identifier_policy_t, forbidden_leading),
                             offsetof(tlv_identifier_policy_t, forbidden_leading_count),
                             offsetof(tlv_identifier_policy_t, reject_zero_first_payload),
                             offsetof(tlv_identifier_policy_t, require_minimal),
                             sizeof(tlv_length_policy_t),
                             offsetof(struct align_tlv_length_policy_t, value),
                             offsetof(tlv_length_policy_t, allow_short),
                             offsetof(tlv_length_policy_t, allow_long),
                             offsetof(tlv_length_policy_t, require_minimal),
                             offsetof(tlv_length_policy_t, max_long_octets),
                             offsetof(tlv_length_policy_t, max_value),
                             sizeof(tlv_variable_identifier_t),
                             offsetof(struct align_tlv_variable_identifier_t, value),
                             offsetof(tlv_variable_identifier_t, inline_mask),
                             offsetof(tlv_variable_identifier_t, escape),
                             offsetof(tlv_variable_identifier_t, continuation_bit),
                             offsetof(tlv_variable_identifier_t, payload_mask),
                             offsetof(tlv_variable_identifier_t, max_size),
                             offsetof(tlv_variable_identifier_t, policy),
                             sizeof(tlv_variable_length_t),
                             offsetof(struct align_tlv_variable_length_t, value),
                             offsetof(tlv_variable_length_t, long_form_bit),
                             offsetof(tlv_variable_length_t, payload_mask),
                             offsetof(tlv_variable_length_t, byte_order),
                             offsetof(tlv_variable_length_t, policy),
                             sizeof(tlv_escaped_length_t),
                             offsetof(struct align_tlv_escaped_length_t, value),
                             offsetof(tlv_escaped_length_t, escape),
                             offsetof(tlv_escaped_length_t, extended_size),
                             offsetof(tlv_escaped_length_t, byte_order),
                             offsetof(tlv_escaped_length_t, min_extended),
                             offsetof(tlv_escaped_length_t, max_length),
                             sizeof(tlv_packed_field_t),
                             offsetof(struct align_tlv_packed_field_t, value),
                             offsetof(tlv_packed_field_t, storage_size),
                             offsetof(tlv_packed_field_t, bit_offset),
                             offsetof(tlv_packed_field_t, bit_width),
                             offsetof(tlv_packed_field_t, byte_order),
                             sizeof(tlv_field_composition_t),
                             offsetof(struct align_tlv_field_composition_t, value),
                             offsetof(tlv_field_composition_t, context),
                             offsetof(tlv_field_composition_t, read_tag),
                             offsetof(tlv_field_composition_t, read_length),
                             offsetof(tlv_field_composition_t, resolve),
                             offsetof(tlv_field_composition_t, write_tag),
                             offsetof(tlv_field_composition_t, write_length),
                             offsetof(tlv_field_composition_t, element_order),
                             offsetof(tlv_field_composition_t, length_scope),
                             sizeof(tlv_tagged_binary_composition_t),
                             offsetof(struct align_tlv_tagged_binary_composition_t, value),
                             offsetof(tlv_tagged_binary_composition_t, fields),
                             offsetof(tlv_tagged_binary_composition_t, tag_only),
                             offsetof(tlv_tagged_binary_composition_t, count),
                             sizeof(tlv_tagged_fields_composition_t),
                             offsetof(struct align_tlv_tagged_fields_composition_t, value),
                             offsetof(tlv_tagged_fields_composition_t, fields),
                             offsetof(tlv_tagged_fields_composition_t, tag_only),
                             offsetof(tlv_tagged_fields_composition_t, count),
                             sizeof(tlv_escaped_format_t),
                             offsetof(struct align_tlv_escaped_format_t, value),
                             offsetof(tlv_escaped_format_t, identifier),
                             offsetof(tlv_escaped_format_t, length),
                             offsetof(tlv_escaped_format_t, element_order),
                             offsetof(tlv_escaped_format_t, length_scope),
                             offsetof(tlv_escaped_format_t, tag_only),
                             offsetof(tlv_escaped_format_t, count),
                             sizeof(tlv_packed_layout_t),
                             offsetof(struct align_tlv_packed_layout_t, value),
                             offsetof(tlv_packed_layout_t, header_size),
                             offsetof(tlv_packed_layout_t, tag),
                             offsetof(tlv_packed_layout_t, length),
                             offsetof(tlv_packed_layout_t, length_scope),
                             offsetof(tlv_packed_layout_t, tag_storage),
                             offsetof(tlv_packed_layout_t, tag_size),
                             offsetof(tlv_packed_layout_t, tag_storage_size),
                             sizeof(tlv_variable_format_t),
                             offsetof(struct align_tlv_variable_format_t, value),
                             offsetof(tlv_variable_format_t, identifier),
                             offsetof(tlv_variable_format_t, length),
                             offsetof(tlv_variable_format_t, element_order),
                             offsetof(tlv_variable_format_t, length_scope),
                             offsetof(tlv_variable_format_t, constructed)};
    return index < sizeof values / sizeof values[0] ? values[index] : SIZE_MAX;
}
