// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Encodes a nested BER-TLV structure into caller-owned buffers with `Writer`,
//! then reads it back.
//!
//! Run with `cargo run --example writer` from `bindings/rust`.

use opentlv::{Error, Format, Reader, Result, Tag, Writer};

fn main() -> Result<()> {
    // Encode the children first; the writer never allocates.
    let mut inner_buf = [0u8; 32];
    let mut inner = Writer::with_format(&mut inner_buf, Format::Ber);
    inner.write(&Tag::from_bytes(&[0x84]), &[0xA0, 0x00, 0x00, 0x00, 0x03])?;
    inner.write(&Tag::from_bytes(&[0x50]), b"VISA")?;

    // Wrap them in a constructed template (6F).
    let mut outer_buf = [0u8; 64];
    let mut outer = Writer::with_format(&mut outer_buf, Format::Ber);
    outer.write(&Tag::from_bytes(&[0x6F]), inner.written())?;

    let encoded = outer.written();
    println!("encoded {} bytes: {encoded:02X?}", encoded.len());

    // Read the template back.
    for element in Reader::with_format(encoded, Format::Ber) {
        let element = element?;
        println!(
            "{:02X?} -> {} value bytes",
            element.tag().as_bytes(),
            element.value().len()
        );
    }

    // A failed write leaves the position unchanged.
    let mut small_buf = [0u8; 4];
    let mut small = Writer::new(&mut small_buf);
    match small.write(&Tag::from_bytes(&[0x01]), b"too long for the buffer") {
        Err(Error::BufferTooShort) => println!("buffer too short, position {}", small.position()),
        other => println!("unexpected result: {other:?}"),
    }
    Ok(())
}
