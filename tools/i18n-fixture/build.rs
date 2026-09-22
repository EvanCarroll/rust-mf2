//! What an application's i18n crate writes: parse `locales/`, write the
//! manifest and the catalogs to `OUT_DIR`, generate the module `src/lib.rs`
//! includes (plans/05-tooling.md §4).

fn main() {
    // With `split-catalogs`, this crate emits only the module: the catalogs
    // belong to a crate the server binary alone depends on (owner question 1).
    let emit = if std::env::var_os("CARGO_FEATURE_SPLIT_CATALOGS").is_some() {
        mf2_build::Emit::Module
    } else {
        mf2_build::Emit::Both
    };
    let outcome = match mf2_build::Build::new().and_then(|b| b.emit_cargo(true).emit(emit).run()) {
        Ok(outcome) => outcome,
        Err(e) => {
            println!("cargo::error={e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = outcome.into_result() {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}
