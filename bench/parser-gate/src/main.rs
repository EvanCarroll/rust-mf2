//! `parser-gate`: runs the D1 parser benchmark and, with `--gate`, applies the
//! gate. Run it in the release profile:
//!
//! ```sh
//! cargo run --release -p parser-gate -- [--gate] [--runs 31] …
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use parser_gate::corpus::{self, CorpusId};
use parser_gate::gate::{MIN_RUNS, Status};
use parser_gate::measure::Settings;
use parser_gate::{Error, Plan, Result, correctness, repo_root, run};

/// The D1 parser gate: `mf2-syntax` against the `ox_mf2_parser` baseline, on
/// the committed corpora.
#[derive(Debug, Parser)]
#[command(name = "parser-gate", version)]
struct Cli {
    /// Timing samples per row and parser (the gate requires at least 30).
    #[arg(long, default_value_t = 31)]
    runs: usize,

    /// Minimum duration of one sample, in milliseconds (sets the passes per sample).
    #[arg(long, default_value_t = 20)]
    min_sample_ms: u64,

    /// Warm-up per row and parser, in milliseconds.
    #[arg(long, default_value_t = 100)]
    warmup_ms: u64,

    /// Measure only this corpus (repeatable) [default: all three].
    #[arg(long = "corpus", value_enum)]
    corpora: Vec<CorpusId>,

    /// Directory with suite.json and workload-1600.json [default: <repo>/bench/corpora].
    #[arg(long)]
    corpora_dir: Option<PathBuf>,

    /// The vendored WG suite's test/tests directory
    /// [default: <repo>/third_party/message-format-wg/test/tests].
    #[arg(long)]
    suite_dir: Option<PathBuf>,

    /// Markdown report [default: <repo>/target/parser-gate/report.md].
    #[arg(long)]
    md: Option<PathBuf>,

    /// JSON report [default: <repo>/target/parser-gate/report.json].
    #[arg(long)]
    json: Option<PathBuf>,

    /// Apply the gate: exit 1 if any rule fails. Requires a release build, all
    /// three corpora and at least 30 runs. With ox alone: "baseline only", exit 0.
    #[arg(long)]
    gate: bool,
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, text)?;
    Ok(())
}

fn main_inner(cli: Cli) -> Result<ExitCode> {
    let corpora = if cli.corpora.is_empty() {
        CorpusId::ALL.to_vec()
    } else {
        let mut c = cli.corpora.clone();
        c.sort_unstable();
        c.dedup();
        c
    };
    if cli.gate {
        if cfg!(debug_assertions) {
            return Err(Error::Settings(
                "--gate needs a release build (`cargo run --release -p parser-gate`)".to_owned(),
            ));
        }
        if cli.runs < MIN_RUNS {
            return Err(Error::Settings(format!("--gate needs --runs ≥ {MIN_RUNS}")));
        }
        if corpora.len() != CorpusId::ALL.len() {
            return Err(Error::Settings(
                "--gate measures every corpus; drop --corpus".to_owned(),
            ));
        }
    }
    if cfg!(debug_assertions) {
        eprintln!("warning: debug build — timings are meaningless; use --release");
    }

    let plan = Plan {
        corpora_dir: cli.corpora_dir.unwrap_or_else(corpus::default_dir),
        suite_dir: cli.suite_dir.unwrap_or_else(correctness::default_suite_dir),
        corpora,
        settings: Settings {
            runs: cli.runs,
            min_sample: Duration::from_millis(cli.min_sample_ms),
            warmup: Duration::from_millis(cli.warmup_ms),
        },
    };
    let report = run(&plan, |line| eprintln!("  {line}"))?;

    let out_dir = repo_root().join("target").join("parser-gate");
    let md_path = cli.md.unwrap_or_else(|| out_dir.join("report.md"));
    let json_path = cli.json.unwrap_or_else(|| out_dir.join("report.json"));
    let markdown = report.to_markdown();
    write(&md_path, &markdown)?;
    write(&json_path, &report.to_json()?)?;
    print!("{markdown}");
    eprintln!("wrote {} and {}", md_path.display(), json_path.display());

    Ok(match (cli.gate, report.gate.status) {
        (true, Status::Fail) => {
            eprintln!("gate: FAIL");
            ExitCode::FAILURE
        }
        (true, Status::Pass) => {
            eprintln!("gate: pass");
            ExitCode::SUCCESS
        }
        (true, Status::BaselineOnly) => {
            eprintln!("gate: baseline only (no second parser registered)");
            ExitCode::SUCCESS
        }
        (false, _) => ExitCode::SUCCESS,
    })
}

fn main() -> ExitCode {
    match main_inner(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("parser-gate: {e}");
            ExitCode::from(2)
        }
    }
}
