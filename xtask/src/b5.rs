//! `cargo xtask b5`: budget **B5**, the marginal wasm per call site
//! (`plans/06-size-and-perf.md` §3; `plans/13-phase-5b-work-order.md` A6).
//!
//! P0.1's method, on the real crates. Two generated applications of the same
//! shape at two scales are built for the client, and the per-site cost is the
//! *difference of the differences*, so that everything fixed — the runtime,
//! the call-site library, the generic instantiations — cancels:
//!
//! ```text
//! marginal = (Δ@big − Δ@small) / (sites@big − sites@small),  Δ = tr − baseline
//! ```
//!
//! Three templates: `tr` (mf2-two), `idlit` (the baseline: a `String` from a
//! short per-site literal, in the same positions) and `dummy` (the harshest
//! bound: the same literal everywhere, which lets the optimiser merge sites a
//! real application keeps apart).
//!
//! The pipeline is `plans/06` §3's, exactly as P0.1 ran it:
//! `wasm32-unknown-unknown`, profile `wasm-release`, `wasm-bindgen`,
//! `wasm-opt -Oz`, `gzip -9`.
//!
//! **What Phase 5b measures.** Without `leptos-mf2` a description does not
//! render itself yet, so every call site of every template formats to a
//! `String`. That is the 45 % of real sites that need one anyway, and the
//! view positions cost the same in `tr` and in its baselines — tachys around
//! a `String` leaf — so they cancel here. Phase 6 re-measures them with the
//! real leaf, which is what P0.1's 24.5 B gz was.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::cmd;
use crate::error::{Error, Result};

/// The budget: B5 ≤ 40 B gz per call site, against the `idlit` baseline.
const BUDGET: f64 = 40.0;

/// One scale of the reference workload: the knobs of `plans/06` §2, at the
/// ratios P0.1 used.
struct Scale {
    sites: usize,
    messages: usize,
    components: usize,
}

const SCALES: [Scale; 2] = [
    Scale {
        sites: 1_860,
        messages: 1_600,
        components: 60,
    },
    Scale {
        sites: 3_720,
        messages: 3_200,
        components: 120,
    },
];

/// The templates, in the order the table shows them.
const TEMPLATES: [&str; 3] = ["tr", "idlit", "dummy"];

/// What one built application measured.
pub(crate) struct Sizes {
    /// After `wasm-bindgen`, before `wasm-opt`.
    pub(crate) bindgen_gz: u64,
    /// After `wasm-opt -Oz` — the shipped artifact.
    pub(crate) opt_raw: u64,
    pub(crate) opt_gz: u64,
}

pub(crate) fn run(root: &Path, out: Option<PathBuf>, keep: bool) -> Result<()> {
    let measured = measure(root, out, keep)?;
    report(&measured)
}

