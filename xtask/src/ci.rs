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
//! `parser-gate`, and `tui-gate` (`cargo xtask tui-gate --gate`: the terminal
//! UI's allocations per frame and stripped size on every push; its time
//! against the 1.x binary nightly).

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

pub(crate) fn run(root: &Path) -> Result<()> {
    // The spec text is not vendored (upstream #1112): without the cache the
    // first build script to read it would stop the run minutes in. Say so now.
    mf2_conformance::spec::spec_dir(root)?;
    let cargo = cargo();
    for step in steps() {
        let shown = format!("cargo {}", step.join(" "));
        eprintln!("==> {shown}");
        let args: Vec<&OsStr> = step.iter().map(OsStr::new).collect();
        run_inherit(&cargo, &args, root).map_err(|_| Error::CiStepFailed(shown))?;
    }
    // What `mf2` refuses: each misuse of its features is `mf2`'s
    // one sentence, whichever crate cargo compiles first; and what it
    // refuses only for the browser compiles on the host.
    eprintln!("==> cargo xtask refusals");
    crate::refusals::run(root)?;
    // The documentation's samples, assembled and the `mf2` commands they
    // run checked; compiling them is the `docs` job's (`cargo xtask docs`).
    eprintln!("==> cargo xtask docs --no-build");
    crate::docs::run(root, false)?;
    eprintln!("==> cargo xtask conformance-report");
    report::check(root, None, None)?;
    // The published crates' public API against the committed `api.txt`
    // (Phase 9 A2): a change to it is committed with its listing.
    eprintln!("==> cargo xtask api --check");
    crate::api::run(root, true)?;
    // What crates.io would receive (Phase 9 A4): each package audited and
    // its file list against the committed `package.txt`. The packages' own
    // tests from their `.crate` files (`--test`) are `cargo xtask release`'s.
    eprintln!("==> cargo xtask package --check");
    crate::package::run(root, true, false)?;
    eprintln!("==> ci: all steps passed");
    Ok(())
}
