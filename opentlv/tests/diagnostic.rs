// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
use opentlv::{
    CodecCause, CodecOperation, CodecViolation, ProgramOptions, QueryErrorKind, QueryProgram,
    ReaderOperation, SchemaDefinitionKind, SchemaIssue, Severity, WriterOperation,
};

#[test]
fn native_names_cover_complete_vocabulary_and_unknown_values() {
    assert_eq!(ReaderOperation::Header.name(), "header");
    assert_eq!(WriterOperation::End.name(), "end");
    assert_eq!(CodecOperation::Measure.name(), "measure");
    assert_eq!(CodecCause::Schema.name(), "schema");
    assert_eq!(CodecViolation::Utf8.name(), "utf8");
    assert_eq!(SchemaDefinitionKind::Component.name(), "component");
    assert_eq!(SchemaIssue::Missing.name(), "missing");
    assert_eq!(Severity::Warning.name(), "warning");
    assert_eq!(QueryErrorKind::State.name(), "state");
    let unknown = QueryErrorKind::from_raw(9876);
    assert!(matches!(unknown, QueryErrorKind::Unrecognized(raw) if raw.as_raw() == 9876));
    assert_eq!(unknown, QueryErrorKind::from_raw(9876));
    assert_eq!(unknown.as_raw(), 9876);
    // Known raw values never become Unknown, so equality agrees with as_raw().
    assert_eq!(QueryErrorKind::from_raw(0), QueryErrorKind::None);
    assert_eq!(unknown.name(), "unknown");
}

#[test]
fn query_categories_and_location_rendering_are_independent() {
    let failure = QueryProgram::compile("/", &ProgramOptions::default())
        .err()
        .unwrap();
    assert_eq!(failure.kind, QueryErrorKind::Syntax);
    assert!(failure.to_string().contains("expression"));
    assert!(failure.reader.is_none());
    let program = QueryProgram::compile("count(//5A)", &ProgramOptions::default()).unwrap();
    let execution = program.execution(4, 20, 100000, true).unwrap();
    let failure = execution.result().unwrap_err();
    assert_eq!(failure.kind, QueryErrorKind::State);
    assert!(failure.to_string().contains("unknown location"));
    assert_eq!(failure.source_offset(), None);
}

#[test]
fn query_metadata_preserves_absent_reader_paths() {
    let failure = {
        let mut wire = vec![0x5a, 3, 0];
        for _ in 0..35 {
            let mut outer = vec![0x70, wire.len() as u8];
            outer.extend(wire);
            wire = outer;
        }
        let program = QueryProgram::compile("//5A", &ProgramOptions::default()).unwrap();
        let mut execution = program.execution(64, 100, 1000000, true).unwrap();
        let mut reader =
            opentlv::TreeReader::new(&wire, opentlv::Format::Ber, 64, 64, 100, true).unwrap();
        execution
            .visit(&mut reader, |_| opentlv::Visit::Continue)
            .unwrap_err()
    };
    assert_eq!(failure.kind, QueryErrorKind::Reader);
    assert_eq!(failure.path(), None);
    assert_eq!(failure.path_omitted(), 0);
    // The Reader cause owns the common evidence; it is not duplicated.
    assert!(failure.metadata.is_none());
    assert_eq!(failure.reader.as_ref().unwrap().path, None);
}
