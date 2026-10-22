// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

#include "tlv/formats/fixed.h"
#include "tlv/schema/query.h"
#include <stddef.h>

/* C99 alignment probe: a leading byte exposes the next member's alignment. */
struct identifier_alignment {
    char                   prefix;
    tlv_fixed_identifier_t value;
};
struct length_alignment {
    char               prefix;
    tlv_fixed_length_t value;
};
struct format_alignment {
    char               prefix;
    tlv_fixed_format_t value;
};
struct path_alignment {
    char                  prefix;
    tlv_diagnostic_path_t value;
};
struct schema_alignment {
    char                    prefix;
    tlv_schema_diagnostic_t value;
};
struct schema_query_alignment {
    char                          prefix;
    tlv_schema_query_diagnostic_t value;
};

struct align_tlv_location_t {
    char           prefix;
    tlv_location_t value;
};
struct align_tlv_diagnostic_t {
    char             prefix;
    tlv_diagnostic_t value;
};
struct align_tlv_schema_definition_location_t {
    char                             prefix;
    tlv_schema_definition_location_t value;
};

struct align_tlv_reader_detail_t {
    char                prefix;
    tlv_reader_detail_t value;
};
struct align_tlv_reader_diagnostic_t {
    char                    prefix;
    tlv_reader_diagnostic_t value;
};
struct align_tlv_query_diagnostic_t {
    char                   prefix;
    tlv_query_diagnostic_t value;
};

struct align_tlv_schema_detail {
    char                prefix;
    tlv_schema_detail_t value;
};

struct align_tlv_codec_detail {
    char               prefix;
    tlv_codec_detail_t value;
};

struct align_tlv_codec_diagnostic {
    char                   prefix;
    tlv_codec_diagnostic_t value;
};

