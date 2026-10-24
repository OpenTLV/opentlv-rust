// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
use opentlv::{
    Error, Format, SchemaDefinitionKind, SchemaIssue, StructureRule, StructureSchema, Tag,
    UnknownPolicy, ValidationLimits, ValueConstraint,
};

fn tag(bytes: &[u8]) -> Tag {
    Tag::from_bytes(bytes)
}

fn schema() -> StructureSchema {
    StructureSchema::new(
        [StructureRule::new(tag(&[0x70]))
            .required_once()
            .children(StructureSchema::new(
                [StructureRule::new(tag(&[0x5a]))
                    .length(1, 2)
                    .required_once()],
                false,
            ))],
        false,
    )
}

#[test]
fn checked_schema_validates_like_the_schema_without_rechecking() {
    let schema = schema();
    schema.check_definition().unwrap();
    let checked = schema.check().unwrap();
    assert!(std::ptr::eq(checked.schema(), &schema));
    let limits = ValidationLimits::default();
    for input in [
        &[0x70, 3, 0x5a, 1, 1][..],
        &[0x70, 0],
        &[0x70, 4, 0x5a, 2, 1],
        &[0x70, 3, 0x5a, 3, 1, 2, 3],
    ] {
        assert_eq!(
            checked.validate(input, Format::Ber, &limits),
            schema.validate(input, Format::Ber, &limits)
        );
    }
    let failure = checked
        .validate(&[0x70, 0], Format::Ber, &limits)
        .unwrap_err();
    assert_eq!(failure.error, Error::Schema);
    assert_eq!(failure.kind, SchemaIssue::Missing);
    let input = [0x70, 7, 0x5a, 0, 0x5a, 1, 1, 0x71, 0];
    let report = checked
        .validate_diagnostics(&input, Format::Ber, &limits, UnknownPolicy::Allow, 4)
        .unwrap();
    let direct = schema
        .validate_diagnostics(&input, Format::Ber, &limits, UnknownPolicy::Allow, 4)
        .unwrap();
    assert_eq!(report.total_count, direct.total_count);
    assert!(report.total_count >= 2);
    let kinds = |report: &opentlv::SchemaDiagnosticReport| {
        report
            .diagnostics
            .iter()
            .map(|item| (item.kind, item.offset))
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds(&report), kinds(&direct));
    // Input failures still come from the Reader.
    assert_eq!(
        checked
            .validate(&[0x70, 5, 0x5a], Format::Ber, &limits)
            .unwrap_err()
            .error,
        Error::Truncated
    );
}

#[test]
fn definition_errors_are_reported_before_any_input() {
    let invalid = StructureSchema::new(
        [
            StructureRule::new(tag(&[0x01])),
            StructureRule::new(tag(&[0x02])).length(5, 1),
        ],
        false,
    );
    let failure = invalid.check_definition().unwrap_err();
    assert_eq!(failure.error, Error::InvalidSchema);
    assert_eq!(failure.definition_kind, SchemaDefinitionKind::Rule);
    assert_eq!(failure.definition_index, 1);
    assert_eq!(failure.offset, None);
    let prepared = invalid.check().unwrap_err();
    assert_eq!(prepared, failure);
    // Child tables are checked too, before input.
    let nested = StructureSchema::new(
        [
            StructureRule::new(tag(&[0x70])).children(StructureSchema::new(
                [StructureRule::new(tag(&[0x5a])).length(2, 1)],
                false,
            )),
        ],
        false,
    );
    assert_eq!(nested.check().unwrap_err().error, Error::InvalidSchema);
    assert_eq!(
        nested
            .validate(&[], Format::Ber, &ValidationLimits::default())
            .unwrap_err()
            .error,
        Error::InvalidSchema
    );
    StructureSchema::emv().check().unwrap();
}

#[test]
fn value_constraints_check_definitions_and_validate_decoded_values() {
    let any = ValueConstraint::any();
    assert_eq!(any.validate(i64::MIN), Ok(()));
    let range = ValueConstraint::range(-1, 1).unwrap();
    assert_eq!(range.validate(1), Ok(()));
    assert_eq!(range.validate(2), Err(Error::Schema));
    assert_eq!(
        ValueConstraint::range(1, 0).unwrap_err(),
        Error::InvalidSchema
    );
    assert_eq!(range.name(0), None);
    let allowed = ValueConstraint::allowed([3, 5]).unwrap();
    assert_eq!(allowed.validate(5), Ok(()));
    assert_eq!(allowed.validate(4), Err(Error::Schema));
    assert_eq!(allowed.name(5), None);
    assert_eq!(
        ValueConstraint::allowed([]).unwrap().validate(0),
        Err(Error::Schema)
    );
    let named = ValueConstraint::named([(0, "red"), (1, "green")]).unwrap();
    let copy = named.clone();
    drop(named);
    assert_eq!(copy.name(1), Some("green"));
    assert_eq!(copy.name(2), None);
    assert_eq!(copy.validate(0), Ok(()));
    assert_eq!(
        ValueConstraint::named([(0, "r\0d")]).unwrap_err(),
        Error::InvalidArg
    );
}
