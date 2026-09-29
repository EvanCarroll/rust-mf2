//! `cargo xtask codegen-matrix`: the module `mf2-build` generates, compiled
//! in every feature combination of the facade — for the server, and for the
//! client on `wasm32-unknown-unknown` (Phase 5a, A5).
//!
//! `tools/i18n-fixture` is an i18n crate exactly as an application writes
//! one: `locales/`, `mf2.toml`, a `build.rs` that calls `mf2-build`, and a
//! `src/lib.rs` that includes what it generated. A plain workspace build
//! compiles it for the server; this command adds the client and the
//! combinations, and greps each client build for the things that may never
//! reach it.

use std::ffi::OsStr;
use std::path::Path;

use crate::cmd;
use crate::error::{Error, Result};

/// The fixture crate.
const FIXTURE: &str = "mf2-i18n-fixture";

/// The client target.
const WASM: &str = "wasm32-unknown-unknown";

/// Server combinations: what an application's `ssr` build forwards. The
/// last two also turn on `mf2`'s Leptos layer, on each line (Phase 10 B1):
/// the generated module beside the description types' Leptos impls.
const SERVER: [&str; 8] = [
    "ssr",
    "ssr,fn-number",
    "ssr,fn-datetime",
    "ssr,fn-datetime,datetime-icu",
    "ssr,fn-number,fn-datetime",
    "ssr,fn-number,fn-datetime,datetime-icu",
    "ssr,fn-number,fn-datetime,mf2/leptos,mf2/ssr",
    "ssr,fn-datetime,mf2/leptos-0-8,mf2/ssr,mf2/mark-fallback-lang",
];

/// Client combinations: what its `hydrate` build forwards. `intl` is the
/// client-only option of decision D4. The last three also turn on `mf2`'s
/// Leptos layer, on each line and in both client modes (Phase 10 B1).
const CLIENT: [&str; 10] = [
    "hydrate",
    "hydrate,fn-number",
    "hydrate,fn-number,intl",
    "hydrate,fn-datetime",
    "hydrate,fn-datetime,datetime-icu",
    "hydrate,fn-datetime,datetime-intl",
    "hydrate,fn-number,fn-datetime,datetime-intl,intl",
    "hydrate,fn-number,fn-datetime,mf2/leptos,mf2/hydrate",
    "hydrate,mf2/leptos,mf2/csr,mf2/static-locale",
    "hydrate,fn-datetime,mf2/leptos-0-8,mf2/hydrate",
];

pub(crate) fn run(root: &Path, quick: bool) -> Result<()> {
    let cargo = cmd::cargo();
    let server: &[&str] = if quick { &SERVER[..2] } else { &SERVER };
    let client: &[&str] = if quick { &CLIENT[..2] } else { &CLIENT };

    for features in server {
        eprintln!("codegen-matrix: native --features {features}");
        check(&cargo, root, features, None)?;
    }
    for features in client {
        eprintln!("codegen-matrix: {WASM} --features {features}");
        check(&cargo, root, features, Some(WASM))?;
    }
    eprintln!(
        "codegen-matrix: {} combinations compiled ({} server, {} client)",
        server.len() + client.len(),
        server.len(),
        client.len()
    );
    canaries(&cargo, root)?;
    Ok(())
}

/// Budget B6 on the generated module: build the fixture for the client and
/// look in the artifact for everything that may never reach it — the canary
/// text of every locale, every catalog's file name and every content hash.
///
/// The catalogs are behind `#[cfg(feature = \"ssr\")]`, so a client build
/// cannot embed them; this is the check that says so about the bytes rather
/// than about the source.
fn canaries(cargo: &OsStr, root: &Path) -> Result<()> {
    eprintln!("codegen-matrix: B6 canaries in the client build");
    let args: Vec<&OsStr> = vec![
        OsStr::new("build"),
        OsStr::new("-p"),
        OsStr::new(FIXTURE),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new("hydrate"),
        OsStr::new("--target"),
        OsStr::new(WASM),
        OsStr::new("--release"),
    ];
    cmd::run_inherit(cargo, &args, root)?;

    let rlib = root
        .join("target")
        .join(WASM)
        .join("release")
        .join("libmf2_i18n_fixture.rlib");
    let bytes = std::fs::read(&rlib).map_err(|source| Error::IoAt {
        path: rlib.clone(),
        source,
    })?;

    let mut patterns: Vec<String> = vec![
        "ZQ7-FIXTURE-CANARY-EN".to_owned(),
        "ZQ7-FIXTURE-CANARY-PL".to_owned(),
    ];
    patterns.extend(catalog_names(root));
    let mut hits = Vec::new();
    for pattern in &patterns {
        if find(&bytes, pattern.as_bytes()) {
            hits.push(pattern.clone());
        }
    }
    if !hits.is_empty() {
        return Err(Error::CommandFailed {
            command: format!("B6 canaries in {}", rlib.display()),
            status: format!("{} of {} patterns found", hits.len(), patterns.len()),
            stderr: hits.join("\n"),
        });
    }
    eprintln!(
        "codegen-matrix: B6 clean — none of {} patterns is in {} ({} B)",
        patterns.len(),
        rlib.file_name().unwrap_or_default().to_string_lossy(),
        bytes.len()
    );
    Ok(())
}

/// The catalog file names (and so their content hashes) the fixture's build
/// wrote, from the newest `OUT_DIR` it has.
fn catalog_names(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let build = root.join("target").join(WASM).join("release").join("build");
    let Ok(entries) = std::fs::read_dir(&build) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path().join("out");
        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if std::path::Path::new(&name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mf2b"))
            {
                out.push(name);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Whether `needle` occurs in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn check(cargo: &OsStr, root: &Path, features: &str, target: Option<&str>) -> Result<()> {
    let mut args: Vec<&OsStr> = vec![
        OsStr::new("check"),
        OsStr::new("-p"),
        OsStr::new(FIXTURE),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(features),
    ];
    if let Some(target) = target {
        args.push(OsStr::new("--target"));
        args.push(OsStr::new(target));
    }
    cmd::run_inherit(cargo, &args, root).map_err(|e| match e {
        Error::CommandFailed { status, .. } => Error::CommandFailed {
            command: format!("cargo check -p {FIXTURE} --features {features}"),
            status,
            stderr: String::new(),
        },
        other => other,
    })
}
