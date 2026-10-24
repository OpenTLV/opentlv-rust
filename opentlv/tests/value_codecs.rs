// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Generic Value codecs: Core's integer, byte, IPv4, text and digit codecs
//! and the Schema number composition through the public C descriptors.

use std::net::Ipv4Addr;

use opentlv::{
    BytesCodec, CodecFailure, CodecOperation, CodecReason, DigitsCodec, Error, Format,
    IntegerCodec, Ipv4Codec, Ipv4ListCodec, LengthRule, NumberCodec, NumberEncoding, Reader,
    SchemaNumberCodec, Tag, TextAlphabet, TextCodec,
};

fn reason(failure: &CodecFailure) -> CodecReason {
    failure.diagnostic.as_ref().expect("C diagnostic").reason
}

#[test]
fn integer_codecs_convert_exact_widths() {
    assert_eq!(IntegerCodec::uint8().decode(&[0xfe]).unwrap(), 0xfe);
    assert_eq!(IntegerCodec::uint16_be().decode(&[1, 2]).unwrap(), 0x0102);
    assert_eq!(IntegerCodec::uint16_le().decode(&[1, 2]).unwrap(), 0x0201);
    assert_eq!(
        IntegerCodec::uint32_be().decode(&[1, 2, 3, 4]).unwrap(),
        0x0102_0304
    );
    assert_eq!(
        IntegerCodec::uint32_le().decode(&[1, 2, 3, 4]).unwrap(),
        0x0403_0201
    );
    assert_eq!(IntegerCodec::uint16_le().encode(0x0102).unwrap(), [2, 1]);
    assert_eq!(
        IntegerCodec::uint32_be().encode(0x0102_0304).unwrap(),
        [1, 2, 3, 4]
    );
    assert_eq!(IntegerCodec::uint8().encoded_size(7).unwrap(), 1);

    let failure = IntegerCodec::uint16_be().decode(&[1]).unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    assert_eq!(reason(&failure), CodecReason::Length);
    assert_eq!(
        failure.diagnostic.as_ref().unwrap().operation,
        CodecOperation::Decode
    );
    assert_eq!(IntegerCodec::uint16_be().name(), "tlv_codec_uint16_be");
    assert_ne!(IntegerCodec::uint16_be(), IntegerCodec::uint16_le());
}

#[test]
fn signed_integers_use_minimal_twos_complement() {
    let codec = IntegerCodec::int64_minimal_be();
    for (value, raw) in [
        (0i64, &[0x00][..]),
        (127, &[0x7f]),
        (128, &[0x00, 0x80]),
        (-1, &[0xff]),
        (-128, &[0x80]),
        (-129, &[0xff, 0x7f]),
        (i64::MIN, &[0x80, 0, 0, 0, 0, 0, 0, 0]),
    ] {
        assert_eq!(codec.encode(value).unwrap(), raw, "{value}");
        assert_eq!(codec.decode(raw).unwrap(), value, "{raw:02X?}");
    }
    // A redundant sign octet is not minimal.
    let failure = codec.decode(&[0x00, 0x7f]).unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    assert_eq!(reason(&failure), CodecReason::Canonical);
    assert_eq!(codec.decode(&[]).unwrap_err().error, Error::InvalidValue);
}

#[test]
fn failed_encoding_leaves_the_output_unchanged() {
    let mut out = [0xaa; 1];
    let failure = IntegerCodec::uint16_be()
        .encode_into(0x0102, &mut out)
        .unwrap_err();
    assert_eq!(failure.error, Error::BufferTooShort);
    assert_eq!(out, [0xaa]);
    let mut out = [0xaa; 3];
    assert_eq!(
        IntegerCodec::uint16_be()
            .encode_into(0x0102, &mut out)
            .unwrap(),
        2
    );
    assert_eq!(out, [1, 2, 0xaa]);
}

#[test]
fn bytes_borrow_the_input_and_copy_on_encode() {
    let raw = [1u8, 2, 3];
    let decoded = BytesCodec.decode(&raw).unwrap();
    assert_eq!(decoded, raw);
    assert_eq!(decoded.as_ptr(), raw.as_ptr());
    assert_eq!(BytesCodec.decode(&[]).unwrap(), &[] as &[u8]);
    assert_eq!(BytesCodec.encode(&raw).unwrap(), raw);
    assert_eq!(BytesCodec.encoded_size(&[]).unwrap(), 0);
    let mut out = [0u8; 2];
    assert_eq!(
        BytesCodec.encode_into(&raw, &mut out).unwrap_err().error,
        Error::BufferTooShort
    );
}

