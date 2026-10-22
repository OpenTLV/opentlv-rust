// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Integration tests for the length and structural schema wrappers.

use opentlv::emv::Context;
use opentlv::{
    Error, Format, Kind, LengthRule, LengthSchema, StructureRule, StructureSchema, Tag,
    ValidationLimits,
};

fn tag(bytes: &[u8]) -> Tag {
    Tag::from_bytes(bytes)
}

fn limits() -> ValidationLimits {
    ValidationLimits::default()
}

#[test]
fn length_schema_finds_and_validates() {
    let schema = LengthSchema::new([
        LengthRule::new(tag(&[0x01]), 2, 4),
        LengthRule::exact(tag(&[0x02]), 1),
    ]);
    assert_eq!(schema.len(), 2);
    assert_eq!(
        schema.find(&tag(&[0x02])),
        Some(LengthRule::exact(tag(&[0x02]), 1))
    );
    assert_eq!(schema.find(&tag(&[0x03])), None);

    assert_eq!(schema.validate_length(&tag(&[0x01]), 2), Ok(()));
    assert_eq!(schema.validate_length(&tag(&[0x01]), 4), Ok(()));
    assert_eq!(schema.validate_length(&tag(&[0x01]), 1), Err(Error::Schema));
    assert_eq!(schema.validate_length(&tag(&[0x01]), 5), Err(Error::Schema));
    assert_eq!(schema.validate_length(&tag(&[0x03]), 1), Err(Error::Schema));
}

#[test]
fn empty_length_schema_knows_no_tag() {
    let schema = LengthSchema::new([]);
    assert!(schema.is_empty());
    assert_eq!(schema.validate_length(&tag(&[0x01]), 0), Err(Error::Schema));
}

#[test]
fn emv_length_schema_uses_the_builtin_dictionary() {
    let schema = LengthSchema::emv();
    assert!(!schema.is_empty());
    let amount = tag(&[0x9F, 0x02]);
    assert_eq!(
        schema.find(&amount),
        Some(LengthRule::exact(amount.clone(), 6))
    );
    assert_eq!(schema.validate_length(&amount, 6), Ok(()));
    assert_eq!(schema.validate_length(&amount, 5), Err(Error::Schema));
    // Contexts are explicit: 9F02 is not part of the biometric context.
    assert_eq!(LengthSchema::emv_for(Context::Bht).find(&amount), None);
}

/// Tag 01 is required exactly once with two bytes, tag 02 is an optional
/// primitive, and tag 30 holds one or two items of tag 11.
fn message_schema() -> StructureSchema {
    let items = StructureSchema::new(
        [StructureRule::new(tag(&[0x11])).length(1, 2).occurs(1, 2)],
        false,
    );
    StructureSchema::new(
        [
            StructureRule::new(tag(&[0x01]))
                .length(2, 2)
                .required_once(),
            StructureRule::new(tag(&[0x02])).kind(Kind::Primitive),
            StructureRule::new(tag(&[0x30])).children(items),
        ],
        false,
    )
}

#[test]
fn structure_schema_accepts_conforming_data() {
    let data = [
        0x01, 0x02, 0xAA, 0xBB, // required tag
        0x02, 0x00, // optional primitive
        0x30, 0x07, 0x11, 0x01, 0x01, 0x11, 0x02, 0x02, 0x03, // container with two items
    ];
    assert_eq!(
        message_schema().validate(&data, Format::Ber, &limits()),
        Ok(())
    );
}

#[test]
fn missing_required_field_reports_schema_missing() {
    let err = message_schema()
        .validate(&[0x02, 0x00], Format::Ber, &limits())
        .unwrap_err();
    assert_eq!(err.error, Error::Schema);
    // Reported at the end of the parent's value: here the end of the input.
    assert_eq!(err.offset, Some(2));
}

#[test]
fn duplicate_of_a_once_only_tag_is_a_schema_error() {
    let data = [0x01, 0x02, 0xAA, 0xBB, 0x01, 0x02, 0xCC, 0xDD];
    let err = message_schema()
        .validate(&data, Format::Ber, &limits())
        .unwrap_err();
    assert_eq!(err.error, Error::Schema);
}

