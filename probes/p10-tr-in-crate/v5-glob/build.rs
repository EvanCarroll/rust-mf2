//! Today's module (`gen-today`), or 3a's local shape (`gen-local`).

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let outcome = mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    let module = if std::env::var_os("CARGO_FEATURE_GEN_LOCAL").is_some() {
        let manifest = outcome.out_dir.join("manifest.mf2m");
        let source = format!("{:?}", manifest.display().to_string());
        let hash = outcome.manifest_hash;
        let cut = outcome
            .generated
            .find("\n/// `tr!(\"id\"")
            .ok_or("the generated module has no tr! wrapper")?;
        let mut module = outcome.generated[..cut].to_owned();
        module.push_str(&format!(
            "\nmacro_rules! tr {{\n    ($($t:tt)*) => {{\n        $crate::__mf2::__tr_impl!({source} 0x{hash:016x}u64 ; $crate ; $($t)*)\n    }};\n}}\n#[allow(unused_imports)]\npub(crate) use tr;\n"
        ));
        module
    } else {
        outcome.generated
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR")?);
    let path = out.join("mf2_generated_v5.rs");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(module.as_str()) {
        std::fs::write(&path, module)?;
    }
    Ok(())
}
