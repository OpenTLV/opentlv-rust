// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Links the OpenTLV C library.
//!
//! `core/core-artifact.json` pins the Core dependency. By default the locked
//! Core commit is cloned into `OUT_DIR` and built with CMake as a static
//! library; once a Core release is locked, `core/verified_core.py`
//! authenticates its artifacts instead. `OPENTLV_CORE_SOURCE` selects a local
//! Core checkout for development. Set `OPENTLV_LIB_DIR` to link a prebuilt
//! library instead; `OPENTLV_LINK_KIND` (`static` or `dylib`, default
//! `dylib`) selects how.

#[path = "core/select.rs"]
mod select;

use select::{Lock, Selection};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    for name in [
        "OPENTLV_LIB_DIR",
        "OPENTLV_LINK_KIND",
        "OPENTLV_CORE_SOURCE",
        "OPENTLV_REQUIRE_VERIFIED_CORE",
        "OPENTLV_PYTHON",
        "CI",
        "GITHUB_ACTIONS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let prebuilt = env::var_os("OPENTLV_LIB_DIR");
    // A prebuilt library still needs the matching Core headers and test data.
    let core = select_core(prebuilt.is_some());
    let source = core
        .source_dir
        .to_str()
        .expect("non-UTF-8 Core source path");
    // The ABI probe and corpus tests read the selected Core tree; `opentlv`
    // receives it as `DEP_TLV_CORE_SOURCE`.
    println!("cargo:rustc-env=OPENTLV_CORE_SOURCE_DIR={source}");
    println!("cargo:core_source={source}");

    match prebuilt {
        Some(dir) => {
            let kind = env::var("OPENTLV_LINK_KIND").unwrap_or_else(|_| "dylib".to_string());
            assert!(
                kind == "static" || kind == "dylib",
                "OPENTLV_LINK_KIND must be `static` or `dylib`, got `{kind}`"
            );
            println!("cargo:rustc-link-search=native={}", dir.to_string_lossy());
            println!("cargo:rustc-link-lib={kind}=tlv");
        }
        None => match (&core.lib_dir, &core.include_dir) {
            (Some(lib_dir), Some(include_dir)) => {
                validate(
                    &include_dir.join("tlv/version.h"),
                    &include_dir.join("tlv/config.h"),
                    &core,
                );
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
                println!("cargo:rustc-link-lib=static=tlv");
            }
            _ => build_from_source(&core),
        },
    }
}

