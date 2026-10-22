// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

#[cfg(feature = "nfc")]
#[test]
fn nfc_stream_roundtrip_and_extended_length() {
    use opentlv::{Error, Format, Reader, Tag, Writer};
    assert_eq!("nfc-type2".parse::<Format>().unwrap(), Format::NfcType2);
    let wire = [0, 3, 3, 0xD1, 1, 0, 0xFE, 0];
    let mut output = [0u8; 8];
    let mut writer = Writer::with_format(&mut output, Format::NfcType2);
    let mut reader = Reader::with_format(&wire, Format::NfcType2);
    let mut count = 0;
    while let Some(element) = reader.next_element() {
        let element = element.unwrap();
        writer.write(element.tag(), element.value()).unwrap();
        count += 1;
    }
    assert_eq!(count, 4);
    assert_eq!(writer.written(), wire);
    let mut buffer = [0; 259];
    let mut writer = Writer::with_format(&mut buffer, Format::NfcType2);
    writer.write(&Tag::from_bytes(&[3]), &[42; 255]).unwrap();
    assert_eq!(&writer.written()[..4], &[3, 255, 0, 255]);
    assert_eq!(
        Reader::with_format(writer.written(), Format::NfcType2)
            .next_element()
            .unwrap()
            .unwrap()
            .value()
            .len(),
        255
    );
    assert_eq!(
        Reader::with_format(&[3, 255, 255, 255], Format::NfcType2)
            .next_element()
            .unwrap()
            .unwrap_err(),
        Error::InvalidLength
    );
}

#[cfg(not(feature = "nfc"))]
#[test]
fn nfc_disabled() {
    assert!("nfc-type2".parse::<opentlv::Format>().is_err());
}
