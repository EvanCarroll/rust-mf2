//! The suite's `en-US` tests, as a corpus and as call sites — plus a **twin**
//! locale carrying the same messages, which is what a locale switch is tested
//! against.
//!
//! `en-GB` is the twin. It is a real locale, not a relabelling: its `:unit`
//! names are the British ones (`42 metres` where `en-US` says `42 meters`),
//! so a switch genuinely rewrites text and switching back genuinely has to
//! restore it. What makes it a *twin* is that the messages are the same, so
//! the manifest is the same one and the two catalogs are switchable at all.

fn main() {
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    // conformance/l6-web → the repository root.
    let root = manifest
        .ancestors()
        .nth(2)
        .expect("this crate sits two directories below the root");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    if let Err(e) = mf2_l5_gen::generate_with_twin(root, &out, "en-US", Some("en-GB")) {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}