#[test]
fn length_violation_reports_invalid_length_at_the_element() {
    let data = [0x01, 0x03, 0xAA, 0xBB, 0xCC];
    let err = message_schema()
        .validate(&data, Format::Ber, &limits())
        .unwrap_err();
    assert_eq!((err.error, err.offset), (Error::Schema, Some(0)));
}

#[test]
fn unknown_tag_is_rejected_unless_allowed() {
    let strict = StructureSchema::new([StructureRule::new(tag(&[0x01]))], false);
    let lenient = StructureSchema::new([StructureRule::new(tag(&[0x01]))], true);
    let data = [0x01, 0x00, 0x09, 0x00];
    let err = strict.validate(&data, Format::Ber, &limits()).unwrap_err();
    assert_eq!((err.error, err.offset), (Error::Schema, Some(2)));
    assert_eq!(lenient.validate(&data, Format::Ber, &limits()), Ok(()));
}

#[test]
fn kind_mismatch_is_a_schema_error() {
    // 0x30 is constructed in BER, but the rule demands a primitive.
    let schema = StructureSchema::new(
        [StructureRule::new(tag(&[0x30])).kind(Kind::Primitive)],
        false,
    );
    let err = schema
        .validate(&[0x30, 0x00], Format::Ber, &limits())
        .unwrap_err();
    assert_eq!(err.error, Error::Schema);
}

#[test]
fn ber_constructed_bit_controls_nesting() {
    // The child claims five bytes but only zero remain inside the container.
    let opaque = StructureSchema::new([], true);
    let data = [0x30, 0x02, 0x11, 0x05];
    // BER leaves primitive contents opaque.
    assert_eq!(
        opaque.validate(&[0x10, 2, 0x11, 5], Format::Ber, &limits()),
        Ok(())
    );
    // In BER the container is traversed and its malformed child is reported.
    assert!(opaque.validate(&data, Format::Ber, &limits()).is_err());
}

#[test]
fn limits_are_enforced() {
    let schema = StructureSchema::new([], true);
    let data = [0x01, 0x00, 0x02, 0x00];
    let tight = ValidationLimits {
        max_depth: 4,
        max_elements: 1,
    };
    let err = schema.validate(&data, Format::Ber, &tight).unwrap_err();
    assert_eq!(err.error, Error::Limit);
}

#[test]
fn malformed_framing_is_reported() {
    let schema = StructureSchema::new([], true);
    let err = schema
        .validate(&[0x01, 0x05, 0x00], Format::Ber, &limits())
        .unwrap_err();
    assert!(matches!(err.error, Error::Truncated | Error::InvalidLength));
}

#[test]
fn emv_structure_schema_validates_an_fci() {
    let schema = StructureSchema::emv();
    let fci = [
        0x6F, 0x09, 0x84, 0x07, 0xA0, 0x00, 0x00, 0x00, 0x03, 0x10, 0x10,
    ];
    assert_eq!(schema.validate(&fci, Format::Ber, &limits()), Ok(()));

    // DF Name is mandatory.
    let err = schema
        .validate(&[0x6F, 0x00], Format::Ber, &limits())
        .unwrap_err();
    assert_eq!(err.error, Error::Schema);

    // Application Label is not allowed directly under the FCI template.
    let bad = [0x6F, 0x0A, 0x84, 0x05, 1, 2, 3, 4, 5, 0x50, 0x01, 0x41];
    let err = schema.validate(&bad, Format::Ber, &limits()).unwrap_err();
    assert_eq!((err.error, err.offset), (Error::Schema, Some(9)));
}

#[test]
fn emv_structure_schema_validates_a_gpo_response() {
    let schema = StructureSchema::emv();
    let gpo = [
        0x77, 0x0A, 0x82, 0x02, 0x20, 0x00, 0x94, 0x04, 0x08, 0x01, 0x01, 0x00,
    ];
    assert_eq!(schema.validate(&gpo, Format::Ber, &limits()), Ok(()));
    let missing_afl = [0x77, 0x04, 0x82, 0x02, 0x20, 0x00];
    let err = schema
        .validate(&missing_afl, Format::Ber, &limits())
        .unwrap_err();
    assert_eq!(err.error, Error::Schema);
}

