// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! The public contract of composed Formats and the generic Format operations.
//! Core owns the wire semantics; these tests check that each composition is
//! reachable, owns what Core borrows and works with every format consumer.

use opentlv::{
    decode, read, ByteOrder, ComposedFormat, ConstructedBit, Element, ElementOrder, Encoding,
    Error, EscapedLength, FieldComposition, FixedFormat, FixedFormatConfig, FixedIdentifier,
    FixedLength, Format, FormatOperation, FormatRef, IdentifierPolicy, LengthPolicy, LengthScope,
    PackedField, PackedLayout, ProgramOptions, QueryProgram, QueryRule, QuerySchema,
    QuerySchemaLimits, QueryValue, Reader, StructureRule, StructureSchema, Tag, TreeReader,
    TreeWriter, ValidationLimits, VariableIdentifier, VariableLength, Visit, Writer,
};

fn element<'a>(tag: &[u8], value: &'a [u8]) -> Element<'a> {
    Element::new(Tag::from_bytes(tag), value)
}

fn read_all(data: &[u8], format: FormatRef<'_>) -> Vec<(Vec<u8>, Vec<u8>)> {
    Reader::with_format(data, format)
        .map(|item| {
            let item = item.unwrap();
            (item.tag().as_bytes().to_vec(), item.value().to_vec())
        })
        .collect()
}

/// (depth, constructed, tag) of every TreeReader item.
fn tree(data: &[u8], format: FormatRef<'_>) -> Vec<(usize, bool, Vec<u8>)> {
    let mut reader = TreeReader::new(data, format, 4, 4, 16, true).unwrap();
    let mut items = Vec::new();
    loop {
        match reader.read() {
            Ok(item) => items.push((
                item.depth,
                item.constructed,
                item.decoded.element.tag().as_bytes().to_vec(),
            )),
            Err(Error::End) => return items,
            Err(error) => panic!("{error}"),
        }
    }
}

fn emv_like() -> FieldComposition {
    FieldComposition::new(
        VariableIdentifier::new(0x1F, 0x1F, 0x80, 0x7F, 2).with_policy(IdentifierPolicy {
            forbidden_leading: vec![0x00],
            reject_zero_first_payload: true,
            ..IdentifierPolicy::default()
        }),
        VariableLength::new(0x80, 0x7F, ByteOrder::Big).with_policy(LengthPolicy {
            max_long_octets: 2,
            max_value: 65535,
            ..LengthPolicy::default()
        }),
    )
    .with_constructed(ConstructedBit::new(0, 0x20, 0x20))
}

fn lldp() -> PackedLayout {
    PackedLayout::new(2, PackedField::new(9, 7), PackedField::new(0, 9))
}

#[test]
fn format_operations_cover_every_descriptor_kind() {
    let config = FixedFormatConfig::new(1, 1, ByteOrder::Big);
    let fixed = FixedFormat::new(&config).unwrap();
    let owned = fixed.owned().unwrap();
    let composed = ComposedFormat::packed(&lldp()).unwrap();
    let item = element(&[0x05], b"host");
    for (format, encoded) in [
        (FormatRef::from(Format::Ber), vec![0x05, 4]),
        (FormatRef::from(&fixed), vec![0x05, 4]),
        (FormatRef::from(&owned), vec![0x05, 4]),
        (composed.as_format(), vec![0x0A, 0x04]),
    ] {
        assert!(format.can_read() && format.can_write());
        let encoded = [encoded, b"host".to_vec()].concat();
        assert_eq!(
            format.measure(&item).unwrap(),
            Encoding {
                header: 2,
                value: 4,
                trailer: 0,
                total: 6
            }
        );
        assert_eq!(format.encode(&item).unwrap(), encoded);
        let mut output = [0u8; 8];
        assert_eq!(format.encode_into(&item, &mut output), Ok(6));
        let input = [&encoded[..], &[0xEE]].concat();
        let decoded = format.decode(&input).unwrap();
        assert_eq!(decoded.element, item);
        assert_eq!(decoded.encoded(), encoded);
    }
}

