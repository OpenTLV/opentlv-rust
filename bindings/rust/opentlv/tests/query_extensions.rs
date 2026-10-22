// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
use opentlv::{
    ByteOrder, Definition, DefinitionRegistry, ElementOrder, Error, FixedFormatConfig, Format,
    LengthScope, OwnedFixedFormat, ProgramOptions, QueryDefinitionScope, QueryProgram,
    QueryResolver, QueryRule, QuerySchema, QuerySchemaLimits, QueryTagAdapter, QueryValue, Tag,
    TreeReader, Visit,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn semantic_tag_adapters_are_owned_and_image_compatible() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut options = ProgramOptions {
        tags: Some(
            QueryTagAdapter::new(1001)
                .with_class(|_| Ok(17))
                .with_number(move |tag| {
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(i64::from(tag[0]) + 100)
                }),
        ),
        ..ProgramOptions::default()
    };
    let program = QueryProgram::compile("//5A[class() = 17 and number() = 190]", &options).unwrap();
    assert!(QueryProgram::load(program.image(), &ProgramOptions::default()).is_err());
    let loaded = QueryProgram::load(program.image(), &options).unwrap();
    let image = program.image().to_vec();
    drop(program);
    options.tags = Some(
        QueryTagAdapter::new(1002)
            .with_class(|_| Ok(17))
            .with_number(|_| Ok(190)),
    );
    assert!(QueryProgram::load(&image, &options).is_err());
    drop(options);
    let mut reader = TreeReader::new(&[0x5a, 0], Format::Ber, 4, 4, 4, true).unwrap();
    let mut execution = loaded.execution(4, 4, 10000, true).unwrap();
    drop(loaded);
    assert!(execution.next(&mut reader).unwrap().is_some());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    drop(execution);
    assert_eq!(Arc::strong_count(&calls), 1);
}

#[test]
fn tag_errors_panics_and_missing_capabilities_stay_native() {
    for panic in [false, true] {
        let options = ProgramOptions {
            tags: Some(QueryTagAdapter::new(1003).with_number(move |_| {
                if panic {
                    panic!("tag adapter");
                }
                Err(Error::InvalidTag)
            })),
            ..ProgramOptions::default()
        };
        assert!(QueryProgram::compile("//5A[class()=1]", &options).is_err());
        let program = QueryProgram::compile("//5A[number()=1]", &options).unwrap();
        let mut execution = program.execution(4, 4, 10000, true).unwrap();
        let mut reader = TreeReader::new(&[0x5a, 0], Format::Ber, 4, 4, 4, true).unwrap();
        assert_eq!(
            execution.next(&mut reader).unwrap_err().error,
            if panic {
                Error::InvalidValue
            } else {
                Error::InvalidTag
            }
        );
    }
}

#[test]
fn dynamic_and_definition_resolvers_preserve_scope_and_ambiguity() {
    let options = ProgramOptions {
        resolver: Some(QueryResolver::new(|namespace, name| {
            assert_eq!((namespace, name), ("app", "payload"));
            Ok(Tag::from_bytes(&[0x5a]))
        })),
        ..ProgramOptions::default()
    };
    let program = QueryProgram::compile("//app:payload", &options).unwrap();
    drop(options);
    let mut reader = TreeReader::new(&[0x5a, 0], Format::Ber, 4, 4, 4, true).unwrap();
    assert!(program
        .execution(4, 4, 10000, true)
        .unwrap()
        .next(&mut reader)
        .unwrap()
        .is_some());
    let registry = Arc::new(DefinitionRegistry::new([Definition::new(Tag::from_bytes(
        &[0x5a],
    ))
    .named("payload")
    .unwrap()]));
    let options = ProgramOptions {
        resolver: Some(
            QueryResolver::definitions(vec![
                QueryDefinitionScope {
                    namespace: "a".into(),
                    definitions: registry.clone(),
                },
                QueryDefinitionScope {
                    namespace: "b".into(),
                    definitions: registry.clone(),
                },
            ])
            .unwrap(),
        ),
        ..ProgramOptions::default()
    };
    drop(registry);
    assert!(QueryProgram::compile("//a:payload", &options).is_ok());
    assert!(QueryProgram::compile("//payload", &options).is_err());
    assert!(QueryProgram::compile("//a:unknown", &options).is_err());
}

#[test]
fn changing_resolver_output_with_equal_size_is_rejected() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut options = ProgramOptions {
        resolver: Some(QueryResolver::new(move |_, _| {
            Ok(Tag::from_bytes(&[
                if counter.fetch_add(1, Ordering::Relaxed) == 0 {
                    0x5a
                } else {
                    0x5b
                },
            ]))
        })),
        ..ProgramOptions::default()
    };
    let failure = QueryProgram::compile("//app:payload", &options)
        .err()
        .unwrap();
    assert_eq!(failure.error, Error::InvalidValue);
    assert!(calls.load(Ordering::Relaxed) >= 2);
    for panic in [false, true] {
        options.resolver = Some(QueryResolver::new(move |_, _| {
            if panic {
                panic!("resolver panic");
            }
            Err(Error::InvalidTag)
        }));
        assert_eq!(
            QueryProgram::compile("//app:payload", &options)
                .err()
                .unwrap()
                .error,
            if panic {
                Error::InvalidValue
            } else {
                Error::InvalidTag
            }
        );
    }
}

