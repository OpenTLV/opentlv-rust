// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Validates and queries a batch of messages with work done once up front: a
//! structural schema checked once and a Query program compiled once. The
//! Python `batch.py` example processes the same messages.
//!
//! Each message is checked against the prepared schema without repeating the
//! definition check, then one reusable execution evaluates the program over it
//! and is reset in place for the next message. A path query also visits the
//! raw buffer directly, and the Document view visits the same matches as nodes.
//!
//! Run with `cargo run --example batch` from the repository root.

use opentlv::{
    Format, NumberCodec, NumberEncoding, ProgramOptions, Query, QueryBinding, QueryConversion,
    QueryProgram, QueryProvider, QueryType, QueryValue, StructureRule, StructureSchema, Tag,
    TreeReader, ValidationLimits, Visit,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let messages: [&[u8]; 4] = [
        &[
            0x6f, 0x11, 0x84, 0x03, 0x41, 0x42, 0x43, 0xa5, 0x0a, 0x9f, 0x02, 0x02, 0x01, 0x2c,
            0x9f, 0x02, 0x02, 0x00, 0xc8,
        ],
        &[0x6f, 0x05, 0x84, 0x03, 0x44, 0x45, 0x46],
        // The amount is too short.
        &[
            0x6f, 0x0b, 0x84, 0x03, 0x47, 0x48, 0x49, 0xa5, 0x04, 0x9f, 0x02, 0x01, 0x01,
        ],
        // The DF name is missing.
        &[0x6f, 0x07, 0xa5, 0x05, 0x9f, 0x02, 0x02, 0x01, 0x2c],
    ];

    // 6F (FCI template) holds one 84 (DF name) and an optional A5 with 9F02 amounts.
    let proprietary = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0x9f, 0x02]))
            .occurs(1, 4)
            .length(2, 2)],
        false,
    );
    let fci = StructureSchema::new(
        [
            StructureRule::new(Tag::from_bytes(&[0x84]))
                .required_once()
                .length(0, 16),
            StructureRule::new(Tag::from_bytes(&[0xa5]))
                .occurs(0, 1)
                .children(proprietary),
        ],
        false,
    );
    let definition = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0x6f]))
            .required_once()
            .children(fci)],
        false,
    );
    let schema = definition.check()?;

    // Amounts are 2-byte big-endian numbers, decoded by Core's number codec provider.
    let amount = NumberCodec::new(NumberEncoding::BigEndian, 2, 0);
    let mut options = ProgramOptions::default();
    options
        .variables
        .insert("minimum".into(), QueryType::Integer);
    options
        .providers
        .push(QueryProvider::number(QueryConversion::Num, 1001, amount));
    let total = QueryProgram::compile("count(//9F02[num(.) >= $minimum])", &options)?;
    println!(
        "{} ({} instructions)",
        total.format()?,
        total.plan_info()?.instructions
    );
    let mut execution = total.execution(8, 64, 100_000, true)?;

    let limits = ValidationLimits::default();
    for (index, message) in messages.iter().enumerate() {
        if let Err(error) = schema.validate(message, Format::Ber, &limits) {
            println!(
                "message {index}: rejected, {} at offset {:?}",
                error.kind, error.offset
            );
            continue;
        }
        execution.reset()?;
        execution.bind("minimum", QueryBinding::Integer(250))?;
        let mut reader = TreeReader::new(message, Format::Ber, 8, 8, 64, true)?;
        execution.visit(&mut reader, |_| Visit::Continue)?;
        if let QueryValue::Integer(count) = execution.result()? {
            println!("message {index}: valid, {count} amount(s) of at least 250");
        }
    }

    let path = Query::parse("6F/A5/9F02")?;
    let mut values = Vec::new();
    path.visit_buffer(messages[0], Format::Ber, 8, 64, |element, _, _| {
        values.push(element.value());
        Visit::Continue
    })?;
    let amounts = values
        .iter()
        .map(|value| amount.decode(value))
        .collect::<Result<Vec<_>, _>>()?;
    println!("{}: {amounts:?}", path.format()?);

    #[cfg(feature = "document")]
    {
        let document = opentlv::Document::parse(messages[0], Format::Ber, 8, 64)?;
        document.visit(&path, |node| {
            println!(
                "  node {:02X?} = {:02X?}",
                node.tag().as_bytes(),
                node.value()
            );
            Visit::Continue
        })?;
    }
    Ok(())
}
