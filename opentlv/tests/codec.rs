// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Integration tests for value codecs and the EMV dictionary.

use opentlv::emv::{self, Context};
use opentlv::{
    AccountType, AflEntry, BiometricType, Codec, CodecFailure, CryptogramInfo, CryptogramType,
    CvmResult, Date, Error, Format, Reader, Tag, Time, Track2, Value, ValueKind,
};

fn tag(bytes: &[u8]) -> Tag {
    Tag::from_bytes(bytes)
}

fn codec(context: Context, bytes: &[u8]) -> Codec {
    emv::find(context, &tag(bytes))
        .unwrap_or_else(|| panic!("no definition for {bytes:02X?}"))
        .codec()
        .expect("definition has a codec")
}

/// Decodes `raw`, checks the value and encodes it back to the same bytes.
fn round_trip(codec: Codec, raw: &[u8], expected: Value) {
    assert_eq!(codec.decode(raw).unwrap(), expected);
    assert_eq!(codec.encoded_size(&expected).unwrap(), raw.len());
    assert_eq!(codec.encode(&expected).unwrap(), raw);
}

#[test]
fn amount_codec_round_trips_bcd() {
    let amount = Codec::amount();
    assert_eq!(amount.kind(), ValueKind::Number);
    round_trip(amount, &[0, 0, 0, 0, 0x12, 0x34], Value::Number(1234));
    assert_eq!(
        amount.decode(&[0, 0, 0, 0, 0x1A, 0x34]),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
    assert_eq!(
        amount.decode(&[0, 0, 0]),
        Err(CodecFailure::from(opentlv::Error::Schema))
    );
}

#[test]
fn dictionary_definitions_expose_metadata() {
    let afl = emv::find(Context::Base, &tag(&[0x94])).unwrap();
    assert_eq!(afl.name(), "afl");
    assert_eq!(afl.tag(), tag(&[0x94]));
    assert_eq!(afl.kind(), ValueKind::Afl);
    assert_eq!(
        (afl.min_length(), afl.max_length(), afl.length_step()),
        (4, Some(252), 4)
    );
    assert_eq!(afl.display_label(), Some("Application File Locator (AFL)"));
    assert_eq!(afl.validate_length(8), Ok(()));
    assert_eq!(afl.validate_length(5), Err(Error::Schema));

    let label = emv::find(Context::Base, &tag(&[0x50])).unwrap();
    assert_eq!(label.kind(), ValueKind::Text);
    assert!(label.codec().is_none());

    let unbounded = emv::find(Context::Base, &tag(&[0x90])).unwrap();
    assert_eq!(unbounded.max_length(), None);

    assert!(emv::find(Context::Base, &tag(&[0x01])).is_none());
    // Contexts never fall back to the base dictionary.
    assert!(emv::find(Context::Bht, &tag(&[0x94])).is_none());
}

#[test]
fn context_follows_templates() {
    assert_eq!(Context::Base.child(&tag(&[0x7F, 0x60])), Some(Context::Bit));
    // A known base template keeps its children in the base context ...
    assert_eq!(Context::Base.child(&tag(&[0x6F])), Some(Context::Base));
    // ... while an unknown, possibly proprietary container is not guessed at.
    assert_eq!(Context::Base.child(&tag(&[0x01])), None);
}

#[test]
fn flags_and_numbers() {
    round_trip(
        codec(Context::Base, &[0x82]),
        &[0x20, 0x00],
        Value::Flags(0x2000),
    );
    round_trip(
        codec(Context::Base, &[0x9F, 0x1A]),
        &[0x08, 0x40],
        Value::Number(840),
    );
    round_trip(
        codec(Context::Base, &[0x9F, 0x03]),
        &[0, 0, 0, 0, 0x05, 0x00],
        Value::Number(500),
    );
}

#[test]
fn digits_drop_the_padding_nibble() {
    let pan = codec(Context::Base, &[0x5A]);
    round_trip(
        pan,
        &[0x41, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11],
        Value::Digits("4111111111111111".into()),
    );
    assert_eq!(
        pan.decode(&[0x41, 0x11, 0x1F]).unwrap(),
        Value::Digits("41111".into())
    );
    assert_eq!(
        pan.encode(&Value::Digits("41111".into())).unwrap(),
        [0x41, 0x11, 0x1F]
    );
    assert_eq!(
        pan.decode(&[]),
        Err(CodecFailure::from(opentlv::Error::Schema))
    );
}

#[test]
fn dates_and_times_are_range_checked() {
    let date = codec(Context::Base, &[0x9A]);
    round_trip(
        date,
        &[0x25, 0x12, 0x31],
        Value::Date(Date {
            year: 25,
            month: 12,
            day: 31,
        }),
    );
    assert_eq!(
        date.decode(&[0x25, 0x13, 0x01]),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );

    let time = codec(Context::Base, &[0x9F, 0x21]);
    round_trip(
        time,
        &[0x12, 0x34, 0x56],
        Value::Time(Time {
            hour: 12,
            minute: 34,
            second: 56,
        }),
    );
    assert_eq!(
        time.decode(&[0x24, 0x00, 0x00]),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
}

#[test]
fn enumerations_reject_undefined_values() {
    let account = codec(Context::Base, &[0x5F, 0x57]);
    round_trip(account, &[0x10], Value::Account(AccountType::Savings));
    round_trip(account, &[0x30], Value::Account(AccountType::Credit));
    assert_eq!(
        account.decode(&[0x11]),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );

    let biometric = codec(Context::Bht, &[0x81]);
    round_trip(biometric, &[0x08], Value::Biometric(BiometricType::Finger));
    assert_eq!(
        biometric.decode(&[0x01]),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );

    let cid = codec(Context::Base, &[0x9F, 0x27]);
    round_trip(
        cid,
        &[0x80],
        Value::Cryptogram(CryptogramInfo {
            kind: CryptogramType::Arqc,
            flags: 0,
        }),
    );
    round_trip(
        cid,
        &[0x45],
        Value::Cryptogram(CryptogramInfo {
            kind: CryptogramType::Tc,
            flags: 0x05,
        }),
    );
}

#[test]
fn afl_round_trips_and_validates_entries() {
    let afl = codec(Context::Base, &[0x94]);
    let elements = vec![
        AflEntry {
            sfi: 1,
            first_record: 1,
            last_record: 3,
            offline_auth_record_count: 1,
        },
        AflEntry {
            sfi: 2,
            first_record: 1,
            last_record: 1,
            offline_auth_record_count: 0,
        },
    ];
    round_trip(
        afl,
        &[0x08, 0x01, 0x03, 0x01, 0x10, 0x01, 0x01, 0x00],
        Value::Afl(elements),
    );
    // SFI 0 is reserved.
    assert_eq!(
        afl.decode(&[0x00, 0x01, 0x01, 0x00]),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
    let too_many = Value::Afl(vec![AflEntry::default(); 64]);
    assert_eq!(
        afl.encoded_size(&too_many),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
}

#[test]
fn cvm_results_are_preserved_raw() {
    round_trip(
        codec(Context::Base, &[0x9F, 0x34]),
        &[0x1E, 0x03, 0x02],
        Value::CvmResult(CvmResult {
            method: 0x1E,
            condition: 0x03,
            result: 0x02,
        }),
    );
}

#[test]
fn track2_round_trips() {
    let raw = [
        0x54, 0x13, 0x33, 0x00, 0x89, 0x02, 0x00, 0x11, 0xD2, 0x51, 0x22, 0x01, 0x00, 0x00,
    ];
    let value = Value::Track2(Track2 {
        pan: "5413330089020011".into(),
        expiration_year: 25,
        expiration_month: 12,
        service_code: 201,
        discretionary_data: "0000".into(),
    });
    let track2 = codec(Context::Base, &[0x57]);
    round_trip(track2, &raw, value);

    // A PAN that does not fit the C representation is rejected, not truncated.
    let long = Value::Track2(Track2 {
        pan: "1".repeat(20),
        ..Track2::default()
    });
    assert_eq!(
        track2.encoded_size(&long),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
    // Month 0 is invalid.
    let bad = Value::Track2(Track2 {
        pan: "1234".into(),
        ..Track2::default()
    });
    assert_eq!(
        track2.encoded_size(&bad),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
}

#[test]
fn encoding_reports_errors() {
    let aip = codec(Context::Base, &[0x82]);
    // Wrong value kind for the codec.
    assert_eq!(
        aip.encode(&Value::Number(1)),
        Err(CodecFailure::from(opentlv::Error::InvalidValue))
    );
    // Value too large for the two-byte field.
    assert_eq!(
        aip.encode(&Value::Flags(0x1_0000)),
        Err(CodecFailure::from(opentlv::Error::Schema))
    );

    let mut small = [0u8; 1];
    assert_eq!(
        aip.encode_into(&Value::Flags(0x2000), &mut small),
        Err(CodecFailure::from(opentlv::Error::BufferTooShort))
    );
    let mut exact = [0u8; 4];
    assert_eq!(aip.encode_into(&Value::Flags(0x2000), &mut exact), Ok(2));
    assert_eq!(&exact[..2], &[0x20, 0x00]);
}

#[test]
fn codec_errors_display_the_c_description() {
    assert!(!CodecFailure::from(opentlv::Error::InvalidValue)
        .to_string()
        .is_empty());
    assert_eq!(
        Error::from_code(CodecFailure::from(opentlv::Error::BufferTooShort).code()),
        Some(Error::BufferTooShort)
    );
    assert_eq!(Error::from_code(0), None);
    assert_eq!(Error::from_code(999), Some(Error::Unknown(999)));
    let _: &dyn std::error::Error = &CodecFailure::from(opentlv::Error::Unsupported);
}

#[test]
fn value_kinds_describe_themselves() {
    assert_eq!(ValueKind::Flags.description(), "Bit flags");
    assert!(!ValueKind::Bytes.has_codec());
    assert!(ValueKind::Track2.has_codec());
}

#[test]
fn reader_and_dictionary_decode_a_gpo_response() {
    let gpo = [
        0x77, 0x0A, 0x82, 0x02, 0x20, 0x00, 0x94, 0x04, 0x08, 0x01, 0x01, 0x00,
    ];
    let template = Reader::with_format(&gpo, Format::Ber)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(template.tag().as_bytes(), &[0x77]);

    let mut decoded = Vec::new();
    for element in Reader::with_format(template.value(), Format::Ber) {
        let element = element.unwrap();
        let definition = emv::find(Context::Base, element.tag()).unwrap();
        definition.validate_length(element.value().len()).unwrap();
        let value = definition.codec().unwrap().decode(element.value()).unwrap();
        decoded.push((definition.name(), value));
    }
    assert_eq!(decoded[0], ("aip", Value::Flags(0x2000)));
    assert_eq!(
        decoded[1],
        (
            "afl",
            Value::Afl(vec![AflEntry {
                sfi: 1,
                first_record: 1,
                last_record: 1,
                offline_auth_record_count: 0
            }])
        )
    );
}

#[test]
fn configured_numeric_codecs_delegate_measure_decode_and_encode_to_c() {
    use opentlv::{NumberCodec, NumberEncoding};
    for (encoding, width, digits, value, wire) in [
        (NumberEncoding::BigEndian, 2, 0, 258, vec![1, 2]),
        (NumberEncoding::LittleEndian, 2, 0, 258, vec![2, 1]),
        (NumberEncoding::Bcd, 3, 6, 12345, vec![1, 0x23, 0x45]),
        (NumberEncoding::BigEndian, 0, 0, 0, vec![0]),
        (NumberEncoding::BigEndian, 8, 0, u64::MAX, vec![255; 8]),
    ] {
        let codec = NumberCodec::new(encoding, width, digits);
        assert_eq!(codec.encode(value).unwrap(), wire);
        assert_eq!(codec.decode(&wire).unwrap(), value);
        assert_eq!(codec.encoded_size(value).unwrap(), wire.len());
        let mut output = vec![0xAA; wire.len() + 1];
        assert_eq!(codec.encode_into(value, &mut output).unwrap(), wire.len());
        assert_eq!(&output[..wire.len()], wire);
        assert_eq!(output[wire.len()], 0xAA);
        let mut short = vec![0xAA; wire.len() - 1];
        assert!(codec.encode_into(value, &mut short).is_err());
        assert_eq!(short, vec![0xAA; wire.len() - 1]);
    }
    assert!(NumberCodec::new(NumberEncoding::BigEndian, 1, 0)
        .encode(256)
        .is_err());
    assert!(NumberCodec::new(NumberEncoding::Bcd, 1, 2)
        .decode(&[0xFA])
        .is_err());
}

#[test]
fn conversion_diagnostic_keeps_shared_result_and_owned_context() {
    let failure = Codec::amount().decode(&[1]).unwrap_err();
    assert_eq!(failure.error, Error::Schema);
    let diagnostic = failure.diagnostic.unwrap();
    assert_eq!(diagnostic.code, Error::Schema.code());
    assert_eq!(diagnostic.reported, Error::Schema.code());
    assert_eq!(diagnostic.operation, opentlv::CodecOperation::Decode);
    assert_eq!(diagnostic.violation, opentlv::CodecViolation::None);
    let cause = diagnostic.schema.unwrap();
    assert_eq!(cause.error, Error::Schema);
    let length = cause.length.unwrap();
    assert_eq!(length.minimum, 6);
    assert_eq!(length.maximum, 6);
    assert_eq!(length.actual, 1);
}
