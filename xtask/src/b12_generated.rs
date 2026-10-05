//! `cargo xtask b12-generated`: budgets **B1′** and **B13** on the module
//! `mf2-build` generates, and the rule that the other side's features cost
//! this side nothing.
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
//! | F | the same | `hydrate,host-web-number-builtin,host-web-datetime-iso` | **B1′** = F − E must be **+0**: two function crates linked, neither reachable from the generated registry |
//! | A | the fixture's own | `hydrate,host-web-number-builtin` | a corpus that uses `:integer` |
//! | B | the same plus `:currency`, `:unit`, `:percent` | `hydrate,host-web-number-builtin` | **B13** = B − A: what a corpus that does not use them does not pay |
//! | each pair of `OTHER_SIDE` | the fixture's own, or a `:datetime` message | a browser's features, and the same with the server's beside them | **the other side** must be **+0** |
//!
//! An application writes both sides' features on its one `mf2` line, so its
//! browser build sees the server's. A feature that belongs to the server —
//! its number formatter, its date formatter's cache, the load number that
//! cache keys on — must leave the browser's bytes alone, and the cost table
//! cannot say so: it builds the client with the client's features only.
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

/// The other side's features: a browser build, then the same with the
/// server's feature an application writes beside it. Each pair must weigh
/// the same.
const OTHER_SIDE: [(&str, &str, &str); 5] = [
    (
        "a browser on `Intl` numbers beside a server on mf2's own number code",
        "hydrate,host-web-number-intl",
        "hydrate,host-web-number-intl,host-std-number-builtin",
    ),
    (
        "a browser in plain digits beside a server on mf2's own number code",
        "hydrate,host-web-number-plain",
        "hydrate,host-web-number-plain,host-std-number-builtin",
    ),
    (
        "a browser on mf2's own number code beside a server in plain digits",
        "hydrate,host-web-number-builtin",
        "hydrate,host-web-number-builtin,host-std-number-plain",
    ),
    (
        "an `Intl` browser beside an ICU4X server",
        "hydrate,host-web-number-plain,host-web-datetime-intl,corpus-dates",
        "hydrate,host-web-number-plain,host-web-datetime-intl,host-std-datetime-icu,corpus-dates",
    ),
    (
        "an ICU4X browser beside an ICU4X server",
        "hydrate,host-web-number-plain,host-web-datetime-icu,corpus-dates",
        "hydrate,host-web-number-plain,host-web-datetime-icu,host-std-datetime-icu,corpus-dates",
    ),
];

pub(crate) fn run(root: &Path) -> Result<()> {
    let target = root.join("target").join("b12-generated");
    let build = |features: &str| -> Result<u64> {
        eprintln!("b12-generated: building --features {features}");
        size(root, &target, features)
    };

    let e = build("hydrate,corpus-plain")?;
    let f = build("hydrate,host-web-number-builtin,host-web-datetime-iso,corpus-plain")?;
    let a = build("hydrate,host-web-number-builtin")?;
    let b = build("hydrate,host-web-number-builtin,corpus-measures")?;
    let others: Vec<(&str, &str, u64, u64)> = OTHER_SIDE
        .iter()
        .map(|&(what, without, with)| Ok((what, with, build(without)?, build(with)?)))
        .collect::<Result<_>>()?;

    #[allow(clippy::cast_possible_wrap)]
    let b1 = f as i64 - e as i64;
    #[allow(clippy::cast_possible_wrap)]
    let b13 = b as i64 - a as i64;

    println!("\n| build | corpus | features | .wasm |");
    println!("|---|---|---|---:|");
    println!("| E | nothing to serve | hydrate | {e} |");
    println!("| F | the same | hydrate,host-web-number-builtin,host-web-datetime-iso | {f} |");
    println!("| A | the fixture's | hydrate,host-web-number-builtin | {a} |");
    println!("| B | A plus the measures | hydrate,host-web-number-builtin | {b} |");
    for (what, with, without_size, with_size) in &others {
        println!("| the other side | {what} | {with} | {without_size} → {with_size} |");
    }
    println!("\nB1′ = F − E = {b1:+} B (must be +0)");
    println!("B13 = B − A = {b13:+} B avoided (Phase 5a: {B13_EXPECTED:+})");

    let mut failures = Vec::new();
    for (what, with, without_size, with_size) in &others {
        #[allow(clippy::cast_possible_wrap)]
        let delta = *with_size as i64 - *without_size as i64;
        println!("the other side, {what}: {delta:+} B (must be +0)");
        if delta != 0 {
            failures.push(format!(
                "the other side's features cost this one {delta:+} B ({what}; with \
                 `{with}`): a feature of the server changed the browser's build"
            ));
        }
    }
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
        eprintln!("b12-generated: B1′, B13 and the other side's +0 hold");
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
