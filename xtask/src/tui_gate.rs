//! `cargo xtask tui-gate`: what translating a terminal UI costs, measured on
//! the trippy-shaped frame of `examples/tui`
//! (a gate from C8).
//!
//! Builds the example's two benchmark binaries in release, stripped —
//! `tui-mf2` (MF2) and `tui-upstream` (the in-house re-implementation of
//! trippy's own approach) — and reports for each:
//!
//! - **allocations per frame**, and the bytes they ask for, in each of the
//!   example's languages, counted by a global allocator. Every run must
//!   report the same counts; if one differs the command fails, since a count
//!   that moves between runs is no measurement;
//! - **time per frame**: the median over the runs of each run's mean. The
//!   binaries run alternately, so that the clock's drift (up to 30 % within
//!   a day on the development machine) and the load fall on all of them
//!   alike;
//! - the **stripped size** of each binary.
//!
//! `--save-baseline DIR` keeps this build's binaries, and `--baseline DIR`
//! puts a kept build's back into the rotation: a later build is compared
//! with an earlier one by alternating the two, never one run after the
//! other. `--baseline-rev REV` builds the rotation's baseline from a git
//! revision instead, in a worktree under `target/tui-gate` (nightly CI
//! builds A1's 1.x binaries so). `--book` adds the sizes of the user guide's
//! native projects, as `cargo xtask docs` assembles them, stripped.
//!
//! **The gate** (`--gate`):
//! - `tui-mf2`'s allocations per frame, in each language, are at most
//!   1.x's ([`ALLOCS_1X`]) and `tui-upstream`'s in the same build. The counts
//!   are deterministic, so CI holds them on every push;
//! - stripped `tui-mf2` is at most [`SIZE_LIMIT`] bytes, the smallest it has
//!   been measured at;
//! - with a baseline in the rotation, `tui-mf2`'s median time per frame is
//!   at most the baseline `tui-mf2`'s: nightly, alternating with the 1.x
//!   binary, since a time is only comparable within one session.
//!
//! The report goes to standard output and to `target/tui-gate/report.md`,
//! the figures to `target/tui-gate/report.json`.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The example's benchmark binaries, in the order they are reported.
const BINARIES: &[&str] = &["tui-mf2", "tui-upstream"];

/// 1.x's allocations per frame (A1, the binaries of `3a296a9`), by locale.
const ALLOCS_1X: &[(&str, u64)] = &[("en", 1816), ("de", 1815), ("es", 1816), ("fr", 1817)];

/// The stripped `tui-mf2`, in bytes: a later change may not make it larger. It
/// began as 1.x's `1_965_320` B (A1) and came down when the native host stopped
/// linking a time-zone database and `ryu` (Phase 12: `1_454_264`) and again when
/// the normalization tables left the binary, the catalog's map deciding
/// canonical equivalence instead (Phase 13: `1_328_664`, on glibc).
///
/// Measured on musl since 2026-10-06, because that is where the gate runs: the
/// CI image is Alpine. A static musl binary is not the same size as a glibc one
/// — the first run measured `1_381_752`, 53,088 B over the glibc figure — and
/// that is a platform difference, not a regression. The ratchet is kept on the
/// platform that enforces it. `tui-upstream` was `1_450_048` in the same run, so
/// ours is still the smaller binary, which is what the figure is for.
///
/// One cost of this, for the owner to overrule if it matters: on glibc the
/// ratchet is now loose by those 53 KB, so a regression smaller than that would
/// pass locally and fail in CI. Two constants chosen by target would keep both
/// tight.
const SIZE_LIMIT: u64 = 1_381_752;

/// What `cargo xtask tui-gate` was asked to do.
pub(crate) struct Options {
    /// Runs of each binary.
    pub(crate) runs: usize,
    /// Frames each run times in each language.
    pub(crate) frames: usize,
    /// A kept build to alternate with.
    pub(crate) baseline: Option<PathBuf>,
    /// A git revision to build the baseline from.
    pub(crate) baseline_rev: Option<String>,
    /// Where to keep this build's binaries.
    pub(crate) save_baseline: Option<PathBuf>,
    /// Fail when a figure is over its limit.
    pub(crate) gate: bool,
    /// Also measure the user guide's native projects.
    pub(crate) book: bool,
}

