//! `catalog-bench`: the Phase 2 catalog measurements (see `README.md`).
//!
//! ```sh
//! cargo run --release -p catalog-bench -- size            # A8: B7 (exit 1 if a threshold fails)
//! cargo run --release -p catalog-bench -- bench --gate    # A9: reader cost (exit 1 if the gate fails)
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use catalog_bench::bench::{self, MIN_RUNS, Settings};
use catalog_bench::compress::Gz;
use catalog_bench::{Error, Result, corpus, repo_root, size};
use clap::{Args, Parser, Subcommand};

/// Phase 2 measurements of the `.mf2b` catalog on the reference workload.
#[derive(Debug, Parser)]
#[command(name = "catalog-bench", version)]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Debug, Subcommand)]
enum Mode {
    /// A8: catalog sizes and the B7 checks (exit 1 if a B7 threshold fails).
    Size {
        /// The gzip implementation of the B7 verdicts.
        #[arg(long, value_enum, default_value_t = Gz::Gnu)]
        gz: Gz,
        /// Also write the catalogs here: `<tag>.mf2b` (stripped),
        /// `<tag>.full.mf2b` (unstripped) and `manifest.mf2m`.
        #[arg(long, value_name = "DIR")]
        emit: Option<PathBuf>,
        #[command(flatten)]
        out: Out,
    },
    /// A9: the native reader bench (with --gate, exit 1 if the gate fails).
    Bench {
        /// Timing samples per row (the gate requires at least 31).
        #[arg(long, default_value_t = MIN_RUNS)]
        runs: usize,
        /// Minimum timed duration of one sample, in milliseconds.
        #[arg(long, default_value_t = 10)]
        min_sample_ms: u64,
        /// Warm-up per row, in milliseconds.
        #[arg(long, default_value_t = 50)]
        warmup_ms: u64,
        /// `Catalog::new` / `clone` calls per timed batch.
        #[arg(long, default_value_t = 32)]
        batch: usize,
        /// Apply the gate: exit 1 if a rule fails. Requires a release build
        /// and at least 31 runs.
        #[arg(long)]
        gate: bool,
        #[command(flatten)]
        out: Out,
    },
}

#[derive(Debug, Args)]
struct Out {
    /// Markdown report [default: <repo>/target/catalog-bench/<mode>.md].
    #[arg(long)]
    md: Option<PathBuf>,
    /// JSON report [default: <repo>/target/catalog-bench/<mode>.json].
    #[arg(long)]
    json: Option<PathBuf>,
}

impl Out {
    fn write(self, mode: &str, markdown: &str, json: &str) -> Result<()> {
        let dir = repo_root().join("target").join("catalog-bench");
        let md = self.md.unwrap_or_else(|| dir.join(format!("{mode}.md")));
        let js = self
            .json
            .unwrap_or_else(|| dir.join(format!("{mode}.json")));
        write(&md, markdown)?;
        write(&js, json)?;
        print!("{markdown}");
        eprintln!("wrote {} and {}", md.display(), js.display());
        Ok(())
    }
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, text)?;
    Ok(())
}

fn verdict(passed: bool, what: &str) -> ExitCode {
    if passed {
        eprintln!("{what}: pass");
        ExitCode::SUCCESS
    } else {
        eprintln!("{what}: FAIL");
        ExitCode::FAILURE
    }
}

fn main_inner(cli: Cli) -> Result<ExitCode> {
    let progress = |line: &str| eprintln!("  {line}");
    match cli.mode {
        Mode::Size { gz, emit, out } => {
            if cfg!(debug_assertions) {
                eprintln!(
                    "note: debug build — sizes are exact, but brotli 11 is slow; use --release"
                );
            }
            let report = size::run(&repo_root(), gz, emit.as_deref(), progress)?;
            out.write("size", &report.to_markdown(), &report.to_json()?)?;
            Ok(verdict(report.passed, "B7"))
        }
        Mode::Bench {
            runs,
            min_sample_ms,
            warmup_ms,
            batch,
            gate,
            out,
        } => {
            if gate {
                if cfg!(debug_assertions) {
                    return Err(Error::Settings(
                        "--gate needs a release build (`cargo run --release -p catalog-bench`)"
                            .to_owned(),
                    ));
                }
                if runs < MIN_RUNS {
                    return Err(Error::Settings(format!("--gate needs --runs ≥ {MIN_RUNS}")));
                }
            }
            if cfg!(debug_assertions) {
                eprintln!("warning: debug build — timings are meaningless; use --release");
            }
            let settings = Settings {
                runs,
                min_sample: Duration::from_millis(min_sample_ms),
                warmup: Duration::from_millis(warmup_ms),
                batch,
            };
            progress("building the catalogs");
            let (_, built) = corpus::build(&corpus::generate(&repo_root())?, false)?;
            let report = bench::run(&built, &settings, progress)?;
            out.write("bench", &report.to_markdown(), &report.to_json()?)?;
            Ok(if gate {
                verdict(report.passed, "gate")
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}

fn main() -> ExitCode {
    match main_inner(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("catalog-bench: {e}");
            ExitCode::from(2)
        }
    }
}
