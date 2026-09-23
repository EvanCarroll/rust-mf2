//! `cargo xtask l6-web`: conformance layer L6 in the browser
//! (`plans/01-conformance.md` §3; `plans/14-phase-6-work-order.md` A6b).
//!
//! Three steps, and the second is the point of the first:
//!
//! 1. `conformance/l6-web`'s `l6-page` binary renders every runtime-valid
//!    `en-US` suite message through the view type and writes the page, its
//!    catalogs and `page.json` into `target/l6-web/`;
//! 2. the **same crate** is built for `wasm32-unknown-unknown` with
//!    `hydrate` and bound with `wasm-bindgen`, so that what hydrates the page
//!    is the same view that wrote it;
//! 3. `tools/e2e/checks/l6.mjs` serves the directory and drives each engine:
//!    hydrate, switch to the twin locale, switch back.
//!
//! The server half of L6 is a Rust harness and runs in `cargo test`; this is
//! the half that needs an engine.

use std::ffi::OsStr;
use std::path::Path;

use crate::cmd;
use crate::error::{Error, Result};

/// The engines this runs in by default. Two are required (master plan §9,
/// P6); `WebKit` joins wherever a build of it is installed.
pub(crate) const ENGINES: [&str; 3] = ["chromium", "firefox", "webkit"];

pub(crate) fn run(root: &Path, engines: &[String], build: bool) -> Result<()> {
    let dir = root.join("target/l6-web");
    if build {
        render_page(root)?;
        build_wasm(root, &dir)?;
    }
    if !dir.join("index.html").is_file() {
        return Err(Error::L6(format!(
            "{} does not exist; run without --no-build",
            dir.join("index.html").display()
        )));
    }
    let list = if engines.is_empty() {
        ENGINES.join(",")
    } else {
        engines.join(",")
    };
    eprintln!("l6-web: running tools/e2e/checks/l6.mjs in {list}");
    cmd::run_inherit(
        OsStr::new("node"),
        &[
            OsStr::new("run.mjs"),
            OsStr::new("l6"),
            OsStr::new("--browser"),
            OsStr::new(&list),
            OsStr::new("--json"),
            root.join("target/l6-web/report.json").as_os_str(),
        ],
        &root.join("tools/e2e"),
    )
    .map_err(|_| Error::L6("the browser run failed (target/l6-web/report.json)".to_owned()))?;
    eprintln!("l6-web: every engine hydrated the page and switched locale (target/l6-web/)");
    Ok(())
}

/// Renders the page with the `ssr` build.
fn render_page(root: &Path) -> Result<()> {
    eprintln!("l6-web: rendering the page (conformance/l6-web, ssr)");
    cmd::run_inherit(
        &cmd::cargo(),
        &[
            "run",
            "--release",
            "--manifest-path",
            "conformance/l6-web/Cargo.toml",
            "--target-dir",
            "target/l6-web/cargo",
            "--no-default-features",
            "--features",
            "ssr",
            "--bin",
            "l6-page",
        ]
        .map(OsStr::new),
        root,
    )
}

/// Builds the same crate for the browser and binds it.
///
/// A separate `cargo` invocation from the one above on purpose: `ssr` and
/// `hydrate` are exclusive, and cargo unifies features within one build.
fn build_wasm(root: &Path, dir: &Path) -> Result<()> {
    eprintln!("l6-web: building conformance/l6-web for wasm32-unknown-unknown (hydrate)");
    cmd::run_inherit(
        &cmd::cargo(),
        &[
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            "conformance/l6-web/Cargo.toml",
            "--target-dir",
            "target/l6-web/cargo",
            "--no-default-features",
            "--features",
            "hydrate",
            "--lib",
        ]
        .map(OsStr::new),
        root,
    )?;
    let wasm = dir.join("cargo/wasm32-unknown-unknown/release/mf2_l6_web.wasm");
    let pkg = dir.join("pkg");
    cmd::run_inherit(
        OsStr::new("wasm-bindgen"),
        &[
            OsStr::new("--target"),
            OsStr::new("web"),
            OsStr::new("--out-dir"),
            pkg.as_os_str(),
            wasm.as_os_str(),
        ],
        root,
    )
}
