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
//!
//! A pair that is not +0 does not stop at the number. Every build's artifact
//! is kept, so the failing pair is printed section by section — 113 B of code
//! and 113 B of data segments are different faults — and when it is the code
//! section that moved, the pair is built again with the name section kept and
//! `twiggy diff` names the symbols. A leak across the side boundary is then a
//! list to fix rather than a byte count to guess at. The diagnosis never
//! decides the gate: if it cannot be made, it says so and the budget still
//! fails.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The fixture's client binary.
const BIN: &str = "mf2-i18n-client";

/// What Phase 5a measured for B13, and how far this may drift before it is a
/// regression rather than a corpus difference.
const B13_EXPECTED: i64 = 13_599;
const B13_TOLERANCE: f64 = 0.10;

/// How many items `twiggy diff` lists for a failing pair.
const DIFF_ITEMS: &str = "40";

/// The other side's features: a browser build, then the same with the
/// server's feature an application writes beside it. Each pair must weigh
/// the same.
const OTHER_SIDE: [(&str, &str, &str); 6] = [
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
    (
        "a cached ICU4X browser beside an ICU4X server",
        "hydrate,host-web-number-plain,host-web-datetime-icu-cached,corpus-dates",
        "hydrate,host-web-number-plain,host-web-datetime-icu-cached,host-std-datetime-icu,corpus-dates",
    ),
];

/// One build of the fixture's client: its size, and where its artifact was
/// kept. Every build writes the same path, so the copy is what lets a pair be
/// compared after both have been built.
struct Built {
    size: u64,
    kept: PathBuf,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let target = root.join("target").join("b12-generated");
    let build = |features: &str, label: &str| -> Result<Built> {
        eprintln!("b12-generated: building --features {features}");
        build_one(root, &target, features, label)
    };

    let e = build("hydrate,corpus-plain", "e")?.size;
    let f = build(
        "hydrate,host-web-number-builtin,host-web-datetime-iso,corpus-plain",
        "f",
    )?
    .size;
    let a = build("hydrate,host-web-number-builtin", "a")?.size;
    let b = build("hydrate,host-web-number-builtin,corpus-measures", "b")?.size;
    let others: Vec<(&str, &str, &str, Built, Built)> = OTHER_SIDE
        .iter()
        .enumerate()
        .map(|(i, &(what, without, with))| {
            Ok((
                what,
                without,
                with,
                build(without, &format!("other-{i}-without"))?,
                build(with, &format!("other-{i}-with"))?,
            ))
        })
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
    for (what, _, with, lhs, rhs) in &others {
        println!(
            "| the other side | {what} | {with} | {} → {} |",
            lhs.size, rhs.size
        );
    }
    println!("\nB1′ = F − E = {b1:+} B (must be +0)");
    println!("B13 = B − A = {b13:+} B avoided (Phase 5a: {B13_EXPECTED:+})");

    let mut failures = Vec::new();
    let mut leaking = Vec::new();
    for (what, without, with, lhs, rhs) in &others {
        #[allow(clippy::cast_possible_wrap)]
        let delta = rhs.size as i64 - lhs.size as i64;
        println!("the other side, {what}: {delta:+} B (must be +0)");
        if delta != 0 {
            failures.push(format!(
                "the other side's features cost this one {delta:+} B ({what}; with \
                 `{with}`): a feature of the server changed the browser's build"
            ));
            leaking.push((*what, *without, *with, lhs, rhs));
        }
    }
    // After every verdict is printed, so that the diagnosis cannot be
    // mistaken for one, and so a long `twiggy diff` does not sit between the
    // rows.
    for (what, without, with, lhs, rhs) in leaking {
        diagnose(root, what, without, with, lhs, rhs);
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

/// What moved in a pair that is not +0: the sections side by side, and, when
/// the code section is one of them, the symbols `twiggy` names.
///
/// A diagnostic, so every failure here is printed and swallowed: the budget
/// has already failed, and a missing tool must not change which error the
/// gate reports.
fn diagnose(root: &Path, what: &str, without: &str, with: &str, lhs: &Built, rhs: &Built) {
    println!("\n#### what the other side's features added: {what}\n");
    let rows = match section_delta(&lhs.kept, &rhs.kept) {
        Ok(rows) => rows,
        Err(err) => {
            println!("the sections could not be read: {err}");
            return;
        }
    };
    println!("| section | browser alone | with the server's features | delta |");
    println!("|---|---:|---:|---:|");
    let mut code_moved = false;
    for (name, alone, beside) in &rows {
        #[allow(clippy::cast_possible_wrap)]
        let delta = *beside as i64 - *alone as i64;
        if delta != 0 && name == "code" {
            code_moved = true;
        }
        println!("| `{name}` | {alone} | {beside} | {delta:+} |");
    }
    if !code_moved {
        println!(
            "\nThe code section is the same in both, so nothing crossed as code: \
             read the section that moved."
        );
        return;
    }
    println!("\nThe code section moved. Building the pair again with the name section kept.");
    match twiggy_diff(root, without, with) {
        Ok(text) => println!("\n```\n{}\n```", text.trim_end()),
        Err(err) => println!("the symbol diff could not be made: {err}"),
    }
}

/// Every section of both modules, by name, in the order the first carries
/// them. A module may hold more than one custom section of a name; the first
/// of each is what this compares, which is enough to say where bytes went.
fn section_delta(alone: &Path, beside: &Path) -> Result<Vec<(String, u64, u64)>> {
    let mut rows: Vec<(String, u64, u64)> = sections(alone)?
        .into_iter()
        .map(|(name, size)| (name, size, 0))
        .collect();
    for (name, size) in sections(beside)? {
        if let Some(row) = rows.iter_mut().find(|row| row.0 == name) {
            row.2 = size;
        } else {
            rows.push((name, 0, size));
        }
    }
    Ok(rows)
}

/// A wasm module's sections and the size of each payload: an eight-byte
/// header, then `(id, size, payload)` for each. A custom section is named by
/// the name its payload begins with.
fn sections(path: &Path) -> Result<Vec<(String, u64)>> {
    let bytes = read(path)?;
    let malformed = |detail: &str| Error::CommandFailed {
        command: format!("reading {}", path.display()),
        status: "malformed wasm".to_owned(),
        stderr: format!(": {detail}"),
    };
    if bytes.get(..4) != Some(&b"\0asm"[..]) {
        return Err(malformed("not a wasm module"));
    }
    let mut out = Vec::new();
    let mut at = 8usize;
    while at < bytes.len() {
        let id = *bytes.get(at).ok_or_else(|| malformed("a section id"))?;
        at += 1;
        let rest = bytes.get(at..).ok_or_else(|| malformed("a section size"))?;
        let (size, used) = uleb(rest).ok_or_else(|| malformed("a section size"))?;
        at += used;
        let end = at
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed("a section past the end"))?;
        let name = if id == 0 {
            custom_name(bytes.get(at..end).unwrap_or_default())
        } else {
            section_name(id).map_or_else(|| format!("section {id}"), str::to_owned)
        };
        out.push((name, u64::try_from(size).unwrap_or(u64::MAX)));
        at = end;
    }
    Ok(out)
}

/// A custom section's name, which its payload begins with.
fn custom_name(payload: &[u8]) -> String {
    match uleb(payload) {
        Some((len, used)) => match used.checked_add(len).and_then(|end| payload.get(used..end)) {
            Some(text) => format!("custom {}", String::from_utf8_lossy(text)),
            None => "custom (a name past the end)".to_owned(),
        },
        None => "custom (unnamed)".to_owned(),
    }
}

/// The name the specification gives a section id, for the ids that have one.
fn section_name(id: u8) -> Option<&'static str> {
    Some(match id {
        1 => "type",
        2 => "import",
        3 => "function",
        4 => "table",
        5 => "memory",
        6 => "global",
        7 => "export",
        8 => "start",
        9 => "element",
        10 => "code",
        11 => "data",
        12 => "data count",
        13 => "tag",
        _ => return None,
    })
}

