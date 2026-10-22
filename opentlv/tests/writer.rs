// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Integration tests for the safe writer API and reader/writer round trips.

use opentlv::{encoded_size, Error, Format, Reader, Tag, Writer};

#[test]
fn element_sizing_and_raw_copy_use_caller_storage() {
    let element = opentlv::Element::new(tag(&[0x04]), b"abc");
    let required = opentlv::element_encoded_size(&element, Format::Ber).unwrap();
    let mut storage = vec![0u8; required + 1];
    let mut writer = Writer::with_format(&mut storage, Format::Ber);
    writer.write_element(&element).unwrap();
    writer.copy_encoded(&[0xFF]).unwrap();
    assert_eq!(writer.written(), b"\x04\x03abc\xff");
    assert_eq!(writer.remaining(), 0);
    assert_eq!(writer.write_element(&element), Err(Error::BufferTooShort));
    assert_eq!(writer.copy_encoded(&[1]), Err(Error::BufferTooShort));
    assert_eq!(writer.position(), required + 1);
    writer.copy_encoded(&[]).unwrap();
}

fn tag(bytes: &[u8]) -> Tag {
    Tag::from_bytes(bytes)
}

#[test]
fn writes_entries_with_expected_bytes() {
    let mut buf = [0u8; 16];
    let mut writer = Writer::new(&mut buf);
    writer.write(&tag(&[0x01]), b"abc").unwrap();
    writer.write(&tag(&[0x02]), &[]).unwrap();
    assert_eq!(writer.position(), 7);
    assert_eq!(
        writer.written(),
        &[0x01, 0x03, b'a', b'b', b'c', 0x02, 0x00]
    );
}

#[test]
fn tracks_capacity_and_remaining() {
    let mut buf = [0u8; 8];
    let mut writer = Writer::new(&mut buf);
    assert_eq!((writer.capacity(), writer.remaining()), (8, 8));
    writer.write(&tag(&[0x01]), &[0xFF]).unwrap();
    assert_eq!((writer.capacity(), writer.remaining()), (8, 5));
}

#[test]
fn buffer_too_short_leaves_position_unchanged() {
    let mut buf = [0u8; 4];
    let mut writer = Writer::new(&mut buf);
    writer.write(&tag(&[0x01]), &[0xAA]).unwrap();
    assert_eq!(
        writer.write(&tag(&[0x02]), &[1, 2, 3]),
        Err(Error::BufferTooShort)
    );
    assert_eq!(writer.position(), 3);
    // A smaller element that does not fit either still reports the same error.
    assert_eq!(writer.write(&tag(&[0x03]), &[]), Err(Error::BufferTooShort));
    assert_eq!(writer.position(), 3);
}

#[test]
fn empty_buffer_rejects_every_entry() {
    let mut writer = Writer::new(&mut []);
    assert_eq!(writer.write(&tag(&[0x01]), &[]), Err(Error::BufferTooShort));
    assert!(writer.written().is_empty());
}

#[test]
fn empty_tag_is_an_error() {
    let mut buf = [0u8; 8];
    let mut writer = Writer::new(&mut buf);
    let empty = Tag::from_bytes(&[]);
    assert!(writer.write(&empty, &[]).is_err());
    assert_eq!(writer.position(), 0);
}

#[test]
fn encoded_size_matches_written_size() {
    let value = vec![0u8; 300];
    let t = tag(&[0x9F, 0x02]);
    let size = encoded_size(&t, value.len(), Format::Ber).unwrap();
    let mut buf = vec![0u8; size];
    let mut writer = Writer::with_format(&mut buf, Format::Ber);
    writer.write(&t, &value).unwrap();
    assert_eq!(writer.position(), size);
    assert_eq!(writer.remaining(), 0);
}

#[test]
fn finish_returns_the_written_prefix() {
    let mut buf = [0u8; 8];
    let mut writer = Writer::new(&mut buf);
    writer.write(&tag(&[0x05]), &[0x01]).unwrap();
    assert_eq!(writer.finish(), &[0x05, 0x01, 0x01]);
}