#[test]
fn format_errors_carry_the_format_detail() {
    let lldp = ComposedFormat::packed(&lldp()).unwrap();
    let format = lldp.as_format();
    let truncated = format.decode(&[0x0A, 0x04, b'h']).unwrap_err();
    assert_eq!(truncated.error, Error::Truncated);
    assert_eq!(truncated.operation, FormatOperation::Decode);
    assert_eq!(format.decode(&[]).unwrap_err().error, Error::End);

    let item = element(&[0x05], b"host");
    let mut short = [0u8; 3];
    let error = format.encode_into(&item, &mut short).unwrap_err();
    assert_eq!(error.error, Error::BufferTooShort);
    assert_eq!(error.operation, FormatOperation::Encode);
    assert_eq!(error.required, Some(6));
    assert_eq!(short, [0; 3]);

    let error = format.measure(&element(&[0x80], b"")).unwrap_err();
    assert_eq!(error.error, Error::InvalidTag);
    assert_eq!(error.operation, FormatOperation::Measure);
    assert_eq!(Error::from(error.clone()), Error::InvalidTag);
    assert_eq!(error.to_string(), Error::InvalidTag.to_string());
}

#[test]
fn fixed_fields_compose_binary_and_tagged_binary_framing() {
    let ltv = ComposedFormat::fields(
        &FieldComposition::new(
            FixedIdentifier::new(1),
            FixedLength::new(2, ByteOrder::Little),
        )
        .with_order(ElementOrder::Ltv)
        .with_length_scope(LengthScope::TagAndValue),
    )
    .unwrap();
    let mut buf = [0u8; 16];
    let mut writer = Writer::with_format(&mut buf, &ltv);
    writer.write(&Tag::from_bytes(&[0x07]), &[0xAA]).unwrap();
    assert_eq!(writer.written(), [0x02, 0x00, 0x07, 0xAA]);

    let tagged = ComposedFormat::fields(
        &FieldComposition::new(FixedIdentifier::new(1), FixedLength::new(1, ByteOrder::Big))
            .with_tag_only([Tag::from_bytes(&[0x00])]),
    )
    .unwrap();
    let wire = [0x01, 0x01, 0xAA, 0x00, 0x02, 0x00];
    assert_eq!(
        read_all(&wire, tagged.as_format()),
        [
            (vec![0x01], vec![0xAA]),
            (vec![0x00], vec![]),
            (vec![0x02], vec![])
        ]
    );
    assert_eq!(
        tagged.as_format().encode(&element(&[0x00], b"")).unwrap(),
        [0x00]
    );
    assert_eq!(
        tagged
            .as_format()
            .measure(&element(&[0x00], &[1]))
            .unwrap_err()
            .error,
        Error::InvalidLength
    );

    let nested = ComposedFormat::fields(
        &FieldComposition::new(FixedIdentifier::new(1), FixedLength::new(1, ByteOrder::Big))
            .with_constructed(ConstructedBit::new(0, 0x20, 0x20)),
    )
    .unwrap();
    assert_eq!(
        tree(&[0x21, 0x03, 0x01, 0x01, 0xAA], nested.as_format()),
        [(0, true, vec![0x21]), (1, false, vec![0x01])]
    );
}

#[test]
fn variable_fields_reproduce_emv_element_framing() {
    let composed = ComposedFormat::fields(&emv_like()).unwrap();
    let long = [0x5A; 200];
    for item in [
        element(&[0x9F, 0x02], &[0, 0, 0, 1, 0, 0]),
        element(&[0x5A], &long),
    ] {
        assert_eq!(
            composed.as_format().encode(&item).unwrap(),
            FormatRef::from(Format::Emv).encode(&item).unwrap()
        );
    }
    let template = [0x70, 0x04, 0x5A, 0x02, 0x12, 0x34];
    assert_eq!(
        tree(&template, composed.as_format()),
        tree(&template, Format::Emv.into())
    );
    assert_eq!(
        tree(&template, composed.as_format())[0],
        (0, true, vec![0x70])
    );
    // The identifier policy rejects a forbidden leading byte.
    assert_eq!(
        decode(&[0x00, 0x00], &composed).unwrap_err(),
        Error::InvalidTag
    );

    let tagged = ComposedFormat::fields(
        &FieldComposition::new(
            VariableIdentifier::new(0x1F, 0x1F, 0x80, 0x7F, 2),
            VariableLength::new(0x80, 0x7F, ByteOrder::Big),
        )
        .with_tag_only([Tag::from_bytes(&[0x05])]),
    )
    .unwrap();
    assert_eq!(
        read_all(&[0x05, 0x04, 0x01, 0xAA], tagged.as_format()),
        [(vec![0x05], vec![]), (vec![0x04], vec![0xAA])]
    );
}