/// A LEB128 unsigned integer: its value, and how many bytes it took.
fn uleb(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    let mut shift = 0u32;
    for (i, byte) in bytes.iter().enumerate().take(10) {
        value |= usize::from(byte & 0x7f).checked_shl(shift)?;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

/// The pair built again with the name section kept, through `twiggy diff`.
/// twiggy is in the image at the version `tools/ci/setup.sh` pins, for
/// `bench/b12/check.sh` and `tools/fmt-check.sh`.
fn twiggy_diff(root: &Path, without: &str, with: &str) -> Result<String> {
    let target = root.join("target").join("b12-generated-names");
    let alone = build_named(root, &target, without, "without")?;
    let beside = build_named(root, &target, with, "with")?;
    let out = cmd::run_capture(
        OsStr::new("twiggy"),
        &[
            OsStr::new("diff"),
            OsStr::new("-n"),
            OsStr::new(DIFF_ITEMS),
            alone.as_os_str(),
            beside.as_os_str(),
        ],
        root,
        &[],
    )?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Builds the fixture's client binary with `features`, keeps the artifact as
/// `label`, and returns what it measured.
fn build_one(root: &Path, target: &Path, features: &str, label: &str) -> Result<Built> {
    let wasm = compile(root, target, features, &[])?;
    let bytes = read(&wasm)?;
    let kept = target.join("kept").join(format!("{label}.wasm"));
    fsx::write(&kept, &bytes)?;
    Ok(Built {
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        kept,
    })
}

/// The same build with the name section kept, which the `wasm-release`
/// profile strips. Its own target directory, so the stripped artifacts the
/// figures came from are not rebuilt over.
fn build_named(root: &Path, target: &Path, features: &str, label: &str) -> Result<PathBuf> {
    eprintln!("b12-generated: building --features {features} with the names kept");
    let wasm = compile(
        root,
        target,
        features,
        &[("CARGO_PROFILE_WASM_RELEASE_STRIP", OsStr::new("none"))],
    )?;
    let kept = target.join("kept").join(format!("{label}.wasm"));
    copy(&wasm, &kept)?;
    Ok(kept)
}

/// Builds the fixture's client binary with `features` and returns the path it
/// wrote. Every feature set writes the same path.
fn compile(
    root: &Path,
    target: &Path,
    features: &str,
    extra: &[(&str, &OsStr)],
) -> Result<PathBuf> {
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
    let mut envs: Vec<(&str, &OsStr)> = vec![
        ("CARGO_TARGET_DIR", target.as_os_str()),
        ("CARGO_BUILD_JOBS", jobs.as_os_str()),
    ];
    envs.extend_from_slice(extra);
    cmd::run_inherit_env(&cargo, &args, root, &envs)?;
    Ok(target
        .join("wasm32-unknown-unknown")
        .join("wasm-release")
        .join(format!("{BIN}.wasm")))
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    let bytes = read(from)?;
    fsx::write(to, &bytes)
}
