// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Single-element reads and source-free Reader pulls through the C entry points.

use opentlv::{
    read, read_element, ByteOrder, Error, FixedFormat, FixedFormatConfig, Format, Reader, Tag,
};

#[test]
fn read_element_returns_zero_copy_value_and_consumed_size() {
    let data = [0x9F, 0x20, 0x82, 0, 2, 0xAA, 0xBB, 0x50, 0];
    let (element, consumed) = read_element(&data, Format::Ber).unwrap();
    assert_eq!(element.tag(), &Tag::from_bytes(&[0x9F, 0x20]));
    assert_eq!(element.value(), &[0xAA, 0xBB]);
    assert_eq!(element.value().as_ptr(), data[5..].as_ptr());
    assert_eq!(consumed, 7);
}

#[test]
fn read_element_outlives_a_borrowed_format() {
    let data = [0x01, 0x00, 0x02, 0xCA, 0xFE];
    let (element, consumed) = {
        let config = FixedFormatConfig::new(1, 2, ByteOrder::Big);
        let format = FixedFormat::new(&config).unwrap();
        read_element(&data, &format).unwrap()
    };
    assert_eq!(element.tag().as_bytes(), &[0x01]);
    assert_eq!(element.value(), &[0xCA, 0xFE]);
    assert_eq!(consumed, data.len());
}

#[test]
fn single_reads_report_end_and_owned_failure_detail() {
    for error in [
        read_element(&[], Format::Ber).unwrap_err(),
        read(&[], Format::Ber).unwrap_err(),
    ] {
        assert_eq!(error.error, Error::End);
    }
    let data = vec![0x50, 3, 0xAA];
    let (element_error, read_error) = (
        read_element(&data, Format::Ber).unwrap_err(),
        read(&data, Format::Ber).unwrap_err(),
    );
    drop(data);
    for error in [element_error, read_error] {
        assert_eq!(error.error, Error::Truncated);
        let diagnostic = error.diagnostic.expect("owned Reader diagnostic");
        assert_eq!(diagnostic.tag.as_deref(), Some(&[0x50][..]));
        assert_eq!(diagnostic.value_offset, Some(2));
        assert_eq!(diagnostic.available, Some(1));
    }
}

#[test]
fn read_matches_a_reader_source_pull() {
    let data = [0x50, 0x81, 1, 0xAA, 0xFF];
    let once = read(&data, Format::Ber).unwrap();
    let pulled = Reader::with_format(&data, Format::Ber)
        .read_source()
        .unwrap();
    assert_eq!(once.element, pulled.element);
    assert_eq!(once.encoded(), &data[..4]);
    assert_eq!(once.encoded(), pulled.encoded());
    assert_eq!(once.layout(), pulled.layout());
    assert_eq!(once.raw_length(), &[0x81, 1]);
    let mut output = [0; 4];
    assert_eq!(once.preserve(&mut output), Ok(4));
    assert_eq!(output, data[..4]);
}

#[test]
fn reader_read_element_resumes_incremental_input() {
    let first = [0x50, 1, 0xAA, 0x51, 2, 0xBB];
    let second = [0x51, 2, 0xBB, 0xCC];
    let mut reader = Reader::incremental(&first, Format::Ber);
    let retained = reader.read_element().unwrap();
    assert_eq!(reader.read_element(), Err(Error::NeedMoreData));
    assert!(reader.diagnostic().is_some());
    let consumed = reader.consumed();
    assert_eq!(consumed, 3);
    reader.set_input(&second, consumed, true).unwrap();
    let element = reader.read_element().unwrap();
    assert!(reader.diagnostic().is_none());
    assert_eq!(element.value(), &[0xBB, 0xCC]);
    assert_eq!(reader.offset(), 7);
    assert_eq!(reader.read_element(), Err(Error::End));
    assert!(reader.is_at_end());
    drop(reader);
    assert_eq!(retained.value(), &[0xAA]);
}

#[test]
fn reader_read_element_stays_callable_after_an_error() {
    let data = [0x50, 1, 0xAA, 0x51, 5, 0xBB];
    let mut reader = Reader::with_format(&data, Format::Ber);
    assert_eq!(reader.read_element().unwrap().value(), &[0xAA]);
    for _ in 0..2 {
        assert_eq!(reader.read_element(), Err(Error::Truncated));
        assert_eq!(reader.position(), 3);
        let diagnostic = reader.diagnostic().unwrap();
        assert_eq!(diagnostic.tag.as_deref(), Some(&[0x51][..]));
    }
    let mut iterator = Reader::with_format(&data, Format::Ber).skip(1);
    assert_eq!(iterator.next(), Some(Err(Error::Truncated)));
    assert_eq!(iterator.next(), None);
}
