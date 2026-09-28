//! Parse `locales/`, check every message, and generate the module
//! `src/lib.rs` includes, with the catalogs embedded (`Emit::Native`).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    Ok(())
}
