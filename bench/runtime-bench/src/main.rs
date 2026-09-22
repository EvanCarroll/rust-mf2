//! `runtime-bench`: Phase 3's runtime measurements (see `README.md`).
//!
//! ```sh
//! cargo run --release -p runtime-bench -- numbers corpus 100000   # A5b: one line per case
//! cargo run --release -p runtime-bench -- numbers ecma 100000 | node bench/runtime-bench/ecma-diff.cjs
//! cargo run --release -p runtime-bench -- numbers speed           # A5b: ns and allocations per format
//! bash bench/runtime-bench/number-ab.sh                           # A5b: both backends, compared
//! cargo run --release -p runtime-bench -- b10 --gate --md bench/runtime-bench/B10-P3.md \
//!     --json bench/runtime-bench/b10-p3.json                       # A11: B10
//! ```

mod b10;
mod error;
mod numbers;

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};

/// Phase 3's runtime measurements.
#[derive(Debug, Parser)]
#[command(name = "runtime-bench", version)]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Debug, Subcommand)]
enum Mode {
    /// The numeric semantics: P0.5's random corpus through the runtime.
    Numbers {
        #[command(subcommand)]
        what: Numbers,
    },
    /// B10: formatting speed and allocations on the four production catalogs.
    B10 {
        /// Exit 1 unless B10 holds on `en`.
        #[arg(long)]
        gate: bool,
        /// Samples per row.
        #[arg(long, default_value_t = 31)]
        runs: usize,
        /// Minimum duration of one sample, ms.
        #[arg(long, default_value_t = 20)]
        min_sample_ms: u64,
        /// Warm-up per row, ms.
        #[arg(long, default_value_t = 200)]
        warmup_ms: u64,
        /// Only this row (for profiling).
        #[arg(long, value_enum)]
        only: Option<b10::Op>,
        /// Write the Markdown report here.
        #[arg(long)]
        md: Option<PathBuf>,
        /// Write the JSON report here.
        #[arg(long)]
        json: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum Numbers {
    /// One line per case (display, errors, cardinal and ordinal selection):
    /// the two decimal backends must print the same lines.
    Corpus {
        /// Cases (P0.5 measured 100,000).
        #[arg(default_value_t = 100_000)]
        n: usize,
    },
    /// P0.5's JSON lines for `ecma-diff.cjs` (ECMA-402 `Intl.NumberFormat`).
    Ecma {
        #[arg(default_value_t = 100_000)]
        n: usize,
    },
    /// Nanoseconds, allocations and bytes per format of the first N cases.
    Speed {
        #[arg(default_value_t = 10_000)]
        n: usize,
        /// Timed passes (the median is reported).
        #[arg(long, default_value_t = 21)]
        runs: usize,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let what = match cli.mode {
        Mode::Numbers { what } => what,
        Mode::B10 {
            gate,
            runs,
            min_sample_ms,
            warmup_ms,
            only,
            md,
            json,
        } => {
            let settings = b10::Settings {
                runs,
                min_sample: Duration::from_millis(min_sample_ms),
                warmup: Duration::from_millis(warmup_ms),
                only,
            };
            return match run_b10(settings, md.as_ref(), json.as_ref()) {
                Ok(pass) if pass || !gate => ExitCode::SUCCESS,
                Ok(_) => ExitCode::FAILURE,
                Err(e) => {
                    eprintln!("runtime-bench: {e}");
                    ExitCode::FAILURE
                }
            };
        }
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let backend = if cfg!(feature = "fixed-decimal") {
        "fixed_decimal"
    } else {
        "own buffer"
    };
    let ok = match what {
        Numbers::Corpus { n } => numbers::corpus(n)
            .iter()
            .all(|c| writeln!(out, "{}", numbers::line(c)).is_ok()),
        Numbers::Ecma { n } => numbers::corpus(n)
            .iter()
            .all(|c| writeln!(out, "{}", numbers::ecma_json(c)).is_ok()),
        Numbers::Speed { n, runs } => {
            let cases = numbers::corpus(n);
            let (ns, allocs, bytes) = numbers::speed(&cases, runs);
            writeln!(
                out,
                "{backend}: {ns:.1} ns per format, {allocs:.3} allocations and {bytes:.1} bytes per format ({n} cases, median of {runs})"
            )
            .is_ok()
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs B10, prints the Markdown report, writes the files; `Ok(true)` when
/// every gate rule passes.
fn run_b10(
    settings: b10::Settings,
    md: Option<&PathBuf>,
    json: Option<&PathBuf>,
) -> error::Result<bool> {
    let report = b10::run(settings)?;
    let text = b10::markdown(&report);
    print!("{text}");
    let write = |path: &PathBuf, bytes: &[u8]| {
        std::fs::write(path, bytes).map_err(|source| error::Error::Write {
            path: path.clone(),
            source,
        })
    };
    if let Some(p) = md {
        write(p, text.as_bytes())?;
    }
    if let Some(p) = json {
        write(p, serde_json::to_string_pretty(&report)?.as_bytes())?;
    }
    Ok(report.checks.iter().all(|c| c.pass || !c.gated))
}
