//! `cargo xtask leptos-beta`: `leptos-mf2` on the newest Leptos 0.9
//! pre-release, with `tachys-0-3` (`plans/15-phase-7-work-order.md` A7;
//! `plans/04-leptos-integration.md` §10).
//!
//! 1. **On 0.8, `tachys-0-3` must fail, and say why.** `cargo check` of
//!    `leptos-mf2` with `ssr,tachys-0-3` in the working tree must fail, and
//!    its first error must carry the fix (`glue/view.rs`' `RenderFlags`
//!    import line).
//! 2. **A copy, never the working tree.** Every tracked file, as it is in the
//!    working tree (uncommitted edits included), is copied to
//!    `target/leptos-beta/tree`; its workspace manifest's six Leptos pins
//!    become `^0.9.0-alpha` / `^0.3.0-alpha`, so cargo resolves the newest
//!    0.9 / 0.3 pre-release (or, once 0.9 is out, the newest 0.9 release —
//!    which is then printed as the cue to move the workspace, D10). The copy
//!    has no `Cargo.lock`, so every run resolves afresh; its build goes to
//!    `target/leptos-beta/target`, which is kept between runs.
//! 3. **On the beta.** `leptos-mf2` checked for `ssr` natively and for
//!    `hydrate` and `csr` on `wasm32-unknown-unknown`, then its `render`
//!    (`ssr`) and `churn` (`csr`) tests, all with `tachys-0-3`; then
//!    `mark-fallback-lang`, whose server glue has a 0.3 form of its own (the
//!    separator follows `flags.hydrate`): `hydrate` checked and the
//!    `fallback_lang` test (`ssr`).
//!
//! `--no-tachys-0-3` runs step 3 without the feature: the negative control,
//! which must fail on a 0.9 pre-release (the two `to_html_with_buf` impls).

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cmd::{cargo, run_capture, run_inherit_env};
use crate::error::{Error, Result};
use crate::fsx;

/// The workspace pins that move together, and the requirement each gets in
/// the copy.
const PINS: &[(&str, &str)] = &[
    ("leptos", "^0.9.0-alpha"),
    ("tachys", "^0.3.0-alpha"),
    ("reactive_graph", "^0.3.0-alpha"),
    ("leptos_axum", "^0.9.0-alpha"),
    ("leptos_meta", "^0.9.0-alpha"),
    ("leptos_router", "^0.9.0-alpha"),
];

/// What the first error of `tachys-0-3` on Leptos 0.8 must say.
const FIX: &str = "`tachys-0-3` needs Leptos 0.9; for Leptos 0.8, turn it off";

pub(crate) fn run(root: &Path, with_feature: bool) -> Result<()> {
    if with_feature {
        eprintln!("==> Leptos 0.8: `tachys-0-3` must be a compile error that names the fix");
        refuses_0_8(root)?;
    }

    let base = root.join("target").join("leptos-beta");
    let tree = base.join("tree");
    eprintln!("==> copying the tracked tree to {}", tree.display());
    copy_tracked(root, &tree)?;
    pin_beta(&tree.join("Cargo.toml"))?;

    let target_dir = base.join("target");
    let envs: &[(&str, &OsStr)] = &[("CARGO_TARGET_DIR", target_dir.as_os_str())];
    let features = |target: &str| {
        if with_feature {
            format!("{target},tachys-0-3")
        } else {
            target.to_owned()
        }
    };
    let (ssr, hydrate, csr) = (features("ssr"), features("hydrate"), features("csr"));
    let (ssr_lang, hydrate_lang) = (
        features("ssr,mark-fallback-lang"),
        features("hydrate,mark-fallback-lang"),
    );
    let wasm = "wasm32-unknown-unknown";
    let base_args = ["-p", "leptos-mf2", "--no-default-features", "--features"];
    let steps: [Vec<&str>; 7] = [
        [&["check"][..], &base_args, &[&ssr]].concat(),
        [&["check", "--target", wasm][..], &base_args, &[&hydrate]].concat(),
        [&["check", "--target", wasm][..], &base_args, &[&csr]].concat(),
        [&["test"][..], &base_args, &[&ssr, "--test", "render"]].concat(),
        [&["test"][..], &base_args, &[&csr, "--test", "churn"]].concat(),
        [
            &["check", "--target", wasm][..],
            &base_args,
            &[&hydrate_lang],
        ]
        .concat(),
        [
            &["test"][..],
            &base_args,
            &[&ssr_lang, "--test", "fallback_lang"],
        ]
        .concat(),
    ];
    let cargo = cargo();
    for (i, step) in steps.iter().enumerate() {
        let shown = format!("cargo {}", step.join(" "));
        eprintln!("==> (beta) {shown}");
        let args: Vec<&OsStr> = step.iter().map(OsStr::new).collect();
        run_inherit_env(&cargo, &args, &tree, envs)
            .map_err(|_| Error::LeptosBeta(format!("on the 0.9 pre-release: `{shown}` failed")))?;
        if i == 0 {
            report_resolved(&tree)?;
        }
    }
    eprintln!(
        "==> leptos-beta: leptos-mf2 checks for ssr, hydrate and csr and passes render, \
         churn and fallback_lang on the pre-release{}",
        if with_feature {
            ", with `tachys-0-3`"
        } else {
            " WITHOUT `tachys-0-3` — the negative control should have failed"
        }
    );
    Ok(())
}

