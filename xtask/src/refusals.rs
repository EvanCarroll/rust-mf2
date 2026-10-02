//! `cargo xtask refusals`: the combinations of `mf2`'s features the native
//! and terminal design refuses, from both sides (Phase 10: B1's review
//! fixes, and the browser-only refusal).
//!
//! **Refused.** Each combination of `mf2`'s features it refuses is a
//! compile error that says what to write, and the only error the user
//! reads. Each case runs `cargo check` and must fail. Its one error must
//! carry `mf2`'s sentence, whichever crate cargo compiles first: with two
//! modes and a Leptos line, that is the line's helper crate
//! (`mf2-leptos-ui-0-9` or `-0-8`), which `mf2` depends on and which
//! therefore says `mf2`'s words. Every other `error` line must be cargo's
//! own "could not compile".
//!
//! **Compiled.** What it refuses only when compiling for the browser
//! (`wasm32`) compiles on the host: cargo unifies features across the
//! packages it builds together, so `cargo check --workspace` over a browser
//! client and a native application turns both on, as 1.x allowed. Each
//! host case runs `cargo check` natively and must pass.
//!
//! Both sides are rows of `feature_sets::SETS` (`plan/01` §6.3): a refused
//! row carries the sentence, a host row beside it the combination that must
//! compile. A feature that joins the refusals adds a row on each side:
//! `ratatui`, which implies `native`, has a sentence of its own (Phase 10
//! B3), so that the one error names what an application turned on, and
//! `axum` adds its own (D1).

use std::path::Path;
use std::process::{Command, Output};

use crate::cmd::cargo;
use crate::error::{Error, Result};
use crate::feature_sets::{self, Set};

fn fail(message: impl Into<String>) -> Error {
    Error::Refusals(message.into())
}

/// `cargo check` with `args`, in `root`; `shown` names it in an error.
fn check(root: &Path, args: &[&str], shown: &str) -> Result<Output> {
    Command::new(cargo())
        .arg("check")
        .args(args)
        .args(["--color", "never"])
        .current_dir(root)
        .output()
        .map_err(|source| Error::Spawn {
            program: shown.to_owned(),
            source,
        })
}

/// The `error` lines of a cargo run, but cargo's own "could not compile".
fn errors(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| line.starts_with("error"))
        .filter(|line| !line.starts_with("error: could not compile"))
        .map(str::to_owned)
        .collect()
}

/// `cargo check` arguments for a set: its selection, then its target.
fn args(set: &Set) -> Vec<&'static str> {
    let mut args = set.selection();
    if let Some(triple) = set.target.triple() {
        args.push("--target");
        args.push(triple);
    }
    args
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let sets = feature_sets::used_by(Set::refusals);
    let (refused, host): (Vec<_>, Vec<_>) =
        sets.into_iter().partition(|set| set.refused().is_some());
    for set in &refused {
        let args = args(set);
        let shown = format!("cargo check {}", args.join(" "));
        let says = set.refused().unwrap_or_default();
        eprintln!("==> refused: {} ({shown})", set.what);
        let output = check(root, &args, &shown)?;
        if output.status.success() {
            return Err(fail(format!("`{shown}` compiled; {} must not", set.what)));
        }
        let found = errors(&output);
        match found.as_slice() {
            [only] if only.contains(says) => eprintln!("    {only}"),
            _ => {
                return Err(fail(format!(
                    "`{shown}` fails, but not with {says:?} alone; its errors:\n{}",
                    found.join("\n")
                )));
            }
        }
    }
    for set in &host {
        let args = args(set);
        let shown = format!("cargo check {}", args.join(" "));
        eprintln!("==> compiles: {} ({shown})", set.what);
        let output = check(root, &args, &shown)?;
        if !output.status.success() {
            return Err(fail(format!(
                "`{shown}` fails; {} must compile, as only a browser build refuses it; \
                 its errors:\n{}",
                set.what,
                errors(&output).join("\n")
            )));
        }
    }
    eprintln!(
        "==> refusals: each of the {} is mf2's one sentence, and the {} host combinations compile",
        refused.len(),
        host.len()
    );
    Ok(())
}
