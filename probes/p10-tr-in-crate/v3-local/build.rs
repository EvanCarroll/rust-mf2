//! mf2-build's native module, with the `tr!` wrapper swapped for the
//! variant's shape — a stand-in for what codegen would emit.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let outcome = mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    let manifest = outcome.out_dir.join("manifest.mf2m");
    let source = format!("{:?}", manifest.display().to_string());
    let hash = outcome.manifest_hash;

    // Everything before the generated `tr!` wrapper's doc comment.
    let cut = outcome
        .generated
        .find("\n/// `tr!(\"id\"")
        .ok_or("the generated module has no tr! wrapper")?;
    let mut module = outcome.generated[..cut].to_owned();

    let body = format!(
        "{{\n    ($($t:tt)*) => {{\n        $crate::__mf2::__tr_impl!({source} 0x{hash:016x}u64 ; $crate ; $($t)*)\n    }};\n}}\n"
    );
    let on = |f: &str| std::env::var_os(format!("CARGO_FEATURE_{f}")).is_some();
    let tail = if on("SHAPE_LOCAL") {
        format!(
            "\n/// In-crate `tr!` (3a).\nmacro_rules! tr {body}\n#[allow(unused_imports)]\npub(crate) use tr;\n\n\
             /// The crate's prelude.\npub mod prelude {{\n    #[allow(unused_imports)]\n    pub(crate) use super::tr;\n}}\n"
        )
    } else if on("SHAPE_EXPORT_USE") {
        format!(
            "\n/// Exported `tr!`, re-imported at the root (3b).\n#[macro_export]\nmacro_rules! tr {body}\n#[allow(unused_imports)]\npub(crate) use tr;\n\n\
             /// The crate's prelude.\npub mod prelude {{\n    #[allow(unused_imports)]\n    pub use super::tr;\n}}\n"
        )
    } else if on("SHAPE_HIDDEN_EXPORT") {
        format!(
            "\n/// Exported under a hidden name, re-exported as `tr` (3c).\n#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_tr {body}\n#[allow(unused_imports)]\npub use __mf2_tr as tr;\n\n\
             /// The crate's prelude.\npub mod prelude {{\n    #[allow(unused_imports)]\n    pub use super::tr;\n}}\n"
        )
    } else if on("SHAPE_HIDDEN_EXPORT_TEXTUAL") {
        format!(
            "\n/// 3c, plus a textual `tr` for the code after the include (3e).\n#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_tr {body}\n#[allow(unused_imports)]\npub use __mf2_tr as tr;\n\n\
             #[allow(unused_macros)]\nmacro_rules! tr {body}\n\n\
             /// The crate's prelude.\npub mod prelude {{\n    #[allow(unused_imports)]\n    pub use super::tr;\n}}\n"
        )
    } else if on("SHAPE_LOCAL_AND_EXPORT") {
        format!(
            "\n/// In-crate `tr!` (3d).\nmacro_rules! tr {body}\n#[allow(unused_imports)]\npub(crate) use tr;\n\n\
             /// For other crates.\n#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_tr_export {body}\n\n\
             /// What other crates name.\npub mod exports {{\n    #[allow(unused_imports)]\n    pub use __mf2_tr_export as tr;\n}}\n\n\
             /// The crate's prelude.\npub mod prelude {{\n    #[allow(unused_imports)]\n    pub(crate) use super::tr;\n}}\n"
        )
    } else {
        return Err("turn on one shape-* feature".into());
    };
    module.push_str(&tail);

    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR")?);
    let path = out.join("mf2_generated_v3.rs");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(module.as_str()) {
        std::fs::write(&path, module)?;
    }
    Ok(())
}
