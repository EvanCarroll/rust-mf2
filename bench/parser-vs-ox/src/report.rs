//! The run's report: a Markdown table in the shape of the D1
//! baseline table (plus the state-mode column), and the same data as JSON.

use std::fmt::Write as _;

use serde::Serialize;

use crate::corpus::{Corpora, CorpusId};
use crate::correctness::Correctness;
use crate::gate::{self, Criterion, Outcome, Status};
use crate::measure::{Row, Settings};

/// How the binary was built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Build {
    /// `rustc -V` of the compiler that built the harness.
    pub rustc: &'static str,
    /// Cargo's `OPT_LEVEL` for the harness.
    pub opt_level: &'static str,
    /// Whether debug assertions were on (a debug build: numbers are meaningless).
    pub debug_assertions: bool,
}

impl Build {
    /// This binary's build.
    pub fn current() -> Self {
        Self {
            rustc: env!("PARSER_GATE_RUSTC"),
            opt_level: env!("PARSER_GATE_OPT_LEVEL"),
            debug_assertions: cfg!(debug_assertions),
        }
    }
}

/// A parser in the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parser {
    /// Key.
    pub key: &'static str,
    /// Name and version.
    pub name: &'static str,
}

/// A corpus in the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorpusInfo {
    /// Which corpus.
    pub id: CorpusId,
    /// Table label.
    pub label: String,
    /// Messages.
    pub messages: usize,
    /// Source bytes.
    pub bytes: usize,
}

/// The knobs of the run, as numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SettingsInfo {
    /// Samples per row and parser.
    pub runs: usize,
    /// Minimum sample duration, ms.
    pub min_sample_ms: u128,
    /// Warm-up per row and parser, ms.
    pub warmup_ms: u128,
}

impl From<&Settings> for SettingsInfo {
    fn from(s: &Settings) -> Self {
        Self {
            runs: s.runs,
            min_sample_ms: s.min_sample.as_millis(),
            warmup_ms: s.warmup.as_millis(),
        }
    }
}

/// Everything a run produced.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// `parser-vs-ox <version>`.
    pub tool: String,
    /// How the harness was built.
    pub build: Build,
    /// Knobs.
    pub settings: SettingsInfo,
    /// `std::thread::available_parallelism` at run time.
    pub available_parallelism: usize,
    /// Seconds since the Unix epoch at the end of the run.
    pub unix_time: u64,
    /// Parsers; index 0 is the baseline.
    pub parsers: Vec<Parser>,
    /// Corpora measured.
    pub corpora: Vec<CorpusInfo>,
    /// The table.
    pub rows: Vec<Row>,
    /// Suite correctness per parser (same order as `parsers`).
    pub correctness: Vec<Correctness>,
    /// The gate's verdict.
    pub gate: Outcome,
}

impl Report {
    /// Corpus info for the report header.
    pub fn corpus_info(corpora: &Corpora, ids: &[CorpusId]) -> Vec<CorpusInfo> {
        ids.iter()
            .map(|id| {
                let c = corpora.get(*id);
                CorpusInfo {
                    id: *id,
                    label: c.label(),
                    messages: c.messages.len(),
                    bytes: c.bytes(),
                }
            })
            .collect()
    }