/// One binary in the rotation, and what its runs reported.
struct Measured {
    label: String,
    path: PathBuf,
    size: u64,
    locales: Vec<String>,
    /// Allocations and bytes per frame, one per locale: every run's.
    allocs: Option<Vec<u64>>,
    bytes: Option<Vec<u64>>,
    ns: Vec<f64>,
}

pub(crate) fn run(root: &Path, opts: &Options) -> Result<()> {
    if opts.runs == 0 || opts.frames == 0 {
        return Err(gate("--runs and --frames must be at least 1"));
    }
    let out = root.join("target/tui-gate");
    let release = build(
        root,
        &root.join("examples/tui/Cargo.toml"),
        &out.join("target"),
        &[],
    )?;

    let mut rotation = Vec::new();
    for name in BINARIES {
        rotation.push(measured((*name).to_owned(), release.join(name))?);
    }
    if let Some(dir) = &opts.save_baseline {
        save(root, dir, &release)?;
    }
    let built_baseline = match &opts.baseline_rev {
        Some(rev) => Some(build_at(root, rev, &out)?),
        None => None,
    };
    if let Some(dir) = opts.baseline.as_ref().or(built_baseline.as_ref()) {
        for name in BINARIES {
            let path = dir.join(name);
            if !path.exists() {
                return Err(gate(&format!("{} has no {name}", dir.display())));
            }
            rotation.push(measured(format!("{name} (baseline)"), path)?);
        }
    }

    let frames = opts.frames.to_string();
    let load = load_average();
    eprintln!(
        "tui-gate: {} runs of {} binaries, alternating (load {load})",
        opts.runs,
        rotation.len()
    );
    for _ in 0..opts.runs {
        for bin in &mut rotation {
            let stdout = cmd::run_capture(
                bin.path.as_os_str(),
                &[OsStr::new("--frames"), OsStr::new(&frames)],
                root,
                &[],
            )?;
            record(bin, &stdout)?;
        }
    }

    let book = if opts.book {
        book_sizes(root, &out)?
    } else {
        Vec::new()
    };
    let report = report(&rotation, &book, opts, &load);
    print!("{report}");
    fsx::write(&out.join("report.md"), report.as_bytes())?;
    let figures = figures(&rotation, &book, opts, &load);
    fsx::write(
        &out.join("report.json"),
        format!("{figures:#}\n").as_bytes(),
    )?;
    if opts.gate {
        let failures = judge(&rotation);
        if !failures.is_empty() {
            return Err(gate(&failures.join("; ")));
        }
        eprintln!("tui-gate: the gate holds");
    }
    Ok(())
}

/// What the gate refuses in a finished rotation, one sentence each.
fn judge(rotation: &[Measured]) -> Vec<String> {
    let find = |label: &str| rotation.iter().find(|b| b.label == label);
    let mut failures = Vec::new();
    let (Some(mf2), Some(upstream)) = (find("tui-mf2"), find("tui-upstream")) else {
        return vec!["the rotation lacks tui-mf2 or tui-upstream".to_owned()];
    };
    let allocs = |bin: &Measured, locale: &str| {
        let at = bin.locales.iter().position(|l| l == locale)?;
        bin.allocs.as_ref()?.get(at).copied()
    };
    for (locale, limit_1x) in ALLOCS_1X {
        let Some(ours) = allocs(mf2, locale) else {
            failures.push(format!("tui-mf2 reported no allocations for {locale}"));
            continue;
        };
        let theirs = allocs(upstream, locale).unwrap_or(0);
        if ours > *limit_1x || ours > theirs {
            failures.push(format!(
                "{locale}: tui-mf2 allocates {ours} times a frame, above 1.x's {limit_1x} \
                 or the baseline renderer's {theirs}"
            ));
        }
    }
    if mf2.size > SIZE_LIMIT {
        failures.push(format!(
            "stripped tui-mf2 is {} B, above the {SIZE_LIMIT} B it has been",
            mf2.size
        ));
    }
    if let Some(base) = find("tui-mf2 (baseline)") {
        let (ours, theirs) = (median(&mf2.ns), median(&base.ns));
        if ours > theirs {
            failures.push(format!(
                "tui-mf2 takes {:.1} µs a frame (median), above the baseline's {:.1} µs",
                ours / 1000.0,
                theirs / 1000.0
            ));
        }
    }
    failures
}

