// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Decodes element Values with opentlv's generic codecs: packed digits, ASCII
//! text and an IPv4 address list, each through a public OpenTLV C codec
//! selected explicitly rather than by tag. The Python `value_codecs.py`
//! example decodes the same message.
//!
//! Run with `cargo run --example value_codecs` from the repository root.

use std::net::Ipv4Addr;

use opentlv::{DigitsCodec, Format, Ipv4ListCodec, Reader, TextCodec};

const MESSAGE: [u8; 23] = [
    0x5a, 0x05, 0x12, 0x34, 0x56, 0x78, 0x9f, // 5A: digits 123456789, F-padded
    0x50, 0x04, 0x56, 0x49, 0x53, 0x41, // 50: label "VISA"
    0x81, 0x08, 0xc0, 0x00, 0x02, 0x01, 0xc6, 0x33, 0x64, 0x01, // 81: two IPv4 addresses
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut digits = String::new();
    let mut label = "";
    let mut addresses = Vec::new();
    for element in Reader::with_format(&MESSAGE, Format::Ber) {
        let element = element?;
        let tag = element.tag();
        match tag.as_bytes() {
            [0x5a] => {
                digits = DigitsCodec::default().decode(element.value())?;
                println!("tag {tag}: {digits:?}");
            }
            [0x50] => {
                label = TextCodec::default().decode(element.value())?;
                println!("tag {tag}: {label:?}");
            }
            _ => {
                addresses = Ipv4ListCodec.decode(element.value())?.iter().collect();
                println!("tag {tag}: {addresses:?}");
            }
        }
    }
    assert_eq!(digits, "123456789");
    assert_eq!(label, "VISA");
    assert_eq!(
        addresses,
        [Ipv4Addr::new(192, 0, 2, 1), Ipv4Addr::new(198, 51, 100, 1)]
    );

    assert_eq!(DigitsCodec::default().encode(&digits)?, MESSAGE[2..7]);
    assert_eq!(Ipv4ListCodec.encode(&addresses)?, MESSAGE[15..]);

    // A is not a decimal digit.
    if let Err(failure) = DigitsCodec::default().decode(&[0x1a]) {
        let reason = failure.diagnostic.as_ref().map(|d| d.reason.name());
        println!("rejected: {failure} ({})", reason.unwrap_or("unknown"));
    }
    Ok(())
}
