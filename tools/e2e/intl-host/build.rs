//! Parse `locales/`, generate the module, and copy the one catalog to a
//! fixed name in `OUT_DIR` for `src/lib.rs` to embed (a client's generated
//! module embeds none).

use std::fs;

fn fail(e: impl std::fmt::Display) -> ! {
    println!("cargo::error={e}");
    std::process::exit(1);
}

fn main() {
    let build = mf2_build::Build::new().unwrap_or_else(|e| fail(e));
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
