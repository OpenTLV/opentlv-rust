// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Deterministic wire generation through Core's `tlv_generate()`: seeds,
//! case indices, caller storage and resource bounds.

use opentlv::{
    ByteOrder, ComposedFormat, ConstructedBit, Error, EscapedLength, FieldComposition,
    FixedIdentifier, Format, FormatRef, Generator, GeneratorCandidate, GeneratorLimits, Tag,
    TreeReader,
};

const LIMITS: GeneratorLimits = GeneratorLimits {
    max_elements: 8,
    max_depth: 2,
    max_value_size: 64,
    max_case_size: 512,
};

fn candidates() -> Vec<GeneratorCandidate> {
    vec![
        GeneratorCandidate::new(Tag::from_bytes(&[0x5a]), 0, 16),
        GeneratorCandidate::new(Tag::from_bytes(&[0x9f, 0x02]), 6, 6),
        GeneratorCandidate::new(Tag::from_bytes(&[0x70]), 0, 64),
    ]
}

/// Reads a whole case and returns its (tag, value size, depth, constructed) items.
fn items<'f>(
    wire: &[u8],
    format: impl Into<FormatRef<'f>>,
    limits: GeneratorLimits,
) -> Vec<(Tag, usize, usize, bool)> {
    let mut reader = TreeReader::new(
        wire,
        format.into(),
        limits.max_depth + 1,
        limits.max_depth,
        limits.max_elements,
        true,
    )
    .unwrap();
    let mut items = Vec::new();
    loop {
        match reader.read() {
            Ok(item) => items.push((
                item.decoded.element.tag().clone(),
                item.decoded.element.value().len(),
                item.depth,
                item.constructed,
            )),
            Err(Error::End) => return items,
            Err(error) => panic!("generated case does not decode: {error}"),
        }
    }
}

#[test]
fn cases_are_reproducible_from_seed_and_index() {
    assert_eq!(Generator::VERSION, 2);
    let generator = Generator::new(Format::Ber, candidates(), LIMITS, 7);
    let again = Generator::new(Format::Ber, candidates(), LIMITS, 7);
    let cases: Vec<Vec<u8>> = (0..32).map(|i| generator.generate(i).unwrap()).collect();
    for (index, case) in cases.iter().enumerate() {
        assert!(!case.is_empty());
        assert!(case.len() <= LIMITS.max_case_size);
        // Independent of earlier calls and of the generator instance.
        assert_eq!(&again.generate(index as u64).unwrap(), case);
    }
    // Random access: a later index first gives the same bytes.
    assert_eq!(again.generate(31).unwrap(), cases[31]);
    assert!(cases.iter().any(|case| case != &cases[0]));
    assert_eq!(generator.clone().generate(3).unwrap(), cases[3]);
    // Zero is a valid seed and index.
    let zero = Generator::new(Format::Ber, candidates(), LIMITS, 0);
    assert_eq!(zero.generate(0).unwrap(), zero.generate(0).unwrap());
    assert_eq!(generator.seed(), 7);
    assert_eq!(generator.limits(), LIMITS);
    assert_eq!(generator.candidates(), candidates());
}

#[test]
fn cases_respect_candidates_and_limits() {
    let generator = Generator::new(Format::Ber, candidates(), LIMITS, 2026);
    for index in 0..64 {
        let case = generator.generate(index).unwrap();
        let items = items(&case, Format::Ber, LIMITS);
        assert!(!items.is_empty() && items.len() <= LIMITS.max_elements);
        for (tag, size, depth, constructed) in items {
            let candidate = candidates()
                .into_iter()
                .find(|candidate| candidate.tag == tag)
                .unwrap_or_else(|| panic!("case {index}: unexpected tag {tag}"));
            assert!(depth <= LIMITS.max_depth);
            assert_eq!(constructed, tag.as_bytes() == [0x70]);
            if !constructed {
                assert!(
                    (candidate.min_value_size..=candidate.max_value_size).contains(&size),
                    "case {index}: {tag} has {size} bytes"
                );
            }
        }
    }
}

