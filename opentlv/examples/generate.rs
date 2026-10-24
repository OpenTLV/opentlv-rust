// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Generates reproducible wire cases for a custom TLV format and checks a
//! round-trip property over them, the way a property-based test or a fuzz
//! corpus would use them. The Python `generate.py` example generates the same
//! cases.
//!
//! Each case is one complete stream produced by the canonical C generator
//! from a candidate table, limits, a seed and a case index. The same inputs
//! always produce the same bytes, so a failing case is replayed from its
//! index alone.
//!
//! Run with `cargo run --example generate` from the repository root.

use opentlv::{
    ByteOrder, ComposedFormat, ConstructedBit, Error, EscapedLength, FieldComposition,
    FixedIdentifier, Generator, GeneratorCandidate, GeneratorLimits, Tag, TreeReader,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let format = ComposedFormat::fields(
        &FieldComposition::new(
            FixedIdentifier::new(1),
            EscapedLength::new(0xFF, 2, ByteOrder::Big),
        )
        .with_constructed(ConstructedBit::new(0, 0x80, 0x80)),
    )?;
    let limits = GeneratorLimits {
        max_elements: 16,
        max_depth: 3,
        max_value_size: 300,
        max_case_size: 4096,
    };
    let generator = Generator::new(
        &format,
        [
            // Crosses the 0xFF length escape.
            GeneratorCandidate::new(Tag::from_bytes(&[0x01]), 0, 300),
            GeneratorCandidate::new(Tag::from_bytes(&[0x02]), 0, 8),
            // Constructed: receives generated children.
            GeneratorCandidate::new(Tag::from_bytes(&[0x81]), 0, 128),
        ],
        limits,
        2026,
    );
    println!(
        "generator version {}, workspace {} bytes",
        Generator::VERSION,
        generator.workspace_size()?
    );

    // Property: every case decodes completely through the Format and, with
    // the `document` feature, re-encodes byte-exactly through a Document.
    for case_index in 0..100 {
        let wire = generator.generate(case_index)?;
        let mut reader = TreeReader::new(&wire, &format, 4, 3, 16, true)?;
        loop {
            match reader.read() {
                Ok(_) => {}
                Err(Error::End) => break,
                Err(error) => return Err(format!("case {case_index}: {error}").into()),
            }
        }
        #[cfg(feature = "document")]
        {
            let document = opentlv::Document::parse(&wire, &format, 3, 16)?;
            assert_eq!(
                document.encode()?,
                wire,
                "case {case_index} does not round-trip"
            );
        }
    }

    let wire = generator.generate(7)?;
    println!("case 7: {} bytes", wire.len());
    let mut reader = TreeReader::new(&wire, &format, 4, 3, 16, true)?;
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
        let tag = item.decoded.element.tag();
        println!("{:indent$}{tag}: {kind}", "", indent = 2 * (item.depth + 1));
    }

    // Reuse caller storage of at least max_case_size bytes across cases,
    // without allocating.
    let mut output = vec![0u8; limits.max_case_size];
    let mut workspace = vec![0u8; generator.workspace_size()?];
    let written = generator.generate_into(7, &mut output, &mut workspace)?;
    assert_eq!(&output[..written], wire);
    Ok(())
}
