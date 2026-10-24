// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
use opentlv::{
    Error, Format, NumberCodec, NumberEncoding, ProgramOptions, Query, QueryBinding,
    QueryConversion, QueryErrorKind, QueryProgram, QueryProvider, QueryResolver, QueryType,
    QueryValue, Tag, TreeReader, Visit,
};

const WIRE: &[u8] = &[0x70, 6, 0x5a, 1, 1, 0x50, 1, 2, 0x5a, 1, 3];

fn count(program: &QueryProgram, input: &[u8]) -> QueryValue<'static> {
    let mut execution = program.execution(4, 20, 100000, true).unwrap();
    let mut reader = TreeReader::new(input, Format::Ber, 4, 4, 20, true).unwrap();
    execution.visit(&mut reader, |_| Visit::Continue).unwrap();
    match execution.result().unwrap() {
        QueryValue::Integer(value) => QueryValue::Integer(value),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn static_names_compile_directly_to_the_resolver_image() {
    let mut options = ProgramOptions::default();
    options.names.insert("app:payload".into(), vec![0x5a]);
    let direct = QueryProgram::compile("count(//app:payload)", &options).unwrap();
    let resolved = QueryProgram::compile(
        "count(//app:payload)",
        &ProgramOptions {
            resolver: Some(QueryResolver::new(|_, _| Ok(Tag::from_bytes(&[0x5a])))),
            ..ProgramOptions::default()
        },
    )
    .unwrap();
    assert_eq!(direct.image(), resolved.image());
    assert_eq!(direct.info().program_size, direct.image().len());
    assert_eq!(count(&direct, WIRE), QueryValue::Integer(2));
    let unknown = QueryProgram::compile("//app:other", &options)
        .err()
        .unwrap();
    assert_eq!(unknown.error, Error::InvalidTag);
    let syntax = QueryProgram::compile("/", &options).err().unwrap();
    assert_eq!(syntax.error, Error::Syntax);
    assert_eq!(syntax.location.domain, opentlv::LocationDomain::Expression);
}

#[test]
fn opened_plan_runs_without_frontend_and_reports_plan_requirements() {
    let program = QueryProgram::compile("count(//5A)", &ProgramOptions::default()).unwrap();
    let plan = QueryProgram::open_plan(program.image(), &ProgramOptions::default()).unwrap();
    assert_eq!(plan.image(), program.image());
    let info = plan.plan_info().unwrap();
    assert_eq!(info.program_size, program.image().len());
    assert_eq!(info.result_kind, program.info().result_kind);
    assert_eq!(info.scratch_size, 0);
    assert_eq!(plan.info().result_kind, info.result_kind);
    assert_eq!(program.plan_info().unwrap().result_kind, info.result_kind);
    assert_eq!(count(&plan, WIRE), QueryValue::Integer(2));
    let truncated = &program.image()[..program.image().len() - 1];
    let failure = QueryProgram::open_plan(truncated, &ProgramOptions::default())
        .err()
        .unwrap();
    assert_eq!(failure.error, Error::InvalidValue);
    assert_eq!(failure.kind, QueryErrorKind::Image);
}

#[test]
fn number_codec_provider_decodes_in_c_and_is_checked_at_execution() {
    let mut options = ProgramOptions::default();
    options.providers.push(QueryProvider::number(
        QueryConversion::Num,
        201,
        NumberCodec::new(NumberEncoding::LittleEndian, 0, 0),
    ));
    options.providers.push(QueryProvider::number(
        QueryConversion::Bcd,
        202,
        NumberCodec::new(NumberEncoding::Bcd, 2, 4),
    ));
    let input = [0x5a, 2, 0x34, 0x12, 0x50, 2, 0x12, 0x34];
    let bcd = QueryProgram::compile("bcd(//50)", &options).unwrap();
    assert_eq!(count(&bcd, &input), QueryValue::Integer(1234));
    let program = QueryProgram::compile("num(//5A)", &options).unwrap();
    assert_eq!(count(&program, &input), QueryValue::Integer(0x1234));
    // A plan carries provider IDs only; executions need the matching providers.
    let plan = QueryProgram::open_plan(program.image(), &ProgramOptions::default()).unwrap();
    assert!(plan.execution(4, 20, 100000, true).is_err());
    let plan = QueryProgram::open_plan(program.image(), &options).unwrap();
    drop(options);
    assert_eq!(count(&plan, &input), QueryValue::Integer(0x1234));
    let mut execution = plan.execution(4, 20, 100000, true).unwrap();
    let overflow = [0x5a, 8, 255, 255, 255, 255, 255, 255, 255, 255];
    let mut reader = TreeReader::new(&overflow, Format::Ber, 4, 4, 20, true).unwrap();
    let failure = execution
        .visit(&mut reader, |_| Visit::Continue)
        .unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    assert_eq!(failure.kind, QueryErrorKind::Codec);
    assert_eq!(
        failure.codec_detail.unwrap().reported,
        Error::InvalidValue.code()
    );
}

#[test]
fn reset_reuses_workspace_and_clears_bindings_counters_and_results() {
    let mut options = ProgramOptions::default();
    options.variables.insert("needle".into(), QueryType::Bytes);
    let program =
        QueryProgram::compile("count(//5A[contains(value(), $needle)])", &options).unwrap();
    let mut execution = program.execution(4, 20, 100000, true).unwrap();
    for (needle, expected) in [(&[3][..], 1), (&[9][..], 0)] {
        execution
            .bind("needle", QueryBinding::Bytes(needle))
            .unwrap();
        let mut reader = TreeReader::new(WIRE, Format::Ber, 4, 4, 20, true).unwrap();
        execution.visit(&mut reader, |_| Visit::Continue).unwrap();
        assert_eq!(execution.result().unwrap(), QueryValue::Integer(expected));
        assert_eq!(execution.info().unwrap().elements, 4);
        execution.reset().unwrap();
        assert_eq!(execution.info().unwrap().elements, 0);
        assert_eq!(execution.result().unwrap_err().kind, QueryErrorKind::State);
    }
    // A failed execution is terminal until reset.
    let program = QueryProgram::compile("//5A", &ProgramOptions::default()).unwrap();
    let mut execution = program.execution(4, 20, 100000, false).unwrap();
    let mut reader = TreeReader::new(WIRE, Format::Ber, 4, 4, 20, true).unwrap();
    assert_eq!(
        execution
            .visit(&mut reader, |_| Visit::Error)
            .unwrap_err()
            .error,
        Error::Visitor
    );
    assert_eq!(execution.info().unwrap().invalid, 1);
    execution.reset().unwrap();
    assert_eq!(execution.info().unwrap().invalid, 0);
    let mut reader = TreeReader::new(WIRE, Format::Ber, 4, 4, 20, true).unwrap();
    assert_eq!(execution.next(&mut reader).unwrap().unwrap().offset, 2);
}

#[test]
fn query_visits_complete_buffer_without_a_reader() {
    let query = Query::parse("70/5A").unwrap();
    let wire = [0x70, 6, 0x5a, 1, 1, 0x5a, 1, 3, 0x5a, 0];
    let mut found = Vec::new();
    query
        .visit_buffer(&wire, Format::Ber, 4, 20, |element, depth, offset| {
            found.push((element, depth, offset));
            Visit::Continue
        })
        .unwrap();
    let found: Vec<_> = found
        .iter()
        .map(|(element, depth, offset)| (element.value(), *depth, *offset))
        .collect();
    assert_eq!(found, [(&[1][..], 1, 2), (&[3][..], 1, 5)]);
    let mut calls = 0;
    query
        .visit_buffer(&wire, Format::Ber, 4, 20, |_, _, _| {
            calls += 1;
            Visit::Stop
        })
        .unwrap();
    assert_eq!(calls, 1);
    let failure = query
        .visit_buffer(&wire, Format::Ber, 4, 20, |_, _, _| Visit::Error)
        .unwrap_err();
    assert_eq!(failure.error, Error::Visitor);
    // Malformed data after the last match is still reported, with its offset.
    let mut input = wire.to_vec();
    input.extend([0x5a, 5]);
    calls = 0;
    let failure = query
        .visit_buffer(&input, Format::Ber, 4, 20, |_, _, _| {
            calls += 1;
            Visit::Continue
        })
        .unwrap_err();
    assert_eq!((failure.error, calls), (Error::Truncated, 2));
    assert_eq!(failure.diagnostic.unwrap().offset, Some(input.len()));
    // The element bound covers nonmatching elements too.
    assert_eq!(
        query
            .visit_buffer(&wire, Format::Ber, 4, 3, |_, _, _| Visit::Continue)
            .unwrap_err()
            .error,
        Error::Limit
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        query.visit_buffer(&wire, Format::Ber, 4, 20, |_, _, _| panic!("visitor panic"))
    }));
    assert!(panic.is_err());
}

