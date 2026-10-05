//! `cargo xtask codegen-matrix`: the module `mf2-build` generates, compiled
//! in every feature combination of the facade — for the server, and for the
//! client on `wasm32-unknown-unknown` (Phase 5a, A5).
//!
//! `tools/i18n-fixture` is an i18n crate exactly as an application writes
//! one: `locales/`, `mf2.toml`, a `build.rs` that calls `mf2-build`, and a
//! `src/lib.rs` that includes what it generated. A plain workspace build
//! compiles it for the server; this command adds the client and the
//! combinations, and greps each client build for the things that may never
//! reach it — among them ICU4X, in a client that formats dates through
//! `Intl` (`plan/08` §7).
//!
//! Each combination is built with `cargo clippy ... -- -D warnings`, not
//! `cargo check` (23.0). The fixture's own `[lints.clippy]` turns `pedantic`
//! and `nursery` on and does not allow `must_use_candidate`, and an
//! `include!`d file is linted as the including crate's own code: so this is
//! what keeps the generated module clean under the lints a strict
//! application runs over its own source.

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
        "codegen-matrix: {} combinations clean under a strict clippy ({} server, {} native, {} client)",
        server.len() + native.len() + client.len(),
        server.len(),
        native.len(),
        client.len()
    );
    canaries(&cargo, root)?;
    intl_links_no_icu(&cargo, root)?;
    Ok(())
}

/// A client whose dates go through the browser's `Intl`
/// (`leptos-client-datetime-intl`, the framework's name for
/// `host-web-datetime-intl`), over the fixture's corpus with a `:datetime`
/// message: with no date message, any date formatter is linked out and the
/// search below could never fail. The corpus counts things too, so the
/// client names a number formatter: plain digits, the smallest.
const CLIENT_INTL: &str = "hydrate,host-web-number-plain,leptos-client-datetime-intl,corpus-dates";

/// The fixture's client binary, the one linked wasm it makes.
const CLIENT_BIN: &str = "mf2-i18n-client";

/// `Intl` formats the client's dates, so ICU4X must not reach it: no ICU4X
/// crate in the browser target's dependency graph, and no ICU4X symbol in the
/// linked wasm. The wasm is built into a target directory of its own with
/// its symbol names kept (the release profile strips debug information,
/// which a build with other profile settings would rebuild the main target
/// directory to undo).
fn intl_links_no_icu(cargo: &OsStr, root: &Path) -> Result<()> {
    eprintln!("codegen-matrix: no ICU4X in a client that formats dates through Intl");
    let tree = [
        "tree",
        "-p",
        FIXTURE,
        "--target",
        WASM,
        "--no-default-features",
        "--features",
        CLIENT_INTL,
        "--edges",
        "normal",
        "--prefix",
        "none",
    ]
    .map(OsStr::new);
    let listing = cmd::run_capture(cargo, &tree, root, &[])?;
    let listing = String::from_utf8_lossy(&listing);
    let mut crates: Vec<&str> = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name == "icu" || name.starts_with("icu_"))
        .collect();
    crates.sort_unstable();
    crates.dedup();
    if !crates.is_empty() {
        return Err(Error::CommandFailed {
            command: format!("cargo tree --target {WASM} --features {CLIENT_INTL}"),
            status: format!("{} ICU4X crate(s) in the client's graph", crates.len()),
            stderr: crates.join("\n"),
        });
    }

    let target = root.join("target").join("client-canaries");
    let args = [
        OsStr::new("build"),
        OsStr::new("-p"),
        OsStr::new(FIXTURE),
        OsStr::new("--bin"),
        OsStr::new(CLIENT_BIN),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(CLIENT_INTL),
        OsStr::new("--target"),
        OsStr::new(WASM),
        OsStr::new("--release"),
        OsStr::new("--target-dir"),
        target.as_os_str(),
    ];
    let keep_names = [("CARGO_PROFILE_RELEASE_STRIP", OsStr::new("none"))];
    cmd::run_inherit_env(cargo, &args, root, &keep_names)?;
    let wasm = target
        .join(WASM)
        .join("release")
        .join(format!("{CLIENT_BIN}.wasm"));
    let bytes = std::fs::read(&wasm).map_err(|source| Error::IoAt {
        path: wasm.clone(),
        source,
    })?;
    // A wasm with no names would pass whatever it links.
    if !find(&bytes, b"core::") && !find(&bytes, b"4core") {
        return Err(Error::CommandFailed {
            command: format!("ICU4X symbols in {}", wasm.display()),
            status: "the wasm carries no symbol names, so the search proves nothing".to_owned(),
            stderr: String::new(),
        });
    }
    if let Some(name) = icu_symbol(&bytes) {
        return Err(Error::CommandFailed {
            command: format!("ICU4X symbols in {}", wasm.display()),
            status: format!("a symbol of `{name}` is linked"),
            stderr: String::new(),
        });
    }
    eprintln!(
        "codegen-matrix: no ICU4X crate in the client's graph, no ICU4X symbol in {} ({} B)",
        wasm.file_name().unwrap_or_default().to_string_lossy(),
        bytes.len()
    );
    Ok(())
}

