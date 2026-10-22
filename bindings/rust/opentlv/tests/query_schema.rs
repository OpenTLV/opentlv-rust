// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
use opentlv::{
    Error, Format, ProgramOptions, QueryConversion, QueryDecoded, QueryProgram, QueryProvider,
    QueryRule, QuerySchema, QuerySchemaLimits,
};

const WIRE: &[u8] = &[0x70, 6, 0x5a, 1, 1, 0x50, 1, 2, 0x5a, 1, 3];

fn rule(context: &str, assertion: &str) -> QueryRule {
    QueryRule::new(
        QueryProgram::compile(context, &ProgramOptions::default()).unwrap(),
        QueryProgram::compile(assertion, &ProgramOptions::default()).unwrap(),
    )
}

#[test]
fn contexts_are_relative_and_empty_selection_and_rules_succeed() {
    let schema = QuerySchema::new(
        vec![rule("//70", "count(./5A) = 1"), rule("//5B", "1 = 0")],
        Format::Ber,
    );
    schema
        .validate_buffer(WIRE, QuerySchemaLimits::default())
        .unwrap();
    // No rules skips input validation and resource allocation, exactly as C does.
    QuerySchema::new(Vec::new(), Format::Ber)
        .validate_buffer(
            &[0xff],
            QuerySchemaLimits {
                depth: usize::MAX,
                contexts: usize::MAX,
                ..QuerySchemaLimits::default()
            },
        )
        .unwrap();
}

#[test]
fn false_assertion_preserves_owned_rule_path_offset_and_expectation() {
    let failure = {
        let input = WIRE.to_vec();
        let schema = QuerySchema::new(
            vec![
                rule("//70", "1 = 1"),
                rule("//5A", "num(.) = 3").named("three-required").unwrap(),
            ],
            Format::Ber,
        );
        schema
            .validate_buffer(&input, QuerySchemaLimits::default())
            .unwrap_err()
    };
    assert_eq!(failure.rule, 1);
    assert_eq!(failure.failure.error, Error::Schema);
    let detail = failure.schema.unwrap();
    assert_eq!(detail.kind, opentlv::SchemaIssue::Assertion);
    assert_eq!(detail.tag.as_bytes(), &[0x5a]);
    assert_eq!(detail.field.as_deref(), Some("three-required"));
    // Common evidence is read once, from the Query failure.
    let common = &failure.failure;
    assert_eq!(common.path().unwrap().len(), 1);
    assert_eq!(common.path().unwrap()[0], [0x70]);
    assert_eq!(common.source_offset(), Some(2));
    assert_eq!(common.expected(), Some("contextual Query assertion true"));
    assert_eq!(common.actual(), None);
}

#[test]
fn native_context_work_and_node_limits_and_overflow_are_preserved() {
    let schema = QuerySchema::new(vec![rule("//5A", "1 = 1")], Format::Ber);
    let failure = schema
        .validate_buffer(
            WIRE,
            QuerySchemaLimits {
                contexts: 1,
                ..QuerySchemaLimits::default()
            },
        )
        .unwrap_err();
    assert_eq!(failure.failure.error, Error::Limit);
    let limit = failure.failure.limit.as_deref().unwrap();
    assert_eq!((&*limit.name, limit.configured), ("schema-contexts", 1));
    assert!(failure.schema.is_none());
    for limits in [
        QuerySchemaLimits {
            work: 1,
            ..QuerySchemaLimits::default()
        },
        QuerySchemaLimits {
            nodes: 1,
            ..QuerySchemaLimits::default()
        },
    ] {
        assert_eq!(
            schema
                .validate_buffer(WIRE, limits)
                .unwrap_err()
                .failure
                .error,
            Error::Limit
        );
    }
    for limits in [
        QuerySchemaLimits {
            depth: usize::MAX,
            ..QuerySchemaLimits::default()
        },
        QuerySchemaLimits {
            contexts: usize::MAX,
            ..QuerySchemaLimits::default()
        },
    ] {
        assert_eq!(
            schema
                .validate_buffer(WIRE, limits)
                .unwrap_err()
                .failure
                .error,
            Error::Overflow
        );
    }
    schema
        .validate_buffer(WIRE, QuerySchemaLimits::default())
        .unwrap();
}

#[test]
fn original_reader_and_invalid_rule_failures_are_not_assertion_failures() {
    let schema = QuerySchema::new(vec![rule("//5A", "1 = 1")], Format::Ber);
    let failure = schema
        .validate_buffer(&[0x5a, 2, 0], QuerySchemaLimits::default())
        .unwrap_err();
    assert_eq!(failure.failure.kind, opentlv::QueryErrorKind::Reader);
    assert!(failure.failure.reader.is_some());
    assert!(failure.schema.is_none());
    for (selector, assertion) in [("count(//5A)", "1 = 1"), ("//5A", "//5A")] {
        let schema = QuerySchema::new(vec![rule(selector, assertion)], Format::Ber);
        assert_eq!(
            schema
                .validate_buffer(WIRE, QuerySchemaLimits::default())
                .unwrap_err()
                .failure
                .error,
            Error::InvalidValue
        );
    }
    assert!(rule("//5A", "1 = 1").named("invalid\0name").is_err());
}

