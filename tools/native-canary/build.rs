//! The i18n build a native application writes: parse the corpus, write the
//! catalogs and the module `src/main.rs` includes (`Emit::Native`).
//!
//! A `:datetime` message is refused by a build without a date formatter (a
//! message may not pull formatting code in by itself), so the rows that have
//! the feature read `variants/dates/`, which is the base corpus and a date.

fn main() {
    let fail = |e: mf2_build::Error| -> ! {
        println!("cargo::error={e}");
        std::process::exit(1)
    };
    let dates = [
        "CARGO_FEATURE_NATIVE_DATETIME_ISO",
        "CARGO_FEATURE_NATIVE_DATETIME_ICU",
    ]
    .into_iter()
    .any(|name| std::env::var_os(name).is_some());
    let made = if dates {
        let root = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
        )
        .join("variants/dates");
        let out =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
        Ok(mf2_build::Build::at(root, out))
    } else {
        mf2_build::Build::new()
    };
    let build = match made {
        Ok(build) => build.emit_cargo(true).emit(mf2_build::Emit::Native),
        Err(e) => fail(e),
    };
    match build.run() {
        Ok(outcome) => {
            if let Err(e) = outcome.into_result() {
                fail(e);
            }
        }
        Err(e) => fail(e),
    }
}
