// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Builds the same nested BER-TLV document `parse.rs` reads, encoding the
//! innermost elements first and using each encoded result as the next
//! level's value: the standard way to build constructed TLV bottom-up.
//!
//! Run with `cargo run --example write` from `bindings/rust`.

use opentlv::{Format, Result, Tag, Writer};

/// Same bytes as `parse.rs`'s document.
const EXPECTED: [u8; 12] = [
    0x6F, 0x0A, 0x84, 0x03, 0x41, 0x42, 0x43, 0xA5, 0x03, 0x50, 0x01, 0x01,
];

fn main() -> Result<()> {
    let mut df_name_buf = [0u8; 5];
    let mut df_name = Writer::with_format(&mut df_name_buf, Format::Ber);
    df_name.write(&Tag::from_bytes(&[0x84]), b"ABC")?;

    let mut label_buf = [0u8; 3];
    let mut label = Writer::with_format(&mut label_buf, Format::Ber);
    label.write(&Tag::from_bytes(&[0x50]), &[0x01])?;

    let mut proprietary_buf = [0u8; 8];
    let mut proprietary = Writer::with_format(&mut proprietary_buf, Format::Ber);
    proprietary.write(&Tag::from_bytes(&[0xA5]), label.written())?;

    let mut value = Vec::new();
    value.extend_from_slice(df_name.written());
    value.extend_from_slice(proprietary.written());

    let mut document_buf = [0u8; 20];
    let mut document = Writer::with_format(&mut document_buf, Format::Ber);
    document.write(&Tag::from_bytes(&[0x6F]), &value)?;

    println!(
        "wrote {} bytes: {:02X?}",
        document.written().len(),
        document.written()
    );
    assert_eq!(document.written(), &EXPECTED);
    Ok(())
}
