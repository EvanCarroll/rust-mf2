//! Builds one of conformance L7's four page sets
//! (`plans/15-phase-7-work-order.md` A4).
//!
//! A set is the suite's tests of **one** locale — `en-US`, `und`, `fr` or
//! `ar` — as an i18n crate: a corpus with the twin `en-GB` beside it, the
//! generated module, one call site per test, and [`SET`], the part a page
//! needs. Four crates rather than one because a generated module belongs to
//! the crate that includes it, and a page is in one locale at a time: the
//! wasm carries all four and installs the one its page names.
//!
//! The configuration follows the set crate's own features: with
//! `fn-number` and a date formatter on it is the whole runtime-valid corpus;
//! with neither it is the default configuration, where the build refuses a
//! message naming a gated function, so the set holds the rest
//! ([`mf2_l5_gen::Configuration::Default`]).

use std::path::PathBuf;

use mf2_l5_gen::Configuration;

/// The locale every set switches to and back from: the same messages under
/// another tag. `en-GB` is real locale data (`42 metres`), so a switch
/// rewrites text rather than relabelling it.
pub const TWIN: &str = "en-GB";

/// What a page needs from a set, beyond its generated module and its call
/// sites. Written into `OUT_DIR` beside them for the reason `mf2-l5-gen`
/// writes `shared.rs` there: rust-analyzer will not load a file outside the
/// crate that includes it.
const SET: &str = include_str!("set.rs");

/// Generates the set for `locale`. Call it from the set crate's `build.rs`.
///
/// # Panics
///
/// Never; a failure is reported to cargo as a build error and exits.
pub fn generate(locale: &str) {
    if let Err(e) = try_generate(locale) {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}

fn try_generate(locale: &str) -> Result<(), mf2_l5_gen::Error> {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or("no manifest dir")?);
    // conformance/l7-web/sets/<set> → the repository root.
    let root = manifest
        .ancestors()
        .nth(4)
        .ok_or("a set sits four directories below the root")?;
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("no OUT_DIR")?);
    let number = std::env::var_os("CARGO_FEATURE_FN_NUMBER").is_some();
    let datetime = [
        "CARGO_FEATURE_HOST_STD_DATETIME_ICU",
        "CARGO_FEATURE_HOST_WEB_DATETIME_ICU",
    ]
    .into_iter()
    .any(|name| std::env::var_os(name).is_some());
    let configuration = match (number, datetime) {
        (true, true) => Configuration::All,
        (false, false) => Configuration::Default,
        _ => {
            return Err("a set is built with every function feature or with none; \
                        the suite has no configuration in between"
                .into());
        }
    };
    mf2_l5_gen::generate_page(root, &out, locale, TWIN, configuration)?;
    let path = out.join("set.rs");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(SET) {
        std::fs::write(&path, SET)?;
    }
    Ok(())
}
