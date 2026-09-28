fn main() -> Result<(), Box<dyn std::error::Error>> {
    let outcome = mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    let manifest = outcome.out_dir.join("manifest.mf2m");
    println!("cargo::rustc-env=MF2_MANIFEST={}", manifest.display());
    println!("cargo::rustc-env=MF2_MANIFEST_HASH={}", outcome.manifest_hash);
    Ok(())
}