#[test]
fn schemas_can_be_shared_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LengthSchema>();
    assert_send_sync::<StructureSchema>();

    let schema = std::sync::Arc::new(message_schema());
    let handle = {
        let schema = schema.clone();
        std::thread::spawn(move || {
            schema.validate(&[0x01, 0x02, 0xAA, 0xBB], Format::Ber, &limits())
        })
    };
    assert_eq!(handle.join().unwrap(), Ok(()));
}

#[test]
fn moving_a_schema_keeps_nested_tables_valid() {
    let moved = *Box::new(message_schema());
    let schemas = vec![moved, message_schema()];
    let data = [0x01, 0x02, 0xAA, 0xBB, 0x30, 0x03, 0x11, 0x01, 0x01];
    for schema in schemas {
        assert_eq!(schema.validate(&data, Format::Ber, &limits()), Ok(()));
    }
}

#[test]
fn schemas_hold_tags_longer_than_any_built_in_format_accepts() {
    let long = tag(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let schema = LengthSchema::new([LengthRule::exact(long.clone(), 3)]);
    assert_eq!(schema.find(&long), Some(LengthRule::exact(long.clone(), 3)));
    assert_eq!(schema.validate_length(&long, 3), Ok(()));
    assert_eq!(schema.validate_length(&long, 4), Err(Error::Schema));
    // A tag that differs only in its last byte is a different tag.
    let other = tag(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13]);
    assert_eq!(schema.find(&other), None);
    // The schema owns its tags, so it outlives the tag it was built from.
    drop(long);
    assert_eq!(schema.len(), 1);
}

#[test]
fn owned_field_references_survive_growth_and_schema_moves() {
    // More entries than the initial Vec capacity; fields must be finalized before borrowing.
    let rules = (0x80..=0x9e).map(|id| StructureRule::new(tag(&[id])).required_once());
    let schema = StructureSchema::new(rules, false);
    let wire: Vec<u8> = (0x80..=0x9e).flat_map(|id| [id, 0]).collect();
    std::thread::spawn(move || {
        assert!(schema.validate(&wire, Format::Ber, &limits()).is_ok());
        assert!(schema.validate(&wire[2..], Format::Ber, &limits()).is_err());
    })
    .join()
    .unwrap();
}
#[test]
fn length_schema_retains_native_endpoint_and_multiple_constraints() {
    let tag = Tag::from_bytes(&[1]);
    let mut rule = LengthRule::new(tag.clone(), 2, 8);
    rule.flags = 1;
    rule.length_multiple = 2;
    let schema = LengthSchema::new([rule]);
    assert_eq!(schema.find(&tag).unwrap().flags, 1);
    assert_eq!(schema.find(&tag).unwrap().length_multiple, 2);
    assert!(schema.validate_length(&tag, 2).is_ok());
    assert!(schema.validate_length(&tag, 8).is_ok());
    assert_eq!(schema.validate_length(&tag, 4), Err(Error::Schema));
}
#[test]
fn sequence_and_alternative_groups_delegate_to_c_after_move() {
    use opentlv::{SchemaOrder, StructureGroup};
    let limits = ValidationLimits::default();
    let sequence = StructureSchema::with_constraints(
        [
            StructureRule::new(Tag::from_bytes(&[1])),
            StructureRule::new(Tag::from_bytes(&[2])),
        ],
        false,
        SchemaOrder::Sequence,
        [],
    );
    sequence
        .validate(&[1, 0, 2, 0], Format::Ber, &limits)
        .unwrap();
    assert_eq!(
        sequence
            .validate(&[2, 0, 1, 0], Format::Ber, &limits)
            .unwrap_err()
            .offset,
        Some(2)
    );
    let choice = StructureSchema::with_constraints(
        [
            StructureRule::new(Tag::from_bytes(&[1])).group(7),
            StructureRule::new(Tag::from_bytes(&[2])).group(7),
        ],
        false,
        SchemaOrder::Any,
        [StructureGroup {
            id: 7,
            name: None,
            min_occurs: 1,
            max_occurs: 1,
        }],
    );
    let moved = Box::new(choice);
    moved.validate(&[2, 0], Format::Ber, &limits).unwrap();
    assert_eq!(
        moved.validate(&[], Format::Ber, &limits).unwrap_err().error,
        Error::Schema
    );
    assert_eq!(
        moved
            .validate(&[1, 0, 2, 0], Format::Ber, &limits)
            .unwrap_err()
            .error,
        Error::Schema
    );
}
#[test]
fn bounded_reports_keep_total_and_owned_paths() {
    use opentlv::UnknownPolicy;
    let limits = ValidationLimits::default();
    let schema = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[1])).required_once()],
        false,
    );
    let report = schema
        .validate_diagnostics(&[2, 0], Format::Ber, &limits, UnknownPolicy::BySchema, 1)
        .unwrap();
    assert_eq!(report.total_count, 2);
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.diagnostics[0].path.as_deref() == Some(&[][..]));
    assert!([Tag::from_bytes(&[1]), Tag::from_bytes(&[2])].contains(&report.diagnostics[0].tag));
    assert_eq!(
        schema
            .validate_diagnostics(&[2, 0], Format::Ber, &limits, UnknownPolicy::BySchema, 0)
            .unwrap()
            .total_count,
        2
    );
    let report = schema
        .validate_diagnostics(&[2, 0], Format::Ber, &limits, UnknownPolicy::Allow, 5)
        .unwrap();
    drop(schema);
    assert_eq!(report.diagnostics[0].kind_name, "missing");
    assert!(report.diagnostics[0].path.as_deref() == Some(&[][..]));
    assert_eq!(report.diagnostics[0].tag, Tag::from_bytes(&[1]));
    assert_eq!(report.diagnostics[0].offset, Some(2));
    assert_eq!(
        report.diagnostics[0].location.kind,
        opentlv::LocationKind::ScopeEnd
    );
}