/// Step 1: in the working tree, on its own Leptos 0.8.
fn refuses_0_8(root: &Path) -> Result<()> {
    let output = Command::new(cargo())
        .args([
            "check",
            "-p",
            "leptos-mf2",
            "--no-default-features",
            "--features",
            "ssr,tachys-0-3",
            "--color",
            "never",
        ])
        .current_dir(root)
        .output()
        .map_err(|source| Error::Spawn {
            program: "cargo check".to_owned(),
            source,
        })?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        return Err(Error::LeptosBeta(
            "`tachys-0-3` compiled on Leptos 0.8; it must be a compile error".to_owned(),
        ));
    }
    // From the first line that starts an error to the blank line ending it.
    let first = stderr
        .match_indices("error")
        .map(|(at, _)| at)
        .find(|&at| at == 0 || stderr[..at].ends_with('\n'))
        .map_or("", |at| {
            let rest = &stderr[at..];
            rest.find("\n\n").map_or(rest, |end| &rest[..end])
        });
    if !first.contains(FIX) {
        return Err(Error::LeptosBeta(format!(
            "`tachys-0-3` on Leptos 0.8 fails, but its first error does not say {FIX:?}:\n{first}"
        )));
    }
    eprintln!("{first}");
    Ok(())
}

/// Copies every tracked file that exists in the working tree to `to`, which
/// is emptied first.
fn copy_tracked(root: &Path, to: &Path) -> Result<()> {
    fsx::remove(to)?;
    let listed = run_capture(
        OsStr::new("git"),
        &[OsStr::new("ls-files"), OsStr::new("-z")],
        root,
        &[],
    )?;
    let mut copied = 0_usize;
    for rel in listed.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let rel = String::from_utf8_lossy(rel);
        let from = root.join(rel.as_ref());
        // A tracked file deleted in the working tree is deleted in the copy.
        if !from.is_file() {
            continue;
        }
        let dest = to.join(rel.as_ref());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::IoAt {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::copy(&from, &dest).map_err(|source| Error::IoAt { path: from, source })?;
        copied += 1;
    }
    eprintln!("    {copied} files");
    Ok(())
}

/// Rewrites the version requirement of each of [`PINS`] in the copy's
/// workspace manifest, whether written `name = "v"` or `name = { version =
/// "v", … }`. A pin that is not there is an error: the list and the manifest
/// have drifted apart.
fn pin_beta(manifest: &Path) -> Result<()> {
    let text = fsx::read_to_string(manifest)?;
    let mut out = String::with_capacity(text.len());
    let mut seen = vec![false; PINS.len()];
    for line in text.split_inclusive('\n') {
        let pin = PINS.iter().position(|(name, _)| {
            line.strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        });
        let Some(i) = pin else {
            out.push_str(line);
            continue;
        };
        let (name, req) = PINS[i];
        // The first quoted string after `version =`, or after `=` alone.
        let value_at = line.find("version").map_or_else(|| line.find('='), Some);
        let rewritten = value_at.and_then(|at| {
            let open = at + line[at..].find('"')?;
            let close = open + 1 + line[open + 1..].find('"')?;
            Some(format!("{}\"{req}\"{}", &line[..open], &line[close + 1..]))
        });
        let Some(rewritten) = rewritten else {
            return Err(Error::LeptosBeta(format!(
                "{}: no version to rewrite on the `{name}` line",
                manifest.display()
            )));
        };
        seen[i] = true;
        out.push_str(&rewritten);
    }
    if let Some(i) = seen.iter().position(|s| !s) {
        return Err(Error::LeptosBeta(format!(
            "{}: no `{}` in [workspace.dependencies]",
            manifest.display(),
            PINS[i].0
        )));
    }
    fsx::write(manifest, out.as_bytes())
}

/// Prints what cargo resolved each pin to, from the copy's fresh lock file,
/// and says so when a pin is no longer a pre-release.
fn report_resolved(tree: &Path) -> Result<()> {
    let lock = fsx::read_to_string(&tree.join("Cargo.lock"))?;
    let mut released = Vec::new();
    for (name, _) in PINS {
        let versions: Vec<&str> = lock
            .split("[[package]]")
            .filter(|p| p.lines().any(|l| l.trim() == format!("name = \"{name}\"")))
            .filter_map(|p| {
                p.lines()
                    .find_map(|l| l.trim().strip_prefix("version = \""))
                    .and_then(|v| v.strip_suffix('"'))
            })
            .collect();
        eprintln!("    {name} {}", versions.join(", "));
        if versions.iter().any(|v| !v.contains('-')) {
            released.push(*name);
        }
    }
    if !released.is_empty() {
        eprintln!(
            "    note: {} resolved to a release, not a pre-release — Leptos 0.9 is out, and \
             the workspace moves to it (D10)",
            released.join(", ")
        );
    }
    Ok(())
}
