// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Compare the field encoding, Format composition and Format operation FFI
//! mirrors with sizeof/alignment/offsetof from the C headers. Like
//! `fixed_abi.rs`, the small C probe uses CMake and the target's native C
//! compiler.

use opentlv_sys::{
    tlv_constructed_bit_t, tlv_encoding_t, tlv_escaped_format_t, tlv_escaped_length_t,
    tlv_field_composition_t, tlv_format_error_t, tlv_identifier_policy_t, tlv_length_policy_t,
    tlv_packed_field_t, tlv_packed_layout_t, tlv_tagged_binary_composition_t,
    tlv_tagged_fields_composition_t, tlv_variable_format_t, tlv_variable_identifier_t,
    tlv_variable_length_t,
};
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
fn composition_layouts_match_c() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let build = std::env::temp_dir().join(format!(
        "opentlv-compose-abi-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&build).expect("cannot create C ABI probe build directory");
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(manifest.join("tests/compose_abi_probe"))
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
        size_of::<tlv_format_error_t>(),
        align_of::<tlv_format_error_t>(),
        member_offset!(tlv_format_error_t, region),
        member_offset!(tlv_format_error_t, offset),
        member_offset!(tlv_format_error_t, has_offset),
        member_offset!(tlv_format_error_t, required),
        member_offset!(tlv_format_error_t, has_required),
        member_offset!(tlv_format_error_t, tag),
        member_offset!(tlv_format_error_t, length),
        member_offset!(tlv_format_error_t, value),
        size_of::<tlv_encoding_t>(),
        align_of::<tlv_encoding_t>(),
        member_offset!(tlv_encoding_t, header),
        member_offset!(tlv_encoding_t, value),
        member_offset!(tlv_encoding_t, trailer),
        member_offset!(tlv_encoding_t, total),
        size_of::<tlv_constructed_bit_t>(),
        align_of::<tlv_constructed_bit_t>(),
        member_offset!(tlv_constructed_bit_t, byte_index),
        member_offset!(tlv_constructed_bit_t, mask),
        member_offset!(tlv_constructed_bit_t, value),
        size_of::<tlv_identifier_policy_t>(),
        align_of::<tlv_identifier_policy_t>(),
        member_offset!(tlv_identifier_policy_t, forbidden_leading),
        member_offset!(tlv_identifier_policy_t, forbidden_leading_count),
        member_offset!(tlv_identifier_policy_t, reject_zero_first_payload),
        member_offset!(tlv_identifier_policy_t, require_minimal),
        size_of::<tlv_length_policy_t>(),
        align_of::<tlv_length_policy_t>(),
        member_offset!(tlv_length_policy_t, allow_short),
        member_offset!(tlv_length_policy_t, allow_long),
        member_offset!(tlv_length_policy_t, require_minimal),
        member_offset!(tlv_length_policy_t, max_long_octets),
        member_offset!(tlv_length_policy_t, max_value),
        size_of::<tlv_variable_identifier_t>(),
        align_of::<tlv_variable_identifier_t>(),
        member_offset!(tlv_variable_identifier_t, inline_mask),
        member_offset!(tlv_variable_identifier_t, escape),
        member_offset!(tlv_variable_identifier_t, continuation_bit),
        member_offset!(tlv_variable_identifier_t, payload_mask),
        member_offset!(tlv_variable_identifier_t, max_size),
        member_offset!(tlv_variable_identifier_t, policy),
        size_of::<tlv_variable_length_t>(),
        align_of::<tlv_variable_length_t>(),
        member_offset!(tlv_variable_length_t, long_form_bit),
        member_offset!(tlv_variable_length_t, payload_mask),
        member_offset!(tlv_variable_length_t, byte_order),
        member_offset!(tlv_variable_length_t, policy),
        size_of::<tlv_escaped_length_t>(),
        align_of::<tlv_escaped_length_t>(),
        member_offset!(tlv_escaped_length_t, escape),
        member_offset!(tlv_escaped_length_t, extended_size),
        member_offset!(tlv_escaped_length_t, byte_order),
        member_offset!(tlv_escaped_length_t, min_extended),
        member_offset!(tlv_escaped_length_t, max_length),
        size_of::<tlv_packed_field_t>(),
        align_of::<tlv_packed_field_t>(),
        member_offset!(tlv_packed_field_t, storage_size),
        member_offset!(tlv_packed_field_t, bit_offset),
        member_offset!(tlv_packed_field_t, bit_width),
        member_offset!(tlv_packed_field_t, byte_order),
        size_of::<tlv_field_composition_t>(),
        align_of::<tlv_field_composition_t>(),
        member_offset!(tlv_field_composition_t, context),
        member_offset!(tlv_field_composition_t, read_tag),
        member_offset!(tlv_field_composition_t, read_length),
        member_offset!(tlv_field_composition_t, resolve),
        member_offset!(tlv_field_composition_t, write_tag),
        member_offset!(tlv_field_composition_t, write_length),
        member_offset!(tlv_field_composition_t, element_order),
        member_offset!(tlv_field_composition_t, length_scope),
        size_of::<tlv_tagged_binary_composition_t>(),
        align_of::<tlv_tagged_binary_composition_t>(),
        member_offset!(tlv_tagged_binary_composition_t, fields),
        member_offset!(tlv_tagged_binary_composition_t, tag_only),
        member_offset!(tlv_tagged_binary_composition_t, count),
        size_of::<tlv_tagged_fields_composition_t>(),
        align_of::<tlv_tagged_fields_composition_t>(),
        member_offset!(tlv_tagged_fields_composition_t, fields),
        member_offset!(tlv_tagged_fields_composition_t, tag_only),
        member_offset!(tlv_tagged_fields_composition_t, count),
        size_of::<tlv_escaped_format_t>(),
        align_of::<tlv_escaped_format_t>(),
        member_offset!(tlv_escaped_format_t, identifier),
        member_offset!(tlv_escaped_format_t, length),
        member_offset!(tlv_escaped_format_t, element_order),
        member_offset!(tlv_escaped_format_t, length_scope),
        member_offset!(tlv_escaped_format_t, tag_only),
        member_offset!(tlv_escaped_format_t, count),
        size_of::<tlv_packed_layout_t>(),
        align_of::<tlv_packed_layout_t>(),
        member_offset!(tlv_packed_layout_t, header_size),
        member_offset!(tlv_packed_layout_t, tag),
        member_offset!(tlv_packed_layout_t, length),
        member_offset!(tlv_packed_layout_t, length_scope),
        member_offset!(tlv_packed_layout_t, tag_storage),
        member_offset!(tlv_packed_layout_t, tag_size),
        member_offset!(tlv_packed_layout_t, tag_storage_size),
        size_of::<tlv_variable_format_t>(),
        align_of::<tlv_variable_format_t>(),
        member_offset!(tlv_variable_format_t, identifier),
        member_offset!(tlv_variable_format_t, length),
        member_offset!(tlv_variable_format_t, element_order),
        member_offset!(tlv_variable_format_t, length_scope),
        member_offset!(tlv_variable_format_t, constructed),
    ];
    assert_eq!(
        native, rust,
        "Rust field encoding or Format composition FFI layout differs from the C headers"
    );
    std::fs::remove_dir_all(&build).expect("cannot clean up C ABI probe build directory");
}