fn round_trip(format: Format, entries: &[(&[u8], &[u8])]) {
    let mut buf = vec![0u8; 512];
    let mut writer = Writer::with_format(&mut buf, format);
    for (t, v) in entries {
        writer.write(&tag(t), v).unwrap();
    }
    let encoded = writer.finish();

    let decoded: Vec<_> = Reader::with_format(encoded, format)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(decoded.len(), entries.len(), "{format:?}");
    for (element, (t, v)) in decoded.iter().zip(entries) {
        assert_eq!(element.tag().as_bytes(), *t, "{format:?}");
        assert_eq!(element.value(), *v, "{format:?}");
    }
}

#[test]
fn round_trips_default_ber_format() {
    round_trip(
        Format::Ber,
        &[(&[0x01], b"hello"), (&[0x02], &[]), (&[0x5A], &[0x55; 300])],
    );
}

#[test]
fn round_trips_ber_format_with_multi_byte_tags() {
    round_trip(
        Format::Ber,
        &[
            (&[0x01], b"hello"),
            (&[0x02], &[]),
            (&[0x9F, 0x02], &[0xDE, 0xAD, 0xBE, 0xEF]),
            (&[0x40], &[0x55; 200]),
        ],
    );
}

#[test]
fn default_ber_format_accepts_multi_byte_tags() {
    let mut buf = [0u8; 16];
    let mut writer = Writer::new(&mut buf);
    assert_eq!(writer.write(&tag(&[0x9F, 0x02]), &[]), Ok(()));
    assert_eq!(writer.position(), 3);
}

#[test]
fn round_trips_default_and_der_formats() {
    for format in [Format::Ber, Format::Der] {
        let mut buf = [0u8; 32];
        let mut writer = Writer::with_format(&mut buf, format);
        writer.write(&tag(&[0x04]), &[1, 2, 3]).unwrap();
        let encoded = writer.finish();
        let element = Reader::with_format(encoded, format)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(element.tag().as_bytes(), &[0x04]);
        assert_eq!(element.value(), &[1, 2, 3]);
    }
}

#[test]
fn write_element_copies_entries_read_from_another_buffer() {
    let source = [0x01, 0x02, 0xAA, 0xBB, 0x02, 0x00];
    let mut buf = [0u8; 6];
    let mut writer = Writer::new(&mut buf);
    for element in Reader::new(&source) {
        writer.write_element(&element.unwrap()).unwrap();
    }
    assert_eq!(writer.written(), &source);
}
#[test]
fn preservation_and_owned_writer_diagnostics() {
    let input = [4, 0x81, 1, 42];
    let mut decoded = opentlv::decode(&input, Format::Ber).unwrap();
    let mut output = [0; 8];
    let mut writer = Writer::new(&mut output);
    writer.copy_encoded(&[1, 0]).unwrap();
    writer.preserve(&decoded).unwrap();
    assert_eq!(writer.written(), [1, 0, 4, 0x81, 1, 42]);
    decoded.element = opentlv::Element::new(Tag::from_bytes(&[4]), &[43]);
    assert_eq!(writer.preserve(&decoded), Err(Error::InvalidArg));
    assert_eq!(writer.diagnostic().unwrap().offset, Some(6));
    assert_eq!(writer.position(), 6);
    assert_eq!(
        writer.write(&Tag::from_bytes(&[4]), &[1]),
        Err(Error::BufferTooShort)
    );
    assert_eq!(
        writer.diagnostic().unwrap().tag,
        Some(Tag::from_bytes(&[4]))
    );
    assert_eq!(writer.diagnostic().unwrap().required, Some(3));
}

#[test]
fn single_element_measurement_and_diagnostics() {
    let element = opentlv::Element::new(Tag::from_bytes(&[4]), &[42]);
    assert_eq!(opentlv::measure_element(&element, Format::Ber).unwrap(), 3);
    let mut output = [0xAA; 2];
    let error = opentlv::write_element(&mut output, &element, Format::Ber).unwrap_err();
    assert_eq!(error.error, Error::BufferTooShort);
    assert_eq!(error.diagnostic.required, Some(3));
    assert_eq!(error.diagnostic.available, Some(2));
    assert_eq!(output, [0xAA; 2]);
    let mut output = [0; 3];
    assert_eq!(
        opentlv::write_element(&mut output, &element, Format::Ber).unwrap(),
        3
    );
    assert_eq!(output, [4, 1, 42]);
}
