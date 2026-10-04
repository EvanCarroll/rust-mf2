//! `cargo xtask b12-generated`: budgets **B1′** and **B13** on the module
//! `mf2-build` generates.
//!
//! Phase 5a measured both by editing the fixture's corpus by hand and putting
//! it back: B1′ =
//! +0 B, B13 = 13,599 B avoided. This turns them into a gate — the corpora
//! are two cargo features of the fixture, so a regression fails a command
//! instead of waiting to be re-measured.
//!
//! | Build | Corpus | Features | What it shows |
//! |---|---|---|---|
//! | E | nothing a function crate could serve | `hydrate` | the floor |
//! | F | the same | `hydrate,fn-number,host-web-datetime-iso` | **B1′** = F − E must be **+0**: two function crates linked, neither reachable from the generated registry |
//! | A | the fixture's own | `hydrate,fn-number` | a corpus that uses `:integer` |
//! | B | the same plus `:currency`, `:unit`, `:percent` | `hydrate,fn-number` | **B13** = B − A: what a corpus that does not use them does not pay |
//!
//! The size method: `wasm32-unknown-unknown`, profile
//! `wasm-release`, raw bytes of the client binary — the same figures Phase 5a
//! reported, so the two are comparable.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::cmd;
use crate::error::{Error, Result};

/// The fixture's client binary.
const BIN: &str = "mf2-i18n-client";

/// What Phase 5a measured for B13, and how far this may drift before it is a
/// regression rather than a corpus difference.
const B13_EXPECTED: i64 = 13_599;
const B13_TOLERANCE: f64 = 0.10;

pub(crate) fn run(root: &Path) -> Result<()> {
    let target = root.join("target").join("b12-generated");
    let build = |features: &str| -> Result<u64> {
        eprintln!("b12-generated: building --features {features}");
        size(root, &target, features)
    };

    let e = build("hydrate,corpus-plain")?;
    let f = build("hydrate,fn-number,host-web-datetime-iso,corpus-plain")?;
    let a = build("hydrate,fn-number")?;
    let b = build("hydrate,fn-number,corpus-measures")?;

    #[allow(clippy::cast_possible_wrap)]
    let b1 = f as i64 - e as i64;
    #[allow(clippy::cast_possible_wrap)]
    let b13 = b as i64 - a as i64;

    println!("\n| build | corpus | features | .wasm |");
    println!("|---|---|---|---:|");
    println!("| E | nothing to serve | hydrate | {e} |");
    println!("| F | the same | hydrate,fn-number,host-web-datetime-iso | {f} |");
    println!("| A | the fixture's | hydrate,fn-number | {a} |");
    println!("| B | A plus the measures | hydrate,fn-number | {b} |");
    println!("\nB1′ = F − E = {b1:+} B (must be +0)");
    println!("B13 = B − A = {b13:+} B avoided (Phase 5a: {B13_EXPECTED:+})");

    let mut failures = Vec::new();
    if b1 != 0 {
        failures.push(format!(
            "B1′ is {b1:+} B: a function crate that the generated registry never names \
             reached the wasm"
        ));
    }
    #[allow(clippy::cast_precision_loss)]
    let drift = (b13 - B13_EXPECTED).abs() as f64 / B13_EXPECTED as f64;
    if drift > B13_TOLERANCE {
        failures.push(format!(
            "B13 is {b13:+} B, {:.0} % from Phase 5a's {B13_EXPECTED:+}: the measure \
             functions' share of the wasm moved",
            drift * 100.0
        ));
    }
    if failures.is_empty() {
        eprintln!("b12-generated: B1′ and B13 hold");
        return Ok(());
    }
    Err(Error::CommandFailed {
        command: "b12-generated".to_owned(),
        status: format!("{} budget(s) failed", failures.len()),
        stderr: failures.join("\n"),
    })
}

/// Builds the fixture's client binary with `features` and returns its size.
fn size(root: &Path, target: &Path, features: &str) -> Result<u64> {
    let cargo = cmd::cargo();
    let args: Vec<&OsStr> = vec![
        OsStr::new("build"),
        OsStr::new("--quiet"),
        OsStr::new("-p"),
        OsStr::new("mf2-i18n-fixture"),
        OsStr::new("--bin"),
        OsStr::new(BIN),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(features),
        OsStr::new("--target"),
        OsStr::new("wasm32-unknown-unknown"),
        OsStr::new("--profile"),
        OsStr::new("wasm-release"),
    ];
    let jobs = std::env::var_os("CARGO_BUILD_JOBS").unwrap_or_else(|| OsString::from("3"));
    cmd::run_inherit_env(
        &cargo,
        &args,
        root,
        &[
            ("CARGO_TARGET_DIR", target.as_os_str()),
            ("CARGO_BUILD_JOBS", jobs.as_os_str()),
        ],
    )?;
    let wasm = target
        .join("wasm32-unknown-unknown")
        .join("wasm-release")
        .join(format!("{BIN}.wasm"));
    std::fs::metadata(&wasm)
        .map(|m| m.len())
        .map_err(|source| Error::IoAt { path: wasm, source })
}
