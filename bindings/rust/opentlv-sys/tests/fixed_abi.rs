// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Compare the FFI mirror with sizeof/alignment/offsetof from the C headers.
//! The small C probe uses CMake and the target's native C compiler.

use opentlv_sys::tlv_codec_detail_t;
use opentlv_sys::tlv_codec_diagnostic_t;
use opentlv_sys::{
    tlv_diagnostic_path_t, tlv_diagnostic_t, tlv_fixed_format_t, tlv_fixed_identifier_t,
    tlv_fixed_length_t, tlv_location_t, tlv_schema_definition_location_t, tlv_schema_detail_t,
    tlv_schema_diagnostic_t, tlv_schema_query_diagnostic_t,
};
use opentlv_sys::{tlv_query_diagnostic_t, tlv_reader_detail_t, tlv_reader_diagnostic_t};
use std::mem::{align_of, size_of, MaybeUninit};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// std::mem::offset_of! requires Rust 1.77; the binding supports Rust 1.70.
macro_rules! member_offset {
    ($type:ty, $member:ident) => {{
        let storage = MaybeUninit::<$type>::uninit();
        let base = storage.as_ptr();
        // SAFETY: addr_of! forms a field pointer without reading uninitialized data.
        unsafe { std::ptr::addr_of!((*base).$member) as usize - base as usize }
    }};
}