#[test]
fn caller_storage_does_not_change_the_case() {
    let generator = Generator::new(Format::Ber, candidates(), LIMITS, 11);
    let workspace_size = generator.workspace_size().unwrap();
    assert_eq!(
        workspace_size,
        (LIMITS.max_depth + 1) * LIMITS.max_case_size
    );
    let expected = generator.generate(5).unwrap();

    let mut output = vec![0xaa; LIMITS.max_case_size];
    let mut workspace = vec![0u8; workspace_size];
    let written = generator
        .generate_into(5, &mut output, &mut workspace)
        .unwrap();
    assert_eq!(&output[..written], expected);

    let mut larger = vec![0u8; LIMITS.max_case_size * 2];
    let mut more = vec![0u8; workspace_size + 100];
    let written = generator.generate_into(5, &mut larger, &mut more).unwrap();
    assert_eq!(&larger[..written], expected);

    // Output below max_case_size or a short workspace is rejected up front.
    let mut short = vec![0u8; LIMITS.max_case_size - 1];
    assert_eq!(
        generator.generate_into(5, &mut short, &mut workspace),
        Err(Error::BufferTooShort)
    );
    assert_eq!(
        generator.generate_into(5, &mut output, &mut workspace[..workspace_size - 1]),
        Err(Error::BufferTooShort)
    );
}

#[test]
fn core_validates_the_configuration() {
    let invalid = |limits: GeneratorLimits, candidates: Vec<GeneratorCandidate>| {
        let generator = Generator::new(Format::Ber, candidates, limits, 0);
        let error = generator.workspace_size().unwrap_err();
        // generate() reports what tlv_generate() reports.
        assert_eq!(generator.generate(0), Err(error));
        assert_eq!(generator.generate_into(0, &mut [], &mut []), Err(error));
        error
    };
    let empty_case = GeneratorLimits {
        max_case_size: 0,
        ..LIMITS
    };
    assert_eq!(invalid(empty_case, candidates()), Error::InvalidArg);
    assert_eq!(invalid(LIMITS, Vec::new()), Error::InvalidArg);
    let reversed = vec![GeneratorCandidate::new(Tag::from_bytes(&[0x5a]), 4, 2)];
    assert_eq!(invalid(LIMITS, reversed), Error::InvalidArg);
    // No candidate's minimum Value fits max_value_size.
    let too_large = vec![GeneratorCandidate::new(Tag::from_bytes(&[0x5a]), 65, 70)];
    assert_eq!(invalid(LIMITS, too_large), Error::InvalidArg);
    let deep = GeneratorLimits {
        max_depth: 65,
        ..LIMITS
    };
    assert_eq!(invalid(deep, candidates()), Error::Unsupported);
    let unrepresentable = GeneratorLimits {
        max_case_size: usize::MAX,
        ..LIMITS
    };
    assert_eq!(invalid(unrepresentable, candidates()), Error::Overflow);
}

#[test]
fn composed_formats_generate_their_own_framing() {
    let format = ComposedFormat::fields(
        &FieldComposition::new(
            FixedIdentifier::new(1),
            EscapedLength::new(0xff, 2, ByteOrder::Big),
        )
        .with_constructed(ConstructedBit::new(0, 0x80, 0x80)),
    )
    .unwrap();
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
            GeneratorCandidate::new(Tag::from_bytes(&[0x81]), 0, 128),
        ],
        limits,
        2026,
    );
    let mut escaped = false;
    for index in 0..50 {
        let case = generator.generate(index).unwrap();
        escaped |= items(&case, &format, limits)
            .iter()
            .any(|(_, size, _, constructed)| !constructed && *size >= 0xff);
        #[cfg(feature = "document")]
        {
            let document = opentlv::Document::parse(&case, &format, 3, 16).unwrap();
            assert_eq!(document.encode().unwrap(), case, "case {index}");
        }
    }
    assert!(escaped, "no case used the escaped length form");
}

#[test]
fn cloned_candidates_survive_the_original_and_support_shared_calls() {
    let original = Generator::new(Format::Ber, candidates(), LIMITS, u64::MAX);
    let expected = original.generate(u64::MAX).unwrap();
    let cloned = original.clone();
    drop(original);
    let moved_generator = cloned.clone();
    std::thread::scope(|scope| {
        let shared = &cloned;
        let first = scope.spawn(move || shared.generate(u64::MAX).unwrap());
        let moved = scope.spawn(move || moved_generator.generate(u64::MAX).unwrap());
        assert_eq!(first.join().unwrap(), expected);
        assert_eq!(moved.join().unwrap(), expected);
    });
}

#[test]
fn impossible_storage_returns_an_error_without_panicking() {
    // C size_t can represent this capacity, but a Rust allocation cannot.
    let capacity = isize::MAX as usize + 1;
    let generator = Generator::new(
        Format::Ber,
        candidates(),
        GeneratorLimits {
            max_depth: 0,
            max_case_size: capacity,
            ..LIMITS
        },
        0,
    );
    assert_eq!(generator.workspace_size().unwrap(), capacity);
    assert_eq!(generator.generate(0), Err(Error::OutOfMemory));
}
