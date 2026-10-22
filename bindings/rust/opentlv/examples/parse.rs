// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Parses a nested BER-TLV document and prints every element in document
//! order. See `write.rs` for building the same bytes and `validate.rs` for
//! checking the document's structure without decoding it. The C, C++ and
//! JavaScript "parse" examples parse the same bytes and report the same
//! fields.
//!
//! Run with `cargo run --example parse` from `bindings/rust`.

use opentlv::{Format, Reader, Result};

/// An FCI Template (6F) holding a DF Name (84) and an FCI Proprietary
/// Template (A5) holding an Application Label (50).
const DOCUMENT: [u8; 12] = [
    0x6F, 0x0A, 0x84, 0x03, 0x41, 0x42, 0x43, 0xA5, 0x03, 0x50, 0x01, 0x01,
];

fn print_elements(data: &[u8], depth: usize, count: &mut usize) -> Result<()> {
    // Elements borrow `data`; nesting is descended by re-reading a child
    // element's value with a new `Reader`.
    for element in Reader::with_format(data, Format::Ber) {
        let element = element?;
        let tag = element.tag();
        println!(
            "{:indent$}tag={:02X?} length={} value={:02X?}",
            "",
            tag.as_bytes(),
            element.value().len(),
            element.value(),
            indent = depth * 2
        );
        *count += 1;

        // Bit 6 of the first tag byte marks a constructed (nested) element.
        if tag.as_bytes()[0] & 0x20 != 0 {
            print_elements(element.value(), depth + 1, count)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut count = 0;
    print_elements(&DOCUMENT, 0, &mut count)?;
    // 6F, its two children (84, A5) and A5's child (50).
    assert_eq!(count, 4);
    Ok(())
}
