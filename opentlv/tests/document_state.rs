#![cfg(feature = "document")]
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::{Document, DocumentSourceLocation, Error, Format, Query, QueryErrorKind, Tag, Visit};

const WIRE: &[u8] = &[0x30, 3, 4, 1, 42, 4, 0];

fn tag(bytes: &[u8]) -> Tag {
    Tag::from_bytes(bytes)
}

#[test]
fn revision_counts_edits_and_epoch_counts_retirements() {
    let mut document = Document::parse(WIRE, Format::Ber, 4, 10).unwrap();
    let (revision, epoch) = (document.revision(), document.retire_epoch());
    assert_eq!(epoch, 0);
    document.append(&tag(&[5]), &[1]).unwrap();
    assert_eq!(document.revision(), revision + 1);
    // A primitive replacement and an insertion retire nothing.
    document
        .first_mut()
        .unwrap()
        .into_next()
        .unwrap()
        .set_value(&[7])
        .unwrap();
    assert_eq!(
        (document.revision(), document.retire_epoch()),
        (revision + 2, 0)
    );
    // A failed edit changes neither.
    assert_eq!(
        document.first_mut().unwrap().set_value(&[4, 2]),
        Err(Error::Truncated)
    );
    assert_eq!(
        (document.revision(), document.retire_epoch()),
        (revision + 2, 0)
    );
    // Replacing a constructed Value with children retires them.
    document
        .first_mut()
        .unwrap()
        .set_value(&[4, 1, 43])
        .unwrap();
    assert_eq!(
        (document.revision(), document.retire_epoch()),
        (revision + 3, 1)
    );
    document.first_mut().unwrap().into_next().unwrap().erase();
    assert_eq!(
        (document.revision(), document.retire_epoch()),
        (revision + 4, 2)
    );
}

#[test]
fn handles_outlive_borrows_and_detect_retired_nodes() {
    let mut document = Document::parse(WIRE, Format::Ber, 4, 10).unwrap();
    let root = document.first().unwrap();
    let (root, child, sibling) = (
        root.handle(),
        root.first_child().unwrap().handle(),
        root.next().unwrap().handle(),
    );
    assert_eq!(root.identity(), document.first().unwrap().identity());
    assert_ne!(root, child);
    document.resolve_mut(sibling).unwrap().erase();
    assert!(document.resolve(sibling).is_none());
    // New nodes never take over a retired identity, even at a reused address.
    for value in 0..4u8 {
        document.append(&tag(&[4]), &[value]).unwrap();
    }
    assert!(document.resolve(sibling).is_none());
    assert_eq!(document.resolve(child).unwrap().value(), &[42]);
    document
        .resolve_mut(root)
        .unwrap()
        .set_value(&[4, 1, 43])
        .unwrap();
    assert!(document.resolve(child).is_none());
    let replaced = document.resolve(root).unwrap().first_child().unwrap();
    assert_eq!(replaced.value(), &[43]);
    assert_ne!(replaced.handle(), child);
    let other = Document::parse(WIRE, Format::Ber, 4, 10).unwrap();
    assert!(other.resolve(root).is_none());
}

#[test]
fn source_locations_are_retained_only_when_requested_and_cleared_by_edits() {
    let plain = Document::parse(WIRE, Format::Ber, 4, 10).unwrap();
    assert_eq!(
        plain.first().unwrap().source_location(),
        DocumentSourceLocation::default()
    );
    let mut document = Document::parse_with_source_locations(WIRE, Format::Ber, 4, 10).unwrap();
    let at = |offset| DocumentSourceLocation {
        offset: Some(offset),
        header_size: Some(2),
    };
    let root = document.first().unwrap();
    assert_eq!(root.source_location(), at(0));
    assert_eq!(root.first_child().unwrap().source_location(), at(2));
    assert_eq!(root.next().unwrap().source_location(), at(5));
    document
        .first_mut()
        .unwrap()
        .into_first_child()
        .unwrap()
        .set_value(&[1, 2])
        .unwrap();
    let root = document.first().unwrap();
    // The edited node and its ancestors lose their coordinates; siblings keep theirs.
    assert_eq!(root.source_location(), DocumentSourceLocation::default());
    assert_eq!(
        root.first_child().unwrap().source_location(),
        DocumentSourceLocation::default()
    );
    assert_eq!(root.next().unwrap().source_location(), at(5));
    let inserted = document.append(&tag(&[5]), &[]).unwrap();
    assert_eq!(
        inserted.as_node().source_location(),
        DocumentSourceLocation::default()
    );
}

#[test]
fn visit_delivers_every_path_match_as_a_document_node() {
    let document = Document::parse(&[0x30, 6, 4, 1, 1, 4, 1, 2, 4, 0], Format::Ber, 4, 10).unwrap();
    let query = Query::parse("30/04").unwrap();
    let mut found = Vec::new();
    document
        .visit(&query, |node| {
            found.push(node);
            Visit::Continue
        })
        .unwrap();
    let values: Vec<_> = found.iter().map(|node| node.value()).collect();
    assert_eq!(values, [&[1][..], &[2]]);
    assert!(found
        .iter()
        .all(|node| node.parent().unwrap().tag() == tag(&[0x30])));
    let mut calls = 0;
    document
        .visit(&query, |_| {
            calls += 1;
            Visit::Stop
        })
        .unwrap();
    assert_eq!(calls, 1);
    document
        .visit(&Query::parse("31").unwrap(), |_| panic!("no match"))
        .unwrap();
    let failure = document.visit(&query, |_| Visit::Error).unwrap_err();
    assert_eq!(failure.error, Error::Visitor);
    assert_eq!(failure.kind, QueryErrorKind::Callback);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        document.visit(&query, |_| panic!("visitor panic"))
    }));
    assert!(panic.is_err());
    // The Document is unchanged and still usable after the resumed panic.
    assert_eq!(document.len(), 4);
}

#[test]
fn own_format_encoding_matches_explicit_destination_format() {
    let document = Document::parse(WIRE, Format::Ber, 4, 10).unwrap();
    assert_eq!(document.encode().unwrap(), WIRE);
    assert_eq!(
        document.encode().unwrap(),
        document.encode_as(Format::Ber).unwrap()
    );
    let root = document.first().unwrap();
    assert_eq!(root.encoded_size().unwrap(), 5);
    let mut short = [0xAA; 4];
    let failure = root.encode_into(&mut short).unwrap_err();
    assert_eq!(
        (failure.error, failure.required),
        (Error::BufferTooShort, Some(5))
    );
    assert_eq!(short, [0xAA; 4]);
}
