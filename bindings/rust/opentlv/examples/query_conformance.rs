// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
//! Common corpus process adapter using the public safe compiled Query facade.
use opentlv::{
    Format, ProgramOptions, QueryBinding, QueryProgram, QueryType, QueryValue, TreeReader,
};
use std::{collections::BTreeMap, env};
fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn print_scalar(value: QueryValue<'_>) {
    match value {
        QueryValue::Boolean(value) => println!("bool:{}", value as i32),
        QueryValue::Integer(value) => println!("int:{value}"),
        QueryValue::Bytes(value) => {
            print!("bytes:");
            for byte in value {
                print!("{byte:02x}");
            }
            println!();
        }
        QueryValue::String(value) => {
            print!("string:");
            for byte in value.as_bytes() {
                print!("{byte:02x}");
            }
            println!();
        }
    }
}
fn main() {
    let args: Vec<_> = env::args().collect();
    if args.get(1).is_some_and(|s| s == "--capabilities") {
        println!("asn1");
        #[cfg(feature = "document")]
        println!("document");
        return;
    }
    if args.get(1).is_some_and(|s| s == "--language-features") {
        // All common fixtures are exercised; the Python driver inventories categories.
        return;
    }
    let mode = args.get(3).map_or("o", |s| s.as_str());
    let mut options = ProgramOptions {
        optimize: !mode.contains('u'),
        ..ProgramOptions::default()
    };
    options.names.insert("fixture:leaf".into(), vec![0x5a]);
    options.names.insert("fixture:container".into(), vec![0x70]);
    let mut bindings = BTreeMap::new();
    for declaration in args.iter().skip(5) {
        let mut fields = declaration.splitn(3, ':');
        let name = fields.next().unwrap().to_owned();
        let kind = fields.next().unwrap();
        let value = fields.next().unwrap();
        options.variables.insert(
            name.clone(),
            match kind {
                "int" => QueryType::Integer,
                "bytes" => QueryType::Bytes,
                _ => QueryType::String,
            },
        );
        bindings.insert(
            name,
            (
                kind.to_owned(),
                value.to_owned(),
                if kind == "bytes" { hex(value) } else { vec![] },
            ),
        );
    }
    let result = (|| -> opentlv::ProgramResult<()> {
        let program = QueryProgram::compile(&args[1], &options)?;
        let retained = mode.contains('r') || mode.contains('d') || program.info().level >= 2;
        let mut execution = program.execution(128, 1024, 100000000, retained)?;
        for (name, _) in program.variables()? {
            let (kind, value, bytes) = &bindings[&name];
            execution.bind(
                &name,
                match kind.as_str() {
                    "int" => QueryBinding::Integer(value.parse().unwrap()),
                    "bytes" => QueryBinding::Bytes(bytes),
                    _ => QueryBinding::String(value),
                },
            )?;
        }
        let wire = hex(&args[2]);
        #[cfg(feature = "document")]
        if mode.contains('d') || program.info().level == 3 {
            let document =
                opentlv::Document::parse_with_source_locations(&wire, Format::Ber, 128, 1024)
                    .unwrap();
            // Execution must be created after input/Document owners so Rust drops it first.
            let mut document_execution = program.execution(128, 1024, 100000000, true)?;
            for (name, _) in program.variables()? {
                let (kind, value, bytes) = &bindings[&name];
                document_execution.bind(
                    &name,
                    match kind.as_str() {
                        "int" => QueryBinding::Integer(value.parse().unwrap()),
                        "bytes" => QueryBinding::Bytes(bytes),
                        _ => QueryBinding::String(value),
                    },
                )?;
            }
            document_execution.evaluate_document(&document, None, None)?;
            if program.info().result_kind != 0 {
                print_scalar(document_execution.result()?);
            } else {
                let reader = TreeReader::new(&wire, Format::Ber, 128, 128, 1024, true).unwrap();
                let mut offsets = Vec::new();
                for item in reader {
                    offsets.push(item.unwrap().offset);
                }
                let mut nodes = Vec::new();
                let mut pending = Vec::new();
                let mut node = document.first();
                while let Some(current) = node {
                    nodes.push(current.identity());
                    if let Some(next) = current.next() {
                        pending.push(next);
                    }
                    node = current.first_child().or_else(|| pending.pop());
                }
                while let Some(node) = document_execution.next_document()? {
                    println!(
                        "{}",
                        offsets[nodes.iter().position(|id| *id == node.identity()).unwrap()]
                    );
                }
            }
            return Ok(());
        }
        let split = args.get(4).map(|value| value.parse::<usize>().unwrap());
        let mut reader = TreeReader::new(
            &wire[..split.unwrap_or(wire.len())],
            Format::Ber,
            128,
            128,
            1024,
            split.is_none(),
        )
        .unwrap();
        if let Some(boundary) = split {
            let status = execution.visit(&mut reader, |matched| {
                println!("{}", matched.offset);
                opentlv::Visit::Continue
            });
            if let Err(error) = status {
                if error.error != opentlv::Error::NeedMoreData {
                    return Err(error);
                }
            }
            reader.set_input(&wire, 0, true).unwrap();
            let _ = boundary;
        }
        execution.visit(&mut reader, |matched| {
            println!("{}", matched.offset);
            opentlv::Visit::Continue
        })?;
        if program.info().result_kind != 0 {
            print_scalar(execution.result()?);
        }
        Ok(())
    })();
    if let Err(error) = result {
        // The expression span is primary, or related evidence beside another location.
        let span = if error.location.domain == opentlv::LocationDomain::Expression {
            error.location
        } else {
            error.expression
        };
        eprintln!(
            "{} {} {} {}",
            error.error.code(),
            error.kind.as_raw(),
            span.begin,
            span.end
        );
        std::process::exit(1);
    }
}
