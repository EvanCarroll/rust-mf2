//! Repository automation for mf2-two (`cargo xtask <command>`).
//!
//! Network access happens only in the `*-sync` commands, and only to the
//! upstreams named in `third_party/*/PIN` (see `CLAUDE.md`, "Boundary").

mod api;
mod b12_generated;
mod b5;
mod churn;
mod ci;
mod cldr_sync;
mod cmd;
mod codegen_matrix;
mod docs;
mod docs_rs;
mod error;
mod fluent_ab;
mod fluent_migrate;
mod fsx;
mod fuzz_seed;
mod git;
mod islands_zero;
mod l4_wasi;
mod l4_web;
mod l6_web;
mod l7_web;
mod leptos_0_8;
mod locale_data;
mod msrv;
mod package;
// A test only until `cargo xtask release` (A7) runs it too.
#[cfg(test)]
mod packages;
mod pin;
mod report;
mod scenarios;
mod size;
mod spec_sync;
mod xliff_sync;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::error::{Error, Result};

/// Repository automation for mf2-two.
#[derive(Parser)]
#[command(name = "xtask", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Re-vendor third_party/message-format-wg (test/, LICENSE) from upstream at the
    /// PIN commit, or at --rev, and print the added/removed/changed tests; fetch
    /// spec/ (not redistributable) into target/xtask-cache/message-format-wg-spec,
    /// checked against the PIN's digests.
    SpecSync {
        /// Upstream commit to vendor (full sha); defaults to the PIN commit.
        #[arg(long, value_name = "SHA", conflicts_with = "check")]
        rev: Option<String>,
        /// Re-fetch the pinned commit and compare it byte-for-byte with the vendored
        /// tree; exit non-zero on any difference. Never writes into `third_party/`
        /// (the spec cache is filled, as without it).
        #[arg(long)]
        check: bool,
    },
    /// Vendor the CLDR JSON subset named in third_party/cldr-json/PIN.
    CldrSync,
    /// Regenerate crates/mf2-locale-data/data/ from the vendored CLDR JSON and the
    /// cldr-sync cache (offline; the all-locale number table needs the cache).
    LocaleData,
    /// Vendor the W3C Message Resource draft (blocked until its license is confirmed).
    ResourceSync,
    /// Vendor the XLIFF 2 core (the OASIS Standard's specification and XML
    /// schemas) named in `third_party/xliff/PIN`, each file checked against
    /// the PIN's SHA-256.
    XliffSync {
        /// List the published versions and their stages; fetch nothing else.
        #[arg(long, conflicts_with = "check")]
        list: bool,
        /// Re-fetch the pinned files and compare them byte for byte with the
        /// vendored ones; never writes into `third_party/`.
        #[arg(long)]
        check: bool,
    },
    /// Check conformance/ledger.toml against the vendored suite and write
    /// conformance/REPORT.md.
    ConformanceReport {
        /// Generate a fresh ledger (every applicable cell `xfail`) instead of checking.
        #[arg(long)]
        init: bool,
        /// With --init: overwrite an existing ledger.
        #[arg(long, requires = "init")]
        force: bool,
        /// Run the layer harnesses and turn every `xfail` cell they pass into
        /// `pass` (rewrites the ledger; review the diff).
        #[arg(long, conflicts_with_all = ["init", "report"])]
        promote: bool,
        /// Ledger to read or write [default: conformance/ledger.toml].
        #[arg(long, value_name = "PATH")]
        ledger: Option<PathBuf>,
        /// Report to write [default: conformance/REPORT.md].
        #[arg(long, value_name = "PATH", conflicts_with = "init")]
        report: Option<PathBuf>,
    },
    /// Conformance L4 on wasm32-wasip1: format every L4 case under wasmtime and
    /// require the native output byte for byte.
    L4Wasi {
        /// Also N generated cases (conformance/src/l4gen.rs), unstripped and
        /// stripped, evenly spaced over the nightly's 1,000,000.
        #[arg(long, value_name = "N")]
        generated: Option<u64>,
    },
    /// Conformance L4 in the browser for the `intl` build: format every L4
    /// case in Chromium, Firefox and `WebKit` (tools/e2e) and require the
    /// suite's expectations, or the ledger's `intl` entries, and every
    /// difference from the Rust path recorded there.
    L4Web {
        /// Engines: `all` or a comma-separated list of chromium, firefox, webkit.
        #[arg(long, default_value = "all", value_name = "ENGINES")]
        browser: String,
        /// Judge the records already in target/l4-web/ (no build, no browser run).
        #[arg(long)]
        no_run: bool,
    },
    /// Conformance L6 in the browser: render every runtime-valid suite
    /// message on the server, hydrate the page in each engine, switch to the
    /// twin locale and back (tools/e2e/checks/l6.mjs).
    L6Web {
        /// Engines: `all` or a comma-separated list of chromium, firefox, webkit.
        #[arg(long, default_value = "chromium,firefox", value_name = "ENGINES")]
        browser: String,
        /// Drive the page already in target/l6-web/ (no build).
        #[arg(long)]
        no_build: bool,
    },
    /// Conformance L7 in the browser: the suite's pages as islands and
    /// client-only, in both configurations, switched to the twin locale and
    /// back in each engine; judges the ledger's L7 columns
    /// (tools/e2e/checks/l7.mjs).
    L7Web {
        /// Engines: `all` (Chromium, Firefox, and `WebKit` where installed)
        /// or a comma-separated list.
        #[arg(long, default_value = "all", value_name = "ENGINES")]
        browser: String,
        /// Drive the pages already in target/l7-web/ (no build).
        #[arg(long)]
        no_build: bool,
        /// Tighten the ledger to the run: every `xfail` L7 cell that passes
        /// becomes `pass`, every documented degradation `degraded`.
        #[arg(long)]
        promote: bool,
    },
    /// The whole-app size gate (plans/06-size-and-perf.md §3): B1 fixed, B5
    /// per call site, and their sum at the reference scale, all measured end
    /// to end on the reference workload.
    Size {
        /// Where to build [default: target/size].
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
        /// Reuse the workloads and applications already there.
        #[arg(long)]
        keep: bool,
    },
    /// The conversions under churn (Phase 7, A5): P0.11's churning list on
    /// leptos-mf2, one row shape per variant, built as a client-only site
    /// and run in the browser; no shape may grow the heap
    /// (tools/e2e/checks/churn.mjs).
    Churn {
        /// Engines: `all` (Chromium, Firefox, and `WebKit` where installed)
        /// or a comma-separated list.
        #[arg(long, default_value = "chromium,firefox", value_name = "ENGINES")]
        browser: String,
        /// Drive the site already in target/churn/ (no build).
        #[arg(long)]
        no_build: bool,
    },
    /// The Leptos 0.8 opt-in (Phase 8, A0): `leptos-0-8` beside the default
    /// line refused with the fix named; then, on 0.8, `leptos-mf2` linted
    /// for ssr, hydrate and csr, its render, churn and `fallback_lang`
    /// tests, `mf2-axum`'s tests and conformance layer L6.
    #[command(name = "leptos-0-8")]
    Leptos08 {
        /// The negative control: the same on a copy of the tree whose glue
        /// gives `leptos-0-8` the 0.9 form of `to_html_with_buf`, which must
        /// fail.
        #[arg(long)]
        negative_control: bool,
    },
    /// The public API of the 16 published crates (Phase 9, A2): written as
    /// `crates/<name>/api.txt`, from a pinned nightly's rustdoc JSON (the
    /// libraries) and clap's command tree (`mf2-cli`).
    Api {
        /// Compare with the committed listings instead of writing them; any
        /// difference fails, naming its lines.
        #[arg(long)]
        check: bool,
    },
    /// The 16 published crates' packages (Phase 9, A4): each `.crate` audited
    /// (every file from its own crate, none a copy of `third_party/`,
    /// `plans/` or the specification cache, under 10 MB) and its file list
    /// written as `crates/<name>/package.txt`.
    Package {
        /// Compare with the committed lists instead of writing them.
        #[arg(long)]
        check: bool,
        /// Then run the packages' own tests from their unpacked `.crate`
        /// files, with crates.io patched to them.
        #[arg(long)]
        test: bool,
    },
    /// The 16 published crates' documentation built as docs.rs builds it
    /// (Phase 9, A5): each library crate's `[package.metadata.docs.rs]`
    /// features and targets, on the pinned nightly with `--cfg docsrs`,
    /// every rustdoc warning (a broken intra-doc link among them) an error,
    /// and each front page pointing to the user guide.
    #[command(name = "docs-rs")]
    DocsRs,
    /// The MSRV (Phase 9, A3): the 16 published crates checked on the
    /// `rust-version` they state, natively and for wasm32-unknown-unknown,
    /// on both Leptos lines.
    Msrv {
        /// The negative control: the release before the MSRV, which must
        /// fail.
        #[arg(long)]
        below: bool,
    },
    /// The reference application migrated from leptos-fluent (Phase 8, A4):
    /// `fluent-view` converted by `mf2 convert --from leptos-fluent`, its
    /// call sites compared with `fluent-converted` byte for byte, finished
    /// as the migration guide says, and built for the client and the
    /// server.
    FluentMigrate {
        /// Convert and compare only (no build).
        #[arg(long)]
        no_build: bool,
    },
    /// The `leptos-fluent` A/B, measured once as a snapshot (Phase 8, A5):
    /// the reference application on `leptos-fluent` and the same
    /// application migrated by `mf2 convert`, built as they ship, checked to
    /// show the same text, then sized and timed alternately in the browser
    /// (tools/e2e/checks/fluent-ab.mjs; bench/fluent-ab/README.md).
    FluentAb {
        /// Engines, comma-separated.
        #[arg(long, default_value = "chromium,firefox", value_name = "ENGINES")]
        browser: String,
        /// Fresh first visits per application per engine.
        #[arg(long, default_value_t = 10)]
        runs: u32,
        /// Measure what target/fluent-ab/ already holds (no build).
        #[arg(long)]
        no_build: bool,
        /// Also write the snapshot to bench/fluent-ab/ (a clean tree only:
        /// the snapshot names the commit it measured).
        #[arg(long)]
        snapshot: bool,
    },
    /// A server-only component costs the client nothing (Phase 7, A1): the
    /// islands example's client, built with and without one more server-only
    /// component full of call sites, must be the same size.
    IslandsZero,
    /// Rewrite the locale-output goldens (conformance/goldens/*.tsv) from a
    /// fresh render; review the diff before committing.
    Goldens,
    /// Run locally exactly what CI runs: fmt, clippy, tests, conformance report.
    Ci,
    /// Compile every code sample in the user documentation (`docs/`): the
    /// samples of each application are assembled under
    /// `target/docs/projects` and checked for the targets it runs on.
    Docs {
        /// Assemble and verify the samples (the `mf2` commands they run,
        /// the files they say were generated) without compiling them.
        #[arg(long)]
        no_build: bool,
    },
    /// Compile the module `mf2-build` generates (tools/i18n-fixture) in every
    /// feature combination of the facade, for the server and for
    /// wasm32-unknown-unknown (Phase 5a, A5).
    CodegenMatrix {
        /// Only the first two combinations of each side.
        #[arg(long)]
        quick: bool,
    },
    /// The incremental-rebuild scenarios of P0.9 on the real pipeline
    /// (Phase 5a, A9): what each kind of edit rewrites, and whether the
    /// client wasm moves.
    Scenarios {
        /// Leave the fixture edited, for looking at what happened.
        #[arg(long)]
        keep: bool,
        /// The arrangement of owner question 1: the catalogs emitted apart,
        /// in a crate only the server binary depends on.
        #[arg(long)]
        split: bool,
    },
    /// Budgets B1′ and B13 on the generated module (Phase 5b, A10): the same
    /// corpus with and without a feature it does not use, and with and
    /// without the gated measure functions.
    B12Generated,
    /// Budget B5 (Phase 5b, A6): the marginal wasm per call site, by P0.1's
    /// method — two scales of the reference workload, `tr` against the
    /// `idlit` baseline and the `dummy` bound.
    B5 {
        /// Where to generate and build [default: target/b5].
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
        /// Reuse what is already generated and built there.
        #[arg(long)]
        keep: bool,
        /// Measure the **whole mix** instead of the `String` path: `tr-view`
        /// against `idlit-view`, with a description rendering itself in the
        /// view positions (plans/14-phase-6-work-order.md A7).
        #[arg(long)]
        view: bool,
    },
    /// Write the seed corpora of the fuzz targets: `parse` (fuzz/corpus/parse/:
    /// the suite's messages and the reference workload) and `catalog`
    /// (fuzz/corpus/catalog/: their catalogs, unstripped and stripped).
    FuzzSeed,
    /// Run the reference-workload generator: `cargo run --release -p workload-gen -- ARGS`
    /// (every argument, `--help` included, goes to workload-gen).
    #[command(disable_help_flag = true)]
    GenWorkload {
        /// Passed through to workload-gen unchanged.
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<OsString>,
    },
    /// The catalog size report and budget B7 (Phase 2, A8):
    /// `cargo run --release -p catalog-bench -- size ARGS`; fails if a B7
    /// threshold fails (every argument, `--help` included, goes to catalog-bench).
    #[command(disable_help_flag = true)]
    CatalogSize {
        /// Passed through to `catalog-bench size` unchanged.
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<OsString>,
    },
}

