#![cfg(feature = "document")]
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::{ByteOrder, Document, Error, FixedFormat, FixedFormatConfig, Format, Query, Tag};

#[test]
fn pipeline_builds_already_matched_subtree() {
    use opentlv::{DocumentBuilder, TreeReader};
    let wire = [1, 0, 0x30, 5, 0x31, 3, 4, 1, 42, 4, 0];
    let mut reader = TreeReader::new(&wire[..3], Format::Ber, 2, 2, 10, false).unwrap();
    let mut matcher = Query::parse("30/31").unwrap().matcher().unwrap();
    let first = reader.read().unwrap();
    assert!(!matcher.matches(first.decoded.element.tag(), first.depth));
    assert_eq!(reader.read().unwrap_err(), Error::NeedMoreData);
    reader.set_input(&wire[2..], 2, true).unwrap();
    loop {
        let item = reader.read().unwrap();
        if matcher.matches(item.decoded.element.tag(), item.depth) {
            break;
        }
    }
    let mut document = {
        let mut builder = DocumentBuilder::current_subtree(&mut reader, 1, 2).unwrap();
        builder.consume().unwrap()
    };
    document
        .first_mut()
        .unwrap()
        .set_value(&[4, 2, 7, 8])
        .unwrap();
    assert_eq!(document.encode().unwrap(), [0x31, 4, 4, 2, 7, 8]);
    assert_eq!(reader.read().unwrap().offset, 9);
}

#[test]
fn current_subtree_cannot_reuse_a_stale_selection() {
    use opentlv::{DocumentBuilder, TreeReader, Visit};
    let wire = [0x30, 3, 4, 1, 42];
    for operation in 0..5 {
        let mut reader = TreeReader::new(&wire, Format::Ber, 2, 2, 10, true).unwrap();
        assert!(DocumentBuilder::current_subtree(&mut reader, 2, 10).is_err());
        reader.read().unwrap();
        match operation {
            0 => reader.set_input(&wire, 0, true).unwrap(),
            1 => reader.skip_subtree().unwrap(),
            2 => reader.visit(|_, _, _| Visit::Continue).unwrap(),
            3 => Query::parse("30/04")
                .unwrap()
                .matcher()
                .unwrap()
                .visit(&mut reader, |_, _, _| Visit::Continue)
                .unwrap(),
            _ => {
                reader.read().unwrap();
                assert_eq!(reader.read().unwrap_err(), Error::End);
            }
        }
        assert_eq!(
            DocumentBuilder::current_subtree(&mut reader, 2, 10)
                .err()
                .unwrap()
                .error,
            Error::InvalidState
        );
    }
}

#[test]
fn builder_resumes_and_materializes_selected_subtree_without_sibling_decode() {
    use opentlv::{DocumentBuilder, TreeReader};
    let first = [1, 0];
    let second = [2, 1, 42];
    let mut reader = TreeReader::new(&first, Format::Ber, 4, 4, 20, false).unwrap();
    {
        let mut builder = DocumentBuilder::new(&mut reader, 4, 20).unwrap();
        assert_eq!(builder.consume().err().unwrap().error, Error::NeedMoreData);
        assert_eq!(builder.consumed(), 2);
        builder.set_input(&second, 2, true).unwrap();
        let document = builder.consume().unwrap();
        assert_eq!(document.encode().unwrap(), [1, 0, 2, 1, 42]);
        assert_eq!(builder.consume().err().unwrap().error, Error::InvalidState);
    }
    let input = [0x30, 3, 4, 1, 42, 4, 2];
    let mut reader = TreeReader::new(&input, Format::Ber, 4, 4, 20, true).unwrap();
    let document = {
        let mut builder = DocumentBuilder::next_subtree(&mut reader, 4, 20).unwrap();
        builder.consume().unwrap()
    };
    assert_eq!(document.encode().unwrap(), &input[..5]);
    assert_eq!(reader.read().err().unwrap(), Error::Truncated);
}

