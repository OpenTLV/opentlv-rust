// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

#include "tlv/codec/digits.h"
#include "tlv/codec/ipv4.h"
#include "tlv/codec/structure.h"
#include "tlv/codec/text.h"
#include "tlv/generator.h"
#include "tlv/schema/number.h"
#include <stddef.h>
#include <stdint.h>

/* C99 alignment probe: a leading byte exposes the next member's alignment. */
struct align_tlv_text_codec_config_t {
    char                    prefix;
    tlv_text_codec_config_t value;
};
struct align_tlv_digits_codec_config_t {
    char                      prefix;
    tlv_digits_codec_config_t value;
};
struct align_tlv_ipv4_t {
    char       prefix;
    tlv_ipv4_t value;
};
struct align_tlv_ipv4_list_t {
    char            prefix;
    tlv_ipv4_list_t value;
};
struct align_tlv_schema_number_t {
    char                prefix;
    tlv_schema_number_t value;
};
struct align_tlv_structure_codec_t {
    char                  prefix;
    tlv_structure_codec_t value;
};
struct align_tlv_generator_candidate_t {
    char                      prefix;
    tlv_generator_candidate_t value;
};
struct align_tlv_generator_options_t {
    char                    prefix;
    tlv_generator_options_t value;
};

size_t opentlv_test_codec_abi(size_t index) {
    const size_t values[] = {sizeof(tlv_text_codec_config_t),
                             offsetof(struct align_tlv_text_codec_config_t, value),
                             offsetof(tlv_text_codec_config_t, alphabet),
                             offsetof(tlv_text_codec_config_t, width),
                             offsetof(tlv_text_codec_config_t, zero_padding),
                             sizeof(tlv_digits_codec_config_t),
                             offsetof(struct align_tlv_digits_codec_config_t, value),
                             offsetof(tlv_digits_codec_config_t, width),
                             sizeof(tlv_ipv4_t),
                             offsetof(struct align_tlv_ipv4_t, value),
                             offsetof(tlv_ipv4_t, bytes),
                             sizeof(tlv_ipv4_list_t),
                             offsetof(struct align_tlv_ipv4_list_t, value),
                             offsetof(tlv_ipv4_list_t, raw),
                             sizeof(tlv_schema_number_t),
                             offsetof(struct align_tlv_schema_number_t, value),
                             offsetof(tlv_schema_number_t, schema),
                             offsetof(tlv_schema_number_t, representation),
                             sizeof(tlv_structure_codec_t),
                             offsetof(struct align_tlv_structure_codec_t, value),
                             offsetof(tlv_structure_codec_t, context),
                             offsetof(tlv_structure_codec_t, format),
                             offsetof(tlv_structure_codec_t, schema),
                             offsetof(tlv_structure_codec_t, max_depth),
                             offsetof(tlv_structure_codec_t, max_elements),
                             offsetof(tlv_structure_codec_t, decode),
                             offsetof(tlv_structure_codec_t, encode),
                             sizeof(tlv_generator_candidate_t),
                             offsetof(struct align_tlv_generator_candidate_t, value),
                             offsetof(tlv_generator_candidate_t, tag),
                             offsetof(tlv_generator_candidate_t, min_value_size),
                             offsetof(tlv_generator_candidate_t, max_value_size),
                             sizeof(tlv_generator_options_t),
                             offsetof(struct align_tlv_generator_options_t, value),
                             offsetof(tlv_generator_options_t, seed),
                             offsetof(tlv_generator_options_t, case_index),
                             offsetof(tlv_generator_options_t, max_elements),
                             offsetof(tlv_generator_options_t, max_depth),
                             offsetof(tlv_generator_options_t, max_value_size),
                             offsetof(tlv_generator_options_t, max_case_size),
                             offsetof(tlv_generator_options_t, candidates),
                             offsetof(tlv_generator_options_t, candidate_count),
                             TLV_GENERATOR_VERSION,
                             TLV_STRUCTURE_MAX_DEPTH,
                             TLV_TEXT_ASCII_PRINTABLE,
                             TLV_TEXT_ASCII_ALNUM};
    return index < sizeof values / sizeof values[0] ? values[index] : SIZE_MAX;
}
