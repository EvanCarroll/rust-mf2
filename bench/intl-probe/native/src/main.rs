//! `intl-probe-native` — the build side of Phase 4 task A0: everything the
//! page needs, compiled natively (`plans/11-phase-4-work-order.md` A0).
//!
//! | Command | Writes (under `--out`, default `target/intl-probe/data`) |
//! |---|---|
//! | `speed` | `speed.{json,bin}`: the timed messages, catalogs for `en` and `pl` |
//! | `l4` | `l4.{json,bin}`: `functions/{number,integer,offset,percent,currency}.json`, one catalog per test (`mf2::compile_str`) |
//! | `plural` | `plural.json`: the 15,041 CLDR 48.2.1 samples with CLDR's and our evaluator's category |
//! | `ecma --input FILE` | `ecma.{json,bin}`: the messages of `runtime-bench numbers ecma`'s cases, 10,000 per catalog |
//! | `edge` | `edge.{json,bin}`: `:integer` rounding, `:offset` arithmetic, exact keys (`rust` vs `intl`) |
//! | `loc` | `loc.{json,bin}`: the locale-symbol cases, one catalog per panel locale |
//! | `loc-format` | `loc-rust.json`: the same cases through the Rust registry |
//! | `observe` | prints native observations the report cites |
//! | `catalog-data` (feature `number-data`, crates ≥ ef65951) | `catalog-data.md`: the LOCALE bytes per panel locale the option would drop |

mod catalogs;
mod error;
mod sets;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::error::Result;

#[derive(Parser)]
#[command(about = "Phase 4 A0: the intl probe's native side (catalogs, samples, cases)")]
struct Cli {
    /// Output directory.
    #[arg(long, global = true, default_value = "target/intl-probe/data")]
    out: PathBuf,
    /// The repository root (for `third_party/`).
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// The timed messages (`en`, `pl`).
    Speed,
    /// The L4 number files, one catalog per test.
    L4,
    /// The CLDR plural samples.
    Plural,
    /// Catalogs of `runtime-bench numbers ecma` cases (its JSONL output).
    Ecma {
        #[arg(long)]
        input: PathBuf,
    },
    /// `:integer` / `:offset` / exact-key edge cases.
    Edge,
    /// The locale-symbol cases of the panel.
    Loc,
    /// The locale-symbol cases formatted natively through the Rust registry.
    LocFormat,
    /// Native observations cited in RESULTS.md.
    Observe,
    /// The LOCALE entries (plural, number symbols and patterns) a catalog of
    /// each panel locale carries today, which the option would drop: raw
    /// bytes, and gzip -9 of a small catalog with and without them.
    #[cfg(feature = "number-data")]
    CatalogData,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("intl-probe-native: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Speed => sets::speed(&cli.out),
        Command::L4 => sets::l4(&cli.repo, &cli.out),
        Command::Plural => sets::plural(&cli.out),
        Command::Ecma { input } => sets::ecma(&input, &cli.out),
        Command::Edge => sets::edge(&cli.out),
        Command::Loc => sets::loc(&cli.out),
        Command::LocFormat => sets::loc_format(&cli.out),
        Command::Observe => sets::observe(),
        #[cfg(feature = "number-data")]
        Command::CatalogData => sets::catalog_data(&cli.out),
    }
}
