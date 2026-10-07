//! `cargo xtask size`: the **whole-app** size gate.
//!
//! B5 gates one number — what a call site costs at the margin. This gates the
//! thing the ambition is actually about: what an application pays for i18n in
//! total, on the reference workload, end to end.
//!
//! It is the same measurement `cargo xtask browser-app-size` makes — two scales, three
//! templates, `wasm32-unknown-unknown` / `wasm-release` / `wasm-bindgen` /
//! `wasm-opt -Oz` / `brotli -q 11` — read three ways:
//!
//! | Gate | What it is | Limit |
//! |---|---|---|
//! | **B1** | the part that does not grow with the call sites: the reader, the evaluator, the call-site library, the Leptos glue, the boot | ≤ 28,046 B br |
//! | **B5** | the part that does, per site, weighted by the workload's mix | ≤ 36 B br |
//! | **whole app** | B1 + sites × B5 at the reference scale | ≤ the ambition, `28,046 B + sites × 36 B` |
//!
//! Against the `idlit` baseline throughout, with `dummy` reported as the
//! conservative bound (06 §3 says why that baseline and not the other).
//!
//! It is a **gate**, so it fails the build rather than printing a number: a
//! regression in any of the three is a regression in the thing the project
//! claims.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::browser_app_size;
use crate::error::{Error, Result};
use crate::fsx;

/// B1: everything fixed, in bytes brotli. The measured figure, from the
/// first run to take it on brotli (`rebaseline.yml` run index 18 at
/// `d930f4d`: musl, brotli 1.2.0, `--lgwin=22`).
///
/// It was `27.0 * 1024.0`, carried over from gzip. B1 is not measured
/// directly: it is `Δ@1860` less `sites × marginal`, so it moves when either
/// term does. Brotli took 8,510 B off the total and 10,681 B off the per-site
/// part, which leaves 2,171 B more over — every build got smaller and B1 rose.
/// A ratchet with no headroom, as `tui_allocs_vs_trippy`'s `SIZE_LIMIT` is;
/// `tools/checks/compare.sh` flags a move beyond ±57 B.
const B1: f64 = 28_046.0;
/// B5: per call site, in bytes brotli.
const B5: f64 = 36.0;

pub(crate) fn run(root: &Path, out: Option<PathBuf>, keep: bool) -> Result<()> {
    let dir = out.unwrap_or_else(|| root.join("target").join("size"));
    let measured = browser_app_size::measure(
        root,
        Some(dir.clone()),
        keep,
        browser_app_size::Mode::String,
    )?;
    let sites = measured
        .first()
        .map(|(sites, _)| *sites)
        .ok_or_else(|| gate("no workload was measured"))?;

    let idlit = browser_app_size::delta(&measured, "tr", "idlit")
        .ok_or_else(|| gate("the `tr` or `idlit` app did not build"))?;
    let dummy = browser_app_size::delta(&measured, "tr", "dummy");

    #[allow(clippy::cast_precision_loss)]
    let scale = sites as f64;
    let whole = idlit.fixed + scale * idlit.marginal;
    let ambition = B1 + scale * B5;

    let mut report = String::from("# Size gate\n");
    report.push_str(&browser_app_size::size_table(&measured));
    let _ = write!(
        report,
        "\n| Gate | Measured | Limit | Verdict |\n|---|---:|---:|---|\n\
         | B1, fixed | {:.0} B br | {B1:.0} | {} |\n\
         | B5, per site | {:.1} B br | {B5:.0} | {} |\n\
         | whole app, {sites} sites | {:.0} B br | {ambition:.0} | {} |\n",
        idlit.fixed,
        verdict(idlit.fixed <= B1),
        idlit.marginal,
        verdict(idlit.marginal <= B5),
        whole,
        verdict(whole <= ambition),
    );
    if let Some(dummy) = &dummy {
        let _ = write!(
            report,
            "\nAgainst the `dummy` bound (one literal everywhere, so the \
             optimiser merges sites a real application keeps apart): \
             {:.1} B br per site, {:.0} B br fixed.\n",
            dummy.marginal, dummy.fixed
        );
    }
    let _ = write!(
        report,
        "\nThe ambition: {B1:.0} B br fixed plus {B5:.0} B br per \
         call site, which at {sites} sites is {ambition:.0} B br — against \
         525 KB gzip for the whole reference application (that comparison has \
         not been re-measured on brotli).\n"
    );

    print!("{report}");
    fsx::write(&dir.join("report.md"), report.as_bytes())?;

    let mut failed = Vec::new();
    if idlit.fixed > B1 {
        failed.push(format!("B1: {:.0} B br fixed, over {B1:.0}", idlit.fixed));
    }
    if idlit.marginal > B5 {
        failed.push(format!(
            "B5: {:.1} B br per call site, over {B5:.0}",
            idlit.marginal
        ));
    }
    if whole > ambition {
        failed.push(format!(
            "whole app: {whole:.0} B br at {sites} sites, over {ambition:.0}"
        ));
    }
    if failed.is_empty() {
        eprintln!(
            "size: {:.0} B br fixed + {:.1} B br × {sites} sites = {whole:.0} B br \
             (target/size/report.md)",
            idlit.fixed, idlit.marginal
        );
        Ok(())
    } else {
        for f in &failed {
            eprintln!("size: {f}");
        }
        Err(gate(&format!(
            "{} size budget(s) exceeded (target/size/report.md)",
            failed.len()
        )))
    }
}

fn verdict(ok: bool) -> &'static str {
    if ok { "met" } else { "**over**" }
}

fn gate(status: &str) -> Error {
    Error::CommandFailed {
        command: "size".to_owned(),
        status: status.to_owned(),
        stderr: String::new(),
    }
}