#[test]
fn schema_retains_programs_and_providers_and_preserves_codec_errors() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let mut options = ProgramOptions::default();
    options.providers.push(QueryProvider::new(
        QueryConversion::Num,
        123,
        0,
        move |bytes, metadata| {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            assert_eq!(metadata.unwrap().tag, &[0x5a]);
            Ok(QueryDecoded::Integer(i64::from(bytes[0])))
        },
    ));
    let context = QueryProgram::compile("//5A", &options).unwrap();
    let assertion = QueryProgram::compile("num(.) > 0", &options).unwrap();
    let schema = QuerySchema::new(
        vec![QueryRule::new(context.clone(), assertion.clone())],
        Format::Ber,
    );
    drop(options);
    drop(context);
    drop(assertion);
    schema
        .validate_buffer(WIRE, QuerySchemaLimits::default())
        .unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 2);
    drop(schema);
    assert_eq!(std::sync::Arc::strong_count(&calls), 1);
    for panic in [false, true] {
        let mut options = ProgramOptions::default();
        options.providers.push(QueryProvider::new(
            QueryConversion::Num,
            124,
            0,
            move |_, _| {
                if panic {
                    panic!("schema provider panic");
                }
                Err(Error::Unsupported)
            },
        ));
        let schema = QuerySchema::new(
            vec![QueryRule::new(
                QueryProgram::compile("//5A", &options).unwrap(),
                QueryProgram::compile("num(.) > 0", &options).unwrap(),
            )],
            Format::Ber,
        );
        let failure = schema
            .validate_buffer(WIRE, QuerySchemaLimits::default())
            .unwrap_err();
        assert_eq!(
            failure.failure.codec_detail.as_ref().unwrap().reported,
            if panic {
                Error::Callback.code()
            } else {
                Error::Unsupported.code()
            }
        );
        assert!(failure.schema.is_none());
    }
}

#[cfg(feature = "document")]
#[test]
fn root_only_document_snapshot_uses_the_frame_at_depth_zero() {
    use opentlv::Document;
    let document = Document::parse(&[0x70, 0], Format::Ber, 0, 1).unwrap();
    let schema = QuerySchema::new(vec![rule("//70", "@len = 0")], Format::Ber);
    schema
        .validate_document(
            &document,
            QuerySchemaLimits {
                depth: 0,
                nodes: 1,
                contexts: 1,
                ..QuerySchemaLimits::default()
            },
        )
        .unwrap();
    let program = QueryProgram::compile("value(//70)", &ProgramOptions::default()).unwrap();
    let mut execution = program.execution(0, 1, 100000, true).unwrap();
    execution.evaluate_document(&document, None, None).unwrap();
}

#[cfg(feature = "document")]
#[test]
fn document_supports_reverse_axes_and_bounded_canonical_values() {
    use opentlv::Document;
    let document = Document::parse(WIRE, Format::Ber, 64, 1024).unwrap();
    let schema = QuerySchema::new(vec![rule("//50", "count(following::5A) = 1")], Format::Ber);
    assert_eq!(
        schema
            .validate_buffer(WIRE, QuerySchemaLimits::default())
            .unwrap_err()
            .failure
            .error,
        Error::Unsupported
    );
    schema
        .validate_document(&document, QuerySchemaLimits::default())
        .unwrap();
    let schema = QuerySchema::new(
        vec![rule("//70", "value(.) = x'5A0101500102'")],
        Format::Ber,
    );
    schema
        .validate_document(&document, QuerySchemaLimits::default())
        .unwrap();
    assert_eq!(
        schema
            .validate_document(
                &document,
                QuerySchemaLimits {
                    value_capacity: Some(0),
                    ..QuerySchemaLimits::default()
                }
            )
            .unwrap_err()
            .failure
            .error,
        Error::Limit
    );
    let failure = QuerySchema::new(
        vec![rule("//5A", "num(.) = 3").named("three-required").unwrap()],
        Format::Ber,
    )
    .validate_document(&document, QuerySchemaLimits::default())
    .unwrap_err();
    drop(document);
    let detail = failure.schema.unwrap();
    assert_eq!(
        failure.failure.location.kind,
        opentlv::LocationKind::Unknown
    );
    assert_eq!(detail.tag.as_bytes(), &[0x5a]);
    assert_eq!(failure.failure.path().unwrap()[0], [0x70]);
    assert_eq!(detail.field.as_deref(), Some("three-required"));
    assert_eq!(
        failure.failure.expected(),
        Some("contextual Query assertion true")
    );
}

#[cfg(feature = "document")]
#[test]
fn document_source_locations_support_global_axes() {
    let document = opentlv::Document::parse_with_source_locations(
        &[0x50, 0, 0x57, 1, 0xaa],
        Format::Ber,
        4,
        4,
    )
    .unwrap();
    let schema = QuerySchema::new(
        vec![rule(
            "//50",
            "count(following::57[@offset = 2 and @hlen = 2]) = 1",
        )],
        Format::Ber,
    );
    schema
        .validate_document(&document, QuerySchemaLimits::default())
        .unwrap();
    let plain = opentlv::Document::parse(&[0x50, 0, 0x57, 1, 0xaa], Format::Ber, 4, 4).unwrap();
    assert!(schema
        .validate_document(&plain, QuerySchemaLimits::default())
        .is_err());
}

#[test]
fn deep_assertion_preserves_outermost_path_and_omitted_count() {
    let schema = QuerySchema::new(vec![rule("//5A", "1 = 0")], Format::Ber);
    let mut wire = vec![0x5a, 0];
    for level in 0..35 {
        let mut enclosing = vec![if level == 34 { 0x70 } else { 0x30 }, wire.len() as u8];
        enclosing.extend(wire);
        wire = enclosing;
    }
    let failure = schema
        .validate_buffer(&wire, QuerySchemaLimits::default())
        .unwrap_err();
    assert!(failure.schema.is_some());
    let path = failure.failure.path().unwrap();
    assert_eq!(path.len(), 32);
    assert_eq!(path[0], [0x70]);
    assert!(path[1..].iter().all(|tag| tag == &[0x30]));
    assert_eq!(failure.failure.path_omitted(), 3);
}
