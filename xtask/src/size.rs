//! `cargo xtask size`: the **whole-app** size gate.
//!
//! B5 gates one number — what a call site costs at the margin. This gates the
//! thing the ambition is actually about: what an application pays for i18n in
//! total, on the reference workload, end to end.
//!
//! It is the same measurement `cargo xtask b5` makes — two scales, three
//! templates, `wasm32-unknown-unknown` / `wasm-release` / `wasm-bindgen` /
//! `wasm-opt -Oz` / `gzip -9` — read three ways:
//!
//! | Gate | What it is | Limit |
//! |---|---|---|
//! | **B1** | the part that does not grow with the call sites: the reader, the evaluator, the call-site library, the Leptos glue, the boot | ≤ 30 KB gz |
//! | **B5** | the part that does, per site, weighted by the workload's mix | ≤ 40 B gz |
//! | **whole app** | B1 + sites × B5 at the reference scale | ≤ the ambition, `30 KB + sites × 40 B` |
//!
//! Against the `idlit` baseline throughout, with `dummy` reported as the
//! conservative bound (06 §3 says why that baseline and not the other).
//!
//! It is a **gate**, so it fails the build rather than printing a number: a
//! regression in any of the three is a regression in the thing the project
//! claims.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::b5;
use crate::error::{Error, Result};
use crate::fsx;

/// B1: everything fixed, in bytes gz (06 §3; KB is 1024 bytes there, as in
/// B7's `0.91 × 25 KB`).
const B1: f64 = 30.0 * 1024.0;
/// B5: per call site, in bytes gz.
const B5: f64 = 40.0;

pub(crate) fn run(root: &Path, out: Option<PathBuf>, keep: bool) -> Result<()> {
    let dir = out.unwrap_or_else(|| root.join("target").join("size"));
    let measured = b5::measure(root, Some(dir.clone()), keep, b5::Mode::String)?;
    let sites = measured
        .first()
        .map(|(sites, _)| *sites)
        .ok_or_else(|| gate("no workload was measured"))?;

    let idlit = b5::delta(&measured, "tr", "idlit")
        .ok_or_else(|| gate("the `tr` or `idlit` app did not build"))?;
    let dummy = b5::delta(&measured, "tr", "dummy");

    #[allow(clippy::cast_precision_loss)]
    let scale = sites as f64;
    let whole = idlit.fixed + scale * idlit.marginal;
    let ambition = B1 + scale * B5;

    let mut report = String::from("# Size gate\n");
    report.push_str(&b5::size_table(&measured));
    let _ = write!(
        report,
        "\n| Gate | Measured | Limit | Verdict |\n|---|---:|---:|---|\n\
         | B1, fixed | {:.0} B gz | {B1:.0} | {} |\n\
         | B5, per site | {:.1} B gz | {B5:.0} | {} |\n\
         | whole app, {sites} sites | {:.0} B gz | {ambition:.0} | {} |\n",
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
             {:.1} B gz per site, {:.0} B gz fixed.\n",
            dummy.marginal, dummy.fixed
        );
    }
    let _ = write!(
        report,
        "\nThe ambition is 06 §3's: {B1:.0} B gz fixed plus {B5:.0} B gz per \
         call site, which at {sites} sites is {ambition:.0} B gz — against \
         525 KB gz for the whole reference application.\n"
    );

    print!("{report}");
    fsx::write(&dir.join("report.md"), report.as_bytes())?;

    let mut failed = Vec::new();
    if idlit.fixed > B1 {
        failed.push(format!("B1: {:.0} B gz fixed, over {B1:.0}", idlit.fixed));
    }
    if idlit.marginal > B5 {
        failed.push(format!(
            "B5: {:.1} B gz per call site, over {B5:.0}",
            idlit.marginal
        ));
    }
    if whole > ambition {
        failed.push(format!(
            "whole app: {whole:.0} B gz at {sites} sites, over {ambition:.0}"
        ));
    }
    if failed.is_empty() {
        eprintln!(
            "size: {:.0} B gz fixed + {:.1} B gz × {sites} sites = {whole:.0} B gz \
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