/// Builds the example's binaries as they were at `rev`, in a worktree under
/// `out`, and returns the directory they are in.
fn build_at(root: &Path, rev: &str, out: &Path) -> Result<PathBuf> {
    let tree = out.join("baseline-src");
    let git = |args: &[&OsStr]| cmd::run_capture(OsStr::new("git"), args, root, &[]);
    if tree.exists() {
        git(&[
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
            tree.as_os_str(),
        ])?;
    }
    git(&[OsStr::new("worktree"), OsStr::new("prune")])?;
    git(&[
        OsStr::new("worktree"),
        OsStr::new("add"),
        OsStr::new("--detach"),
        tree.as_os_str(),
        OsStr::new(rev),
    ])?;
    let built = build(
        root,
        &tree.join("examples/tui/Cargo.toml"),
        &out.join("baseline-target"),
        &[],
    );
    let kept = out.join("baseline-bin");
    let result = built.and_then(|release| save(&tree, &kept, &release));
    git(&[
        OsStr::new("worktree"),
        OsStr::new("remove"),
        OsStr::new("--force"),
        tree.as_os_str(),
    ])?;
    result?;
    Ok(kept)
}

/// Builds a manifest's binaries in release, stripped, into `target`, and
/// returns the directory they are in.
fn build(root: &Path, manifest: &Path, target: &Path, features: &[&str]) -> Result<PathBuf> {
    eprintln!(
        "tui-gate: building {} (release, stripped)",
        manifest.display()
    );
    let mut args = vec![
        OsStr::new("build"),
        OsStr::new("--release"),
        OsStr::new("--bins"),
        OsStr::new("--manifest-path"),
        manifest.as_os_str(),
        OsStr::new("--target-dir"),
        target.as_os_str(),
    ];
    for feature in features {
        args.push(OsStr::new("--features"));
        args.push(OsStr::new(feature));
    }
    cmd::run_inherit_env(
        &cmd::cargo(),
        &args,
        root,
        &[("CARGO_PROFILE_RELEASE_STRIP", OsStr::new("symbols"))],
    )?;
    Ok(target.join("release"))
}

fn measured(label: String, path: PathBuf) -> Result<Measured> {
    let size = file_size(&path)?;
    Ok(Measured {
        label,
        path,
        size,
        locales: Vec::new(),
        allocs: None,
        bytes: None,
        ns: Vec::new(),
    })
}

fn file_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path).map_err(|source| Error::IoAt {
        path: path.to_owned(),
        source,
    })?;
    Ok(metadata.len())
}

/// Reads one run's JSON line into `bin`, refusing a count that differs from
/// an earlier run's.
fn record(bin: &mut Measured, stdout: &[u8]) -> Result<()> {
    let line = String::from_utf8_lossy(stdout);
    let value: Value = serde_json::from_str(line.trim()).map_err(|e| Error::Json {
        path: bin.path.clone(),
        message: e.to_string(),
    })?;
    let numbers = |key: &str| -> Result<Vec<u64>> {
        value[key]
            .as_array()
            .and_then(|a| a.iter().map(Value::as_u64).collect())
            .ok_or_else(|| gate(&format!("{}: no `{key}` in {line}", bin.label)))
    };
    let allocs = numbers("allocs")?;
    let bytes = numbers("bytes")?;
    let ns = value["ns_per_frame"]
        .as_f64()
        .ok_or_else(|| gate(&format!("{}: no `ns_per_frame` in {line}", bin.label)))?;
    if bin.locales.is_empty() {
        bin.locales = value["locales"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
    }
    for (kept, now, what) in [
        (&mut bin.allocs, allocs, "allocations"),
        (&mut bin.bytes, bytes, "bytes"),
    ] {
        match kept {
            Some(earlier) if *earlier != now => {
                return Err(gate(&format!(
                    "{}: {what} per frame moved between runs ({earlier:?}, then {now:?}); \
                     the measurement is not deterministic",
                    bin.label
                )));
            }
            Some(_) => {}
            None => *kept = Some(now),
        }
    }
    bin.ns.push(ns);
    Ok(())
}

/// Keeps this build's binaries in `dir`, with the commit they were built at.
fn save(root: &Path, dir: &Path, release: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|source| Error::IoAt {
        path: dir.to_owned(),
        source,
    })?;
    for name in BINARIES {
        let to = dir.join(name);
        fs::copy(release.join(name), &to).map_err(|source| Error::IoAt { path: to, source })?;
    }
    let head = cmd::run_capture(
        OsStr::new("git"),
        &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        root,
        &[],
    )?;
    let status = cmd::run_capture(
        OsStr::new("git"),
        &[
            OsStr::new("status"),
            OsStr::new("--porcelain"),
            OsStr::new("--"),
            OsStr::new("crates"),
            OsStr::new("examples/tui"),
        ],
        root,
        &[],
    )?;
    let note = format!(
        "Built by `cargo xtask tui-gate --save-baseline` at {}{}\n",
        String::from_utf8_lossy(&head).trim(),
        if status.is_empty() {
            ""
        } else {
            " (crates/ or examples/tui/ had uncommitted changes)"
        }
    );
    fsx::write(&dir.join("BUILT-AT"), note.as_bytes())?;
    eprintln!(
        "tui-gate: kept {} and {} in {}",
        BINARIES[0],
        BINARIES[1],
        dir.display()
    );
    Ok(())
}

