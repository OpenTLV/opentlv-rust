// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Integration tests for format and format selection.

use opentlv::{Element, Error, Format, Limits, Reader, Strictness, Tag, ValidationError, Writer};

fn tag(bytes: &[u8]) -> Tag {
    Tag::from_bytes(bytes)
}

#[test]
fn formats_are_selected_by_name() {
    for format in Format::ALL {
        assert_eq!(format.name().parse::<Format>(), Ok(format));
        assert_eq!(format.to_string(), format.name());
    }
    assert_eq!("DER".parse::<Format>(), Ok(Format::Der));
    assert_eq!("nope".parse::<Format>(), Err(Error::InvalidArg));
}

#[test]
fn selected_format_drives_reader_and_writer() {
    let format: Format = "ber".parse().unwrap();
    let mut buf = [0u8; 16];
    let mut writer = Writer::with_format(&mut buf, format);
    writer.write(&tag(&[0x05]), &[0xAA]).unwrap();
    let encoded = writer.finish();
    assert_eq!(encoded, &[0x05, 0x01, 0xAA]);
    let element = Reader::with_format(encoded, format)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(element.value(), &[0xAA]);
}

#[test]
fn canonical_formats_have_default_limits() {
    for format in [Format::Der, Format::Cer] {
        let limits = format.default_limits().unwrap();
        assert!(limits.max_depth > 0 && limits.max_elements > 0);
    }
}

#[test]
fn der_validation_is_canonical() {
    let limits = Format::Der.default_limits().unwrap();
    let check = |data: &[u8]| Format::Der.validate(data, &limits, Strictness::Canonical);

    assert_eq!(check(&[]), Ok(()));
    assert_eq!(check(&[0x02, 0x01, 0x05]), Ok(()));
    // Non-minimal length encoding.
    assert!(check(&[0x02, 0x81, 0x01, 0x05]).is_err());
    // Indefinite length is not DER.
    assert!(check(&[0x30, 0x80, 0x00, 0x00]).is_err());
}

#[test]
fn errors_carry_the_offset_of_the_failure() {
    let limits = Format::Der.default_limits().unwrap();
    let data = [0x02, 0x01, 0x05, 0x02, 0x81, 0x01, 0x05];
    let err: ValidationError = Format::Der
        .validate(&data, &limits, Strictness::Canonical)
        .unwrap_err();
    assert!(
        err.location.offset().unwrap() >= 3,
        "offset {}",
        err.location.offset().unwrap()
    );
    assert_eq!(Error::from(err), err.error);
    assert!(err.to_string().contains("offset"));
}

#[test]
fn strict_validation_checks_universal_content() {
    let limits = Format::Der.default_limits().unwrap();
    // BOOLEAN TRUE must be encoded as FF in DER.
    let data = [0x01, 0x01, 0x01];
    assert_eq!(
        Format::Der.validate(&data, &limits, Strictness::Canonical),
        Ok(())
    );
    let err = Format::Der
        .validate(&data, &limits, Strictness::Strict)
        .unwrap_err();
    assert_eq!(err.error, Error::InvalidValue);
    assert_eq!(
        Format::Der.validate(&[0x01, 0x01, 0xFF], &limits, Strictness::Strict),
        Ok(())
    );
}

#[test]
fn limits_are_enforced() {
    let mut limits = Format::Der.default_limits().unwrap();
    limits.max_input_size = 2;
    let err = Format::Der
        .validate(&[0x02, 0x01, 0x05], &limits, Strictness::Canonical)
        .unwrap_err();
    assert_eq!(err.error, Error::Limit);

    let tight = Limits {
        max_depth: 0,
        ..Format::Der.default_limits().unwrap()
    };
    let nested = [0x30, 0x03, 0x02, 0x01, 0x05];
    assert_eq!(
        Format::Der
            .validate(&nested, &tight, Strictness::Canonical)
            .unwrap_err()
            .error,
        Error::Limit
    );
}