#[test]
fn escaped_lengths_use_inline_and_extended_counts() {
    let format = ComposedFormat::fields(&FieldComposition::new(
        FixedIdentifier::new(1),
        EscapedLength::new(0xFF, 2, ByteOrder::Big),
    ))
    .unwrap();
    let long = [0xAB; 300];
    let encoded = format.as_format().encode(&element(&[0x01], &long)).unwrap();
    assert_eq!(encoded[..4], [0x01, 0xFF, 0x01, 0x2C]);
    assert_eq!(
        format
            .as_format()
            .encode(&element(&[0x01], &[1, 2, 3]))
            .unwrap(),
        [0x01, 0x03, 1, 2, 3]
    );
    let nonminimal = [0x01, 0xFF, 0x00, 0x03, 1, 2, 3];
    assert_eq!(
        decode(&nonminimal, &format).unwrap().element.value(),
        [1, 2, 3]
    );

    let strict = ComposedFormat::fields(&FieldComposition::new(
        FixedIdentifier::new(1),
        EscapedLength::new(0xFF, 2, ByteOrder::Big)
            .with_min_extended(0xFF)
            .with_max_length(400),
    ))
    .unwrap();
    assert_eq!(
        decode(&nonminimal, &strict).unwrap_err(),
        Error::InvalidLength
    );
    assert_eq!(
        strict
            .as_format()
            .measure(&element(&[0x01], &[0; 401]))
            .unwrap_err()
            .error,
        Error::InvalidLength
    );
}

#[test]
fn mixed_encodings_compose_through_generic_field_composition() {
    let fixed_tag = ComposedFormat::fields(&FieldComposition::new(
        FixedIdentifier::new(2),
        VariableLength::new(0x80, 0x7F, ByteOrder::Big),
    ))
    .unwrap();
    let value = [0x11; 130];
    let encoded = fixed_tag
        .as_format()
        .encode(&element(&[0x12, 0x34], &value))
        .unwrap();
    assert_eq!(encoded[..4], [0x12, 0x34, 0x81, 0x82]);
    assert_eq!(
        read_all(&encoded, fixed_tag.as_format()),
        [(vec![0x12, 0x34], value.to_vec())]
    );

    let variable_tag = ComposedFormat::fields(&FieldComposition::new(
        VariableIdentifier::new(0x1F, 0x1F, 0x80, 0x7F, 3),
        FixedLength::new(2, ByteOrder::Little),
    ))
    .unwrap();
    let mut buf = [0u8; 8];
    let mut writer = Writer::with_format(&mut buf, &variable_tag);
    writer
        .write(&Tag::from_bytes(&[0x9F, 0x01]), &[0xAA])
        .unwrap();
    assert_eq!(writer.written(), [0x9F, 0x01, 0x01, 0x00, 0xAA]);

    let tagged = ComposedFormat::fields(
        &FieldComposition::new(
            FixedIdentifier::new(1),
            VariableLength::new(0x80, 0x7F, ByteOrder::Big),
        )
        .with_tag_only([Tag::from_bytes(&[0x00])]),
    )
    .unwrap();
    assert_eq!(
        read_all(&[0x00, 0x01, 0x01, 0xAA], tagged.as_format()),
        [(vec![0x00], vec![]), (vec![0x01], vec![0xAA])]
    );
}

