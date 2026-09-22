//! What one `build.rs` pass costs (Phase 5a, A11).
//!
//! A build script runs on every compile of the i18n crate, so its cost is
//! paid over and over. This measures it the way an application pays it: a
//! whole pass over a corpus — read, parse, validate, lint, flatten, slice,
//! write — timed, and the peak resident memory of the process that did it.
//!
//! *Cold* is the first pass into an empty output directory, which writes
//! every file; *warm* is a second pass over an unchanged corpus, which writes
//! nothing but still does all the reading and parsing (cargo has no early
//! cut-off, P0.9). *Edited* is a pass after one message changed, which is
//! what a translator's save costs.

// The build's error names a path and what it wraps; this binary returns one
// at most once, at the end.
#![allow(clippy::result_large_err)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use mf2_build::{Build, Config, Features};

#[derive(Debug, Parser)]
#[command(name = "build-cost", version, about)]
struct Cli {
    /// The corpus (where `mf2.toml` and `locales/` are).
    #[arg(value_name = "DIR")]
    corpus: PathBuf,
    /// Where to write; emptied first.
    #[arg(long, short, value_name = "DIR", default_value = "target/build-cost")]
    out: PathBuf,
    /// The client feature set.
    #[arg(long, value_name = "LIST", default_value = "fn-number")]
    features: String,
    /// How many warm passes to time (the median is reported).
    #[arg(long, default_value_t = 5)]
    runs: u32,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("build-cost: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), mf2_build::Error> {
    let features = Features::parse(&cli.features);
    let config = Config::load(&cli.corpus)?;
    let _ = std::fs::remove_dir_all(&cli.out);

    let pass = || -> Result<(usize, usize), mf2_build::Error> {
        let outcome = Build::at(&cli.corpus, &cli.out)
            .config(config.clone())
            .features(features.clone())
            .run()?;
        Ok((outcome.manifest.ids.len(), outcome.written.len()))
    };

    let start = Instant::now();
    let (messages, written) = pass()?;
    let cold = start.elapsed();

    let mut warm: Vec<Duration> = Vec::new();
    let mut warm_written = 0;
    for _ in 0..cli.runs {
        let start = Instant::now();
        let (_, w) = pass()?;
        warm.push(start.elapsed());
        warm_written = w;
    }
    warm.sort_unstable();
    let warm_median = warm[warm.len() / 2];

    // What a translator's save costs: one message changed, then a pass.
    let edited = edit_and_time(cli, &config, &features);

    let peak = peak_rss_kb();
    println!(
        "corpus {} — {messages} messages, {} locale(s), features {:?}",
        cli.corpus.display(),
        config.fallback.len().max(1),
        cli.features
    );
    println!("{:<28} {:>9}  files written", "pass", "ms");
    println!(
        "{:<28} {:>9.1}  {written}",
        "cold (empty OUT_DIR)",
        ms(cold)
    );
    println!(
        "{:<28} {:>9.1}  {warm_written}",
        format!("warm (median of {})", cli.runs),
        ms(warm_median)
    );
    println!(
        "{:<28} {:>9.1}  {}",
        "after one message changed",
        ms(edited.0),
        edited.1
    );
    match peak {
        Some(kb) => println!(
            "peak resident memory: {:.1} MB",
            f64::from(u32::try_from(kb).unwrap_or(u32::MAX)) / 1024.0
        ),
        None => println!("peak resident memory: not available on this system"),
    }
    Ok(())
}

/// Times a pass after changing one message, and puts the corpus back.
fn edit_and_time(cli: &Cli, config: &Config, features: &Features) -> (Duration, usize) {
    let source = cli.corpus.join("locales").join(&config.source_locale);
    let Some(file) = std::fs::read_dir(&source)
        .ok()
        .and_then(|d| d.flatten().map(|e| e.path()).find(|p| p.is_file()))
    else {
        return (Duration::ZERO, 0);
    };
    let before = std::fs::read_to_string(&file).unwrap_or_default();
    let after = format!("{before}\nbuild-cost-probe = One more message.\n");
    let _ = std::fs::write(&file, &after);
    let start = Instant::now();
    let outcome = Build::at(&cli.corpus, &cli.out)
        .config(config.clone())
        .features(features.clone())
        .run();
    let elapsed = start.elapsed();
    let _ = std::fs::write(&file, &before);
    let written = outcome.map_or(0, |o| o.written.len());
    // Put the outputs back as they were, so a later run measures the same.
    let _ = Build::at(&cli.corpus, &cli.out)
        .config(config.clone())
        .features(features.clone())
        .run();
    (elapsed, written)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The high-water mark of resident memory, from the kernel.
fn peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|n| n.parse().ok())
}
