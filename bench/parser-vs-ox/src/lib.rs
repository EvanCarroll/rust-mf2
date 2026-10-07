//! `parser-vs-ox` — the D1 gate: `mf2-syntax` against the `ox_mf2_parser` baseline.
//!
//! Measures parsers on the committed corpora (`bench/corpora/`) in the same
//! process, interleaved: ns/msg and MB/s (median of the samples), allocations
//! and allocated bytes per message (counting allocator), for the rows
//! *corpus* (suite, workload, placeholder-free) × *stage* (CST, + model) ×
//! *state* (fresh per message, reused per pass). Also records each parser's
//! correctness on the WG suite. `ox_mf2_parser` is the baseline; `mf2-syntax`
//! (Phase 1) is the second [`adapter::Adapter`] in [`contenders`], held to the
//! gate's rules ([`gate`]).
//!
//! The benchmark is the binary (`cargo run --release -p parser-vs-ox`); the
//! library's unit tests use tiny inputs only.

// Statistics over counts and nanoseconds, all far below 2^52: `as f64` is exact
// enough everywhere it is used.
#![allow(clippy::cast_precision_loss)]

pub mod adapter;
pub mod alloc;
pub mod corpus;
pub mod correctness;
mod error;
pub mod gate;
pub mod measure;
pub mod mf2_syntax;
pub mod ox;
pub mod report;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use error::{Error, Result};

use adapter::{Contender, Mode, Stage};
use corpus::{Corpora, CorpusId};
use measure::Settings;
use report::{Build, Parser, Report, SettingsInfo};

/// Every allocation of every binary linking this crate goes through the
/// counting allocator (the bench binary and the unit tests).
#[global_allocator]
static GLOBAL: alloc::Counting = alloc::Counting;

/// The parsers under comparison. **Index 0 is the baseline** (`ox_mf2_parser`);
/// index 1 is this workspace's `mf2-syntax`.
pub fn contenders() -> Vec<Box<dyn Contender>> {
    vec![Box::new(ox::Ox), Box::new(mf2_syntax::Mf2Syntax)]
}

/// The repository root (two levels above this crate's manifest).
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest.to_path_buf(), Path::to_path_buf)
}

/// What to run.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Directory holding `suite.json` and `workload-1600.json`.
    pub corpora_dir: PathBuf,
    /// The vendored WG suite's `test/tests` directory (expected errors).
    pub suite_dir: PathBuf,
    /// Corpora to measure, in order.
    pub corpora: Vec<CorpusId>,
    /// Measurement knobs.
    pub settings: Settings,
}

/// Runs the whole benchmark: every row of every selected corpus, the
/// correctness check, and the gate evaluation. `progress` receives status
/// lines.
pub fn run(plan: &Plan, mut progress: impl FnMut(&str)) -> Result<Report> {
    if plan.settings.runs == 0 {
        return Err(Error::Settings("--runs must be at least 1".to_owned()));
    }
    let corpora = Corpora::load(&plan.corpora_dir)?;
    let expected =
        correctness::expected(&plan.suite_dir, &corpora.suite_entries, &plan.corpora_dir)?;
    let contenders = contenders();

    let correctness: Vec<_> = contenders
        .iter()
        .map(|c| correctness::check(c.as_ref(), &expected, &corpora.workload))
        .collect();

    let specs: Vec<measure::RowSpec<'_>> = plan
        .corpora
        .iter()
        .flat_map(|id| {
            let corpus = corpora.get(*id);
            Stage::ALL.into_iter().flat_map(move |stage| {
                Mode::ALL.into_iter().map(move |mode| measure::RowSpec {
                    corpus,
                    stage,
                    mode,
                })
            })
        })
        .collect();
    progress(&format!(
        "{} rows × {} parser(s), {} rounds",
        specs.len(),
        contenders.len(),
        plan.settings.runs
    ));
    let rows = measure::measure_rows(&contenders, &specs, &plan.settings, &mut progress);
    for row in &rows {
        let summary: Vec<String> = row
            .results
            .iter()
            .map(|m| {
                format!(
                    "{} {:.0} ns/msg {:.1} allocs/msg",
                    m.parser, m.ns_per_msg.median, m.allocs_per_msg
                )
            })
            .collect();
        progress(&format!(
            "{} / {} / {}: {}",
            row.corpus.key(),
            row.stage.label(),
            row.mode.label(),
            summary.join(" | ")
        ));
    }

    let gate = gate::evaluate(&rows, &correctness);
    Ok(Report {
        tool: format!("parser-vs-ox {}", env!("CARGO_PKG_VERSION")),
        build: Build::current(),
        settings: SettingsInfo::from(&plan.settings),
        available_parallelism: std::thread::available_parallelism().map_or(0, usize::from),
        unix_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        parsers: contenders
            .iter()
            .map(|c| Parser {
                key: c.key(),
                name: c.name(),
            })
            .collect(),
        corpora: Report::corpus_info(&corpora, &plan.corpora),
        rows,
        correctness,
        gate,
    })
}
