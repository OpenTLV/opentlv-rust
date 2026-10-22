// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Passes the Core source tree selected by `opentlv-sys` to the tests, which
//! replay Core-owned fuzz seeds.

fn main() {
    println!("cargo:rerun-if-env-changed=DEP_TLV_CORE_SOURCE");
    let source = std::env::var("DEP_TLV_CORE_SOURCE")
        .expect("opentlv-sys did not report the selected Core source");
    println!("cargo:rustc-env=OPENTLV_CORE_SOURCE_DIR={source}");
}
