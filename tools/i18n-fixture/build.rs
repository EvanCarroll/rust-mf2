//! What an application's i18n crate writes: parse `locales/`, write the
//! manifest and the catalogs to `OUT_DIR`, generate the module `src/lib.rs`
//! includes (plans/05-tooling.md §4).

fn main() {
    let outcome = match mf2_build::Build::new().and_then(|b| b.emit_cargo(true).run()) {
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
