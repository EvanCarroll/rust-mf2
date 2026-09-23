//! Parse `locales/`, write the manifest and the catalogs into `OUT_DIR`, and
//! generate the module `src/lib.rs` includes (`plans/05-tooling.md` §4).

fn main() {
    let build = match mf2_build::Build::new() {
        Ok(build) => build.emit_cargo(true),
        Err(e) => {
            println!("cargo::error={e}");
            std::process::exit(1);
        }
    };
    let outcome = match build.run() {
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