#[test]
fn ipv4_addresses_and_lists_keep_order_and_duplicates() {
    let address = Ipv4Addr::new(192, 0, 2, 1);
    assert_eq!(Ipv4Codec.decode(&[192, 0, 2, 1]).unwrap(), address);
    assert_eq!(Ipv4Codec.encode(address).unwrap(), [192, 0, 2, 1]);
    assert_eq!(
        Ipv4Codec.decode(&[192, 0, 2]).unwrap_err().error,
        Error::InvalidValue
    );

    let raw = [192, 0, 2, 1, 198, 51, 100, 1, 192, 0, 2, 1];
    let list = Ipv4ListCodec.decode(&raw).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list.as_bytes().as_ptr(), raw.as_ptr());
    assert_eq!(list.get(2), Some(address));
    assert_eq!(list.get(3), None);
    let addresses: Vec<Ipv4Addr> = list.iter().collect();
    assert_eq!(
        addresses,
        [address, Ipv4Addr::new(198, 51, 100, 1), address]
    );
    assert_eq!(Ipv4ListCodec.encode(&addresses).unwrap(), raw);

    let empty = Ipv4ListCodec.decode(&[]).unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.iter().count(), 0);
    assert_eq!(Ipv4ListCodec.encode(&[]).unwrap(), [] as [u8; 0]);
    let failure = Ipv4ListCodec.decode(&raw[..5]).unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    assert_eq!(reason(&failure), CodecReason::Length);
}

#[test]
fn text_validates_its_alphabet_width_and_padding() {
    let text = TextCodec::default();
    assert_eq!(text.alphabet(), TextAlphabet::AsciiPrintable);
    let raw = b"VISA card";
    let decoded = text.decode(raw).unwrap();
    assert_eq!(decoded, "VISA card");
    assert_eq!(decoded.as_ptr(), raw.as_ptr());
    assert_eq!(text.encode("VISA").unwrap(), b"VISA");

    let alnum = TextCodec::new(TextAlphabet::AsciiAlnum, 0, false);
    let failure = alnum.decode(b"AB C").unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    let diagnostic = failure.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic.reason, CodecReason::Text);
    assert_eq!(diagnostic.location.offset(), Some(2));
    assert_eq!(alnum.encode("AB C").unwrap_err().error, Error::InvalidValue);
    // Non-ASCII text reaches C as UTF-8 and is rejected there.
    assert_eq!(
        text.encode("caf\u{e9}").unwrap_err().error,
        Error::InvalidValue
    );

    let padded = TextCodec::new(TextAlphabet::AsciiPrintable, 4, true);
    assert_eq!(padded.encode("ab").unwrap(), b"ab\0\0");
    assert_eq!(padded.encoded_size("ab").unwrap(), 4);
    assert_eq!(padded.decode(b"ab\0\0").unwrap(), "ab");
    assert_eq!(
        padded.encode("abcde").unwrap_err().error,
        Error::InvalidValue
    );
    let fixed = TextCodec::new(TextAlphabet::AsciiPrintable, 4, false);
    assert_eq!(fixed.encode("ab").unwrap_err().error, Error::InvalidValue);
    assert_eq!(
        fixed.decode(b"ab\0\0").unwrap_err().error,
        Error::InvalidValue
    );
}

#[test]
fn digits_keep_leading_zeros_and_f_padding() {
    let digits = DigitsCodec::default();
    assert_eq!(digits.decode(&[0x01, 0x23, 0x4f]).unwrap(), "01234");
    assert_eq!(digits.encode("01234").unwrap(), [0x01, 0x23, 0x4f]);
    assert_eq!(digits.decode(&[]).unwrap(), "");
    let fixed = DigitsCodec::new(3);
    assert_eq!(fixed.width(), 3);
    assert_eq!(fixed.encode("0123").unwrap(), [0x01, 0x23, 0xff]);
    assert_eq!(fixed.decode(&[0x01, 0x23, 0xff]).unwrap(), "0123");
    assert_eq!(
        fixed.encode("0123456").unwrap_err().error,
        Error::InvalidValue
    );

    let failure = digits.decode(&[0x1a]).unwrap_err();
    assert_eq!(failure.error, Error::InvalidValue);
    assert_eq!(reason(&failure), CodecReason::Digit);
    assert_eq!(digits.encode("12a").unwrap_err().error, Error::InvalidValue);
}