#[test]
fn packed_headers_bind_tags_to_the_owned_table() {
    let format = ComposedFormat::packed(&lldp()).unwrap();
    let mut output = [0u8; 32];
    let mut writer = TreeWriter::new(&mut output, &format, 4, 64, 4, 16).unwrap();
    writer.write(&element(&[0x01], b"id")).unwrap();
    writer.write(&element(&[0x05], b"host")).unwrap();
    let wire = writer.finish().unwrap().to_vec();
    assert_eq!(
        wire,
        [0x02, 0x02, b'i', b'd', 0x0A, 0x04, b'h', b'o', b's', b't']
    );
    let decoded = read(&wire, &format).unwrap();
    assert_eq!(decoded.element, element(&[0x01], b"id"));
    #[cfg(feature = "document")]
    {
        let document = opentlv::Document::parse(&wire, &format, 4, 16).unwrap();
        assert_eq!(document.encode().unwrap(), wire);
        assert_eq!(document.encoded_size_as(&format).unwrap(), wire.len());
    }

    let wide = ComposedFormat::packed(&lldp().with_tag_size(2)).unwrap();
    assert_eq!(
        decode(&wire, &wide).unwrap().element.tag().as_bytes(),
        [0x00, 0x01]
    );
    assert_eq!(
        wide.as_format()
            .measure(&element(&[0x01], b""))
            .unwrap_err()
            .error,
        Error::InvalidTagSize
    );

    let little = ComposedFormat::packed(
        &PackedLayout::new(2, PackedField::new(8, 8), PackedField::new(0, 8))
            .with_byte_order(ByteOrder::Little)
            .with_constructed(ConstructedBit::new(0, 0x80, 0x80)),
    )
    .unwrap();
    let nested = [0x03, 0x81, 0x01, 0x01, 0xAA];
    assert_eq!(
        tree(&nested, little.as_format()),
        [(0, true, vec![0x81]), (1, false, vec![0x01])]
    );
}

#[test]
fn invalid_compositions_are_rejected() {
    let fixed = |size| FixedLength::new(size, ByteOrder::Big);
    for composition in [
        FieldComposition::new(FixedIdentifier::new(0), fixed(1)),
        FieldComposition::new(FixedIdentifier::new(1), fixed(9)),
        FieldComposition::new(
            FixedIdentifier::new(1),
            VariableLength::new(0, 0x7F, ByteOrder::Big),
        ),
        FieldComposition::new(VariableIdentifier::new(0, 0, 0x80, 0x7F, 2), fixed(1)),
        FieldComposition::new(FixedIdentifier::new(1), fixed(1))
            .with_constructed(ConstructedBit::new(0, 0, 0)),
        FieldComposition::new(FixedIdentifier::new(1), fixed(1))
            .with_constructed(ConstructedBit::new(0, 0x20, 0x40)),
        FieldComposition::new(FixedIdentifier::new(1), fixed(1))
            .with_order(ElementOrder::Ltv)
            .with_tag_only([Tag::from_bytes(&[0x00])]),
        FieldComposition::new(FixedIdentifier::new(1), fixed(1))
            .with_tag_only([Tag::from_bytes(&[0x00, 0x01])]),
    ] {
        assert_eq!(
            ComposedFormat::fields(&composition).unwrap_err(),
            Error::InvalidArg,
            "{composition:?}"
        );
    }
    assert_eq!(
        ComposedFormat::packed(&PackedLayout::new(
            4,
            PackedField::new(15, 17),
            PackedField::new(0, 15)
        ))
        .unwrap_err(),
        Error::Unsupported
    );
    for layout in [
        PackedLayout::new(2, PackedField::new(9, 0), PackedField::new(0, 9)),
        PackedLayout::new(2, PackedField::new(9, 7), PackedField::new(0, 10)),
        lldp().with_tag_size(9),
        PackedLayout::new(9, PackedField::new(9, 7), PackedField::new(0, 9)),
    ] {
        assert_eq!(
            ComposedFormat::packed(&layout).unwrap_err(),
            Error::InvalidArg,
            "{layout:?}"
        );
    }
}

#[test]
fn packed_fields_read_and_write_through_core() {
    let field = PackedField::new(4, 4);
    assert_eq!(field.read(&[0x12, 0x34], 2, ByteOrder::Big), Ok(0x3));
    assert_eq!(field.read(&[0x12, 0x34], 2, ByteOrder::Little), Ok(0x1));
    let mut header = [0xFF, 0xFF];
    field.write(&mut header, 0, 2, ByteOrder::Big).unwrap();
    assert_eq!(header, [0xFF, 0x0F]);
    assert_eq!(
        field.read(&[0x12], 2, ByteOrder::Big),
        Err(Error::Truncated)
    );
    assert_eq!(
        field.write(&mut header, 16, 2, ByteOrder::Big),
        Err(Error::Overflow)
    );
    assert_eq!(
        field.write(&mut [0u8; 1], 1, 2, ByteOrder::Big),
        Err(Error::BufferTooShort)
    );
    assert_eq!(header, [0xFF, 0x0F]);
    assert_eq!(
        PackedField::new(5, 4).read(&[0], 1, ByteOrder::Big),
        Err(Error::InvalidArg)
    );
}

