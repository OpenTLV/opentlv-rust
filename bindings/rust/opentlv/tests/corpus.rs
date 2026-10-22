// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Replays the C fuzz seed corpus (the `corpus/` folder next to each fuzz
//! harness under `tests/fuzz/`) through the safe Rust API, so the bindings
//! are checked against the same inputs as the C library.
//!
//! The seeds are shared, never copied: a new seed added for the C harnesses is
//! picked up here automatically. The checks are the same invariants the C
//! harnesses assert (bounds, termination, round trips), not fixed outputs.

use std::fs;
use std::path::PathBuf;

use opentlv::emv::{self, Context};
use opentlv::{encoded_size, Codec, Format, Limits, Reader, Strictness, Tag, Writer};

/// Candidate tags up to this size are tried, which reaches past the longest tag any format accepts.
const MAX_TAG_SIZE: usize = 16;

/// Reads every seed file of one corpus directory as `(file name, bytes)`.
///
/// `path` is relative to `tests/fuzz/`, e.g. `corpus/read` or
/// `builtins/asn1/corpus/der`, matching where each fuzz harness's own corpus lives.
fn seeds(path: &str) -> Vec<(String, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fuzz")
        .join(path);
    let mut seeds: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read corpus {}: {e}", dir.display()))
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read(&path).unwrap())
        })
        .collect();
    assert!(!seeds.is_empty(), "corpus {path} is empty");
    seeds.sort();
    seeds
}

#[test]
fn reader_handles_every_read_seed_in_every_format() {
    for (name, data) in seeds("reader/corpus/read") {
        for format in Format::ALL {
            let mut reader = Reader::with_format(&data, format);
            let mut previous = 0;
            let mut failed = false;
            let base = data.as_ptr() as usize;

            while let Some(item) = reader.next_element() {
                assert!(!failed, "{name} {format}: item after an error");
                match item {
                    Ok(element) => {
                        let start = element.value().as_ptr() as usize;
                        // An empty value may be a dangling slice, so only non-empty ones
                        // are located inside the input.
                        assert!(
                            element.value().is_empty()
                                || (start >= base
                                    && start - base + element.value().len() <= data.len()),
                            "{name} {format}: value outside the input"
                        );
                        assert!(reader.position() > previous, "{name} {format}: no progress");
                        assert!(reader.position() <= data.len());
                        previous = reader.position();
                    }
                    Err(_) => failed = true,
                }
            }
            assert!(
                failed || reader.is_at_end(),
                "{name} {format}: stopped early without an error"
            );
        }
    }
}

#[test]
fn canonical_formats_are_consistent_on_every_der_seed() {
    for (name, data) in seeds("builtins/asn1/corpus/der") {
        for format in [Format::Der, Format::Cer] {
            let limits = format.default_limits().unwrap();
            let canonical = format.validate(&data, &limits, Strictness::Canonical);
            let strict = format.validate(&data, &limits, Strictness::Strict);

            // Strict adds checks on top of canonical, so it can never accept more.
            if strict.is_ok() {
                assert!(
                    canonical.is_ok(),
                    "{name} {format:?}: strict but not canonical"
                );
            }
            for error in [&canonical, &strict]
                .into_iter()
                .filter_map(|r| r.as_ref().err())
            {
                assert!(
                    error
                        .location
                        .offset()
                        .map_or(true, |offset| offset <= data.len()),
                    "{name} {format:?}: offset past end"
                );
            }
            if canonical.is_ok() && !data.is_empty() {
                let (element, consumed) = format
                    .read(&data, &limits, Strictness::Canonical)
                    .unwrap_or_else(|e| panic!("{name} {format:?}: validated but unreadable: {e}"));
                assert!(consumed > 0 && consumed <= data.len());
                assert!(element.value().len() <= consumed);
            }
        }
    }
}

