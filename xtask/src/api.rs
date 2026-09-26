//! `cargo xtask api [--check]`: the public API of the 16 published crates,
//! listed and committed as `crates/<name>/api.txt`
//! (`plans/17-phase-9-work-order.md` A2; `docs/versioning.md`).
//!
//! A library crate's listing is `cargo public-api -ss`'s (no blanket or
//! auto-trait impls; derived ones stay, since removing a derive breaks a
//! caller), made in-process with the
//! `public-api` and `rustdoc-json` crates from the rustdoc JSON of a pinned
//! nightly (installed through rustup when missing; `--document-hidden-items`
//! is not passed, so `#[doc(hidden)]` items — which 1.x does not promise —
//! are not listed). Each crate is listed with the features and target its
//! `[package.metadata.docs.rs]` gives docs.rs (`cargo xtask docs-rs`), so
//! the published documentation shows what the listing promises: the feature
//! set an application turns on for its server, and `mf2-host-web` for
//! `wasm32-unknown-unknown`, the only target it has.
//!
//! `mf2-cli` is a binary: its promise is the command tree, and its listing
//! is the commands and their arguments as clap declares them, written and
//! checked by its own test (`listing::api_txt`, which `cargo test` runs
//! too).
//!
//! Without `--check`, the listings are written; with it, each is compared
//! with the committed file and any difference fails, naming the lines — a
//! change to the public API is committed together with its `api.txt`.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::{cargo, run_capture, run_inherit_env};
use crate::docs_rs::Presented;
use crate::error::{Error, Result};
use crate::fsx;

/// The nightly whose rustdoc JSON `public-api` reads. Pinned, as the JSON
/// format changes between nightlies (this one writes format 61, which
/// `public-api` 0.52.2 reads).
pub(crate) const NIGHTLY: &str = "nightly-2026-09-24";

const WASM: &str = "wasm32-unknown-unknown";

/// The first line of every listing.
const HEADER: &str = "# The public API that 1.x promises (docs/versioning.md). Written by \
                      `cargo xtask api`, checked by `cargo xtask ci`; commit it with the change.";

fn fail(message: impl Into<String>) -> Error {
    Error::Api(message.into())
}

pub(crate) fn run(root: &Path, check: bool) -> Result<()> {
    install(root)?;
    let listed = crate::docs_rs::presented(root)?;
    let mut stale = Vec::new();
    for listed in &listed {
        eprintln!("==> api: {}", listed.name);
        let text = listing(root, listed)?;
        if let Some(diff) = compare(root, &listed.name, &text, check)? {
            stale.push(diff);
        }
    }
    eprintln!("==> api: mf2-cli (its command tree)");
    cli(root, check)?;
    if stale.is_empty() {
        eprintln!(
            "api: {} listings {}",
            listed.len() + 1,
            if check { "unchanged" } else { "written" }
        );
        Ok(())
    } else {
        Err(fail(format!(
            "the public API differs from the committed api.txt; run `cargo xtask api` and \
             commit the listings with the change\n{}",
            stale.join("\n")
        )))
    }
}

/// `listed`'s public API, as `api.txt` holds it.
fn listing(root: &Path, listed: &Presented) -> Result<String> {
    // Documented for another target than the host's: listed for it too.
    let target = listed.default_target.as_ref();
    let mut builder = rustdoc_json::Builder::default()
        .toolchain(NIGHTLY)
        .manifest_path(root.join("crates").join(&listed.name).join("Cargo.toml"))
        .target_dir(root.join("target").join("api"))
        .features(&listed.features)
        .all_features(listed.all_features)
        .no_default_features(listed.no_default_features)
        .quiet(true);
    if let Some(target) = target {
        builder = builder.target(target.clone());
    }
    let json = builder
        .build()
        .map_err(|e| fail(format!("{}: rustdoc JSON: {e}", listed.name)))?;
    let api = public_api::Builder::from_rustdoc_json(&json)
        .omit_blanket_impls(true)
        .omit_auto_trait_impls(true)
        .build()
        .map_err(|e| fail(format!("{}: {e}", listed.name)))?;
    let features = if listed.all_features {
        "(all)".to_owned()
    } else if listed.features.is_empty() {
        "(default)".to_owned()
    } else {
        listed.features.join(",")
    };
    let target = target.map(|t| format!("; target: {t}")).unwrap_or_default();
    let mut text = format!("{HEADER}\n# {NIGHTLY}; features: {features}{target}\n");
    for item in api.items() {
        let _ = writeln!(text, "{item}");
    }
    Ok(text)
}

/// Writes `text` as `crates/<name>/api.txt`, or with `check` compares the
/// two: `Some(the difference)` when they differ.
fn compare(root: &Path, name: &str, text: &str, check: bool) -> Result<Option<String>> {
    let path = root.join("crates").join(name).join("api.txt");
    if !check {
        fsx::write(&path, text.as_bytes())?;
        return Ok(None);
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(diff(name, &committed, text))
}

/// The lines only one side has, `-` committed and `+` built; `None` when
/// the two are the same.
pub(crate) fn diff(name: &str, committed: &str, built: &str) -> Option<String> {
    if committed == built {
        return None;
    }
    let old: BTreeSet<&str> = committed.lines().collect();
    let new: BTreeSet<&str> = built.lines().collect();
    let mut out = format!("crates/{name}/api.txt:");
    for line in old.difference(&new) {
        let _ = write!(out, "\n  - {line}");
    }
    for line in new.difference(&old) {
        let _ = write!(out, "\n  + {line}");
    }
    if old == new {
        out.push_str("\n  (the same lines in another order)");
    }
    Some(out)
}

/// `mf2-cli`'s listing, through its test: written when `MF2_CLI_API_WRITE`
/// is set, compared otherwise.
fn cli(root: &Path, check: bool) -> Result<()> {
    let args = [
        "test",
        "-q",
        "-p",
        "mf2-cli",
        "--bin",
        "mf2",
        "--",
        "listing::",
    ]
    .map(OsStr::new);
    let write = OsStr::new("1");
    let envs: &[(&str, &OsStr)] = if check {
        &[]
    } else {
        &[("MF2_CLI_API_WRITE", write)]
    };
    run_inherit_env(&cargo(), &args, root, envs)
        .map_err(|_| fail("crates/mf2-cli/api.txt differs from the command tree (above)"))
}

/// The pinned nightly, through rustup (a no-op when present), with the
/// wasm target `mf2-host-web` needs.
pub(crate) fn install(root: &Path) -> Result<()> {
    let args = [
        "toolchain",
        "install",
        NIGHTLY,
        "--profile",
        "minimal",
        "--target",
        WASM,
    ]
    .map(OsStr::new);
    run_capture(OsStr::new("rustup"), &args, root, &[]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::diff;

    #[test]
    fn a_difference_names_its_lines() {
        assert_eq!(diff("x", "a\nb\n", "a\nb\n"), None);
        let d = diff("x", "a\nb\n", "a\nc\n").unwrap();
        assert!(
            d.contains("- b") && d.contains("+ c") && !d.contains("- a"),
            "{d}"
        );
        let d = diff("x", "a\nb\n", "b\na\n").unwrap();
        assert!(d.contains("another order"), "{d}");
    }
}
