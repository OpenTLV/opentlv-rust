// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Compare the generic codec, structure codec, Schema number and generator
//! FFI mirrors and constants with sizeof/alignment/offsetof and the values
//! from the C headers. Like `compose_abi.rs`, the small C probe uses CMake
//! and the target's native C compiler.

use opentlv_sys::{
    tlv_digits_codec_config_t, tlv_generator_candidate_t, tlv_generator_options_t, tlv_ipv4_list_t,
    tlv_ipv4_t, tlv_schema_number_t, tlv_structure_codec_t, tlv_text_codec_config_t,
    TLV_GENERATOR_VERSION, TLV_STRUCTURE_MAX_DEPTH, TLV_TEXT_ASCII_ALNUM, TLV_TEXT_ASCII_PRINTABLE,
};
use std::mem::{align_of, size_of, MaybeUninit};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// The same field offset helper as `compose_abi.rs` and `fixed_abi.rs`.
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
fn codec_and_generator_layouts_match_c() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let build =
        std::env::temp_dir().join(format!("opentlv-codec-abi-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&build).expect("cannot create C ABI probe build directory");
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(manifest.join("tests/codec_abi_probe"))
        .arg("-B")
        .arg(&build)
        .arg(format!(
            "-DOPENTLV_SOURCE_DIR={}",
            env!("OPENTLV_CORE_SOURCE_DIR")
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
        size_of::<tlv_text_codec_config_t>(),
        align_of::<tlv_text_codec_config_t>(),
        member_offset!(tlv_text_codec_config_t, alphabet),
        member_offset!(tlv_text_codec_config_t, width),
        member_offset!(tlv_text_codec_config_t, zero_padding),
        size_of::<tlv_digits_codec_config_t>(),
        align_of::<tlv_digits_codec_config_t>(),
        member_offset!(tlv_digits_codec_config_t, width),
        size_of::<tlv_ipv4_t>(),
        align_of::<tlv_ipv4_t>(),
        member_offset!(tlv_ipv4_t, bytes),
        size_of::<tlv_ipv4_list_t>(),
        align_of::<tlv_ipv4_list_t>(),
        member_offset!(tlv_ipv4_list_t, raw),
        size_of::<tlv_schema_number_t>(),
        align_of::<tlv_schema_number_t>(),
        member_offset!(tlv_schema_number_t, schema),
        member_offset!(tlv_schema_number_t, representation),
        size_of::<tlv_structure_codec_t>(),
        align_of::<tlv_structure_codec_t>(),
        member_offset!(tlv_structure_codec_t, context),
        member_offset!(tlv_structure_codec_t, format),
        member_offset!(tlv_structure_codec_t, schema),
        member_offset!(tlv_structure_codec_t, max_depth),
        member_offset!(tlv_structure_codec_t, max_elements),
        member_offset!(tlv_structure_codec_t, decode),
        member_offset!(tlv_structure_codec_t, encode),
        size_of::<tlv_generator_candidate_t>(),
        align_of::<tlv_generator_candidate_t>(),
        member_offset!(tlv_generator_candidate_t, tag),
        member_offset!(tlv_generator_candidate_t, min_value_size),
        member_offset!(tlv_generator_candidate_t, max_value_size),
        size_of::<tlv_generator_options_t>(),
        align_of::<tlv_generator_options_t>(),
        member_offset!(tlv_generator_options_t, seed),
        member_offset!(tlv_generator_options_t, case_index),
        member_offset!(tlv_generator_options_t, max_elements),
        member_offset!(tlv_generator_options_t, max_depth),
        member_offset!(tlv_generator_options_t, max_value_size),
        member_offset!(tlv_generator_options_t, max_case_size),
        member_offset!(tlv_generator_options_t, candidates),
        member_offset!(tlv_generator_options_t, candidate_count),
        TLV_GENERATOR_VERSION as usize,
        TLV_STRUCTURE_MAX_DEPTH,
        TLV_TEXT_ASCII_PRINTABLE as usize,
        TLV_TEXT_ASCII_ALNUM as usize,
    ];
    assert_eq!(
        native, rust,
        "Rust generic codec, structure codec or generator FFI layout differs from the C headers"
    );
    std::fs::remove_dir_all(&build).expect("cannot clean up C ABI probe build directory");
}