#[test]
fn schema_numbers_choose_a_permitted_width() {
    let tag = Tag::from_bytes(&[0x9f, 0x02]);
    let mut rule = LengthRule::new(tag.clone(), 2, 4);
    rule.flags = 1;
    let codec = SchemaNumberCodec::new(
        rule.clone(),
        NumberCodec::new(NumberEncoding::BigEndian, 0, 0),
    );
    assert_eq!(codec.rule(), &rule);
    assert_eq!(codec.encode(5).unwrap(), [0x00, 0x05]);
    assert_eq!(codec.encode(0x0001_0000).unwrap(), [0, 1, 0, 0]);
    assert_eq!(codec.encoded_size(0x0001_0000).unwrap(), 4);
    assert_eq!(codec.decode(&[0, 1, 0, 0]).unwrap(), 0x0001_0000);

    // Three bytes are a valid number but not a permitted length.
    let failure = codec.decode(&[1, 0, 0]).unwrap_err();
    assert_eq!(failure.error, Error::Schema);
    assert!(failure.diagnostic.as_ref().unwrap().schema.is_some());

    // A fixed width the rule rejects is never changed.
    let fixed = SchemaNumberCodec::new(rule, NumberCodec::new(NumberEncoding::BigEndian, 3, 0));
    assert_eq!(fixed.encode(5).unwrap_err().error, Error::Schema);

    let bcd = SchemaNumberCodec::new(
        LengthRule::exact(tag, 6),
        NumberCodec::new(NumberEncoding::Bcd, 0, 12),
    );
    assert_eq!(bcd.encode(1234).unwrap(), [0, 0, 0, 0, 0x12, 0x34]);
    assert_eq!(bcd.decode(&[0, 0, 0, 0, 0x12, 0x34]).unwrap(), 1234);
}

#[test]
fn codecs_are_selected_explicitly_per_value() {
    let message = [
        0x5a, 0x05, 0x12, 0x34, 0x56, 0x78, 0x9f, // digits
        0x50, 0x04, b'V', b'I', b'S', b'A', // label
        0x81, 0x08, 192, 0, 2, 1, 198, 51, 100, 1, // IPv4 list
    ];
    let mut values = Vec::new();
    for element in Reader::with_format(&message, Format::Ber) {
        let element = element.unwrap();
        values.push(match element.tag().as_bytes() {
            [0x5a] => DigitsCodec::default().decode(element.value()).unwrap(),
            [0x50] => TextCodec::default()
                .decode(element.value())
                .unwrap()
                .to_string(),
            _ => Ipv4ListCodec
                .decode(element.value())
                .unwrap()
                .iter()
                .map(|address| address.to_string())
                .collect::<Vec<_>>()
                .join(","),
        });
    }
    assert_eq!(values, ["123456789", "VISA", "192.0.2.1,198.51.100.1"]);
}

#[test]
fn configured_widths_report_capacity_and_allocation_errors() {
    let text = TextCodec::new(TextAlphabet::AsciiPrintable, usize::MAX, true);
    assert_eq!(text.encoded_size("x").unwrap(), usize::MAX);
    assert_eq!(text.encode("x").unwrap_err().error, Error::OutOfMemory);

    let mut out = [0xaa; 1];
    let text = TextCodec::new(TextAlphabet::AsciiPrintable, 4, true);
    assert_eq!(
        text.encode_into("x", &mut out).unwrap_err().error,
        Error::BufferTooShort
    );
    assert_eq!(
        DigitsCodec::new(4)
            .encode_into("1", &mut out)
            .unwrap_err()
            .error,
        Error::BufferTooShort
    );
    assert_eq!(
        Ipv4Codec
            .encode_into(Ipv4Addr::LOCALHOST, &mut out)
            .unwrap_err()
            .error,
        Error::BufferTooShort
    );
    assert_eq!(
        Ipv4ListCodec
            .encode_into(&[Ipv4Addr::LOCALHOST], &mut out)
            .unwrap_err()
            .error,
        Error::BufferTooShort
    );
    let number = SchemaNumberCodec::new(
        LengthRule::exact(Tag::from_bytes(&[0x80]), 2),
        NumberCodec::new(NumberEncoding::BigEndian, 0, 0),
    );
    assert_eq!(
        number.encode_into(1, &mut out).unwrap_err().error,
        Error::BufferTooShort
    );
    assert_eq!(out, [0xaa]);
}