    /// The JSON form.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self).map(|mut s| {
            s.push('\n');
            s
        })
    }

    /// The Markdown form.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let names: Vec<&str> = self.parsers.iter().map(|p| p.name).collect();
        let _ = writeln!(out, "# Parser gate report\n");
        let _ = writeln!(
            out,
            "Parsers: {} (baseline: {}). {}; opt-level {}{}. ns/msg and MB/s: median of {} \
             samples per cell, each ≥ {} ms, interleaved over every row and parser; best ns: \
             the fastest sample; IQR: interquartile range / median. Allocations: one pass \
             under the counting allocator (exact, load-independent).\n",
            names.join(", "),
            names.first().copied().unwrap_or("none"),
            self.build.rustc,
            self.build.opt_level,
            if self.build.debug_assertions {
                " — **DEBUG BUILD, timings meaningless**"
            } else {
                ""
            },
            self.settings.runs,
            self.settings.min_sample_ms,
        );

        let _ = writeln!(
            out,
            "| Input | Stage | State | Parser | ns/msg | MB/s | allocs/msg | alloc B/msg | best ns | IQR |"
        );
        let _ = writeln!(out, "|---|---|---|---|---:|---:|---:|---:|---:|---:|");
        let mut last_input = None;
        for row in &self.rows {
            let input = self
                .corpora
                .iter()
                .find(|c| c.id == row.corpus)
                .map_or_else(|| row.corpus.key().to_owned(), |c| c.label.clone());
            let first_of_input = last_input != Some(row.corpus);
            last_input = Some(row.corpus);
            for (i, m) in row.results.iter().enumerate() {
                let head = i == 0;
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {:.0} | {:.1} | {} | {} | {:.1} % |",
                    if head && first_of_input {
                        input.as_str()
                    } else {
                        ""
                    },
                    if head { row.stage.label() } else { "" },
                    if head { row.mode.label() } else { "" },
                    m.parser,
                    thousands_f(m.ns_per_msg.median),
                    m.mb_per_s,
                    m.allocs_per_msg,
                    thousands_f(m.alloc_bytes_per_msg),
                    thousands_f(m.ns_per_msg.min),
                    m.ns_per_msg.relative_iqr() * 100.0,
                );
            }
        }

        let _ = writeln!(out, "\nWhat each row calls:\n");
        for p in &self.parsers {
            // The API depends on (stage, mode) only; list each pair once.
            let mut seen = Vec::new();
            for row in &self.rows {
                if let Some(m) = row.results.iter().find(|m| m.parser == p.key)
                    && !seen.contains(&(row.stage, row.mode))
                {
                    seen.push((row.stage, row.mode));
                    let _ = writeln!(
                        out,
                        "* {} — {}, {}: {}",
                        p.key,
                        row.stage.label(),
                        row.mode.label(),
                        m.api
                    );
                }
            }
        }

        let _ = writeln!(out, "\n## Correctness (syntax + Data Model errors)\n");
        for c in &self.correctness {
            let _ = writeln!(
                out,
                "* {}: **{}/{}** suite tests exact; {} of {} workload messages report an error.",
                c.parser, c.suite_exact, c.suite_total, c.workload_with_errors, c.workload_total
            );
            for m in &c.mismatches {
                let _ = writeln!(
                    out,
                    "  * `{}#{}` want {:?} got {:?} — src {:?}",
                    m.file, m.index, m.want, m.got, m.src
                );
            }
        }

        let _ = writeln!(out, "\n## Gate\n");
        match self.gate.status {
            Status::BaselineOnly => {
                let _ = writeln!(
                    out,
                    "**Baseline only** — no second parser is registered, nothing to compare."
                );
            }
            Status::Pass | Status::Fail => {
                let _ = writeln!(
                    out,
                    "**{}** — time ≤ {:.2} × baseline median, allocations and bytes ≤ baseline, \
                     every suite test exact.\n",
                    if self.gate.status == Status::Pass {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    gate::TIME_RATIO_MAX
                );
                let _ = writeln!(out, "| Parser | Row | Criterion | Value | Limit | Result |");
                let _ = writeln!(out, "|---|---|---|---:|---:|---|");
                for c in &self.gate.checks {
                    let (value, limit) = match c.criterion {
                        Criterion::Time => (
                            format!("{} ns", thousands_f(c.value)),
                            format!("{} ns", thousands_f(c.limit)),
                        ),
                        _ => (thousands_f(c.value), thousands_f(c.limit)),
                    };
                    let _ = writeln!(
                        out,
                        "| {} | {} | {:?} | {} | {} | {} |",
                        c.parser,
                        c.row,
                        c.criterion,
                        value,
                        limit,
                        if c.passed { "pass" } else { "**FAIL**" }
                    );
                }
            }
        }
        out
    }
}

/// `1234567` → `1,234,567`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A non-negative float rounded to an integer, with thousands separators.
pub fn thousands_f(x: f64) -> String {
    let rounded = format!("{:.0}", x.max(0.0));
    rounded.parse::<u64>().map_or(rounded, thousands)
}

#[cfg(test)]
mod tests {
    use super::{thousands, thousands_f};

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands_f(1435.4), "1,435");
        assert_eq!(thousands_f(2868.6), "2,869");
    }
}
