//! What an application's i18n crate writes: parse `locales/`, write the
//! manifest and the catalogs to `OUT_DIR`, generate the module `src/lib.rs`
//! includes (plans/05-tooling.md §4).
//!
//! Two features swap the corpus for a smaller one, so that B1′ and B13 can be
//! *gated* rather than measured by hand (`cargo xtask b12-generated`,
//! plans/13 A10): `corpus-plain` has nothing a function crate could serve,
//! and `corpus-measures` is this crate's corpus plus the three measure
//! functions.

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
    // The corpus: this crate's, or one of the two size variants.
    let plain = std::env::var_os("CARGO_FEATURE_CORPUS_PLAIN").is_some();
    let measures = std::env::var_os("CARGO_FEATURE_CORPUS_MEASURES").is_some();
    if plain && measures {
        println!("cargo::error=corpus-plain and corpus-measures are one corpus each: turn on one");
        std::process::exit(1);
    }
    let variant = if std::env::var_os("CARGO_FEATURE_CORPUS_PLAIN").is_some() {
        Some("plain")
    } else if std::env::var_os("CARGO_FEATURE_CORPUS_MEASURES").is_some() {
        Some("measures")
    } else {
        None
    };
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
