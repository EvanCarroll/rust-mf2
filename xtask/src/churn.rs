//! `cargo xtask churn`: the conversions under churn
//! (`plans/15-phase-7-work-order.md` A5).
//!
//! Builds `bench/churn` — P0.11's churning list on `leptos-mf2`, one row
//! shape per variant — into `target/churn/site/` as a client-only site (the
//! harness page, the wasm bound with `wasm-bindgen`, the catalogs and their
//! index from `mf2 compile --site`), then runs `tools/e2e/checks/churn.mjs`,
//! which serves it and asserts that no row shape grows the heap over
//! 100,000 churned rows. `--browser` picks the engines; the measurements go
//! to `target/churn/report.json`.
//!
//! A release build (`opt-level = "z"`, fat LTO, as the shipped client), so
//! the churn runs at the speed an application would.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::cmd;
use crate::error::{Error, Result};

pub(crate) fn run(root: &Path, engines: &str, build: bool) -> Result<()> {
    let out = root.join("target/churn");
    let site = out.join("site");
    if build {
        build_site(root, &out, &site)?;
    }
    let report = out.join("report.json");
    eprintln!("churn: running tools/e2e/checks/churn.mjs in {engines}");
    cmd::run_inherit(
        OsStr::new("node"),
        &[
            OsStr::new("run.mjs"),
            OsStr::new("churn"),
            OsStr::new("--browser"),
            OsStr::new(engines),
            OsStr::new("--json"),
            report.as_os_str(),
        ],
        &root.join("tools/e2e"),
    )
}

fn build_site(root: &Path, out: &Path, site: &Path) -> Result<()> {
    if site.exists() {
        // Stale catalogs from an earlier corpus would be served.
        fs::remove_dir_all(site).map_err(|source| Error::IoAt {
            path: site.to_owned(),
            source,
        })?;
    }
    let target = out.join("target");
    eprintln!("churn: building bench/churn (wasm32, release)");
    cmd::run_inherit(
        &cmd::cargo(),
        &[
            OsStr::new("build"),
            OsStr::new("--release"),
            OsStr::new("--lib"),
            OsStr::new("--target"),
            OsStr::new("wasm32-unknown-unknown"),
            OsStr::new("--manifest-path"),
            OsStr::new("bench/churn/Cargo.toml"),
            OsStr::new("--target-dir"),
            target.as_os_str(),
        ],
        root,
    )?;
    let wasm = target.join("wasm32-unknown-unknown/release/churn_harness.wasm");
    let pkg = site.join("pkg");
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
    let page = site.join("index.html");
    fs::copy(root.join("bench/churn/web/index.html"), &page).map_err(|source| Error::IoAt {
        path: page.clone(),
        source,
    })?;
    eprintln!("churn: publishing the catalogs");
    let i18n = site.join("i18n");
    cmd::run_inherit(
        &cmd::cargo(),
        &[
            OsStr::new("run"),
            OsStr::new("--quiet"),
            OsStr::new("-p"),
            OsStr::new("mf2-cli"),
            OsStr::new("--"),
            OsStr::new("-C"),
            OsStr::new("bench/churn/i18n"),
            OsStr::new("compile"),
            OsStr::new("--site"),
            i18n.as_os_str(),
        ],
        root,
    )
}
