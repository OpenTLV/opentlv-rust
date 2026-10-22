// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! The simplest possible round trip: write one element with the
//! configurable fixed-width format, then read it back. See parse.rs and
//! write.rs for a nested BER document, and the C `quick_start.c` and C++
//! `quick_start.cpp` examples for the same round trip in those languages.
//!
//! Run with `cargo run --example quick_start` from `bindings/rust`.

use opentlv::{ByteOrder, FixedFormat, FixedFormatConfig, Reader, Tag, Writer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // One tag byte and one length byte; config must outlive format, and format
    // must outlive its readers and writers.
    let config = FixedFormatConfig::new(1, 1, ByteOrder::Big);
    let format = FixedFormat::new(&config)?;

    let mut buf = [0u8; 64];
    let mut writer = Writer::with_fixed_format(&mut buf, &format);
    writer.write(&Tag::from_bytes(&[0x01]), b"Hello, world!")?;

    for element in Reader::with_fixed_format(writer.written(), &format) {
        println!("{}", std::str::from_utf8(element?.value())?);
    }
    Ok(())
}