fn run(command: Command) -> Result<()> {
    let root = fsx::repo_root();
    match command {
        Command::SpecSync { rev, check } => spec_sync::run(&root, rev.as_deref(), check),
        Command::CldrSync => cldr_sync::run(&root),
        Command::LocaleData => locale_data::run(&root),
        Command::ResourceSync => Err(Error::ResourceSyncBlocked),
        Command::XliffSync { list, check } => xliff_sync::run(&root, list, check),
        Command::ConformanceReport {
            init,
            force,
            promote,
            ledger,
            report,
        } => {
            if init {
                report::init(&root, ledger.as_deref(), force)
            } else if promote {
                report::promote(&root, ledger.as_deref())
            } else {
                report::check(&root, ledger.as_deref(), report.as_deref())
            }
        }
        Command::Ci => ci::run(&root),
        Command::Docs { no_build } => docs::run(&root, !no_build),
        Command::Size { out, keep } => size::run(&root, out, keep),
        Command::IslandsZero => islands_zero::run(&root),
        Command::FluentMigrate { no_build } => fluent_migrate::run(&root, !no_build),
        Command::FluentAb {
            browser,
            runs,
            no_build,
            snapshot,
        } => fluent_ab::run(
            &root,
            &fluent_ab::Options {
                build: !no_build,
                browser,
                runs,
                snapshot,
            },
        ),
        Command::Leptos08 { negative_control } => leptos_0_8::run(&root, negative_control),
        Command::Api { check } => api::run(&root, check),
        Command::DocsRs => docs_rs::run(&root),
        Command::Msrv { below } => msrv::run(&root, below),
        Command::Package { check, test } => package::run(&root, check, test),
        Command::Churn { browser, no_build } => churn::run(&root, &browser, !no_build),
        Command::L6Web { browser, no_build } => {
            let engines: Vec<String> = if browser == "all" {
                l6_web::ENGINES.iter().map(|e| (*e).to_owned()).collect()
            } else {
                browser.split(',').map(str::to_owned).collect()
            };
            if let Some(bad) = engines
                .iter()
                .find(|e| !l6_web::ENGINES.contains(&e.as_str()))
            {
                return Err(Error::L6(format!("unknown engine {bad:?}")));
            }
            l6_web::run(&root, &engines, !no_build)
        }
        Command::L7Web {
            browser,
            no_build,
            promote,
        } => {
            if let Some(bad) = browser
                .split(',')
                .find(|e| *e != "all" && !l7_web::ENGINES.contains(e))
            {
                return Err(Error::L7(format!("unknown engine {bad:?}")));
            }
            l7_web::run(&root, &browser, !no_build, promote)
        }
        Command::Goldens => goldens(&root),
        Command::L4Wasi { generated } => l4_wasi::run(&root, generated),
        Command::L4Web { browser, no_run } => {
            let engines: Vec<String> = if browser == "all" {
                l4_web::ENGINES.iter().map(|e| (*e).to_owned()).collect()
            } else {
                browser.split(',').map(str::to_owned).collect()
            };
            if let Some(bad) = engines
                .iter()
                .find(|e| !l4_web::ENGINES.contains(&e.as_str()))
            {
                return Err(Error::L4(format!("unknown engine {bad:?}")));
            }
            l4_web::run(&root, &engines, !no_run)
        }
        Command::CodegenMatrix { quick } => codegen_matrix::run(&root, quick),
        Command::Scenarios { keep, split } => scenarios::run(&root, keep, split),
        Command::B5 { out, keep, view } => b5::run(
            &root,
            out,
            keep,
            if view {
                b5::Mode::View
            } else {
                b5::Mode::String
            },
        ),
        Command::B12Generated => b12_generated::run(&root),
        Command::FuzzSeed => fuzz_seed::run(&root),
        Command::GenWorkload { args } => {
            let mut full: Vec<OsString> = ["run", "--release", "-p", "workload-gen", "--"]
                .into_iter()
                .map(OsString::from)
                .collect();
            full.extend(args);
            let refs: Vec<&std::ffi::OsStr> = full.iter().map(OsString::as_os_str).collect();
            cmd::run_inherit(&cmd::cargo(), &refs, &root)
        }
        Command::CatalogSize { args } => {
            let mut full: Vec<OsString> = ["run", "--release", "-p", "catalog-bench", "--", "size"]
                .into_iter()
                .map(OsString::from)
                .collect();
            full.extend(args);
            let refs: Vec<&std::ffi::OsStr> = full.iter().map(OsString::as_os_str).collect();
            cmd::run_inherit(&cmd::cargo(), &refs, &root)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask goldens`: writes every family's golden file.
fn goldens(root: &std::path::Path) -> Result<()> {
    for family in mf2_conformance::goldens::FAMILIES {
        let text = mf2_conformance::goldens::render(family).map_err(Error::L4)?;
        let path = root.join(mf2_conformance::goldens::path(family));
        fsx::write(&path, text.as_bytes())?;
        eprintln!(
            "goldens: {} ({} cases)",
            path.display(),
            text.lines().filter(|l| !l.starts_with('#')).count()
        );
    }
    Ok(())
}
