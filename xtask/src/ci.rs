//! `cargo xtask ci`: exactly what `.forgejo/workflows/ci.yml` runs, in order,
//! stopping at the first failure. No network access beyond what cargo itself
//! needs to resolve dependencies, and rustup to install the nightly the API
//! listings are made with (`cargo xtask api`) when it is missing: the
//! specification text must already be in its cache (`cargo xtask spec-sync`,
//! which CI runs first).

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
    // `--workspace` builds `mf2`'s Leptos layer with `ssr` (mf2-axum turns
    // it on through the `leptos-mf2` shim, and cargo unifies), so nothing
    // above ever compiles the **client** half: the hydration cursor, the
    // boot, the fetch. Lint it where it runs.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,hydrate",
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
        "mf2",
        "--features",
        "leptos,hydrate,static-locale",
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
        "mf2",
        "--features",
        "leptos,csr",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,csr,static-locale",
        "--",
        "-D",
        "warnings",
    ],
    // `mark-fallback-lang` (Phase 7 A14) compiles a second shape of the text
    // glue — the adopted wrapper, the fitted span — that no other step
    // builds. Lint its client half with and without `static-locale`, whose
    // unregistered nodes fit the wrapper through a rebuild.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,hydrate,mark-fallback-lang",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,csr,static-locale,mark-fallback-lang",
        "--",
        "-D",
        "warnings",
    ],
    // `fn-datetime` (Phase 8 A7) compiles the reader's time zone: the boot's
    // correction and the glue's hydration queue for `hydrate` — with and
    // without `static-locale`, where the queue, not the registry, holds the
    // nodes — and the mount's zone for `csr`. The server's half is in
    // `--workspace`, which unifies the feature in.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,hydrate,fn-datetime",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,hydrate,fn-datetime,static-locale,mark-fallback-lang",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "leptos,csr,fn-datetime",
        "--",
        "-D",
        "warnings",
    ],
    // The call-site core with no mode, on the client target: what a
    // Leptos-free client (the size workload `tr`) compiles.
    &[
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "mf2",
        "--features",
        "host-web",
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
        "mf2",
        "--features",
        "leptos,csr,compile,host-std",
        "--test",
        "churn",
    ],
    // The server's half of `mark-fallback-lang`, with its test and the
    // test's catalogs that borrow; the browser's is `demo.mjs`.
    &[
        "clippy",
        "-p",
        "mf2",
        "--features",
        "leptos,ssr,compile,mark-fallback-lang",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "test",
        "-p",
        "mf2",
        "--features",
        "leptos,ssr,mark-fallback-lang",
        "--test",
        "fallback_lang",
    ],
    // The native module (Phase 10 B2) as a native application builds it,
    // with no Leptos layer beside it: `--workspace` always unifies `ssr`
    // into `mf2` (mf2-axum turns it on), so nothing above compiles `native`
    // alone. Its tests and the matcher's run here too.
    &[
        "clippy",
        "-p",
        "mf2",
        "--features",
        "native,compile",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ],
    &[
        "test",
        "-p",
        "mf2",
        "--features",
        "native,compile",
        "--lib",
        "--test",
        "native",
    ],
    // The `leptos-mf2` shim with no mode of its own while `mf2` has one: an
    // application that names its mode on `mf2` alone, beside a crate on the
    // shim's `leptos` (as `conformance/l7-web/sets/*` are). In 1.x `mf2/ssr`
    // turned on `leptos-mf2/ssr`; the layer's 1.x paths must still resolve.
    &[
        "test",
        "-p",
        "leptos-mf2",
        "--features",
        "leptos,mf2/ssr",
        "--test",
        "layer",
    ],
    // `mf2-resource`'s `serde` feature is optional and nothing in the
    // workspace turns it on, so `--workspace` alone never builds `src/json.rs`
    // or runs `tests/json.rs`.
    &["test", "-p", "mf2-resource", "--features", "serde"],
];

pub(crate) fn run(root: &Path) -> Result<()> {
    // The spec text is not vendored (upstream #1112): without the cache the
    // first build script to read it would stop the run minutes in. Say so now.
    mf2_conformance::spec::spec_dir(root)?;
    let cargo = cargo();
    for step in STEPS {
        let shown = format!("cargo {}", step.join(" "));
        eprintln!("==> {shown}");
        let args: Vec<&OsStr> = step.iter().map(OsStr::new).collect();
        run_inherit(&cargo, &args, root).map_err(|_| Error::CiStepFailed(shown))?;
    }
    // What plans/19 §3 refuses: each misuse of `mf2`'s features is `mf2`'s
    // one sentence, whichever crate cargo compiles first.
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