size_t opentlv_test_fixed_abi(size_t index) {
    const size_t values[] = {sizeof(tlv_fixed_identifier_t),
                             offsetof(struct identifier_alignment, value),
                             offsetof(tlv_fixed_identifier_t, size),
                             sizeof(tlv_fixed_length_t),
                             offsetof(struct length_alignment, value),
                             offsetof(tlv_fixed_length_t, size),
                             offsetof(tlv_fixed_length_t, byte_order),
                             sizeof(tlv_fixed_format_t),
                             offsetof(struct format_alignment, value),
                             offsetof(tlv_fixed_format_t, identifier),
                             offsetof(tlv_fixed_format_t, length),
                             offsetof(tlv_fixed_format_t, element_order),
                             offsetof(tlv_fixed_format_t, length_scope),
                             sizeof(tlv_location_t),
                             offsetof(struct align_tlv_location_t, value),
                             offsetof(tlv_location_t, domain),
                             offsetof(tlv_location_t, kind),
                             offsetof(tlv_location_t, begin),
                             offsetof(tlv_location_t, end),
                             sizeof(tlv_diagnostic_t),
                             offsetof(struct align_tlv_diagnostic_t, value),
                             offsetof(tlv_diagnostic_t, code),
                             offsetof(tlv_diagnostic_t, severity),
                             offsetof(tlv_diagnostic_t, location),
                             offsetof(tlv_diagnostic_t, expected),
                             offsetof(tlv_diagnostic_t, actual),
                             offsetof(tlv_diagnostic_t, contexts),
                             offsetof(tlv_diagnostic_t, has_path),
                             offsetof(tlv_diagnostic_t, path),
                             sizeof(tlv_schema_definition_location_t),
                             offsetof(struct align_tlv_schema_definition_location_t, value),
                             offsetof(tlv_schema_definition_location_t, kind),
                             offsetof(tlv_schema_definition_location_t, owner),
                             offsetof(tlv_schema_definition_location_t, index),
                             sizeof(tlv_diagnostic_path_t),
                             offsetof(struct path_alignment, value),
                             offsetof(tlv_diagnostic_path_t, tags),
                             offsetof(tlv_diagnostic_path_t, length),
                             offsetof(tlv_diagnostic_path_t, omitted),
                             sizeof(tlv_schema_diagnostic_t),
                             offsetof(struct schema_alignment, value),
                             offsetof(tlv_schema_diagnostic_t, diagnostic),
                             offsetof(tlv_schema_diagnostic_t, cause),
                             offsetof(tlv_schema_diagnostic_t, detail),
                             sizeof(tlv_reader_detail_t),
                             offsetof(struct align_tlv_reader_detail_t, value),
                             offsetof(tlv_reader_detail_t, operation),
                             offsetof(tlv_reader_detail_t, has_tag),
                             offsetof(tlv_reader_detail_t, tag),
                             offsetof(tlv_reader_detail_t, has_tag_offset),
                             offsetof(tlv_reader_detail_t, tag_offset),
                             offsetof(tlv_reader_detail_t, has_length_offset),
                             offsetof(tlv_reader_detail_t, length_offset),
                             offsetof(tlv_reader_detail_t, has_value_offset),
                             offsetof(tlv_reader_detail_t, value_offset),
                             offsetof(tlv_reader_detail_t, has_declared_length),
                             offsetof(tlv_reader_detail_t, declared_length),
                             offsetof(tlv_reader_detail_t, has_raw_length),
                             offsetof(tlv_reader_detail_t, raw_length),
                             offsetof(tlv_reader_detail_t, has_available),
                             offsetof(tlv_reader_detail_t, available),
                             offsetof(tlv_reader_detail_t, has_enclosing_end),
                             offsetof(tlv_reader_detail_t, enclosing_end),
                             offsetof(tlv_reader_detail_t, has_required),
                             offsetof(tlv_reader_detail_t, required),
                             sizeof(tlv_reader_diagnostic_t),
                             offsetof(struct align_tlv_reader_diagnostic_t, value),
                             offsetof(tlv_reader_diagnostic_t, diagnostic),
                             offsetof(tlv_reader_diagnostic_t, detail),
                             sizeof(tlv_query_diagnostic_t),
                             offsetof(struct align_tlv_query_diagnostic_t, value),
                             offsetof(tlv_query_diagnostic_t, diagnostic),
                             offsetof(tlv_query_diagnostic_t, kind),
                             offsetof(tlv_query_diagnostic_t, expression),
                             offsetof(tlv_query_diagnostic_t, limit),
                             offsetof(tlv_query_diagnostic_t, configured),
                             offsetof(tlv_query_diagnostic_t, cause),
                             offsetof(tlv_query_diagnostic_t, detail),
                             sizeof(tlv_schema_detail_t),
                             offsetof(struct align_tlv_schema_detail, value),
                             offsetof(tlv_schema_detail_t, kind),
                             offsetof(tlv_schema_detail_t, tag),
                             offsetof(tlv_schema_detail_t, definition),
                             offsetof(tlv_schema_detail_t, field),
                             offsetof(tlv_schema_detail_t, is_group),
                             offsetof(tlv_schema_detail_t, has_occurs),
                             offsetof(tlv_schema_detail_t, min_occurs),
                             offsetof(tlv_schema_detail_t, max_occurs),
                             offsetof(tlv_schema_detail_t, occurs),
                             offsetof(tlv_schema_detail_t, has_length),
                             offsetof(tlv_schema_detail_t, min_length),
                             offsetof(tlv_schema_detail_t, max_length),
                             offsetof(tlv_schema_detail_t, actual_length),
                             offsetof(tlv_schema_detail_t, has_form),
                             offsetof(tlv_schema_detail_t, expected_form),
                             offsetof(tlv_schema_detail_t, actual_constructed),
                             offsetof(tlv_schema_detail_t, length_multiple),
                             offsetof(tlv_schema_detail_t, length_flags),
                             sizeof(tlv_codec_detail_t),
                             offsetof(struct align_tlv_codec_detail, value),
                             offsetof(tlv_codec_detail_t, operation),
                             offsetof(tlv_codec_detail_t, reported),
                             offsetof(tlv_codec_detail_t, violation),
                             offsetof(tlv_codec_detail_t, reason),
                             offsetof(tlv_codec_detail_t, representation),
                             offsetof(tlv_codec_detail_t, cause),
                             offsetof(tlv_codec_detail_t, detail),
                             sizeof(tlv_codec_diagnostic_t),
                             offsetof(struct align_tlv_codec_diagnostic, value),
                             offsetof(tlv_codec_diagnostic_t, diagnostic),
                             offsetof(tlv_codec_diagnostic_t, codec),
                             sizeof(tlv_schema_query_diagnostic_t),
                             offsetof(struct schema_query_alignment, value),
                             offsetof(tlv_schema_query_diagnostic_t, rule),
                             offsetof(tlv_schema_query_diagnostic_t, query)};
    return index < sizeof values / sizeof values[0] ? values[index] : SIZE_MAX;
}
