// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Diagnostic path text formatted by Core's `tlv_diagnostic_path_string()`.

use opentlv::{
    DiagnosticMetadata, Element, Format, ProgramOptions, QueryProgram, QueryRule, QuerySchema,
    QuerySchemaLimits, StructureRule, StructureSchema, Tag, TreeWriter, UnknownPolicy,
    ValidationLimits,
};

/// `depth` levels of 0x30 inside an outermost 0x70, around `inner`.
fn nested(inner: &[u8], depth: usize) -> Vec<u8> {
    let mut wire = inner.to_vec();
    for level in 0..depth {
        let mut enclosing = vec![
            if level + 1 == depth { 0x70 } else { 0x30 },
            wire.len() as u8,
        ];
        enclosing.extend(wire);
        wire = enclosing;
    }
    wire
}

#[test]
fn owned_paths_format_through_core() {
    let path = |tags: &[&[u8]], omitted| DiagnosticMetadata {
        path: Some(tags.iter().map(|tag| tag.to_vec()).collect()),
        path_omitted: omitted,
        ..Default::default()
    };
    assert_eq!(
        path(&[&[0x6f], &[0xbf, 0x0c], &[0x61]], 0)
            .path_text()
            .as_deref(),
        Some("6F > BF0C > 61")
    );
    assert_eq!(path(&[], 0).path_text().as_deref(), Some(""));
    assert_eq!(path(&[&[0x70]], 2).path_text().as_deref(), Some("70 > ..."));
    assert_eq!(path(&[], 1).path_text().as_deref(), Some("..."));
    assert_eq!(DiagnosticMetadata::default().path_text(), None);
    // Core rejects an empty Tag; a native path cannot hold more than 32 Tags.
    assert_eq!(path(&[&[0x70], &[]], 0).path_text(), None);
    assert_eq!(path(&[&[0x30][..]; 33], 0).path_text(), None);
}

#[test]
fn schema_paths_keep_text_after_input_and_schema_are_dropped() {
    let limits = ValidationLimits {
        max_depth: 40,
        max_elements: 100,
    };
    let top = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[1])).required_once()],
        false,
    );
    let report = top
        .validate_diagnostics(&[2, 0], Format::Ber, &limits, UnknownPolicy::Allow, 1)
        .unwrap();
    // A tracked empty path at the top level differs from no path.
    assert_eq!(report.diagnostics[0].path_text().as_deref(), Some(""));

    let report = {
        let wire = nested(&[4, 1, 42], 1);
        let schema = StructureSchema::new(
            [
                StructureRule::new(Tag::from_bytes(&[0x70])).children(StructureSchema::new(
                    [StructureRule::new(Tag::from_bytes(&[4])).length(2, 3)],
                    false,
                )),
            ],
            false,
        );
        schema
            .validate_diagnostics(&wire, Format::Ber, &limits, UnknownPolicy::BySchema, 1)
            .unwrap()
    };
    assert_eq!(report.diagnostics[0].path_text().as_deref(), Some("70"));

    let mut schema = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[4])).length(2, 3)],
        false,
    );
    for level in 0..35 {
        let tag = if level == 34 { 0x70 } else { 0x30 };
        schema = StructureSchema::new(
            [StructureRule::new(Tag::from_bytes(&[tag])).children(schema)],
            false,
        );
    }
    let report = schema
        .validate_diagnostics(
            &nested(&[4, 1, 42], 35),
            Format::Ber,
            &limits,
            UnknownPolicy::BySchema,
            1,
        )
        .unwrap();
    let issue = &report.diagnostics[0];
    assert_eq!(issue.path_omitted, 3);
    assert_eq!(
        issue.path_text().unwrap(),
        format!("70{} > ...", " > 30".repeat(31))
    );
}

#[test]
fn query_failures_format_their_common_path() {
    let rule = |context, assertion| {
        QueryRule::new(
            QueryProgram::compile(context, &ProgramOptions::default()).unwrap(),
            QueryProgram::compile(assertion, &ProgramOptions::default()).unwrap(),
        )
    };
    let schema = QuerySchema::new(vec![rule("//5A", "1 = 0")], Format::Ber);
    let failure = schema
        .validate_buffer(&nested(&[0x5a, 0], 1), QuerySchemaLimits::default())
        .unwrap_err();
    assert_eq!(failure.failure.path_text().as_deref(), Some("70"));
    let failure = schema
        .validate_buffer(&nested(&[0x5a, 0], 35), QuerySchemaLimits::default())
        .unwrap_err();
    assert_eq!(failure.failure.path_omitted(), 3);
    assert_eq!(
        failure.failure.path_text().unwrap(),
        format!("70{} > ...", " > 30".repeat(31))
    );
    // Reader failures without path evidence have no text.
    let wire = nested(&[0x5a, 3, 0], 35);
    let program = QueryProgram::compile("//5A", &ProgramOptions::default()).unwrap();
    let mut execution = program.execution(64, 100, 1000000, true).unwrap();
    let mut reader = opentlv::TreeReader::new(&wire, Format::Ber, 64, 64, 100, true).unwrap();
    let failure = execution
        .visit(&mut reader, |_| opentlv::Visit::Continue)
        .unwrap_err();
    assert_eq!(failure.path(), None);
    assert_eq!(failure.path_text(), None);
    assert_eq!(failure.reader.unwrap().path_text(), None);
}

#[test]
fn writer_failures_without_path_evidence_have_no_text() {
    let mut output = [0; 4];
    let mut writer = TreeWriter::new(&mut output, Format::Ber, 2, 32, 2, 5).unwrap();
    writer.begin(&Tag::from_bytes(&[0x70])).unwrap();
    assert!(writer
        .write(&Element::new(Tag::from_bytes(&[4]), &[42; 8]))
        .is_err());
    let diagnostic = writer.diagnostic().unwrap();
    assert_eq!(diagnostic.path, None);
    assert_eq!(diagnostic.path_text(), None);
}
