//! [`run`]: the whole build script of the crate that includes the generated
//! module.
//!
//! `mf2` has `links = "mf2-v3"` and a build script that prints its features,
//! its target and its version; cargo hands them to the build script of each
//! crate that names `mf2` as a normal dependency, as `DEP_MF2_V3_*`. So the
//! features are written once, on `mf2`, and the build sees exactly what the
//! crate is compiled with, in each of cargo-leptos's two builds too.

use crate::build::{Build, Emit};
use crate::error::{Error, Result};
use crate::features::Features;

/// `mf2`'s features, sorted and comma-separated: set for a crate that names
/// `mf2` as a normal dependency, and only for one.
const FEATURES: &str = "DEP_MF2_V3_FEATURES";
/// `mf2`'s version, which must be this crate's.
const VERSION: &str = "DEP_MF2_V3_VERSION";

/// Everything a build script does: `fn main() { mf2_build::run(); }`.
///
/// It reads `mf2`'s features, builds `locales/` with `mf2.toml` (optional)
/// into `OUT_DIR`, and writes what those features need:
///
/// | `mf2` has | The build writes |
/// |---|---|
/// | `native` | the module; the catalogs, embedded, uncompressed |
/// | `ssr`, or `axum` without a Leptos mode (a web server) | the module; the catalogs, embedded, with `.br` and `.gz` |
/// | `hydrate`, `csr`, or no mode | the module only |
///
/// A debug build compresses at a fast level, a release build at the
/// maximum. It prints the corpus's warnings and errors as
/// `cargo::warning=` and `cargo::error=` lines and its `rerun-if-changed`
/// lines, and on an error exits with a failure, so a build script needs no
/// `Result`.
pub fn run() {
    if let Err(e) = build() {
        for line in e.to_string().lines() {
            println!("cargo::error={line}");
        }
        std::process::exit(1);
    }
}

fn build() -> Result<()> {
    let features = mf2_features(
        std::env::var(FEATURES).ok().as_deref(),
        std::env::var(VERSION).ok().as_deref(),
        cfg!(feature = "icu-blob"),
    )?;
    let emit = emit_for(&features);
    Build::new()?
        .features(features)
        .emit(emit)
        .emit_cargo(true)
        .run()?
        .into_result()
        .map(drop)
}

/// `mf2`'s features from its `links` metadata, checked against this crate:
/// the same version, and `icu-blob` when either side's date formatter is
/// `icu` (the build cuts the date slice for both builds).
fn mf2_features(features: Option<&str>, version: Option<&str>, icu_blob: bool) -> Result<Features> {
    // An empty value is `mf2` with no features; an absent one, a crate that
    // does not name it (or names it only as a build-dependency).
    let features = Features::parse(features.ok_or(Error::NoMf2)?);
    let ours = env!("CARGO_PKG_VERSION");
    if version != Some(ours) {
        return Err(Error::Mf2Version {
            mf2: version.unwrap_or("unknown").to_owned(),
            build: ours.to_owned(),
        });
    }
    features.check_icu_blob(icu_blob)?;
    Ok(features)
}

/// What a build with `mf2`'s `features` writes.
fn emit_for(features: &Features) -> Emit {
    let leptos_mode = ["ssr", "hydrate", "csr"].iter().any(|m| features.has(m));
    if features.has("ssr") || (features.has("axum") && !leptos_mode) {
        // With `native` too, `CORPUS` shares the table `CATALOGS` embeds.
        Emit::Both
    } else if features.has("native") {
        Emit::Native
    } else {
        // `hydrate`: the server's build embeds the catalogs; `csr`: `mf2
        // compile --site` publishes them; no mode: a library of descriptions.
        Emit::Module
    }
}

#[cfg(test)]
mod tests {
    use super::{emit_for, mf2_features};
    use crate::build::Emit;
    use crate::error::Error;
    use crate::features::Features;

    const OURS: Option<&str> = Some(env!("CARGO_PKG_VERSION"));

    #[test]
    fn each_mode_emits_what_it_serves() {
        let emit = |list: &str| emit_for(&Features::parse(list));
        assert_eq!(emit("fn-number,native"), Emit::Native);
        assert_eq!(emit("native,ratatui"), Emit::Native);
        assert_eq!(emit("leptos,ssr"), Emit::Both);
        assert_eq!(emit("leptos,native,ssr"), Emit::Both);
        assert_eq!(emit("axum"), Emit::Both);
        assert_eq!(emit("axum,hydrate,leptos"), Emit::Module);
        assert_eq!(emit("hydrate,leptos"), Emit::Module);
        assert_eq!(emit("csr,leptos"), Emit::Module);
        assert_eq!(emit(""), Emit::Module);
        assert_eq!(emit("host-std"), Emit::Module);
    }

    #[test]
    fn the_features_come_from_mf2_and_its_version_must_match() {
        let features = mf2_features(Some("fn-number,native"), OURS, false).expect("read");
        assert!(features.fn_number() && features.has("native"));
        assert_eq!(
            mf2_features(Some(""), OURS, false).expect("no features"),
            Features::default()
        );
        assert!(matches!(mf2_features(None, None, false), Err(Error::NoMf2)));
        let Err(Error::Mf2Version { mf2, .. }) = mf2_features(Some(""), Some("1.0.0"), false)
        else {
            panic!("a version mismatch is refused");
        };
        assert_eq!(mf2, "1.0.0");
    }

    #[test]
    fn datetime_icu_needs_icu_blob_here() {
        let list = Some("datetime,host-std-datetime-icu,native-datetime-icu");
        let e = mf2_features(list, OURS, false).expect_err("refused without icu-blob");
        let message = e.to_string();
        assert!(
            message.contains(&format!(
                "under [build-dependencies], write: mf2-build = {{ version = \"{}\", \
                 features = [\"icu-blob\"] }}",
                env!("CARGO_PKG_VERSION")
            )),
            "{message}"
        );
        assert!(
            message.starts_with("mf2-build: the date formatter of `native-datetime-icu` is ICU4X"),
            "{message}"
        );
        assert!(mf2_features(list, OURS, true).is_ok());
    }

    #[test]
    fn an_icu_formatter_on_either_side_needs_icu_blob_here() {
        for list in [
            "datetime,host-std-datetime-icu,native-datetime-icu",
            "datetime,host-web-datetime-icu,leptos-client-datetime-icu",
            "datetime,host-web-datetime-intl,host-std-datetime-icu",
            "datetime,host-web-datetime-intl,host-web-datetime-icu",
        ] {
            let e = mf2_features(Some(list), OURS, false).expect_err(list);
            assert!(matches!(e, Error::IcuBlob { .. }), "{list}");
            assert!(mf2_features(Some(list), OURS, true).is_ok(), "{list}");
        }
        for list in [
            "datetime,host-web-datetime-intl,host-std-datetime-iso",
            "datetime,host-web-datetime-iso,host-std-datetime-iso",
            "datetime",
        ] {
            assert!(mf2_features(Some(list), OURS, false).is_ok(), "{list}");
        }
    }
}
