//! `runtime-bench`: Phase 3's runtime measurements (see `README.md`).
//!
//! ```sh
//! cargo run --release -p runtime-bench -- numbers corpus 100000   # A5b: one line per case
//! cargo run --release -p runtime-bench -- numbers ecma 100000 | node bench/runtime-bench/ecma-diff.cjs
//! cargo run --release -p runtime-bench -- numbers speed           # A5b: ns and allocations per format
//! bash bench/runtime-bench/number-ab.sh                           # A5b: both backends, compared
//! ```

mod numbers;

use std::io::Write as _;
use std::process::ExitCode;

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
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let Mode::Numbers { what } = cli.mode;
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
