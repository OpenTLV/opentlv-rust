// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Integration tests for the safe reader API.

use opentlv::{Error, Format, Reader, Tag};

#[test]
fn parses_multiple_entries() {
    let data = [0x01, 0x03, b'a', b'b', b'c', 0x02, 0x00, 0x03, 0x01, 0xFF];
    let elements: Vec<_> = Reader::new(&data).collect::<Result<_, _>>().unwrap();

    assert_eq!(elements.len(), 3);
    assert_eq!(elements[0].tag(), &Tag::from_bytes(&[0x01]));
    assert_eq!(elements[0].value(), b"abc");
    assert_eq!(elements[1].tag().as_bytes(), &[0x02]);
    assert!(elements[1].value().is_empty());
    assert_eq!(elements[2].value(), &[0xFF]);
}

#[test]
fn values_are_zero_copy_slices_of_the_input() {
    let data = vec![0x07, 0x02, 0x10, 0x20];
    let element = Reader::new(&data).next().unwrap().unwrap();
    assert_eq!(element.value().as_ptr(), data[2..].as_ptr());
}

#[test]
fn empty_input_yields_no_entries() {
    let mut reader = Reader::new(&[]);
    assert!(reader.is_at_end());
    assert!(reader.next().is_none());
}

#[test]
fn tracks_position() {
    let data = [0x01, 0x01, 0xAA, 0x02, 0x00];
    let mut reader = Reader::new(&data);
    assert_eq!(reader.position(), 0);
    reader.next_element().unwrap().unwrap();
    assert_eq!(reader.position(), 3);
    reader.next_element().unwrap().unwrap();
    assert!(reader.is_at_end());
    assert!(reader.next_element().is_none());
}

#[test]
fn truncated_value_is_an_error_then_iteration_stops() {
    let data = [0x01, 0x01, 0xAA, 0x02, 0x05, 0x00];
    let mut reader = Reader::new(&data);
    assert!(reader.next().unwrap().is_ok());
    assert_eq!(reader.next().unwrap().unwrap_err(), Error::Truncated);
    assert!(reader.next().is_none());
    assert!(reader.next().is_none());
}

#[test]
fn truncated_length_is_an_error() {
    let data = [0x01];
    let err = Reader::new(&data).next().unwrap().unwrap_err();
    assert_eq!(err, Error::Truncated);
}

#[test]
fn collecting_into_result_surfaces_malformed_input() {
    let data = [0x01, 0x04, 0x00];
    let result: Result<Vec<_>, _> = Reader::new(&data).collect();
    assert!(result.is_err());
}

#[test]
fn error_propagates_with_question_mark() {
    fn count(data: &[u8]) -> opentlv::Result<usize> {
        let mut n = 0;
        for element in Reader::new(data) {
            element?;
            n += 1;
        }
        Ok(n)
    }
    assert_eq!(count(&[0x01, 0x00, 0x02, 0x00]), Ok(2));
    assert_eq!(count(&[0x01, 0x02, 0x00]), Err(Error::Truncated));
}

#[test]
fn reads_ber_multi_byte_tags() {
    let data = [0x9F, 0x02, 0x02, 0x12, 0x34];
    let element = Reader::with_format(&data, Format::Ber)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(element.tag().as_bytes(), &[0x9F, 0x02]);
    assert_eq!(element.value(), &[0x12, 0x34]);
}

#[test]
fn reads_der_format() {
    let der = [0x04, 0x02, 0xCA, 0xFE];
    let element = Reader::with_format(&der, Format::Der)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(element.value(), &[0xCA, 0xFE]);
}

#[test]
fn entries_outlive_the_reader() {
    let data = [0x01, 0x02, 0xAA, 0xBB];
    let element = {
        let mut reader = Reader::new(&data);
        reader.next().unwrap().unwrap()
    };
    assert_eq!(element.value(), &[0xAA, 0xBB]);
}

#[test]
fn element_borrows_original_length_bytes() {
    let data = [0x9F, 0x20, 0x82, 0, 1, 0xAA];
    let element = Reader::with_format(&data, Format::Ber)
        .next()
        .unwrap()
        .unwrap();

    assert_eq!(element.value(), &data[5..]);
}
#[test]
fn single_read_retains_layout_and_owned_failure_detail() {
    let decoded = opentlv::read(&[1, 0, 0xff], opentlv::Format::Ber).unwrap();
    assert_eq!(decoded.encoded(), &[1, 0]);
    let error = opentlv::read(&[1, 2], opentlv::Format::Ber).unwrap_err();
    assert_eq!(error.error, opentlv::Error::Truncated);
    assert!(error.diagnostic.is_some());
}
