//! Parse `locales/`, write the manifest, and generate the module `src/lib.rs`
//! includes (`plans/05-tooling.md` §4).
//!
//! `Emit::Module`: a client-only application has no server to embed the
//! catalogs in. They are published by `mf2 compile --site`, which trunk runs
//! after the build (`../Trunk.toml`), so this crate never names one — and a
//! translation edit leaves the wasm as it was.

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
