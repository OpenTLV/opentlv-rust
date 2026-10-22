// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::{Error, Format, Reader, Tag, Writer};

#[test]
fn emv_roundtrip_and_indefinite_rejection() {
    let mut buffer = [0u8; 16];
    let mut writer = Writer::with_format(&mut buffer, Format::Emv);
    writer.write(&Tag::from_bytes(&[0x9f, 2]), &[0; 6]).unwrap();
    assert_eq!(writer.written(), &[0x9f, 2, 6, 0, 0, 0, 0, 0, 0]);
    let element = Reader::with_format(writer.written(), Format::Emv)
        .next_element()
        .unwrap()
        .unwrap();
    assert_eq!(element.value(), &[0; 6]);
    let indefinite = [0x70, 0x80, 0, 0];
    assert_eq!(
        Reader::with_format(&indefinite, Format::Emv)
            .next_element()
            .unwrap()
            .unwrap_err(),
        Error::InvalidLength
    );
    assert!(Reader::with_format(&indefinite, Format::Ber)
        .next_element()
        .unwrap()
        .is_ok());
}