/// The user guide's native projects, as `cargo xtask docs` assembled them:
/// each binary's stripped size.
fn book_sizes(root: &Path, out: &Path) -> Result<Vec<(String, u64)>> {
    let projects = root.join("target/docs/projects");
    let target = out.join("book-target");
    let mut sizes = Vec::new();
    for (project, binary) in [("count", "count"), ("hops", "hops"), ("trace", "trace-tui")] {
        let manifest = projects.join(project).join("Cargo.toml");
        if !manifest.exists() {
            return Err(gate(
                "the user guide's native projects are not assembled; run `cargo xtask docs` first",
            ));
        }
        let release = build(root, &manifest, &target, &[])?;
        sizes.push((binary.to_owned(), file_size(&release.join(binary))?));
    }
    Ok(sizes)
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[mid - 1], sorted[mid])
    } else {
        sorted[mid]
    }
}

fn load_average() -> String {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn report(rotation: &[Measured], book: &[(String, u64)], opts: &Options, load: &str) -> String {
    let mut out = String::from("# tui-gate\n\n");
    let locales = rotation
        .first()
        .map(|b| b.locales.join(" / "))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "| Binary | Stripped size (B) | Allocations per frame ({locales}) | Bytes per frame | \
         Median µs per frame | Range µs |\n|---|---:|---:|---:|---:|---:|"
    );
    for bin in rotation {
        let join = |v: &Option<Vec<u64>>| {
            v.as_ref()
                .map(|v| v.iter().map(u64::to_string).collect::<Vec<_>>().join(" / "))
                .unwrap_or_default()
        };
        let lo = bin.ns.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = bin.ns.iter().copied().fold(0.0, f64::max);
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {:.1} | {:.1}–{:.1} |",
            bin.label,
            bin.size,
            join(&bin.allocs),
            join(&bin.bytes),
            median(&bin.ns) / 1000.0,
            lo / 1000.0,
            hi / 1000.0,
        );
    }
    let _ = writeln!(
        out,
        "\n{} runs of each binary, alternating, {} frames per language per run; \
         load average {load} at the start (timings taken under that load; sizes and \
         allocation counts do not depend on it).",
        opts.runs, opts.frames,
    );
    if !book.is_empty() {
        out.push_str("\n| The user guide's native projects | Stripped size (B) |\n|---|---:|\n");
        for (label, size) in book {
            let _ = writeln!(out, "| `{label}` | {size} |");
        }
    }
    out
}

