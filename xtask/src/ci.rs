//! `cargo xtask ci`: exactly what `.forgejo/workflows/ci.yml` runs, in order,
//! stopping at the first failure. No network access beyond what cargo itself
//! needs to resolve dependencies, and rustup to install the nightly the API
//! listings are made with (`cargo xtask api`) when it is missing: the
//! specification text must already be in its cache (`cargo xtask spec-sync`,
//! which CI runs first).
//!
//! The feature sets the steps build are not listed here: they are rows of
//! `feature_sets::SETS` (`plan/01` §6.3), which `codegen-matrix`, `msrv` and
//! `refusals` read too. The browser sets run first, then the workspace's
//! tests, then the host sets.
//!
//! The release-build gates are jobs of their own beside it, not steps:
//! `parser-vs-ox`, and `tui-allocs-vs-trippy` (`cargo xtask tui-allocs-vs-trippy --gate`: the terminal
//! UI's allocations per frame and stripped size on every push; its time
//! against the 1.x binary nightly).
//!
//! `cargo xtask ci --compile` runs the cargo steps with nothing run: `fmt
//! --check` and every `clippy` step as they are, every `test` step with
//! `--no-run`, and none of the closing checks (`plan/09`, 16.0; the compile
//! phase runs it). `cargo xtask ci --keep-going` runs every step even after
//! one fails, gives `cargo test` `--no-fail-fast`, and lists the failed steps
//! at the end with exit 1, so that one run finds every failure (the test
//! phase runs it). Plain `cargo xtask ci` is what CI runs: it stops at the
//! first failure.

use std::ffi::OsStr;
use std::path::Path;

use crate::cmd::{cargo, run_inherit};
use crate::error::{Error, Result};
use crate::feature_sets::{self, CiStep, Set, Target};
use crate::report;

/// The cargo invocations the workspace's own lists give, in order; the rest
/// come from the feature-set table.
const FIRST: &[&[&str]] = &[
    &["fmt", "--all", "--check"],
    &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ],
];