#[cfg(feature = "document")]
#[test]
fn document_results_visit_through_the_shared_cursor() {
    use opentlv::Document;
    let document = Document::parse(WIRE, Format::Ber, 4, 20).unwrap();
    let program = QueryProgram::compile("//5A", &ProgramOptions::default()).unwrap();
    let mut execution = program.execution(4, 20, 100000, true).unwrap();
    assert_eq!(
        execution
            .visit_document(|_| Visit::Continue)
            .unwrap_err()
            .error,
        Error::InvalidArg
    );
    execution.evaluate_document(&document, None, None).unwrap();
    let mut seen = Vec::new();
    execution
        .visit_document(|node| {
            seen.push(node);
            Visit::Stop
        })
        .unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].value(), &[1]);
    assert_eq!(execution.next_document().unwrap().unwrap().value(), &[3]);
    execution
        .visit_document(|_| panic!("no results remain"))
        .unwrap();
    execution.reset().unwrap();
    execution.evaluate_document(&document, None, None).unwrap();
    let failure = execution.visit_document(|_| Visit::Error).unwrap_err();
    assert_eq!(failure.error, Error::Visitor);
    assert_eq!(failure.kind, QueryErrorKind::Callback);
    execution.reset().unwrap();
    execution.evaluate_document(&document, None, None).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execution.visit_document(|_| panic!("visitor panic"))
    }));
    assert!(panic.is_err());
}

