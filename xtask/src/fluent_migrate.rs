//! `cargo xtask fluent-migrate`: the reference application migrated from
//! `leptos-fluent` and built (`plans/16-phase-8-work-order.md` A4).
//!
//! 1. The reference workload, with the `fluent-view` application (its call
//!    sites in `leptos-fluent`'s idiom, its messages the workload's `.ftl`
//!    files) and `fluent-converted` (what the migration must make of it).
//! 2. `mf2 convert --from leptos-fluent --write` on a copy of `fluent-view`.
//!    Its report must hold only what the guide finishes by hand: the
//!    initializer, the `use` beside it, and the manifest's dependencies.
//! 3. Every file with a call site must then be `fluent-converted`'s, byte
//!    for byte.
//! 4. The hand-finishing the guide describes — the manifest, the
//!    initializer's module, the entry points — is done by taking those
//!    files from `fluent-converted`; after it, the migrated application is
//!    `fluent-converted` in every file.
//! 5. It is built: the client for `wasm32-unknown-unknown` with `hydrate`,
//!    the server with `ssr`, against the converted messages.
//!
//! The same comparison, and its negative control, run in `cargo test -p
//! mf2-cli` in memory; this command adds the command line, the files on
//! disk and the build.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use crate::cmd::{cargo, run_inherit, run_inherit_env};
use crate::error::{Error, Result};
use crate::fsx;

/// The files the guide finishes by hand, which come from `fluent-converted`.
const BY_HAND: &[&str] = &[
    "Cargo.toml",
    "src/app.rs",
    "src/lib.rs",
    "src/main.rs",
    "src/support.rs",
];

fn fail(message: impl Into<String>) -> Error {
    Error::FluentMigrate(message.into())
}

