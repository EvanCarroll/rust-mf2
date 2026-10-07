//! The workload's corpus, compiled as an application's i18n crate.
//!
//! `MF2_WORKLOAD_LOCALES` names the generated workload directory — the one
//! holding `locales/<tag>/*.mf2` — so that one crate serves every scale and
//! every template variant the B5 measurement builds.

fn main() {
    println!("cargo::rerun-if-env-changed=MF2_WORKLOAD_LOCALES");
    let Some(root) = std::env::var_os("MF2_WORKLOAD_LOCALES") else {
        println!(
            "cargo::error=MF2_WORKLOAD_LOCALES is not set: it must name a generated workload \
             directory (the one with locales/), which `cargo xtask browser-app-size` sets"
        );
        std::process::exit(1);
    };
    let root = std::path::PathBuf::from(root);
    let out = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"),
    );
    let mut config = mf2_build::Config::default();
    config.source_locale = "en".to_owned();
    // The generated corpus has pseudo-locales that do not translate every
    // message, and a shape chosen for size, not for lint cleanliness.
    for &lint in mf2_build::Lint::ALL {
        let floor = lint.floor();
        if floor != mf2_build::Level::Error {
            config.lints.insert(lint, floor);
        }
    }
    // `MF2_WORKLOAD_INLINE_MANIFEST=1` bakes the manifest's bytes into the
    // `tr!` wrapper instead of its path (05 §4): what a build whose target
    // directory moves under it needs, and what A7 times against the path
    // mode.
    println!("cargo::rerun-if-env-changed=MF2_WORKLOAD_INLINE_MANIFEST");
    let inline = std::env::var_os("MF2_WORKLOAD_INLINE_MANIFEST").is_some_and(|v| v == "1");
    let build = mf2_build::Build::at(&root, &out)
        .config(config)
        .manifest_inline(inline)
        .emit_cargo(true);
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