#[cfg(feature = "document")]
#[test]
fn document_value_snapshot_is_discovered_without_a_capacity() {
    use opentlv::Document;
    let mut document = Document::parse(WIRE, Format::Ber, 4, 20).unwrap();
    document
        .first_mut()
        .unwrap()
        .append(&Tag::from_bytes(&[0x5a]), &[4])
        .unwrap();
    let program = QueryProgram::compile("value(//70)", &ProgramOptions::default()).unwrap();
    assert_ne!(program.info().constructed_values_required, 0);
    let expected = [0x5a, 1, 1, 0x50, 1, 2, 0x5a, 1, 4];
    for capacity in [None, Some(document.encoded_size().unwrap())] {
        let mut execution = program.execution(4, 20, 100000, true).unwrap();
        execution
            .evaluate_document(&document, None, capacity)
            .unwrap();
        assert_eq!(execution.result().unwrap(), QueryValue::Bytes(&expected));
    }
    let mut execution = program.execution(4, 20, 100000, true).unwrap();
    assert!(execution
        .evaluate_document(&document, None, Some(1))
        .is_err());
    // Programs without constructed Values skip discovery entirely.
    let program = QueryProgram::compile("count(//5A)", &ProgramOptions::default()).unwrap();
    assert_eq!(program.info().constructed_values_required, 0);
    let mut execution = program.execution(4, 20, 100000, true).unwrap();
    execution.evaluate_document(&document, None, None).unwrap();
    assert_eq!(execution.result().unwrap(), QueryValue::Integer(3));
}