#[test]
fn composed_formats_work_for_query_and_schemas() {
    let format = ComposedFormat::fields(&FieldComposition::new(
        FixedIdentifier::new(2),
        VariableLength::new(0x80, 0x7F, ByteOrder::Big),
    ))
    .unwrap();
    let options = ProgramOptions {
        composed_format: Some(format.clone()),
        ..ProgramOptions::default()
    };
    let program = QueryProgram::compile("count(//1234)", &options).unwrap();
    let selector = QueryProgram::compile("//1234", &options).unwrap();
    let assertion = QueryProgram::compile("@len=1", &options).unwrap();
    let schema = QuerySchema::with_composed_format(
        vec![QueryRule::new(selector, assertion)],
        format.clone(),
    );
    let wire = [0x12, 0x34, 0x01, 0x56];
    schema
        .validate_buffer(&wire, QuerySchemaLimits::default())
        .unwrap();
    let mut reader = TreeReader::new(&wire, &format, 4, 4, 4, true).unwrap();
    let mut execution = program.execution(4, 4, 10000, true).unwrap();
    drop(program);
    drop(options);
    execution.visit(&mut reader, |_| Visit::Continue).unwrap();
    assert_eq!(execution.result().unwrap(), QueryValue::Integer(1));
    #[cfg(feature = "document")]
    {
        let document = opentlv::Document::parse(&wire, &format, 4, 4).unwrap();
        schema
            .validate_document(&document, QuerySchemaLimits::default())
            .unwrap();
    }

    let structure = StructureSchema::new(
        [StructureRule::new(Tag::from_bytes(&[0x12, 0x34])).length(1, 1)],
        false,
    );
    let limits = ValidationLimits::default();
    structure.validate(&wire, &format, &limits).unwrap();
    assert!(structure
        .validate(&[0x12, 0x34, 0x02, 0x56, 0x78], &format, &limits)
        .is_err());

    // Equal compositions are distinct native Formats; share clones instead.
    let independent = ComposedFormat::fields(&FieldComposition::new(
        FixedIdentifier::new(2),
        VariableLength::new(0x80, 0x7F, ByteOrder::Big),
    ))
    .unwrap();
    let mut other_reader = TreeReader::new(&wire, &independent, 4, 4, 4, true).unwrap();
    let other_program = QueryProgram::compile(
        "count(//1234)",
        &ProgramOptions {
            composed_format: Some(format.clone()),
            ..ProgramOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        other_program
            .execution(4, 4, 10000, true)
            .unwrap()
            .next(&mut other_reader)
            .unwrap_err()
            .error,
        Error::InvalidArg
    );

    let both = ProgramOptions {
        composed_format: Some(format),
        fixed_format: Some(
            FixedFormat::new(&FixedFormatConfig::new(2, 1, ByteOrder::Big))
                .unwrap()
                .owned()
                .unwrap(),
        ),
        ..ProgramOptions::default()
    };
    assert_eq!(
        QueryProgram::compile("count(//1234)", &both)
            .err()
            .unwrap()
            .error,
        Error::InvalidArg
    );
}

#[test]
fn composed_formats_are_shared_thread_safe_owners() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ComposedFormat>();
    assert_send_sync::<FormatRef<'static>>();
    let format = ComposedFormat::fields(&emv_like()).unwrap();
    let clone = format.clone();
    drop(format);
    let wire = [0x5A, 0x02, 0x12, 0x34];
    let worker = std::thread::spawn(move || read_all(&wire, clone.as_format()));
    assert_eq!(worker.join().unwrap(), [(vec![0x5A], vec![0x12, 0x34])]);
    let debug = format!("{:?}", ComposedFormat::packed(&lldp()).unwrap());
    assert!(debug.starts_with("ComposedFormat::packed"), "{debug}");
}