fn run(command: &mut Command) -> Output {
    let output = command
        .output()
        .expect("ABI probe requires CMake and a C compiler");
    assert!(
        output.status.success(),
        "{command:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn fixed_and_diagnostic_layouts_match_c() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let build =
        std::env::temp_dir().join(format!("opentlv-fixed-abi-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&build).expect("cannot create C ABI probe build directory");
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(manifest.join("tests/abi_probe"))
        .arg("-B")
        .arg(&build)
        .arg(format!(
            "-DOPENTLV_SOURCE_DIR={}",
            manifest.join("../../..").display()
        ));
    if cfg!(all(target_os = "windows", target_env = "msvc")) {
        configure.arg("-A").arg(if cfg!(target_arch = "x86") {
            "Win32"
        } else if cfg!(target_arch = "aarch64") {
            "ARM64"
        } else {
            "x64"
        });
    } else if cfg!(target_arch = "x86") {
        configure.arg("-DCMAKE_C_FLAGS=-m32");
    }
    run(&mut configure);
    run(Command::new("cmake")
        .arg("--build")
        .arg(&build)
        .args(["--config", "Release"]));
    let name = if cfg!(windows) {
        "abi_probe.exe"
    } else {
        "abi_probe"
    };
    let executable = [build.join("Release").join(name), build.join(name)]
        .into_iter()
        .find(|path| path.is_file())
        .expect("missing C ABI probe executable");
    let output = run(&mut Command::new(executable));
    let native: Vec<usize> = std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();
    let rust = [
        size_of::<tlv_fixed_identifier_t>(),
        align_of::<tlv_fixed_identifier_t>(),
        member_offset!(tlv_fixed_identifier_t, size),
        size_of::<tlv_fixed_length_t>(),
        align_of::<tlv_fixed_length_t>(),
        member_offset!(tlv_fixed_length_t, size),
        member_offset!(tlv_fixed_length_t, byte_order),
        size_of::<tlv_fixed_format_t>(),
        align_of::<tlv_fixed_format_t>(),
        member_offset!(tlv_fixed_format_t, identifier),
        member_offset!(tlv_fixed_format_t, length),
        member_offset!(tlv_fixed_format_t, element_order),
        member_offset!(tlv_fixed_format_t, length_scope),
        size_of::<tlv_location_t>(),
        align_of::<tlv_location_t>(),
        member_offset!(tlv_location_t, domain),
        member_offset!(tlv_location_t, kind),
        member_offset!(tlv_location_t, begin),
        member_offset!(tlv_location_t, end),
        size_of::<tlv_diagnostic_t>(),
        align_of::<tlv_diagnostic_t>(),
        member_offset!(tlv_diagnostic_t, code),
        member_offset!(tlv_diagnostic_t, severity),
        member_offset!(tlv_diagnostic_t, location),
        member_offset!(tlv_diagnostic_t, expected),
        member_offset!(tlv_diagnostic_t, actual),
        member_offset!(tlv_diagnostic_t, contexts),
        member_offset!(tlv_diagnostic_t, has_path),
        member_offset!(tlv_diagnostic_t, path),
        size_of::<tlv_schema_definition_location_t>(),
        align_of::<tlv_schema_definition_location_t>(),
        member_offset!(tlv_schema_definition_location_t, kind),
        member_offset!(tlv_schema_definition_location_t, owner),
        member_offset!(tlv_schema_definition_location_t, index),
        size_of::<tlv_diagnostic_path_t>(),
        align_of::<tlv_diagnostic_path_t>(),
        member_offset!(tlv_diagnostic_path_t, tags),
        member_offset!(tlv_diagnostic_path_t, length),
        member_offset!(tlv_diagnostic_path_t, omitted),
        size_of::<tlv_schema_diagnostic_t>(),
        align_of::<tlv_schema_diagnostic_t>(),
        member_offset!(tlv_schema_diagnostic_t, diagnostic),
        member_offset!(tlv_schema_diagnostic_t, cause),
        member_offset!(tlv_schema_diagnostic_t, detail),
        size_of::<tlv_reader_detail_t>(),
        align_of::<tlv_reader_detail_t>(),
        member_offset!(tlv_reader_detail_t, operation),
        member_offset!(tlv_reader_detail_t, has_tag),
        member_offset!(tlv_reader_detail_t, tag),
        member_offset!(tlv_reader_detail_t, has_tag_offset),
        member_offset!(tlv_reader_detail_t, tag_offset),
        member_offset!(tlv_reader_detail_t, has_length_offset),
        member_offset!(tlv_reader_detail_t, length_offset),
        member_offset!(tlv_reader_detail_t, has_value_offset),
        member_offset!(tlv_reader_detail_t, value_offset),
        member_offset!(tlv_reader_detail_t, has_declared_length),
        member_offset!(tlv_reader_detail_t, declared_length),
        member_offset!(tlv_reader_detail_t, has_raw_length),
        member_offset!(tlv_reader_detail_t, raw_length),
        member_offset!(tlv_reader_detail_t, has_available),
        member_offset!(tlv_reader_detail_t, available),
        member_offset!(tlv_reader_detail_t, has_enclosing_end),
        member_offset!(tlv_reader_detail_t, enclosing_end),
        member_offset!(tlv_reader_detail_t, has_required),
        member_offset!(tlv_reader_detail_t, required),
        size_of::<tlv_reader_diagnostic_t>(),
        align_of::<tlv_reader_diagnostic_t>(),
        member_offset!(tlv_reader_diagnostic_t, diagnostic),
        member_offset!(tlv_reader_diagnostic_t, detail),
        size_of::<tlv_query_diagnostic_t>(),
        align_of::<tlv_query_diagnostic_t>(),
        member_offset!(tlv_query_diagnostic_t, diagnostic),
        member_offset!(tlv_query_diagnostic_t, kind),
        member_offset!(tlv_query_diagnostic_t, expression),
        member_offset!(tlv_query_diagnostic_t, limit),
        member_offset!(tlv_query_diagnostic_t, configured),
        member_offset!(tlv_query_diagnostic_t, cause),
        member_offset!(tlv_query_diagnostic_t, detail),
        size_of::<tlv_schema_detail_t>(),
        align_of::<tlv_schema_detail_t>(),
        member_offset!(tlv_schema_detail_t, kind),
        member_offset!(tlv_schema_detail_t, tag),
        member_offset!(tlv_schema_detail_t, definition),
        member_offset!(tlv_schema_detail_t, field),
        member_offset!(tlv_schema_detail_t, is_group),
        member_offset!(tlv_schema_detail_t, has_occurs),
        member_offset!(tlv_schema_detail_t, min_occurs),
        member_offset!(tlv_schema_detail_t, max_occurs),
        member_offset!(tlv_schema_detail_t, occurs),
        member_offset!(tlv_schema_detail_t, has_length),
        member_offset!(tlv_schema_detail_t, min_length),
        member_offset!(tlv_schema_detail_t, max_length),
        member_offset!(tlv_schema_detail_t, actual_length),
        member_offset!(tlv_schema_detail_t, has_form),
        member_offset!(tlv_schema_detail_t, expected_form),
        member_offset!(tlv_schema_detail_t, actual_constructed),
        member_offset!(tlv_schema_detail_t, length_multiple),
        member_offset!(tlv_schema_detail_t, length_flags),
        size_of::<tlv_codec_detail_t>(),
        align_of::<tlv_codec_detail_t>(),
        member_offset!(tlv_codec_detail_t, operation),
        member_offset!(tlv_codec_detail_t, reported),
        member_offset!(tlv_codec_detail_t, violation),
        member_offset!(tlv_codec_detail_t, reason),
        member_offset!(tlv_codec_detail_t, representation),
        member_offset!(tlv_codec_detail_t, cause),
        member_offset!(tlv_codec_detail_t, detail),
        size_of::<tlv_codec_diagnostic_t>(),
        align_of::<tlv_codec_diagnostic_t>(),
        member_offset!(tlv_codec_diagnostic_t, diagnostic),
        member_offset!(tlv_codec_diagnostic_t, codec),
        size_of::<tlv_schema_query_diagnostic_t>(),
        align_of::<tlv_schema_query_diagnostic_t>(),
        member_offset!(tlv_schema_query_diagnostic_t, rule),
        member_offset!(tlv_schema_query_diagnostic_t, query),
    ];
    assert_eq!(
        native, rust,
        "Rust Fixed or diagnostic FFI layout differs from the C headers"
    );
    std::fs::remove_dir_all(&build).expect("cannot clean up C ABI probe build directory");
}
