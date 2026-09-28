//! The translation crate's build script, in the application: mf2-build over
//! locales/ (Emit::Both — the catalogs embedded for the server under `ssr`),
//! with the generated `tr!` / `msg_id!` wrappers swapped for A3's shape
//! (3c): each exported under a hidden name and re-exported by a `use`, plus a
//! `prelude`. A stand-in for what 2.0's codegen would emit; everything else
//! is mf2-build's module, byte for byte.

use std::path::PathBuf;

fn main() {
    if let Err(e) = run() {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let outcome = mf2_build::Build::new()?.emit_cargo(true).run()?.into_result()?;
    let manifest = outcome.out_dir.join("manifest.mf2m");
    let source = format!("{:?}", manifest.display().to_string());
    let hash = outcome.manifest_hash;
    let cut = outcome
        .generated
        .find("\n/// `tr!(\"id\"")
        .ok_or("the generated module has no tr! wrapper")?;
    let mut module = outcome.generated[..cut].to_owned();
    let wrapper = |imp: &str| {
        format!(
            "{{\n    ($($t:tt)*) => {{\n        $crate::__mf2::{imp}!({source} 0x{hash:016x}u64 ; $crate ; $($t)*)\n    }};\n}}\n"
        )
    };
    module.push_str(&format!(
        "\n/// `tr!(\"id\", name = value, …)`.\n#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_tr {}\n#[allow(unused_imports)]\npub use __mf2_tr as tr;\n\n\
         /// `msg_id!(\"id\")`.\n#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_msg_id {}\n#[allow(unused_imports)]\npub use __mf2_msg_id as msg_id;\n\n\
         /// What a module of this crate, or of a crate that depends on it, imports.\npub mod prelude {{\n    #[allow(unused_imports)]\n    pub use super::{{msg_id, tr}};\n}}\n",
        wrapper("__tr_impl"),
        wrapper("__msg_id_impl")
    ));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR")?);
    let path = out.join("mf2_generated_3c.rs");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(module.as_str()) {
        std::fs::write(&path, module)?;
    }
    Ok(())
}
