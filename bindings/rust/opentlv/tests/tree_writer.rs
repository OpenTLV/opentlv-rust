// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::{Element, Error, Format, Tag, TreeWriter};

#[test]
fn owned_open_tags_and_borrowed_output_roundtrip() {
    let mut output = [0; 32];
    let mut writer = TreeWriter::new(&mut output, Format::Ber, 2, 32, 2, 5).unwrap();
    writer.begin(&Tag::from_bytes(&[0x30])).unwrap();
    writer
        .write(&Element::new(Tag::from_bytes(&[4]), &[42]))
        .unwrap();
    assert_eq!(writer.written(), &[]);
    assert!(writer.finish().is_err());
    writer.end().unwrap();
    assert_eq!(writer.finish().unwrap(), &[0x30, 3, 4, 1, 42]);
}

#[test]
fn canonical_cer_closing_and_resource_failures() {
    let mut output = [0; 32];
    let mut writer = TreeWriter::new(&mut output, Format::Cer, 1, 32, 1, 2).unwrap();
    writer.begin(&Tag::from_bytes(&[0x30])).unwrap();
    writer
        .write(&Element::new(Tag::from_bytes(&[4]), &[42]))
        .unwrap();
    writer.end().unwrap();
    assert_eq!(writer.finish().unwrap(), &[0x30, 0x80, 4, 1, 42, 0, 0]);
    assert_eq!(writer.begin(&Tag::from_bytes(&[0x30])), Err(Error::Limit));
    assert_eq!(writer.written(), &[0x30, 0x80, 4, 1, 42, 0, 0]);
}

#[test]
fn scratch_exhaustion_keeps_parent_open() {
    let mut output = [0; 32];
    let mut writer = TreeWriter::new(&mut output, Format::Ber, 1, 0, 1, 2).unwrap();
    writer.begin(&Tag::from_bytes(&[0x30])).unwrap();
    writer
        .write(&Element::new(Tag::from_bytes(&[4]), &[42]))
        .unwrap();
    assert_eq!(writer.end(), Err(Error::BufferTooShort));
    assert!(writer.diagnostic().is_some());
    assert!(writer.finish().is_err());
    assert!(writer.written().is_empty());
}
fn measure_items() -> impl Iterator<Item = opentlv::Result<opentlv::TreeWriteItem<'static>>> {
    [
        (0x30, 0, true, &b"ignored"[..]),
        (4, 1, false, &b"\x2a"[..]),
        (0x30, 1, true, &b""[..]),
    ]
    .into_iter()
    .map(|(tag, depth, constructed, value)| {
        Ok(opentlv::TreeWriteItem {
            element: Element::new(Tag::from_bytes(&[tag]), value),
            depth,
            constructed,
        })
    })
}

#[test]
fn measurement_stages_content_and_reports_workspace_shortage() {
    for (format, expected) in [
        (Format::Ber, &b"\x30\x05\x04\x01\x2a\x30\x00"[..]),
        (
            Format::Cer,
            &b"\x30\x80\x04\x01\x2a\x30\x80\x00\x00\x00\x00"[..],
        ),
    ] {
        let mut output = [0; 32];
        let mut writer = TreeWriter::new(&mut output, format, 4, 32, 4, 10).unwrap();
        assert_eq!(writer.measure(measure_items()).unwrap(), expected);
        assert_eq!(writer.required_workspace(), (0, 0));
    }
    let mut output = [0; 32];
    let mut writer = TreeWriter::new(&mut output, Format::Ber, 4, 0, 4, 10).unwrap();
    assert_eq!(writer.measure(measure_items()), Err(Error::BufferTooShort));
    assert_eq!(writer.required_workspace(), (0, 5));
    assert!(writer.diagnostic().is_some());
}

#[test]
fn measurement_source_errors_and_panics_return_after_c() {
    let mut output = [0; 32];
    let mut writer = TreeWriter::new(&mut output, Format::Ber, 4, 32, 4, 10).unwrap();
    assert_eq!(
        writer.measure([Err(Error::InvalidValue)]),
        Err(Error::InvalidValue)
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let source = std::iter::from_fn(
            || -> Option<opentlv::Result<opentlv::TreeWriteItem<'static>>> {
                panic!("measurement source");
            },
        );
        let _ = writer.measure(source);
    }));
    assert!(panic.is_err());
    assert_eq!(writer.measure(measure_items()).unwrap().len(), 7);
}
