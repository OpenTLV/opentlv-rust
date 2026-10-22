// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Checks a document's structure -- which tags are required, how many
//! times, in what nesting, and with what value lengths -- without decoding
//! it. See `parse.rs` for the document this schema describes.
//!
//! Run with `cargo run --example validate` from `bindings/rust`.

use opentlv::{Error, Format, Kind, StructureRule, StructureSchema, Tag, ValidationLimits};

/// Same bytes as `parse.rs`'s document.
const DOCUMENT: [u8; 12] = [
    0x6F, 0x0A, 0x84, 0x03, 0x41, 0x42, 0x43, 0xA5, 0x03, 0x50, 0x01, 0x01,
];
/// Missing the FCI Proprietary Template (A5) the schema requires.
const INCOMPLETE: [u8; 7] = [0x6F, 0x05, 0x84, 0x03, 0x41, 0x42, 0x43];

fn main() {
    let proprietary_schema = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0x50]))
            .length(1, 1)
            .required_once()
            .kind(Kind::Primitive)],
        false,
    );
    let fci_schema = StructureSchema::new(
        [
            StructureRule::new(Tag::from_bytes(&[0x84]))
                .length(1, 16)
                .required_once()
                .kind(Kind::Primitive),
            StructureRule::new(Tag::from_bytes(&[0xA5]))
                .required_once()
                .children(proprietary_schema),
        ],
        false,
    );
    let top_schema = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0x6F]))
            .required_once()
            .children(fci_schema)],
        false,
    );
    let limits = ValidationLimits::default();

    top_schema
        .validate(&DOCUMENT, Format::Ber, &limits)
        .expect("the document conforms to the schema");
    println!("Document conforms to the schema");

    let rejected = top_schema
        .validate(&INCOMPLETE, Format::Ber, &limits)
        .expect_err("a required field is missing");
    assert_eq!(rejected.error, Error::Schema);
    println!("Incomplete document rejected: {}", rejected.error);
}
