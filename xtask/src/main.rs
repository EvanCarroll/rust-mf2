//! Repository automation for mf2-two (`cargo xtask <command>`).
//!
//! Network access happens only in the `*-sync` commands, and only to the
//! upstreams named in `third_party/*/PIN` (see `CLAUDE.md`, "Boundary").

mod ci;
mod cldr_sync;
mod cmd;
mod codegen_matrix;
mod error;
mod fsx;
mod fuzz_seed;
mod git;
mod l4_wasi;
mod l4_web;
mod locale_data;
mod pin;
mod report;
mod spec_sync;

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
    /// Re-vendor third_party/message-format-wg (spec/, test/, LICENSE) from upstream
    /// at the PIN commit, or at --rev, and print the added/removed/changed tests.
    SpecSync {
        /// Upstream commit to vendor (full sha); defaults to the PIN commit.
        #[arg(long, value_name = "SHA", conflicts_with = "check")]
        rev: Option<String>,
        /// Re-fetch the pinned commit and compare it byte-for-byte with the vendored
        /// tree; exit non-zero on any difference. Never writes into `third_party/`.
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
    /// Rewrite the locale-output goldens (conformance/goldens/*.tsv) from a
    /// fresh render; review the diff before committing.
    Goldens,
    /// Run locally exactly what CI runs: fmt, clippy, tests, conformance report.
    Ci,
    /// Compile the module `mf2-build` generates (tools/i18n-fixture) in every
    /// feature combination of the facade, for the server and for
    /// wasm32-unknown-unknown (Phase 5a, A5).
    CodegenMatrix {
        /// Only the first two combinations of each side.
        #[arg(long)]
        quick: bool,
    },
    /// Size gate (Phase 5; not implemented yet).
    Size,
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
        Command::Size => Err(Error::SizeNotImplemented),
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