/// `cargo clippy` for a set, with warnings denied; `--all-targets` on the
/// host, where the tests and examples compile.
fn clippy(set: &'static Set) -> Vec<&'static str> {
    let mut args = vec!["clippy"];
    if let Some(triple) = set.target.triple() {
        args.push("--target");
        args.push(triple);
    }
    args.extend(set.selection());
    if set.target == Target::Host {
        args.push("--all-targets");
    }
    args.extend(["--", "-D", "warnings"]);
    args
}

/// `cargo test` for a set, with the targets only this set builds.
fn test(set: &'static Set, targets: &'static [&'static str]) -> Vec<&'static str> {
    let mut args = vec!["test"];
    args.extend(set.selection());
    args.extend(targets);
    args
}

/// How `cargo xtask ci` runs; the default is what CI runs.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Mode {
    /// Compile only: every `cargo test` gets `--no-run`, and the closing
    /// checks (`refusals`, `docs`, `conformance-report`, `api`, `package`)
    /// are left out.
    pub(crate) compile: bool,
    /// Every step runs, a failed one does not stop the rest, `cargo test`
    /// gets `--no-fail-fast`, and the failed steps are listed at the end.
    pub(crate) keep_going: bool,
}

/// `step` as `mode` runs it: a `cargo test` step gets its flags straight
/// after `test`, before the selection and the targets.
fn adjust(mut step: Vec<&'static str>, mode: Mode) -> Vec<&'static str> {
    if step.first() == Some(&"test") {
        if mode.keep_going {
            step.insert(1, "--no-fail-fast");
        }
        if mode.compile {
            step.insert(1, "--no-run");
        }
    }
    step
}

/// The cargo invocations, in order: the workspace's fmt and lint, then every
/// browser set, then the workspace's tests, then every host set.
fn steps() -> Vec<Vec<&'static str>> {
    let mut steps: Vec<Vec<&'static str>> = FIRST.iter().map(|step| step.to_vec()).collect();
    let sets = feature_sets::used_by(|set| set.ci().is_some());
    let push = |steps: &mut Vec<Vec<&'static str>>, target: Target| {
        for set in sets.iter().filter(|set| set.target == target) {
            for step in set.ci().unwrap_or_default() {
                steps.push(match step {
                    CiStep::Clippy => clippy(set),
                    CiStep::Test(targets) => test(set, targets),
                });
            }
        }
    };
    push(&mut steps, Target::Wasm);
    steps.push(vec!["test", "--workspace"]);
    push(&mut steps, Target::Host);
    steps
}

pub(crate) fn run(root: &Path, mode: Mode) -> Result<()> {
    // The spec text is not vendored (upstream #1112): without the cache the
    // first build script to read it would stop the run minutes in. Say so now.
    mf2_conformance::spec::spec_dir(root)?;
    let cargo = cargo();
    // A failed step stops the run, or, with `--keep-going`, is listed at the
    // end and the run goes on.
    let mut failed: Vec<String> = Vec::new();
    let mut check = |shown: &str, result: Result<()>| -> Result<()> {
        match result {
            Err(e) if mode.keep_going => {
                eprintln!("==> failed: {shown}: {e}");
                failed.push(shown.to_owned());
                Ok(())
            }
            other => other,
        }
    };
    for step in steps() {
        let step = adjust(step, mode);
        let shown = format!("cargo {}", step.join(" "));
        eprintln!("==> {shown}");
        let args: Vec<&OsStr> = step.iter().map(OsStr::new).collect();
        let result =
            run_inherit(&cargo, &args, root).map_err(|_| Error::CiStepFailed(shown.clone()));
        check(&shown, result)?;
    }
    if !mode.compile {
        // What `mf2` refuses: each misuse of its features is `mf2`'s
        // one sentence, whichever crate cargo compiles first; and what it
        // refuses only for the browser compiles on the host.
        eprintln!("==> cargo xtask refusals");
        check("cargo xtask refusals", crate::refusals::run(root))?;
        // The documentation's samples, assembled and the `mf2` commands they
        // run checked; compiling them is the `docs` job's (`cargo xtask docs`).
        eprintln!("==> cargo xtask docs --no-build");
        check("cargo xtask docs --no-build", crate::docs::run(root, false))?;
        eprintln!("==> cargo xtask conformance-report");
        check(
            "cargo xtask conformance-report",
            report::check(root, None, None),
        )?;
        // The published crates' public API against the committed `api.txt`
        // (Phase 9 A2): a change to it is committed with its listing.
        eprintln!("==> cargo xtask api --check");
        check("cargo xtask api --check", crate::api::run(root, true))?;
        // What crates.io would receive (Phase 9 A4): each package audited and
        // its file list against the committed `package.txt`. The packages' own
        // tests from their `.crate` files (`--test`) are `cargo xtask release`'s.
        eprintln!("==> cargo xtask package --check");
        check(
            "cargo xtask package --check",
            crate::package::run(root, true, false),
        )?;
    }
    if !failed.is_empty() {
        eprintln!("==> ci --keep-going: {} step(s) failed:", failed.len());
        for step in &failed {
            eprintln!("    {step}");
        }
        return Err(Error::CiStepsFailed(failed));
    }
    if mode.compile {
        eprintln!("==> ci --compile: every step compiled");
    } else {
        eprintln!("==> ci: all steps passed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Mode, adjust, steps};

    #[test]
    fn compile_mode_runs_no_test() {
        let mode = Mode {
            compile: true,
            ..Mode::default()
        };
        let all: Vec<_> = steps().into_iter().map(|s| adjust(s, mode)).collect();
        assert!(all.iter().any(|s| s.first() == Some(&"clippy")));
        for step in all.iter().filter(|s| s.first() == Some(&"test")) {
            assert_eq!(step.get(1), Some(&"--no-run"), "{step:?}");
        }
        assert_eq!(all.first(), steps().first(), "fmt --check is unchanged");
    }

    #[test]
    fn plain_ci_is_unchanged() {
        let plain: Vec<_> = steps()
            .into_iter()
            .map(|s| adjust(s, Mode::default()))
            .collect();
        assert_eq!(plain, steps());
    }

    #[test]
    fn keep_going_runs_every_test() {
        let mode = Mode {
            keep_going: true,
            ..Mode::default()
        };
        for step in steps().into_iter().map(|s| adjust(s, mode)) {
            if step.first() == Some(&"test") {
                assert_eq!(step.get(1), Some(&"--no-fail-fast"), "{step:?}");
            }
        }
    }
}
