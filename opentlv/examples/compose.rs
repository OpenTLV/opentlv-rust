// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Defines a proprietary TLV format from generic field encodings and uses it
//! with the standard Tree Writer, Tree Reader, Query and Document, without
//! writing C. The Python `compose.py` example builds the same format.
//!
//! Each element has a one-byte tag and an escaped length: one byte below 0xFF,
//! or 0xFF followed by a two-byte big-endian count. Tags with the top bit set
//! hold nested elements.
//!
//! Run with `cargo run --example compose` from the repository root.

use opentlv::{
    ByteOrder, ComposedFormat, ConstructedBit, Element, Error, EscapedLength, FieldComposition,
    FixedIdentifier, ProgramOptions, QueryProgram, QueryValue, Tag, TreeReader, TreeWriter, Visit,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let format = ComposedFormat::fields(
        &FieldComposition::new(
            FixedIdentifier::new(1),
            EscapedLength::new(0xFF, 2, ByteOrder::Big),
        )
        .with_constructed(ConstructedBit::new(0, 0x80, 0x80)),
    )?;

    let mut output = [0u8; 1024];
    let mut writer = TreeWriter::new(&mut output, &format, 4, 1024, 4, 16)?;
    writer.begin(&Tag::from_bytes(&[0x81]))?;
    writer.write(&Element::new(Tag::from_bytes(&[0x01]), &[0; 300]))?;
    writer.write(&Element::new(Tag::from_bytes(&[0x02]), b"name"))?;
    writer.end()?;
    let wire = writer.finish()?.to_vec();
    println!("wire: {:02X?} ... {} bytes", &wire[..6], wire.len());

    let mut reader = TreeReader::new(&wire, &format, 4, 4, 16, true)?;
    loop {
        let item = match reader.read() {
            Ok(item) => item,
            Err(Error::End) => break,
            Err(error) => return Err(error.into()),
        };
        let kind = if item.constructed {
            "constructed".to_string()
        } else {
            format!("{} bytes", item.decoded.element.value().len())
        };
        let tag = item.decoded.element.tag().as_bytes().to_vec();
        println!("{:indent$}{tag:02X?}: {kind}", "", indent = 2 * item.depth);
    }

    // A Query program compiled for the composed Format; share clones of one
    // owner between the program and its inputs.
    let options = ProgramOptions {
        composed_format: Some(format.clone()),
        ..ProgramOptions::default()
    };
    let program = QueryProgram::compile("count(//02)", &options)?;
    let mut execution = program.execution(4, 16, 10_000, true)?;
    let mut reader = TreeReader::new(&wire, &format, 4, 4, 16, true)?;
    execution.visit(&mut reader, |_| Visit::Continue)?;
    if let QueryValue::Integer(count) = execution.result()? {
        println!("count(//02) -> {count}");
    }

    #[cfg(feature = "document")]
    {
        let document = opentlv::Document::parse(&wire, &format, 4, 16)?;
        let name = document
            .first()
            .and_then(|parent| parent.find(&Tag::from_bytes(&[0x02])))
            .ok_or("missing name")?;
        println!("name -> {}", std::str::from_utf8(name.value())?);
        assert_eq!(document.encode()?, wire);
    }

    // The Format itself decodes, measures and encodes one element.
    let operations = format.as_format();
    println!(
        "{:?}",
        operations.measure(&Element::new(Tag::from_bytes(&[0x02]), b"name"))?
    );
    assert_eq!(
        operations.decode(&wire)?.element.tag(),
        &Tag::from_bytes(&[0x81])
    );
    Ok(())
}