/// Core components as CMake options: the fixed API plus the Cargo features.
fn components() -> [(&'static str, bool); 8] {
    [
        ("OPENTLV_READER", true),
        ("OPENTLV_WRITER", true),
        ("OPENTLV_QUERY", true),
        ("OPENTLV_SCHEMA", true),
        ("OPENTLV_CODEC", true),
        ("OPENTLV_DOCUMENT", cfg!(feature = "document")),
        ("OPENTLV_NFC", cfg!(feature = "nfc")),
        ("OPENTLV_LLDP", cfg!(feature = "lldp")),
    ]
}

fn select_core(source_only: bool) -> Selection {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let lock_path = manifest_dir.join("core/core-artifact.json");
    println!("cargo:rerun-if-changed={}", lock_path.display());
    let lock = select::read_lock(&lock_path)
        .unwrap_or_else(|e| panic!("Core dependency verification failed: {e}"));
    let require_verified = select::is_set(env::var("OPENTLV_REQUIRE_VERIFIED_CORE").ok());
    // A failed selection must not leave apparently valid previous evidence.
    let evidence = out_dir.join("core-selection.json");
    let _ = fs::remove_file(&evidence);

    let selected = match env::var_os("OPENTLV_CORE_SOURCE") {
        Some(source) => select::select_local(
            Path::new(&source),
            select::is_ci(|key| env::var(key).ok()),
            require_verified,
        ),
        // Never fall back to Git when a release is locked or required.
        None if lock.release_tag.is_some() || require_verified => {
            resolve_verified(&lock, &lock_path, &manifest_dir, &out_dir, source_only)
        }
        None => select::select_pinned(
            &lock,
            &out_dir.join("core"),
            &format!("https://github.com/{}.git", select::REPOSITORY),
        ),
    };
    let core = selected.unwrap_or_else(|e| panic!("Core dependency verification failed: {e}"));
    fs::write(&evidence, select::evidence(&core, &lock)).expect("cannot record the Core selection");
    if core.mode == "local-source" {
        println!(
            "cargo:warning=Using unverified local Core {} at {}{}; the lock pins {}",
            core.source_dir.display(),
            core.identity.commit,
            if core.identity.dirty {
                " (modified)"
            } else {
                ""
            },
            lock.commit
        );
    }
    core
}

/// Authenticates a locked Core release with `core/verified_core.py`.
fn resolve_verified(
    lock: &Lock,
    lock_path: &Path,
    manifest_dir: &Path,
    out_dir: &Path,
    source_only: bool,
) -> Result<Selection, String> {
    if lock.release_tag.is_none() {
        return Err("a published Core release must be locked for verified builds".to_string());
    }
    let script = manifest_dir.join("core/verified_core.py");
    let probe_dir = manifest_dir.join("core/probe");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed={}", probe_dir.display());
    let output = out_dir.join("core-release.txt");
    let _ = fs::remove_file(&output);
    let wanted: Vec<String> = components()
        .iter()
        .map(|(name, on)| format!("{name}={}", if *on { "ON" } else { "OFF" }))
        .collect();
    let python = env::var_os("OPENTLV_PYTHON")
        .unwrap_or_else(|| if cfg!(windows) { "python" } else { "python3" }.into());
    let mut command = Command::new(python);
    command
        .arg("-B")
        .arg(&script)
        .arg("--lock")
        .arg(lock_path)
        .arg("--work")
        .arg(out_dir.join("core-release"))
        .arg("--output")
        .arg(&output)
        .arg("--target")
        .arg(env::var("TARGET").unwrap())
        .arg("--host")
        .arg(env::var("HOST").unwrap())
        .arg("--components")
        .arg(wanted.join(","));
    if source_only {
        command.arg("--source-only");
    } else {
        // Describe the C toolchain a Verified Source build would use, so
        // only an equivalent binary can replace it.
        let probe = out_dir.join("core-probe");
        run(Command::new("cmake")
            .arg("-S")
            .arg(&probe_dir)
            .arg("-B")
            .arg(&probe));
        command.arg("--probe").arg(probe.join("core-probe.json"));
        let features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        if features.split(',').any(|f| f == "crt-static") {
            command.arg("--crt-static");
        }
    }
    run(&mut command);
    let text = fs::read_to_string(&output).map_err(|e| format!("no verified Core result: {e}"))?;
    select::parse_verified(&text, lock)
}

fn build_from_source(core: &Selection) {
    // On Windows a `\\?\` verbatim path would stop MSVC from opening source
    // files, so strip the prefix.
    let source_dir = match core
        .source_dir
        .to_str()
        .and_then(|p| p.strip_prefix(r"\\?\"))
    {
        Some(plain) => PathBuf::from(plain),
        None => core.source_dir.clone(),
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
        .args(
            components()
                .iter()
                .map(|(name, on)| format!("-D{name}={}", if *on { "ON" } else { "OFF" })),
        )
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
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

    validate(
        &build_dir.join("generated/include/tlv/version.h"),
        &build_dir.join("generated/include/tlv/config.h"),
        core,
    );
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=tlv");
}

/// Rejects a Core library whose public headers differ from the selection.
fn validate(version_h: &Path, config_h: &Path, core: &Selection) {
    select::validate_headers(version_h, config_h, &core.identity, &components())
        .unwrap_or_else(|e| panic!("Core dependency verification failed: {e}"));
}

fn has_library(dir: &Path) -> bool {
    dir.join("libtlv.a").exists() || dir.join("tlv.lib").exists()
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .unwrap_or_else(|e| panic!("failed to run {command:?} (is it installed?): {e}"));
    assert!(status.success(), "{command:?} failed with {status}");
}
