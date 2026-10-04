//! Parse `locales/`, write the manifest, and generate the module `src/lib.rs`
//! includes.
//!
//! `Emit::Module`, as in `examples/demo-csr`: the harness is a client-only
//! page, and `cargo xtask churn` publishes the catalogs beside it with
//! `mf2 compile --site`.

fn main() {
    let build = match mf2_build::Build::new() {
        Ok(build) => build.emit(mf2_build::Emit::Module).emit_cargo(true),
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