#[test]
fn owns_input_and_delegates_navigation_query_and_mutation() {
    let mut document = {
        let input = vec![0x30, 3, 4, 1, 42, 4, 0];
        Document::parse(&input, Format::Ber, 64, 100).unwrap()
    };
    assert_eq!(document.len(), 3);
    let query = Query::parse("30/04").unwrap();
    assert_eq!(document.find_path(&query).unwrap().unwrap().value(), &[42]);
    assert!(document
        .find_path(&Query::parse("FF").unwrap())
        .unwrap()
        .is_none());
    let root = document.first().unwrap();
    assert!(root.is_constructed());
    assert_eq!(
        root.first_child().unwrap().parent().unwrap().tag(),
        Tag::from_bytes(&[0x30])
    );
    assert_eq!(root.next().unwrap().value(), &[]);
    document
        .find_path_mut(&query)
        .unwrap()
        .unwrap()
        .set_value(&[43])
        .unwrap();
    {
        let mut root = document.first_mut().unwrap();
        root.append(&Tag::from_bytes(&[4]), &[44]).unwrap();
        assert_eq!(
            root.as_node()
                .first_child()
                .unwrap()
                .next_same_tag()
                .unwrap()
                .value(),
            &[44]
        );
    }
    document.first_mut().unwrap().into_next().unwrap().erase();
    assert_eq!(document.encode().unwrap(), [0x30, 6, 4, 1, 43, 4, 1, 44]);
    assert_eq!(document.encoded_size().unwrap(), 8);
    document
        .first_mut()
        .unwrap()
        .into_first_child()
        .unwrap()
        .erase();
    assert_eq!(document.len(), 2);
}

#[test]
fn failed_mutation_preserves_document_and_encoding_capacity_is_reported() {
    let mut document = Document::parse(&[0x30, 3, 4, 1, 42], Format::Ber, 2, 2).unwrap();
    assert_eq!(
        document.first_mut().unwrap().set_value(&[4, 2]),
        Err(Error::Truncated)
    );
    assert_eq!(
        document
            .find(&Tag::from_bytes(&[0x30]))
            .unwrap()
            .first_child()
            .unwrap()
            .value(),
        &[42]
    );
    assert!(document.append(&Tag::from_bytes(&[4]), &[]).is_err());
    let mut output = [0xAA; 2];
    let error = document.encode_into(&mut output).unwrap_err();
    assert_eq!(error.error, Error::BufferTooShort);
    assert_eq!(error.required, Some(5));
    assert_eq!(output, [0xAA; 2]);
}

#[test]
fn destination_format_and_subtree_encoding_use_c() {
    let document = Document::parse(&[0x30, 3, 4, 1, 42], Format::Ber, 64, 100).unwrap();
    let expected = [0x30, 0x80, 4, 1, 42, 0, 0];
    assert_eq!(document.encode_as(Format::Cer).unwrap(), expected);
    assert_eq!(
        document.encoded_size_as(Format::Cer).unwrap(),
        expected.len()
    );
    let mut output = [0; 7];
    assert_eq!(
        document
            .first()
            .unwrap()
            .encode_into_as(&mut output, Format::Cer)
            .unwrap(),
        7
    );
    assert_eq!(output, expected);
    assert_eq!(
        document
            .first()
            .unwrap()
            .first_child()
            .unwrap()
            .encode()
            .unwrap(),
        [4, 1, 42]
    );
}

#[test]
fn fixed_format_and_insert_before_are_available() {
    let config = FixedFormatConfig::new(1, 1, ByteOrder::Big);
    let format = FixedFormat::new(&config).unwrap();
    let mut document = Document::with_fixed_format(&format, 0, 5).unwrap();
    document.append(&Tag::from_bytes(&[2]), &[20]).unwrap();
    document
        .first_mut()
        .unwrap()
        .insert_before(&Tag::from_bytes(&[1]), &[10])
        .unwrap();
    assert_eq!(document.encode().unwrap(), [1, 1, 10, 2, 1, 20]);
}

#[test]
fn parse_reports_source_offset_and_limits() {
    let error = Document::parse(&[1, 0, 2, 2], Format::Ber, 64, 100)
        .err()
        .unwrap();
    assert_eq!(error.error, Error::Truncated);
    assert_eq!(error.offset, Some(4));
    assert_eq!(error.location.domain, opentlv::LocationDomain::Input);
    assert_eq!(
        Document::parse(&[1, 0], Format::Ber, 0, 0)
            .err()
            .unwrap()
            .error,
        Error::Limit
    );
}
