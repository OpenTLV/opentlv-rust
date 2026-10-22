#![cfg(feature = "lldp")]
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::{decode, Format, Reader, Tag, Writer};

#[test]
fn named_preset_roundtrips_and_preserves_packed_headers() {
    assert_eq!("LLDP".parse::<Format>().unwrap(), Format::Lldp);
    for length in [0, 255, 256, 511] {
        let value = vec![0xAA; length];
        let mut storage = vec![0; length + 2];
        let mut writer = Writer::with_format(&mut storage, Format::Lldp);
        writer.write(&Tag::from_bytes(&[127]), &value).unwrap();
        let wire = writer.finish();
        assert_eq!(wire[0], if length >= 256 { 255 } else { 254 });
        let element = Reader::with_format(wire, Format::Lldp)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(element.tag().as_bytes(), &[127]);
        assert_eq!(element.value(), &value);
        let decoded = decode(wire, Format::Lldp).unwrap();
        let mut preserved = vec![0; wire.len()];
        assert_eq!(decoded.preserve(&mut preserved).unwrap(), wire.len());
        assert_eq!(preserved, wire);
    }
}
