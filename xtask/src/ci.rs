//! `cargo xtask ci`: exactly what `.forgejo/workflows/ci.yml` runs, in order,
//! stopping at the first failure. No network access beyond what cargo itself
//! needs to resolve dependencies.

use std::ffi::OsStr;
use std::path::Path;

use crate::cmd::{cargo, run_inherit};
use crate::error::{Error, Result};
use crate::report;

/// The cargo invocations, in order.
const STEPS: &[&[&str]] = &[
    &["fmt", "--all", "--check"],
    &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ],
    // The `intl` client option's code compiles only for wasm32-unknown-unknown
    // (plans/03-runtime.md §5.3): lint it there, or nothing does.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2-runtime",
        "-p",
        "mf2-fn-number",
        "-p",
        "mf2-host-web",
        "--features",
        "mf2-fn-number/intl,mf2-host-web/intl",
        "--",
        "-D",
        "warnings",
    ],
    // `--workspace` builds `leptos-mf2` with `ssr` (mf2-axum turns it on and
    // cargo unifies), so nothing above ever compiles the **client** half:
    // the hydration cursor, the boot, the fetch. Lint it where it runs.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "leptos-mf2",
        "--no-default-features",
        "--features",
        "hydrate",
        "--",
        "-D",
        "warnings",
    ],
    // `static-locale` (strategy C, the islands default) changes what the
    // registry and the glue compile; nothing else builds it, and Phase 7
    // found it had rotted — a signal-valued argument that never updated.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "leptos-mf2",
        "--no-default-features",
        "--features",
        "hydrate,static-locale",
        "--",
        "-D",
        "warnings",
    ],
    // `csr` (Phase 7 A2) has a boot of its own — the index, the stored
    // locale, `navigator.languages` — and, with `static-locale`, a switch
    // that reloads rather than writing a cookie. Nothing else builds either.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "leptos-mf2",
        "--no-default-features",
        "--features",
        "csr",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "leptos-mf2",
        "--no-default-features",
        "--features",
        "csr,static-locale",
        "--",
        "-D",
        "warnings",
    ],
    &["test", "--workspace"],
    // The client half's conversions under churn (Phase 7 A5): natively, with
    // `csr`, which `--workspace` never builds (it unifies `ssr`). The browser
    // measurement of every row shape is `cargo xtask churn`.
    &[
        "test",
        "-p",
        "leptos-mf2",
        "--no-default-features",
        "--features",
        "csr",
        "--test",
        "churn",
    ],
    // `mf2-resource`'s `serde` feature is optional and nothing in the
    // workspace turns it on, so `--workspace` alone never builds `src/json.rs`
    // or runs `tests/json.rs`.
    &["test", "-p", "mf2-resource", "--features", "serde"],
];

pub(crate) fn run(root: &Path) -> Result<()> {
    let cargo = cargo();
    for step in STEPS {
        let shown = format!("cargo {}", step.join(" "));
        eprintln!("==> {shown}");
        let args: Vec<&OsStr> = step.iter().map(OsStr::new).collect();
        run_inherit(&cargo, &args, root).map_err(|_| Error::CiStepFailed(shown))?;
    }
    // The documentation's samples, assembled and the `mf2` commands they
    // run checked; compiling them is the `docs` job's (`cargo xtask docs`).
    eprintln!("==> cargo xtask docs --no-build");
    crate::docs::run(root, false)?;
    eprintln!("==> cargo xtask conformance-report");
    report::check(root, None, None)?;
    eprintln!("==> ci: all steps passed");
    Ok(())
}