#[test]
fn every_roundtrip_seed_round_trips_in_every_format() {
    for (name, data) in seeds("corpus/roundtrip") {
        // Layout (see docs/development/fuzzing.md#seed-corpus): a tag size byte, that many tag
        // bytes, then the value. Every input is also a value under tag 0x04.
        let tag_len = data.first().map_or(0, |b| *b as usize % (MAX_TAG_SIZE + 1));
        let tag_len = tag_len.min(data.len().saturating_sub(1));
        let prefix = if data.is_empty() { 0 } else { 1 + tag_len };
        let candidate = Tag::from_bytes(&data[data.len().min(1)..data.len().min(1) + tag_len]);
        let primitive = Tag::from_bytes(&[0x04]);

        for format in Format::ALL {
            check_roundtrip(&name, format, &candidate, &data[prefix..]);
            check_roundtrip(&name, format, &primitive, &data);
        }
    }
}

fn check_roundtrip(name: &str, format: Format, tag: &Tag, value: &[u8]) {
    let Ok(total) = encoded_size(tag, value.len(), format) else {
        return;
    };
    assert!(
        total >= value.len(),
        "{name} {format}: size below the value"
    );

    let mut short = vec![0xA5; total - 1];
    let error = Writer::with_format(&mut short, format)
        .write(tag, value)
        .unwrap_err();
    assert_eq!(error, opentlv::Error::BufferTooShort, "{name} {format}");
    assert!(
        short.iter().all(|b| *b == 0xA5),
        "{name} {format}: short buffer written"
    );

    let mut buf = vec![0u8; total];
    let mut writer = Writer::with_format(&mut buf, format);
    writer.write(tag, value).unwrap();
    assert_eq!(writer.position(), total, "{name} {format}");
    let encoded = writer.finish();

    let mut reader = Reader::with_format(encoded, format);
    let element = reader.next_element().unwrap().unwrap();
    assert_eq!(element.tag(), tag, "{name} {format}");
    assert_eq!(element.value(), value, "{name} {format}");
    assert!(reader.is_at_end(), "{name} {format}: trailing bytes");
}

#[test]
fn every_emv_codec_round_trips_on_every_codec_seed() {
    let mut codecs: Vec<(String, Codec)> = vec![("amount".to_string(), Codec::amount())];
    // Every one- and two-byte tag in every dictionary context.
    let contexts = [
        Context::Base,
        Context::Bit,
        Context::Bht,
        Context::BhtFormat,
        Context::BitGroup,
        Context::BiometricCounters,
        Context::BiometricAttempts,
        Context::BiometricVerification,
    ];
    for context in contexts {
        for raw in 0..=0xFFFFu16 {
            let bytes = raw.to_be_bytes();
            let bytes = if raw <= 0xFF { &bytes[1..] } else { &bytes[..] };
            let tag = Tag::from_bytes(bytes);
            if let Some(codec) = emv::find(context, &tag).and_then(|d| d.codec()) {
                codecs.push((format!("{context:?}/{tag:?}"), codec));
            }
        }
    }
    assert!(
        codecs.len() > 50,
        "EMV dictionary enumeration found {}",
        codecs.len()
    );

    for (seed, data) in seeds("builtins/emv/corpus/codec") {
        for (codec_name, codec) in &codecs {
            let Ok(value) = codec.decode(&data) else {
                continue;
            };
            let encoded = codec
                .encode(&value)
                .unwrap_or_else(|e| panic!("{seed} {codec_name}: decoded but not encodable: {e}"));
            assert_eq!(
                codec.decode(&encoded).as_ref(),
                Ok(&value),
                "{seed} {codec_name}: re-decode differs"
            );
        }
    }
}

#[test]
fn default_limits_accept_a_minimal_element() {
    // Guards the corpus tests above against vacuous passes: a plain
    // element must be accepted by the limits they use.
    let limits: Limits = Format::Der.default_limits().unwrap();
    assert!(Format::Der
        .validate(&[0x04, 0x00], &limits, Strictness::Strict)
        .is_ok());
}
