//! The suite's `en-US` tests, as a corpus and as call sites.

fn main() {
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    // conformance/l5/<tag> → the repository root.
    let root = manifest
        .ancestors()
        .nth(3)
        .expect("an L5 crate sits three directories below the root");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    if let Err(e) = mf2_l5_gen::generate(root, &out, "en-US") {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}