/// The first ICU4X crate a symbol name in `wasm` belongs to. A name is
/// written demangled (`icu_calendar::…`) or mangled, where each path
/// component follows its length (`12icu_calendar`); no item of the client's
/// own crates is named `icu_…`, so either form is ICU4X's.
fn icu_symbol(wasm: &[u8]) -> Option<String> {
    let needle = b"icu_";
    let mut from = 0;
    while let Some(found) = wasm
        .get(from..)
        .and_then(|rest| rest.windows(needle.len()).position(|w| w == needle))
    {
        let start = from + found;
        let tail = wasm.get(start..).unwrap_or_default();
        let len = tail
            .iter()
            .position(|b| !(b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_'))
            .unwrap_or(tail.len());
        // Mangled, the digits before the name are its length, which may be
        // shorter than the run of name characters (`12icu_provider5load`).
        let head = wasm.get(start.saturating_sub(3)..start).unwrap_or_default();
        let digits = head.len()
            - head
                .iter()
                .rposition(|b| !b.is_ascii_digit())
                .map_or(0, |at| at + 1);
        let mangled = std::str::from_utf8(head.get(head.len() - digits..).unwrap_or_default())
            .ok()
            .and_then(|text| text.parse::<usize>().ok())
            .filter(|length| *length > needle.len() && *length <= len);
        let demangled = tail.get(len..len + 2) == Some(b"::".as_slice());
        let name_len = match mangled {
            Some(length) => Some(length),
            None if demangled => Some(len),
            None => None,
        };
        if let Some(name) = name_len.and_then(|n| tail.get(..n)) {
            return Some(String::from_utf8_lossy(name).into_owned());
        }
        from = start + 1;
    }
    None
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
        OsStr::new("hydrate,host-web-number-plain"),
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

/// `cargo clippy ... -- -D warnings` of the fixture for one set.
///
/// Clippy, not `cargo check` (23.0): the fixture carries its own
/// `[lints.clippy]` with `pedantic` and `nursery` at warn and
/// `must_use_candidate` not allowed, so the generated module — which is
/// `include!`d, and therefore linted as the including crate's own code — is
/// held to what a strict application holds its own source to, in every set.
fn check(cargo: &OsStr, root: &Path, set: &Set) -> Result<()> {
    let mut args: Vec<&str> = vec!["clippy"];
    args.extend(set.selection());
    if let Some(triple) = set.target.triple() {
        args.push("--target");
        args.push(triple);
    }
    args.extend(["--", "-D", "warnings"]);
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    cmd::run_inherit(cargo, &args, root).map_err(|e| match e {
        Error::CommandFailed { status, .. } => Error::CommandFailed {
            command: format!(
                "cargo clippy -p {FIXTURE} --features {} -- -D warnings",
                set.features
            ),
            status,
            stderr: String::new(),
        },
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::icu_symbol;

    /// An ICU4X crate is found in either way a wasm writes a symbol's name,
    /// and the date crate whose name ends in `web_icu` is not taken for one.
    #[test]
    fn an_icu4x_symbol_is_found_demangled_or_mangled() {
        assert_eq!(
            icu_symbol(b"\x00\x2aicu_calendar::any_calendar::AnyCalendar::new::h0123"),
            Some("icu_calendar".to_owned())
        );
        assert_eq!(
            icu_symbol(b"_RNvCs1_12icu_provider5load"),
            Some("icu_provider".to_owned())
        );
        assert_eq!(
            icu_symbol(b"<core::ptr::drop_in_place<icu_datetime::DateTimeFormatter>>"),
            Some("icu_datetime".to_owned())
        );
        assert_eq!(
            icu_symbol(b"mf2_fn_datetime_web_icu::format core::fmt::write icu.blob"),
            None
        );
    }
}
