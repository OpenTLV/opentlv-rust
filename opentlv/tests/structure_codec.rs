// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Structure codecs: Core validates a complete sequence before decoding and
//! the produced bytes before an encoding is returned.

use opentlv::{
    CodecCause, CodecFailure, Error, Format, IntegerCodec, Kind, LocationDomain, StructureCodec,
    StructureRule, StructureSchema, Tag, TextCodec, ValidationLimits, Writer,
};

/// A 30 record holding a 50 label and an optional 02 count.
fn schema() -> StructureSchema {
    let record = StructureSchema::new(
        [
            StructureRule::new(Tag::from_bytes(&[0x50])).required_once(),
            StructureRule::new(Tag::from_bytes(&[0x02])).length(1, 1),
        ],
        false,
    );
    StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0x30]))
            .required_once()
            .kind(Kind::Constructed)
            .children(record)],
        false,
    )
}

#[derive(Debug, PartialEq)]
struct Record<'a> {
    label: &'a str,
    count: Option<u8>,
}

fn decode_record(data: &[u8]) -> Result<Record<'_>, CodecFailure> {
    let mut label = "";
    let mut count = None;
    let record = opentlv::Reader::with_format(data, Format::Ber)
        .next()
        .ok_or(Error::End)??;
    for element in opentlv::Reader::with_format(record.value(), Format::Ber) {
        let element = element?;
        match element.tag().as_bytes() {
            [0x50] => label = TextCodec::default().decode(element.value())?,
            _ => count = Some(IntegerCodec::uint8().decode(element.value())?),
        }
    }
    Ok(Record { label, count })
}

fn encode_record(record: &Record<'_>) -> Result<Vec<u8>, CodecFailure> {
    let mut children = [0u8; 64];
    let mut writer = Writer::with_format(&mut children, Format::Ber);
    writer.write(
        &Tag::from_bytes(&[0x50]),
        &TextCodec::default().encode(record.label)?,
    )?;
    if let Some(count) = record.count {
        writer.write(
            &Tag::from_bytes(&[0x02]),
            &IntegerCodec::uint8().encode(count)?,
        )?;
    }
    let children = writer.written().to_vec();
    let mut out = vec![0u8; children.len() + 4];
    let mut writer = Writer::with_format(&mut out, Format::Ber);
    writer.write(&Tag::from_bytes(&[0x30]), &children)?;
    Ok(writer.written().to_vec())
}

const RECORD: [u8; 11] = [
    0x30, 0x09, 0x50, 0x04, b'V', b'I', b'S', b'A', 0x02, 0x01, 0x07,
];

#[test]
fn decode_validates_before_converting() {
    let schema = schema();
    let codec = StructureCodec::new(Format::Ber).with_schema(&schema);
    assert!(codec.schema().is_some());
    let record = codec.decode(&RECORD, decode_record).unwrap();
    assert_eq!(
        record,
        Record {
            label: "VISA",
            count: Some(7)
        }
    );
    // The decoded object borrows the input.
    assert_eq!(record.label.as_ptr(), RECORD[4..].as_ptr());

    // A missing required label is a Schema failure; the closure never runs.
    let missing = [0x30, 0x03, 0x02, 0x01, 0x07];
    let mut called = false;
    let failure = codec
        .decode(&missing, |_| {
            called = true;
            Ok(())
        })
        .unwrap_err();
    assert!(!called);
    assert_eq!(failure.error, Error::Schema);
    let diagnostic = failure.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic.cause, CodecCause::Schema);
    assert!(diagnostic.schema.is_some());
}

#[test]
fn without_a_schema_framing_and_limits_are_validated() {
    let codec = StructureCodec::new(Format::Ber);
    assert_eq!(codec.decode(&RECORD, |data| Ok(data.len())).unwrap(), 11);

    let truncated = &RECORD[..10];
    let failure = codec.decode(truncated, |_| Ok(())).unwrap_err();
    assert_eq!(
        failure.diagnostic.as_ref().unwrap().cause,
        CodecCause::Reader
    );

    let limited = codec.with_limits(ValidationLimits {
        max_depth: 32,
        max_elements: 2,
    });
    assert_eq!(limited.limits().max_elements, 2);
    let failure = limited.decode(&RECORD, |_| Ok(())).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(
        failure.diagnostic.as_ref().unwrap().cause,
        CodecCause::Reader
    );
}

#[test]
fn closure_failures_are_returned_unchanged() {
    let codec = StructureCodec::new(Format::Ber);
    let failure = codec
        .decode(&RECORD, |_| -> Result<(), _> {
            Err(CodecFailure::from(Error::InvalidValue))
        })
        .unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    assert!(failure.diagnostic.is_none());
    let failure = codec
        .encode(|| -> Result<Vec<u8>, _> { Err(CodecFailure::from(Error::Unsupported)) })
        .unwrap_err();
    assert_eq!(failure.error, Error::Unsupported);
}

#[test]
fn encode_validates_the_produced_bytes() {
    let schema = schema();
    let codec = StructureCodec::new(Format::Ber).with_schema(&schema);
    let record = Record {
        label: "VISA",
        count: Some(7),
    };
    let mut calls = 0;
    let encoded = codec
        .encode(|| {
            calls += 1;
            encode_record(&record)
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(encoded, RECORD);
    assert_eq!(codec.decode(&encoded, decode_record).unwrap(), record);
    assert_eq!(codec.encoded_size(|| encode_record(&record)).unwrap(), 11);

    let mut out = [0u8; 16];
    let written = codec
        .encode_into(&mut out, || encode_record(&record))
        .unwrap();
    assert_eq!(&out[..written], RECORD);
    let mut short = [0u8; 10];
    assert_eq!(
        codec
            .encode_into(&mut short, || encode_record(&record))
            .unwrap_err()
            .error,
        Error::BufferTooShort
    );

    // An invalid 02 count length is reported in output coordinates.
    let invalid = [
        0x30, 0x0a, 0x50, 0x04, b'V', b'I', b'S', b'A', 0x02, 0x02, 0, 7,
    ];
    let failure = codec.encode(|| Ok(invalid)).unwrap_err();
    assert_eq!(failure.error, Error::Schema);
    let diagnostic = failure.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic.location.domain, LocationDomain::Output);
    assert_eq!(diagnostic.location.offset(), Some(8));
    // A size query does not validate.
    assert_eq!(codec.encoded_size(|| Ok(invalid)).unwrap(), invalid.len());
}

#[test]
fn checked_schemas_and_empty_sequences() {
    let schema = schema();
    let checked = schema.check().unwrap();
    let codec = StructureCodec::new(Format::Der).with_schema(checked.schema());
    assert_eq!(codec.decode(&RECORD, decode_record).unwrap().label, "VISA");
    // The required record is missing from an empty sequence.
    assert_eq!(codec.encode(|| Ok([])).unwrap_err().error, Error::Schema);
    let framing = StructureCodec::new(Format::Ber);
    assert_eq!(framing.encode(|| Ok([])).unwrap(), [] as [u8; 0]);
    assert_eq!(framing.decode(&[], |data| Ok(data.len())).unwrap(), 0);
}