/// Builds the six applications and measures them. Shared with
/// `cargo xtask size`, which applies the whole-app gates to the same numbers
/// rather than building them again.
pub(crate) fn measure(
    root: &Path,
    out: Option<PathBuf>,
    keep: bool,
) -> Result<Vec<(usize, Vec<(&'static str, Sizes)>)>> {
    let out = out.unwrap_or_else(|| root.join("target").join("b5"));
    if !keep && out.exists() {
        std::fs::remove_dir_all(&out).map_err(|source| Error::IoAt {
            path: out.clone(),
            source,
        })?;
    }
    std::fs::create_dir_all(&out).map_err(|source| Error::IoAt {
        path: out.clone(),
        source,
    })?;

    let mut measured: Vec<(usize, Vec<(&str, Sizes)>)> = Vec::new();
    for scale in &SCALES {
        let workload = out.join(format!("wl-{}", scale.sites));
        generate(root, &workload, scale)?;
        let sites = site_count(&workload)?;
        let mut sizes = Vec::new();
        for template in TEMPLATES {
            eprintln!("b5: building app-{template} at {sites} sites");
            sizes.push((template, build(root, &workload, template)?));
        }
        measured.push((sites, sizes));
    }
    Ok(measured)
}

/// Generates one workload with all three applications.
fn generate(root: &Path, workload: &Path, scale: &Scale) -> Result<()> {
    if workload.join(".workload-gen").is_file() {
        eprintln!("b5: reusing {}", workload.display());
        return Ok(());
    }
    eprintln!(
        "b5: generating {} sites, {} messages",
        scale.sites, scale.messages
    );
    let cargo = cmd::cargo();
    let tr = root.join("bench/workload-gen/templates/tr");
    let sites = scale.sites.to_string();
    let messages = scale.messages.to_string();
    let components = scale.components.to_string();
    let args: Vec<&OsStr> = vec![
        OsStr::new("run"),
        OsStr::new("--release"),
        OsStr::new("--quiet"),
        OsStr::new("-p"),
        OsStr::new("workload-gen"),
        OsStr::new("--"),
        OsStr::new("all"),
        OsStr::new("-t"),
        tr.as_os_str(),
        OsStr::new("-t"),
        OsStr::new("idlit"),
        OsStr::new("-t"),
        OsStr::new("dummy"),
        OsStr::new("-m"),
        OsStr::new(&sites),
        OsStr::new("-n"),
        OsStr::new(&messages),
        OsStr::new("-k"),
        OsStr::new(&components),
        OsStr::new("--out"),
        workload.as_os_str(),
    ];
    cmd::run_inherit(&cargo, &args, root)
}

/// How many call sites the workload actually has, from `sites.json`.
fn site_count(workload: &Path) -> Result<usize> {
    let path = workload.join("sites.json");
    let text = std::fs::read_to_string(&path).map_err(|source| Error::IoAt {
        path: path.clone(),
        source,
    })?;
    // One object per line, as the generator writes it.
    Ok(text.lines().filter(|l| l.contains("\"site\"")).count())
}

/// Builds one application's client wasm, exactly as `plans/06` §3 says, and
/// measures it.
fn build(root: &Path, workload: &Path, template: &str) -> Result<Sizes> {
    let app = workload.join(format!("app-{template}"));
    // Apps of different workloads share package names, so each workload gets
    // its own target directory (the generator's README says why).
    let target = workload.join("target-apps");
    let lib = format!("workload_app_{}", template.replace('-', "_"));

    let cargo = cmd::cargo();
    let args: Vec<&OsStr> = vec![
        OsStr::new("build"),
        OsStr::new("--quiet"),
        OsStr::new("--lib"),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new("hydrate"),
        OsStr::new("--target"),
        OsStr::new("wasm32-unknown-unknown"),
        OsStr::new("--profile"),
        OsStr::new("wasm-release"),
    ];
    let jobs = std::env::var_os("CARGO_BUILD_JOBS").unwrap_or_else(|| OsString::from("3"));
    cmd::run_inherit_env(
        &cargo,
        &args,
        &app,
        &[
            ("CARGO_TARGET_DIR", target.as_os_str()),
            ("MF2_WORKLOAD_LOCALES", workload.as_os_str()),
            ("CARGO_BUILD_JOBS", jobs.as_os_str()),
        ],
    )?;

    let wasm = target
        .join("wasm32-unknown-unknown")
        .join("wasm-release")
        .join(format!("{lib}.wasm"));
    let pkg = workload.join(format!("pkg-{template}"));
    let _ = std::fs::remove_dir_all(&pkg);
    std::fs::create_dir_all(&pkg).map_err(|source| Error::IoAt {
        path: pkg.clone(),
        source,
    })?;
    cmd::run_inherit(
        OsStr::new("wasm-bindgen"),
        &[
            OsStr::new("--target"),
            OsStr::new("web"),
            OsStr::new("--no-typescript"),
            OsStr::new("--out-dir"),
            pkg.as_os_str(),
            wasm.as_os_str(),
        ],
        root,
    )?;

    let bg = pkg.join(format!("{lib}_bg.wasm"));
    let opt = pkg.join("opt.wasm");
    // Rust 1.98's default target features for wasm32-unknown-unknown.
    cmd::run_inherit(
        OsStr::new("wasm-opt"),
        &[
            OsStr::new("-Oz"),
            OsStr::new("--enable-bulk-memory"),
            OsStr::new("--enable-nontrapping-float-to-int"),
            OsStr::new("--enable-sign-ext"),
            OsStr::new("--enable-mutable-globals"),
            OsStr::new("--enable-reference-types"),
            OsStr::new("--enable-multivalue"),
            bg.as_os_str(),
            OsStr::new("-o"),
            opt.as_os_str(),
        ],
        root,
    )?;

    Ok(Sizes {
        bindgen_gz: gzipped(&bg)?,
        opt_raw: len(&opt)?,
        opt_gz: gzipped(&opt)?,
    })
}

fn len(path: &Path) -> Result<u64> {
    std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|source| Error::IoAt {
            path: path.to_path_buf(),
            source,
        })
}

/// The file through `gzip -9`, in bytes — what `plans/06` §3 measures.
fn gzipped(path: &Path) -> Result<u64> {
    let bytes = std::fs::read(path).map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(9));
    std::io::Write::write_all(&mut encoder, &bytes).map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    let out = encoder.finish().map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(out.len() as u64)
}

/// What the difference of the differences says, for one baseline.
pub(crate) struct Delta {
    /// Bytes gz per call site.
    pub(crate) marginal: f64,
    /// Bytes gz that do not depend on the number of sites — B1's territory.
    pub(crate) fixed: f64,
}

