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
use crate::feature_sets::{self, FIXTURE, Group, Set, WASM};

pub(crate) fn run(root: &Path, quick: bool) -> Result<()> {
    let cargo = cmd::cargo();
    let list = |group: Group| {
        let mut sets = feature_sets::used_by(move |set| set.group() == Some(group));
        if quick {
            sets.truncate(2);
        }
        sets
    };
    let server = list(Group::Server);
    let native = list(Group::Native);
    let client = list(Group::Client);

    for set in server.iter().chain(&native) {
        eprintln!("codegen-matrix: native --features {}", set.features);
        check(&cargo, root, set)?;
    }
    for set in &client {
        eprintln!("codegen-matrix: {WASM} --features {}", set.features);
        check(&cargo, root, set)?;
    }
    eprintln!(
        "codegen-matrix: {} combinations compiled ({} server, {} native, {} client)",
        server.len() + native.len() + client.len(),
        server.len(),
        native.len(),
        client.len()
    );
    canaries(&cargo, root)?;
    Ok(())
}

/// Budget B6 on the generated module: build the fixture for the client and
/// look in the artifact for everything that may never reach it — the canary
/// text of every locale, every catalog's file name and every content hash.
///
/// The catalogs are behind `__mf2::__if_host_std!`, which a client build of
/// `mf2` defines to drop them; this is the check that says so about the
/// bytes rather than about the source.
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

/// `cargo check` of the fixture for one set.
fn check(cargo: &OsStr, root: &Path, set: &Set) -> Result<()> {
    let mut args: Vec<&str> = vec!["check"];
    args.extend(set.selection());
    if let Some(triple) = set.target.triple() {
        args.push("--target");
        args.push(triple);
    }
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    cmd::run_inherit(cargo, &args, root).map_err(|e| match e {
        Error::CommandFailed { status, .. } => Error::CommandFailed {
            command: format!("cargo check -p {FIXTURE} --features {}", set.features),
            status,
            stderr: String::new(),
        },
        other => other,
    })
}