#[test]
fn cer_requires_indefinite_constructed_lengths() {
    let indefinite = [0x30, 0x80, 0x02, 0x01, 0x05, 0x00, 0x00];
    let cer = Format::Cer.default_limits().unwrap();
    let der = Format::Der.default_limits().unwrap();
    assert_eq!(
        Format::Cer.validate(&indefinite, &cer, Strictness::Canonical),
        Ok(())
    );
    assert!(Format::Der
        .validate(&indefinite, &der, Strictness::Canonical)
        .is_err());
    let definite = [0x30, 0x03, 0x02, 0x01, 0x05];
    assert!(Format::Cer
        .validate(&definite, &cer, Strictness::Canonical)
        .is_err());
    assert_eq!(
        Format::Der.validate(&definite, &der, Strictness::Canonical),
        Ok(())
    );
}

#[test]
fn read_returns_the_first_element_and_its_size() {
    let limits = Format::Der.default_limits().unwrap();
    let data = [0x04, 0x02, 0xAB, 0xCD, 0xFF];
    let (element, consumed): (Element<'_>, usize) = Format::Der
        .read(&data, &limits, Strictness::Strict)
        .unwrap();
    assert_eq!(element.tag().as_bytes(), &[0x04]);
    assert_eq!(element.value(), &[0xAB, 0xCD]);
    assert_eq!(consumed, 4);

    assert_eq!(
        Format::Der
            .read(&[], &limits, Strictness::Canonical)
            .unwrap_err()
            .error,
        Error::End
    );
}

#[test]
fn write_produces_canonical_bytes() {
    let limits = Format::Der.default_limits().unwrap();
    let octets = tag(&[0x04]);
    let value = [0xAB; 200];

    let size = Format::Der
        .encoded_size(&octets, &value, &limits, Strictness::Strict)
        .unwrap();
    assert_eq!(size, 203); // tag, two length bytes (81 C8), 200 value bytes
    let mut out = vec![0u8; size];
    let written = Format::Der
        .write(&octets, &value, &limits, Strictness::Strict, &mut out)
        .unwrap();
    assert_eq!(written, size);
    assert_eq!(&out[..3], &[0x04, 0x81, 0xC8]);

    // What was written reads back through the same format.
    let (element, consumed) = Format::Der.read(&out, &limits, Strictness::Strict).unwrap();
    assert_eq!((element.value(), consumed), (&value[..], size));

    let mut short = [0u8; 4];
    assert_eq!(
        Format::Der
            .write(&octets, &value, &limits, Strictness::Strict, &mut short)
            .unwrap_err()
            .error,
        Error::BufferTooShort
    );
}

#[test]
fn write_rejects_noncanonical_children() {
    let limits = Format::Der.default_limits().unwrap();
    let sequence = tag(&[0x30]);
    let mut out = [0u8; 16];
    // The child has a non-minimal length, so it is not canonical DER.
    let err = Format::Der
        .write(
            &sequence,
            &[0x02, 0x81, 0x01, 0x05],
            &limits,
            Strictness::Canonical,
            &mut out,
        )
        .unwrap_err();
    assert_ne!(err.error, Error::BufferTooShort);
    let ok = Format::Der
        .write(
            &sequence,
            &[0x02, 0x01, 0x05],
            &limits,
            Strictness::Canonical,
            &mut out,
        )
        .unwrap();
    assert_eq!(&out[..ok], &[0x30, 0x03, 0x02, 0x01, 0x05]);
}

#[test]
fn canonical_operations_reject_other_formats_without_writing() {
    let limits = Format::Der.default_limits().unwrap();
    let expected = ValidationError {
        error: Error::InvalidArg,
        location: opentlv::Location::default(),
    };
    for format in Format::ALL {
        if matches!(format, Format::Der | Format::Cer) {
            continue;
        }
        assert_eq!(format.default_limits(), Err(expected));
        for strictness in [Strictness::Canonical, Strictness::Strict] {
            assert_eq!(format.validate(&[], &limits, strictness), Err(expected));
            assert_eq!(
                format.read(&[0x04, 0], &limits, strictness).unwrap_err(),
                expected
            );
            assert_eq!(
                format.encoded_size(&tag(&[0x04]), &[], &limits, strictness),
                Err(expected)
            );
            let mut output = [0xAA; 8];
            assert_eq!(
                format.write(&tag(&[0x04]), &[], &limits, strictness, &mut output),
                Err(expected)
            );
            assert_eq!(output, [0xAA; 8]);
        }
    }
}