/// The marginal and fixed cost against `baseline`, from two scales.
pub(crate) fn delta(measured: &[(usize, Vec<(&str, Sizes)>)], baseline: &str) -> Option<Delta> {
    let ((small, small_sizes), (big, big_sizes)) = measured
        .first()
        .zip(measured.get(1))
        .map(|((a, b), (c, d))| ((*a, b), (*c, d)))?;
    let span = big.checked_sub(small).filter(|s| *s > 0)?;
    let find = |sizes: &[(&str, Sizes)], name: &str| -> Option<u64> {
        sizes
            .iter()
            .find(|(t, _)| *t == name)
            .map(|(_, s)| s.opt_gz)
    };
    let (tr_small, tr_big, b_small, b_big) = (
        find(small_sizes, "tr")?,
        find(big_sizes, "tr")?,
        find(small_sizes, baseline)?,
        find(big_sizes, baseline)?,
    );
    // Every size here is a few megabytes; `f64` carries them exactly.
    #[allow(clippy::cast_precision_loss)]
    let (delta_small, delta_big) = (
        tr_small as f64 - b_small as f64,
        tr_big as f64 - b_big as f64,
    );
    #[allow(clippy::cast_precision_loss)]
    let marginal = (delta_big - delta_small) / span as f64;
    #[allow(clippy::cast_precision_loss)]
    let fixed = delta_small - small as f64 * marginal;
    Some(Delta { marginal, fixed })
}

/// The per-scale table, shared by both commands.
pub(crate) fn size_table(measured: &[(usize, Vec<(&str, Sizes)>)]) -> String {
    let mut out = String::from(
        "\n| workload | template | bindgen gz | opt raw | opt gz |\n|---|---|---:|---:|---:|\n",
    );
    for (sites, sizes) in measured {
        for (template, s) in sizes {
            out.push_str(&format!(
                "| {sites} sites | {template} | {} | {} | {} |\n",
                s.bindgen_gz, s.opt_raw, s.opt_gz
            ));
        }
    }
    out
}

/// The table, and the gate.
fn report(measured: &[(usize, Vec<(&str, Sizes)>)]) -> Result<()> {
    let Some(((small, small_sizes), (big, big_sizes))) = measured
        .first()
        .zip(measured.get(1))
        .map(|((a, b), (c, d))| ((*a, b), (*c, d)))
    else {
        return Err(Error::CommandFailed {
            command: "b5".to_owned(),
            status: "two scales are needed".to_owned(),
            stderr: String::new(),
        });
    };
    let find = |sizes: &'_ [(&str, Sizes)], name: &str| -> Option<u64> {
        sizes
            .iter()
            .find(|(t, _)| *t == name)
            .map(|(_, s)| s.opt_gz)
    };

    println!("\n| workload | template | bindgen gz | opt raw | opt gz |");
    println!("|---|---|---:|---:|---:|");
    for (sites, sizes) in measured {
        for (template, s) in sizes {
            println!(
                "| {sites} sites | {template} | {} | {} | {} |",
                s.bindgen_gz, s.opt_raw, s.opt_gz
            );
        }
    }

    let span = big.saturating_sub(small);
    if span == 0 {
        return Err(Error::CommandFailed {
            command: "b5".to_owned(),
            status: "the two scales have the same number of sites".to_owned(),
            stderr: String::new(),
        });
    }
    println!("\n| baseline | marginal B gz/site | fixed B gz | budget |");
    println!("|---|---:|---:|---|");
    let mut verdict = None;
    for baseline in ["idlit", "dummy"] {
        let (Some(tr_small), Some(tr_big), Some(b_small), Some(b_big)) = (
            find(small_sizes, "tr"),
            find(big_sizes, "tr"),
            find(small_sizes, baseline),
            find(big_sizes, baseline),
        ) else {
            continue;
        };
        // Every size here is a few megabytes; `f64` carries them exactly.
        #[allow(clippy::cast_precision_loss)]
        let (delta_small, delta_big) = (
            tr_small as f64 - b_small as f64,
            tr_big as f64 - b_big as f64,
        );
        #[allow(clippy::cast_precision_loss)]
        let marginal = (delta_big - delta_small) / span as f64;
        #[allow(clippy::cast_precision_loss)]
        let fixed = delta_small - small as f64 * marginal;
        let gate = if baseline == "idlit" {
            verdict = Some(marginal);
            format!("≤ {BUDGET:.0}")
        } else {
            "(bound)".to_owned()
        };
        println!("| {baseline} | {marginal:.1} | {fixed:.0} | {gate} |");
    }

    match verdict {
        Some(marginal) if marginal <= BUDGET => {
            println!("\nb5: {marginal:.1} B gz per call site against `idlit` — within {BUDGET:.0}");
            Ok(())
        }
        Some(marginal) => Err(Error::CommandFailed {
            command: "b5".to_owned(),
            status: format!(
                "B5 is {marginal:.1} B gz per call site, over the budget of {BUDGET:.0}"
            ),
            stderr: String::new(),
        }),
        None => Err(Error::CommandFailed {
            command: "b5".to_owned(),
            status: "the `tr` or `idlit` app did not build".to_owned(),
            stderr: String::new(),
        }),
    }
}
