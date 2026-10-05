//! Parse the corpus, generate the module, and copy the one catalog to a
//! fixed name in `OUT_DIR` for `src/lib.rs` to embed (a client's generated
//! module embeds none).
//!
//! `plain` reads `variants/plain/`: plain digits cannot show a currency, so
//! the build refuses `:currency` there, and that variant's corpus has none.

use std::fs;
use std::path::PathBuf;

fn fail(e: impl std::fmt::Display) -> ! {
    println!("cargo::error={e}");
    std::process::exit(1);
}

fn main() {
    let build = if std::env::var_os("CARGO_FEATURE_PLAIN").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| fail("no manifest dir")),
        )
        .join("variants/plain");
        let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_else(|| fail("no OUT_DIR")));
        mf2_build::Build::at(root, out)
    } else {
        mf2_build::Build::new().unwrap_or_else(|e| fail(e))
    };
    let outcome = build
        .emit(mf2_build::Emit::Both)
        .emit_cargo(true)
        .run()
        .unwrap_or_else(|e| fail(e))
        .into_result()
        .unwrap_or_else(|e| fail(e));
    let Some(en) = outcome.locales.iter().find(|l| l.tag == "en") else {
        fail("no `en` catalog");
    };
    fs::copy(
        outcome.out_dir.join(&en.file_name),
        outcome.out_dir.join("intl-host-en.mf2b"),
    )
    .unwrap_or_else(|e| fail(e));
}
