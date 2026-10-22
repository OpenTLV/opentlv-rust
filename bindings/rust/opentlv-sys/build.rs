// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Links the OpenTLV C library.
//!
//! By default the library is built from the repository root with CMake as a
//! static library. Set `OPENTLV_LIB_DIR` to link a prebuilt library instead;
//! `OPENTLV_LINK_KIND` (`static` or `dylib`, default `dylib`) selects how.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=OPENTLV_LIB_DIR");
    println!("cargo:rerun-if-env-changed=OPENTLV_LINK_KIND");

    match env::var_os("OPENTLV_LIB_DIR") {
        Some(dir) => {
            let kind = env::var("OPENTLV_LINK_KIND").unwrap_or_else(|_| "dylib".to_string());
            assert!(
                kind == "static" || kind == "dylib",
                "OPENTLV_LINK_KIND must be `static` or `dylib`, got `{kind}`"
            );
            println!("cargo:rustc-link-search=native={}", dir.to_string_lossy());
            println!("cargo:rustc-link-lib={kind}=tlv");
        }
        None => build_from_source(),
    }
}

fn build_from_source() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source_dir = manifest_dir
        .join("../../..")
        .canonicalize()
        .expect("cannot locate the OpenTLV source tree");
    // On Windows `canonicalize` returns a `\?\` verbatim path that MSVC
    // cannot open source files through, so strip the prefix.
    let source_dir = match source_dir.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => source_dir,
    };
    let build_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("cmake-build");

    for path in ["CMakeLists.txt", "cmake", "tlv"] {
        println!("cargo:rerun-if-changed={}", source_dir.join(path).display());
    }

    // Always Release: Rust links the release C runtime on MSVC even for
    // `cargo build`, so a Debug C build would mix runtimes.
    run(Command::new("cmake")
        .arg("-S")
        .arg(&source_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg(if cfg!(feature = "document") {
            "-DOPENTLV_DOCUMENT=ON"
        } else {
            "-DOPENTLV_DOCUMENT=OFF"
        })
        .arg(if cfg!(feature = "nfc") {
            "-DOPENTLV_NFC=ON"
        } else {
            "-DOPENTLV_NFC=OFF"
        })
        .arg(if cfg!(feature = "lldp") {
            "-DOPENTLV_LLDP=ON"
        } else {
            "-DOPENTLV_LLDP=OFF"
        })
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DOPENTLV_READER=ON",
            "-DOPENTLV_WRITER=ON",
            "-DOPENTLV_QUERY=ON",
            "-DOPENTLV_SCHEMA=ON",
            "-DOPENTLV_CODEC=ON",
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
            "-DOPENTLV_BUILD_SHARED_LIBS=OFF",
            "-DOPENTLV_BUILD_CXX=OFF",
            "-DOPENTLV_BUILD_TESTS=OFF",
            "-DOPENTLV_BUILD_EXAMPLES=OFF",
            "-DOPENTLV_WARNINGS_AS_ERRORS=OFF",
        ]));
    run(Command::new("cmake")
        .arg("--build")
        .arg(&build_dir)
        .args(["--target", "tlv", "--config", "Release"]));

    // Single-config generators put the library in `tlv/`, multi-config ones
    // (Visual Studio, Xcode) in `tlv/Release/`.
    let lib_dir = ["tlv", "tlv/Release"]
        .iter()
        .map(|sub| build_dir.join(sub))
        .find(|dir| has_library(dir))
        .expect("CMake did not produce the tlv static library");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=tlv");
}

fn has_library(dir: &Path) -> bool {
    dir.join("libtlv.a").exists() || dir.join("tlv.lib").exists()
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .unwrap_or_else(|e| panic!("failed to run {command:?} (is CMake installed?): {e}"));
    assert!(status.success(), "{command:?} failed with {status}");
}
