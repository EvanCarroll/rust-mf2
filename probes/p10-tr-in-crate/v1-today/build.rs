// Today's native build script, as docs/native-apps.md writes it.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    Ok(())
}
