//! Build script, active only under the `dds-reference` feature.
//!
//! Every other build — the library, the CLI binaries, wasm, CI — takes the
//! early return and does nothing, so no ordinary build needs DDS present.

use std::path::PathBuf;

/// Where dealer3's `build-dealerv2-macos.sh` leaves its arm64 build, used when
/// `DDS_LIB_DIR` says nothing.
const DEFAULT_DDS_DIR: &str = "../Dealer-Version-2-/macos-build";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DDS_LIB_DIR");

    if std::env::var_os("CARGO_FEATURE_DDS_REFERENCE").is_none() {
        return;
    }

    let dir = std::env::var("DDS_LIB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_DDS_DIR));
    let lib = dir.join("lib");
    let archive = lib.join("libdds.a");

    if !archive.exists() {
        // A missing DDS is a plain configuration problem, so say exactly how
        // to fix it rather than letting the linker fail obscurely.
        println!("cargo:warning=DDS not found at {}", archive.display());
        println!("cargo:warning=The `dds-reference` feature needs an arm64 libdds.a.");
        println!("cargo:warning=Build one with dealer3's scripts/build-dealerv2-macos.sh,");
        println!("cargo:warning=or point DDS_LIB_DIR at a tree containing lib/libdds.a.");
        panic!("dds-reference enabled but libdds.a not found");
    }

    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=dds");
    // DDS is C++ and was built with the GCD + STL threading backend, so the
    // standard library is the only extra link input it needs.
    println!("cargo:rustc-link-lib=dylib=c++");
}