#[test]
fn nested_report_outlives_input_and_schema() {
    use opentlv::UnknownPolicy;
    let report = {
        let input = vec![0x30, 3, 4, 1, 42];
        let schema = StructureSchema::new(
            [
                StructureRule::new(Tag::from_bytes(&[0x30])).children(StructureSchema::new(
                    [StructureRule::new(Tag::from_bytes(&[4])).length(2, 3)],
                    false,
                )),
            ],
            false,
        );
        schema
            .validate_diagnostics(
                &input,
                Format::Ber,
                &ValidationLimits::default(),
                UnknownPolicy::BySchema,
                4,
            )
            .unwrap()
    };
    assert_eq!(report.diagnostics[0].kind_name, "length");
    assert_eq!(report.diagnostics[0].tag, Tag::from_bytes(&[4]));
    assert_eq!(
        report.diagnostics[0].path.as_deref().unwrap(),
        [Tag::from_bytes(&[0x30])]
    );
    assert_eq!(report.diagnostics[0].offset, Some(2));
}

#[test]
fn detailed_report_owns_names_and_expected_actual() {
    use opentlv::{SchemaBounds, UnknownPolicy};
    let report = {
        let input = vec![0x30, 3, 4, 1, 42];
        let schema = StructureSchema::new(
            [
                StructureRule::new(Tag::from_bytes(&[0x30])).children(StructureSchema::new(
                    [StructureRule::new(Tag::from_bytes(&[4]))
                        .length(2, 8)
                        .length_multiple(2)
                        .length_flags(1)
                        .named("payload-?")
                        .unwrap()],
                    false,
                )),
            ],
            false,
        );
        schema
            .validate_diagnostics(
                &input,
                Format::Ber,
                &ValidationLimits::default(),
                UnknownPolicy::BySchema,
                4,
            )
            .unwrap()
    };
    assert_eq!(report.total_count, 1);
    let issue = &report.diagnostics[0];
    assert_eq!(issue.field.as_deref(), Some("payload-?"));
    assert_eq!(issue.kind_name, "length");
    assert_eq!(issue.path.as_deref().unwrap(), [Tag::from_bytes(&[0x30])]);
    assert_eq!(issue.tag, Tag::from_bytes(&[4]));
    assert_eq!(issue.offset, Some(2));
    assert_eq!(
        issue.length,
        Some(SchemaBounds {
            minimum: 2,
            maximum: 8,
            actual: 1
        })
    );
    assert_eq!(issue.length_multiple, 2);
    assert_eq!(issue.length_flags, 1);
    assert!(issue.occurrences.is_none() && issue.form.is_none());
}

