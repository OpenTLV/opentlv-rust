// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Reads a BER-TLV buffer with `Reader`, descends into a constructed element and
//! decodes values with the EMV dictionary.
//!
//! Run with `cargo run --example reader` from `bindings/rust`.

use opentlv::emv::{self, Context};
use opentlv::{Format, Reader, Result};

/// An FCI template (6F) holding a DF name (84) and an application label (50).
const FCI: [u8; 17] = [
    0x6F, 0x0F, 0x84, 0x07, 0xA0, 0x00, 0x00, 0x00, 0x03, 0x10, 0x10, 0x50, 0x04, b'V', b'I', b'S',
    b'A',
];

fn print_elements(data: &[u8], depth: usize) -> Result<()> {
    // Elements borrow `data`; the reader itself can be dropped early.
    for element in Reader::with_format(data, Format::Ber) {
        let element = element?;
        let tag = element.tag();
        let name = emv::find(Context::Base, tag).map_or("unknown", |definition| definition.name());
        println!(
            "{:indent$}{:02X?} {name}: {:02X?}",
            "",
            tag.as_bytes(),
            element.value(),
            indent = depth * 2
        );

        // Bit 6 of the first tag byte marks a constructed (template) element.
        if tag.as_bytes()[0] & 0x20 != 0 {
            print_elements(element.value(), depth + 1)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    println!("OpenTLV {}", opentlv::version());
    print_elements(&FCI, 0)?;

    // Malformed input surfaces as an `Err` item; the iterator then ends.
    let truncated = &FCI[..FCI.len() - 2];
    for element in Reader::with_format(truncated, Format::Ber) {
        if let Err(error) = element {
            println!("truncated input: {error} ({error:?})");
        }
    }
    Ok(())
}