pub(crate) fn run(root: &Path, build: bool) -> Result<()> {
    let out = root.join("target/fluent-migrate");
    fsx::remove(&out)?;
    let templates = root.join("bench/workload-gen/templates");
    let view = templates.join("fluent-view");
    let converted = templates.join("fluent-converted");
    let wl = out.join("wl");

    eprintln!("==> workload-gen all -t fluent-view -t fluent-converted --format ftl");
    run_inherit(
        &cargo(),
        &[
            OsStr::new("run"),
            OsStr::new("--release"),
            OsStr::new("--quiet"),
            OsStr::new("-p"),
            OsStr::new("workload-gen"),
            OsStr::new("--"),
            OsStr::new("all"),
            OsStr::new("-t"),
            view.as_os_str(),
            OsStr::new("-t"),
            converted.as_os_str(),
            OsStr::new("--format"),
            OsStr::new("ftl"),
            OsStr::new("--out"),
            wl.as_os_str(),
        ],
        root,
    )?;
    run_inherit(
        &cargo(),
        &[
            OsStr::new("build"),
            OsStr::new("-q"),
            OsStr::new("-p"),
            OsStr::new("mf2-cli"),
        ],
        root,
    )?;

    // A copy, so that the original stays beside it for reading.
    let app = wl.join("app-migrated");
    for (path, bytes) in fsx::read_tree(&wl.join("app-fluent-view"), ".")? {
        fsx::write(&app.join(path.trim_start_matches("./")), &bytes)?;
    }
    let i18n = wl.join("converted");
    eprintln!("==> mf2 -C converted convert --from leptos-fluent app-migrated --write");
    let output = Command::new(root.join("target/debug/mf2"))
        .arg("-C")
        .arg(&i18n)
        .args(["convert", "--from", "leptos-fluent"])
        .arg(&app)
        .args([
            "--i18n-crate",
            "workload_i18n",
            "--write",
            "--format",
            "json",
        ])
        .output()
        .map_err(|source| Error::Spawn {
            program: "mf2".to_owned(),
            source,
        })?;
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        fail(format!(
            "the report is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    })?;
    check_report(&report, &app)?;

    // Every file with a call site is `fluent-converted`'s.
    let expected = fsx::read_tree(&wl.join("app-fluent-converted"), ".")?;
    let migrated = fsx::read_tree(&app, ".")?;
    let mut compared = 0;
    for (path, bytes) in &expected {
        let rel = path.trim_start_matches("./");
        if BY_HAND.contains(&rel) {
            continue;
        }
        if migrated.get(path) != Some(bytes) {
            return Err(fail(format!(
                "{rel} differs from fluent-converted's (diff -r {} {})",
                app.display(),
                wl.join("app-fluent-converted").display()
            )));
        }
        compared += 1;
    }
    eprintln!("==> fluent-migrate: {compared} files are fluent-converted's, byte for byte");

    for rel in BY_HAND {
        let bytes = expected
            .get(&format!("./{rel}"))
            .ok_or_else(|| fail(format!("fluent-converted has no {rel}")))?;
        fsx::write(&app.join(rel), bytes)?;
    }
    if !build {
        return Ok(());
    }

    let target = out.join("target");
    let jobs = std::env::var_os("CARGO_BUILD_JOBS").unwrap_or_else(|| OsString::from("3"));
    let envs: [(&str, &OsStr); 3] = [
        ("CARGO_TARGET_DIR", target.as_os_str()),
        ("MF2_WORKLOAD_LOCALES", i18n.as_os_str()),
        ("CARGO_BUILD_JOBS", jobs.as_os_str()),
    ];
    eprintln!(
        "==> app-migrated: cargo build --lib --features hydrate --target wasm32-unknown-unknown"
    );
    run_inherit_env(
        &cargo(),
        &[
            OsStr::new("build"),
            OsStr::new("--quiet"),
            OsStr::new("--lib"),
            OsStr::new("--no-default-features"),
            OsStr::new("--features"),
            OsStr::new("hydrate"),
            OsStr::new("--target"),
            OsStr::new("wasm32-unknown-unknown"),
        ],
        &app,
        &envs,
    )?;
    eprintln!("==> app-migrated: cargo build --features ssr");
    run_inherit_env(
        &cargo(),
        &[
            OsStr::new("build"),
            OsStr::new("--quiet"),
            OsStr::new("--no-default-features"),
            OsStr::new("--features"),
            OsStr::new("ssr"),
        ],
        &app,
        &envs,
    )?;
    eprintln!(
        "==> fluent-migrate: the migrated reference application builds for the client and the server"
    );
    Ok(())
}

/// The report holds exactly what the guide finishes by hand.
pub(crate) fn check_report(report: &serde_json::Value, app: &Path) -> Result<()> {
    let mut found: BTreeMap<(String, String), usize> = BTreeMap::new();
    for d in report["diagnostics"].as_array().into_iter().flatten() {
        let file = d["file"].as_str().unwrap_or("");
        let rel = Path::new(file)
            .strip_prefix(app)
            .map_or_else(|_| file.to_owned(), |p| p.display().to_string());
        let code = d["code"].as_str().unwrap_or("").to_owned();
        *found.entry((code, rel)).or_default() += 1;
    }
    let expected: BTreeMap<(String, String), usize> = [
        ("leptos-fluent-dependency", "Cargo.toml", 3),
        ("leptos-fluent-import", "src/support.rs", 1),
        ("leptos-fluent-initializer", "src/support.rs", 1),
    ]
    .into_iter()
    .map(|(c, f, n)| ((c.to_owned(), f.to_owned()), n))
    .collect();
    if found != expected {
        return Err(fail(format!(
            "the report is not the hand-finishing the guide describes: {found:?}"
        )));
    }
    let entries = &report["entries"];
    for tag in ["en", "pl", "en-XA", "ar-XB"] {
        if entries[tag] != 1_616 {
            return Err(fail(format!("{tag}: {} entries, not 1,616", entries[tag])));
        }
    }
    eprintln!("==> fluent-migrate: the report is the initializer, its `use` and the dependencies");
    Ok(())
}