#[test]
fn detailed_group_report_capacity_and_wire_failure() {
    use opentlv::{SchemaBounds, SchemaOrder, StructureGroup, UnknownPolicy};
    let schema = StructureSchema::with_constraints(
        [StructureRule::new(Tag::from_bytes(&[4])).group(7)],
        false,
        SchemaOrder::Any,
        [StructureGroup {
            id: 7,
            min_occurs: 1,
            max_occurs: 1,
            name: Some(std::ffi::CString::new("choice").unwrap()),
        }],
    );
    let limits = ValidationLimits::default();
    let report = schema
        .validate_diagnostics(&[], Format::Ber, &limits, UnknownPolicy::BySchema, 1)
        .unwrap();
    let issue = &report.diagnostics[0];
    assert!(issue.is_group);
    assert_eq!(issue.field.as_deref(), Some("choice"));
    assert_eq!(
        issue.occurrences,
        Some(SchemaBounds {
            minimum: 1,
            maximum: 1,
            actual: 0
        })
    );
    assert_eq!(issue.offset, Some(0));
    assert_eq!(issue.location.kind, opentlv::LocationKind::ScopeEnd);
    let report = schema
        .validate_diagnostics(&[5, 0], Format::Ber, &limits, UnknownPolicy::BySchema, 0)
        .unwrap();
    assert_eq!(report.total_count, 2);
    assert!(report.diagnostics.is_empty());
    assert!(schema
        .validate_diagnostics(&[4, 1], Format::Ber, &limits, UnknownPolicy::BySchema, 2)
        .is_err());
}

#[test]
fn configured_fixed_schema_validation_and_reports() {
    use opentlv::{ByteOrder, FixedFormat, FixedFormatConfig, UnknownPolicy};
    let config = FixedFormatConfig::new(2, 2, ByteOrder::Little);
    let format = FixedFormat::new(&config).unwrap();
    let schema = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0, 4])).length(2, 4)],
        false,
    );
    let limits = ValidationLimits::default();
    schema
        .validate_fixed(&[0, 4, 2, 0, 1, 2], &format, &limits)
        .unwrap();
    assert!(schema
        .validate_fixed(&[0, 4, 1, 0, 42], &format, &limits)
        .is_err());
    assert_eq!(
        schema
            .validate_diagnostics_fixed(
                &[0, 4, 1, 0, 42],
                &format,
                &limits,
                UnknownPolicy::BySchema,
                1
            )
            .unwrap()
            .total_count,
        1
    );
    let report = schema
        .validate_diagnostics_fixed(
            &[0, 4, 1, 0, 42],
            &format,
            &limits,
            UnknownPolicy::BySchema,
            1,
        )
        .unwrap();
    assert_eq!(report.diagnostics[0].length.as_ref().unwrap().actual, 1);
}

#[test]
fn deep_report_preserves_outermost_path_and_omitted_count() {
    use opentlv::UnknownPolicy;
    let mut schema = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[4])).length(1, 8)],
        false,
    );
    let mut wire = vec![4, 0];
    for level in 0..35 {
        let tag = if level == 34 { 0x70 } else { 0x30 };
        schema = StructureSchema::new(
            [StructureRule::new(Tag::from_bytes(&[tag])).children(schema)],
            false,
        );
        let mut enclosing = vec![tag, wire.len() as u8];
        enclosing.extend(wire);
        wire = enclosing;
    }
    let report = schema
        .validate_diagnostics(
            &wire,
            Format::Ber,
            &ValidationLimits {
                max_depth: 40,
                max_elements: 100,
            },
            UnknownPolicy::BySchema,
            1,
        )
        .unwrap();
    drop(schema);
    drop(wire);
    let issue = &report.diagnostics[0];
    assert_eq!(issue.path.as_deref().unwrap().len(), 32);
    assert_eq!(issue.path.as_deref().unwrap()[0], Tag::from_bytes(&[0x70]));
    assert!(issue.path.as_deref().unwrap()[1..]
        .iter()
        .all(|tag| *tag == Tag::from_bytes(&[0x30])));
    assert_eq!(issue.path_omitted, 3);
    assert_eq!(issue.tag, Tag::from_bytes(&[4]));
}