#[test]
fn source_bearing_feed_preserves_native_metadata() {
    let input = [0x5a, 1, 3];
    let mut reader = TreeReader::new(&input, Format::Ber, 1, 1, 1, true).unwrap();
    let event = reader.read_event().unwrap();
    drop(reader);
    let program =
        QueryProgram::compile("//5A[@offset=0 and @hlen=2]", &ProgramOptions::default()).unwrap();
    let mut execution = program.execution(1, 1, 10000, true).unwrap();
    execution.feed(&event).unwrap();
    execution.finish().unwrap();
    assert_eq!(
        execution.next_result().unwrap().unwrap().element.value(),
        &[3]
    );
}

#[test]
fn owning_fixed_formats_work_for_query_and_schema() {
    let format = OwnedFixedFormat::new(
        FixedFormatConfig::new(2, 1, ByteOrder::Big)
            .with_order(ElementOrder::Ltv)
            .with_length_scope(LengthScope::TagAndValue),
    )
    .unwrap();
    let options = ProgramOptions {
        fixed_format: Some(format.clone()),
        ..ProgramOptions::default()
    };
    let program = QueryProgram::compile("count(//1234)", &options).unwrap();
    let selector = QueryProgram::compile("//1234", &options).unwrap();
    let assertion = QueryProgram::compile("@len=1", &options).unwrap();
    let schema =
        QuerySchema::with_fixed_format(vec![QueryRule::new(selector, assertion)], format.clone());
    let wire = [3, 0x12, 0x34, 0x56];
    schema
        .validate_buffer(&wire, QuerySchemaLimits::default())
        .unwrap();
    let borrowed = format.as_borrowed();
    let mut reader = TreeReader::with_fixed_format(&wire, &borrowed, 4, 4, 4, true).unwrap();
    let mut execution = program.execution(4, 4, 10000, true).unwrap();
    drop(program);
    drop(options);
    execution.visit(&mut reader, |_| Visit::Continue).unwrap();
    assert_eq!(execution.result().unwrap(), QueryValue::Integer(1));
    #[cfg(feature = "document")]
    {
        let document = opentlv::Document::parse_fixed(&wire, &borrowed, 4, 4).unwrap();
        schema
            .validate_document(&document, QuerySchemaLimits::default())
            .unwrap();
    }
    // Native Format identity is explicit: clone one owner for interoperating
    // Readers, Documents and Programs instead of recreating equal contexts.
    let independent = OwnedFixedFormat::new(
        FixedFormatConfig::new(2, 1, ByteOrder::Big)
            .with_order(ElementOrder::Ltv)
            .with_length_scope(LengthScope::TagAndValue),
    )
    .unwrap();
    let independent_borrowed = independent.as_borrowed();
    let mut other_reader =
        TreeReader::with_fixed_format(&wire, &independent_borrowed, 4, 4, 4, true).unwrap();
    let other_program = QueryProgram::compile(
        "count(//1234)",
        &ProgramOptions {
            fixed_format: Some(format.clone()),
            ..ProgramOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        other_program
            .execution(4, 4, 10000, true)
            .unwrap()
            .next(&mut other_reader)
            .unwrap_err()
            .error,
        Error::InvalidArg
    );
    #[cfg(feature = "document")]
    {
        let other_document =
            opentlv::Document::parse_fixed(&wire, &independent_borrowed, 4, 4).unwrap();
        assert_eq!(
            other_program
                .execution(4, 4, 10000, true)
                .unwrap()
                .evaluate_document(&other_document, None, None)
                .unwrap_err()
                .error,
            Error::InvalidArg
        );
    }
}

#[test]
fn native_emv_name_resolver_is_available() {
    let options = ProgramOptions {
        resolver: Some(QueryResolver::emv()),
        ..ProgramOptions::default()
    };
    for text in ["//emv:PAN", "//pan"] {
        let program = QueryProgram::compile(text, &options).unwrap();
        let mut reader = TreeReader::new(&[0x5a, 0], Format::Ber, 4, 4, 4, true).unwrap();
        assert!(program
            .execution(4, 4, 10000, true)
            .unwrap()
            .next(&mut reader)
            .unwrap()
            .is_some());
    }
    assert!(QueryProgram::compile("//unknown:PAN", &options).is_err());
}

#[test]
fn v1_format_reset_and_rebind_preserve_native_state() {
    let mut matcher = {
        let query = opentlv::Query::parse("6f/a5/50").unwrap();
        assert_eq!(query.format().unwrap(), "6F/A5/50");
        query.matcher().unwrap()
    };
    assert!(!matcher.matches(&Tag::from_bytes(&[0x6f]), 0));
    matcher
        .rebind(&opentlv::Query::parse("6F/A5/50").unwrap())
        .unwrap();
    assert_eq!(
        matcher.rebind(&opentlv::Query::parse("6F/A5/51").unwrap()),
        Err(Error::InvalidArg)
    );
    assert!(!matcher.matches(&Tag::from_bytes(&[0xa5]), 1));
    assert!(matcher.matches(&Tag::from_bytes(&[0x50]), 2));
    matcher.reset().unwrap();
    assert!(!matcher.matches(&Tag::from_bytes(&[0x50]), 2));
}