fn figures(rotation: &[Measured], book: &[(String, u64)], opts: &Options, load: &str) -> Value {
    json!({
        "runs": opts.runs,
        "frames_per_locale": opts.frames,
        "load_average": load,
        "binaries": rotation.iter().map(|bin| json!({
            "label": bin.label,
            "path": bin.path.display().to_string(),
            "stripped_size": bin.size,
            "locales": bin.locales,
            "allocs_per_frame": bin.allocs,
            "bytes_per_frame": bin.bytes,
            "median_ns_per_frame": median(&bin.ns),
            "ns_per_frame": bin.ns,
        })).collect::<Vec<_>>(),
        "book": book.iter().map(|(label, size)| json!({
            "label": label,
            "stripped_size": size,
        })).collect::<Vec<_>>(),
    })
}

fn gate(message: &str) -> Error {
    Error::TuiGate(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Measured, SIZE_LIMIT, judge, median, record};
    use std::path::PathBuf;

    fn bin() -> Measured {
        Measured {
            label: "tui-test".to_owned(),
            path: PathBuf::from("tui-test"),
            size: 0,
            locales: Vec::new(),
            allocs: None,
            bytes: None,
            ns: Vec::new(),
        }
    }

    const RUN: &str = r#"{"renderer":"t","locales":["en","fr"],"allocs":[3,4],"bytes":[30,40],"frames":2,"ns_per_frame":12.5}"#;

    #[test]
    fn a_run_is_recorded() {
        let mut b = bin();
        record(&mut b, RUN.as_bytes()).unwrap();
        record(&mut b, RUN.as_bytes()).unwrap();
        assert_eq!(b.locales, ["en", "fr"]);
        assert_eq!(b.allocs, Some(vec![3, 4]));
        assert_eq!(b.ns.len(), 2);
        assert!(b.ns.iter().all(|ns| (ns - 12.5).abs() < f64::EPSILON));
    }

    // The negative control: a count that moves between runs is refused.
    #[test]
    fn a_count_that_moves_is_refused() {
        let mut b = bin();
        record(&mut b, RUN.as_bytes()).unwrap();
        let moved = RUN.replace("[3,4]", "[3,5]");
        let err = record(&mut b, moved.as_bytes()).unwrap_err().to_string();
        assert!(err.contains("not deterministic"), "{err}");
    }

    fn measured(label: &str, allocs: [u64; 4], size: u64, ns: f64) -> Measured {
        Measured {
            label: label.to_owned(),
            path: PathBuf::from(label),
            size,
            locales: ["en", "de", "es", "fr"].map(str::to_owned).to_vec(),
            allocs: Some(allocs.to_vec()),
            bytes: None,
            ns: vec![ns],
        }
    }

    #[test]
    fn the_gate_holds_below_every_limit() {
        let rotation = [
            measured("tui-mf2", [1329; 4], SIZE_LIMIT, 280.0),
            measured("tui-upstream", [1517; 4], 1_400_000, 260.0),
            measured("tui-mf2 (baseline)", [1816; 4], 1_965_320, 300.0),
        ];
        assert!(judge(&rotation).is_empty());
    }

    // The negative controls: each limit, crossed alone, is refused.
    #[test]
    fn the_gate_refuses_each_limit_crossed() {
        let upstream = measured("tui-upstream", [1517; 4], 1_400_000, 260.0);
        let over_upstream = [
            measured("tui-mf2", [1329, 1518, 1329, 1329], SIZE_LIMIT, 280.0),
            measured("tui-upstream", [1517; 4], 1_400_000, 260.0),
        ];
        let failures = judge(&over_upstream);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].starts_with("de:"), "{failures:?}");
        let too_big = [
            measured("tui-mf2", [1329; 4], SIZE_LIMIT + 1, 280.0),
            upstream,
        ];
        assert!(judge(&too_big)[0].contains("stripped"));
        let slower = [
            measured("tui-mf2", [1329; 4], SIZE_LIMIT, 301.0),
            measured("tui-upstream", [1517; 4], 1_400_000, 260.0),
            measured("tui-mf2 (baseline)", [1816; 4], 1_965_320, 300.0),
        ];
        assert!(judge(&slower)[0].contains("median"));
    }

    #[test]
    fn the_median_of_an_even_count_is_the_midpoint() {
        assert!((median(&[4.0, 1.0, 3.0, 2.0]) - 2.5).abs() < f64::EPSILON);
        assert!((median(&[5.0, 1.0, 3.0]) - 3.0).abs() < f64::EPSILON);
    }
}
