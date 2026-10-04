//! What an application's i18n crate writes: parse `locales/`, write the
//! manifest and the catalogs to `OUT_DIR`, generate the module `src/lib.rs`
//! includes.
//!
//! Two features swap the corpus for a smaller one, so that B1′ and B13 can be
//! *gated* rather than measured by hand (`cargo xtask b12-generated`):
//! `corpus-plain` has nothing a function crate could serve,
//! and `corpus-measures` is this crate's corpus plus the three measure
//! functions. A third, `corpus-dates`, has a `:datetime` message, for
//! `cargo xtask codegen-matrix`'s check that an `Intl` client links no ICU4X.

fn main() {
    // With `split-catalogs`, this crate emits only the module: the catalogs
    // belong to a crate the server binary alone depends on (owner question 1).
    // With `native`, the module a native application includes.
    let emit = if std::env::var_os("CARGO_FEATURE_SPLIT_CATALOGS").is_some() {
        mf2_build::Emit::Module
    } else if std::env::var_os("CARGO_FEATURE_NATIVE").is_some() {
        mf2_build::Emit::Native
    } else {
        mf2_build::Emit::Both
    };
    // The corpus: this crate's, or one of its variants.
    let variants: Vec<&str> = [
        ("CARGO_FEATURE_CORPUS_PLAIN", "plain"),
        ("CARGO_FEATURE_CORPUS_MEASURES", "measures"),
        ("CARGO_FEATURE_CORPUS_DATES", "dates"),
    ]
    .into_iter()
    .filter(|(feature, _)| std::env::var_os(feature).is_some())
    .map(|(_, variant)| variant)
    .collect();
    if variants.len() > 1 {
        println!(
            "cargo::error=corpus-plain, corpus-measures and corpus-dates are one corpus each: turn on one"
        );
        std::process::exit(1);
    }
    let variant = variants.first().copied();
    let build = match (mf2_build::Build::new(), variant) {
        (Ok(build), None) => build.emit_cargo(true).emit(emit),
        (Ok(_), Some(variant)) => {
            let root = std::path::PathBuf::from(
                std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
            )
            .join("variants")
            .join(variant);
            let out =
                std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
            mf2_build::Build::at(root, out).emit_cargo(true).emit(emit)
        }
        (Err(e), _) => {
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
