//! Measuring one row: allocations (one counted pass) and time (interleaved
//! samples, median).
//!
//! For every row (corpus × stage × mode) and contender:
//!
//! 1. **Warm-up** — passes until `warmup` has elapsed (at least one).
//! 2. **Allocations** — one pass under the counting allocator. Deterministic:
//!    it does not depend on the machine or its load.
//! 3. **Calibration** — the baseline's fastest of three single passes decides
//!    the row's `passes_per_sample`, so that one sample lasts at least
//!    `min_sample`. All contenders use the same number.
//! 4. **Samples** — `runs` rounds; each round takes one sample
//!    (`passes_per_sample` passes, timed as a whole) of every (row,
//!    contender) pair, back to back, the order reversing every other round
//!    (A B C …, … C B A) so drift and load hit every cell of the table alike.
//!    A sample's value is ns per message.
//!
//! Each cell reports the median (and spread) of its samples.

use std::hint::black_box;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::adapter::{Contender, Mode, Stage};
use crate::alloc;
use crate::corpus::{Corpus, CorpusId};

/// Knobs of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Timing samples per row and contender.
    pub runs: usize,
    /// Minimum duration of one sample.
    pub min_sample: Duration,
    /// Warm-up per row and contender.
    pub warmup: Duration,
}

/// Order statistics of a row's samples (ns per message).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Spread {
    /// Fastest sample.
    pub min: f64,
    /// First quartile.
    pub q1: f64,
    /// Median — the figure the gate compares.
    pub median: f64,
    /// Third quartile.
    pub q3: f64,
    /// Slowest sample.
    pub max: f64,
}

impl Spread {
    /// Order statistics of `samples` (sorted in place); `None` if empty.
    /// Quartiles interpolate linearly between closest ranks.
    pub fn of(samples: &mut [f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        samples.sort_by(f64::total_cmp);
        let last = samples.len() - 1;
        // The `quarters`/4 quantile: position `quarters * last / 4`, integer
        // part and remainder kept exact; both ranks are within `0..=last`.
        let q = |quarters: usize| {
            let scaled = quarters * last;
            let lo = scaled / 4;
            let frac = (scaled % 4) as f64 / 4.0;
            let hi = (lo + 1).min(last);
            samples[lo] + (samples[hi] - samples[lo]) * frac
        };
        Some(Self {
            min: samples[0],
            q1: q(1),
            median: q(2),
            q3: q(3),
            max: samples[last],
        })
    }

    /// Interquartile range as a fraction of the median.
    pub fn relative_iqr(&self) -> f64 {
        if self.median > 0.0 {
            (self.q3 - self.q1) / self.median
        } else {
            0.0
        }
    }
}

/// One contender's result on one row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurement {
    /// Contender key (`ox`, later `mf2-syntax`).
    pub parser: &'static str,
    /// The API calls measured.
    pub api: &'static str,
    /// ns per message over the samples.
    pub ns_per_msg: Spread,
    /// Throughput at the median: source MB (10⁶ bytes) per second.
    pub mb_per_s: f64,
    /// Allocation calls in one pass.
    pub allocs: u64,
    /// Bytes requested in one pass.
    pub alloc_bytes: u64,
    /// `allocs / messages`.
    pub allocs_per_msg: f64,
    /// `alloc_bytes / messages`.
    pub alloc_bytes_per_msg: f64,
    /// Diagnostics reported in one pass (a checksum; equal across modes).
    pub diagnostics: usize,
}

/// One row of the table: a corpus, a stage, a mode, every contender.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// The corpus.
    pub corpus: CorpusId,
    /// The stage.
    pub stage: Stage,
    /// The state mode.
    pub mode: Mode,
    /// Messages in the corpus.
    pub messages: usize,
    /// Source bytes in the corpus.
    pub bytes: usize,
    /// Samples per contender.
    pub runs: usize,
    /// Passes over the corpus per sample.
    pub passes_per_sample: u64,
    /// One entry per contender; index 0 is the baseline.
    pub results: Vec<Measurement>,
}

fn timed(c: &dyn Contender, msgs: &[String], stage: Stage, mode: Mode, passes: u64) -> Duration {
    let start = Instant::now();
    for _ in 0..passes {
        black_box(c.pass(black_box(msgs), stage, mode));
    }
    start.elapsed()
}

/// One row to measure.
#[derive(Debug, Clone, Copy)]
pub struct RowSpec<'a> {
    /// The corpus.
    pub corpus: &'a Corpus,
    /// The stage.
    pub stage: Stage,
    /// The state mode.
    pub mode: Mode,
}

/// Per-row working data between the phases.
struct Work<'a> {
    spec: RowSpec<'a>,
    counted: Vec<(usize, alloc::Counts)>,
    passes_per_sample: u64,
    samples: Vec<Vec<f64>>,
}

/// Measures `specs` against `contenders` (`contenders[0]` is the baseline).
///
/// Samples are interleaved over **all rows and contenders**: each round takes
/// one sample of every (row, contender) pair, the order reversing every other
/// round. Load that comes and goes during the run is therefore spread over
/// every cell of the table, not only over the contenders of one row.
/// `progress` receives one line per phase and every few rounds.
pub fn measure_rows(
    contenders: &[Box<dyn Contender>],
    specs: &[RowSpec<'_>],
    settings: &Settings,
    mut progress: impl FnMut(&str),
) -> Vec<Row> {
    // 1 + 2: warm-up, then one counted pass, per row and contender.
    let mut work: Vec<Work<'_>> = specs
        .iter()
        .map(|spec| {
            let msgs = spec.corpus.messages.as_slice();
            let counted = contenders
                .iter()
                .map(|c| {
                    let deadline = Instant::now() + settings.warmup;
                    loop {
                        black_box(c.pass(msgs, spec.stage, spec.mode));
                        if Instant::now() >= deadline {
                            break;
                        }
                    }
                    alloc::count(|| black_box(c.pass(msgs, spec.stage, spec.mode)))
                })
                .collect();
            Work {
                spec: *spec,
                counted,
                passes_per_sample: 1,
                samples: (0..contenders.len())
                    .map(|_| Vec::with_capacity(settings.runs))
                    .collect(),
            }
        })
        .collect();
    progress("warm-up and allocation counts done");

    // 3: calibrate every row on the baseline.
    for w in &mut work {
        let RowSpec {
            corpus,
            stage,
            mode,
        } = w.spec;
        w.passes_per_sample = contenders.first().map_or(1, |base| {
            let one = (0..3)
                .map(|_| timed(base.as_ref(), &corpus.messages, stage, mode, 1))
                .min()
                .unwrap_or(Duration::from_nanos(1));
            let per = one.as_nanos().max(1);
            u64::try_from(settings.min_sample.as_nanos().div_ceil(per))
                .unwrap_or(u64::MAX)
                .max(1)
        });
    }

    // 4: interleaved samples over every (row, contender) pair.
    let cells: Vec<(usize, usize)> = (0..work.len())
        .flat_map(|r| (0..contenders.len()).map(move |c| (r, c)))
        .collect();
    for round in 0..settings.runs {
        let forward = round.is_multiple_of(2);
        for k in 0..cells.len() {
            let (r, c) = cells[if forward { k } else { cells.len() - 1 - k }];
            let w = &mut work[r];
            let RowSpec {
                corpus,
                stage,
                mode,
            } = w.spec;
            let n = corpus.messages.len().max(1) as f64;
            let dt = timed(
                contenders[c].as_ref(),
                &corpus.messages,
                stage,
                mode,
                w.passes_per_sample,
            );
            w.samples[c].push(dt.as_nanos() as f64 / (w.passes_per_sample as f64 * n));
        }
        if (round + 1).is_multiple_of(5) || round + 1 == settings.runs {
            progress(&format!("round {}/{}", round + 1, settings.runs));
        }
    }

    work.into_iter()
        .map(|w| finish(contenders, w, settings.runs))
        .collect()
}

fn finish(contenders: &[Box<dyn Contender>], mut w: Work<'_>, runs: usize) -> Row {
    let RowSpec {
        corpus,
        stage,
        mode,
    } = w.spec;
    let n = corpus.messages.len().max(1) as f64;
    let bytes = corpus.bytes();
    let results = contenders
        .iter()
        .zip(w.counted)
        .zip(w.samples.iter_mut())
        .map(|((c, (diagnostics, used)), s)| {
            let spread = Spread::of(s).unwrap_or(Spread {
                min: 0.0,
                q1: 0.0,
                median: 0.0,
                q3: 0.0,
                max: 0.0,
            });
            let mb_per_s = if spread.median > 0.0 {
                (bytes as f64 / n) / spread.median * 1000.0
            } else {
                0.0
            };
            Measurement {
                parser: c.key(),
                api: c.api(stage, mode),
                ns_per_msg: spread,
                mb_per_s,
                allocs: used.allocs,
                alloc_bytes: used.bytes,
                allocs_per_msg: used.allocs as f64 / n,
                alloc_bytes_per_msg: used.bytes as f64 / n,
                diagnostics,
            }
        })
        .collect();
    Row {
        corpus: corpus.id,
        stage,
        mode,
        messages: corpus.messages.len(),
        bytes,
        runs,
        passes_per_sample: w.passes_per_sample,
        results,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{RowSpec, Settings, Spread, measure_rows};
    use crate::adapter::{Mode, Stage};
    use crate::corpus::{Corpus, CorpusId};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn spread_of_odd_and_even_samples() {
        let mut odd = [5.0, 1.0, 3.0, 2.0, 4.0];
        let s = Spread::of(&mut odd).expect("non-empty");
        assert!(close(s.min, 1.0) && close(s.max, 5.0));
        assert!(close(s.median, 3.0) && close(s.q1, 2.0) && close(s.q3, 4.0));

        let mut even = [4.0, 1.0, 3.0, 2.0];
        let s = Spread::of(&mut even).expect("non-empty");
        assert!(close(s.median, 2.5));
        assert!(close(s.q1, 1.75) && close(s.q3, 3.25));
        assert!(close(s.relative_iqr(), 0.6));

        assert!(Spread::of(&mut []).is_none());
    }

    #[test]
    fn measures_a_tiny_row() {
        let corpus = Corpus {
            id: CorpusId::Suite,
            messages: vec!["Hello".into(), "Hi {$name}".into(), "{".into()],
        };
        let settings = Settings {
            runs: 3,
            min_sample: Duration::from_micros(200),
            warmup: Duration::ZERO,
        };
        let contenders = crate::contenders();
        let specs = [
            RowSpec {
                corpus: &corpus,
                stage: Stage::Cst,
                mode: Mode::Fresh,
            },
            RowSpec {
                corpus: &corpus,
                stage: Stage::Model,
                mode: Mode::Reused,
            },
        ];
        let mut lines = 0;
        let rows = measure_rows(&contenders, &specs, &settings, |_| lines += 1);
        assert_eq!(rows.len(), 2);
        assert!(lines >= 2);
        assert_eq!((rows[1].stage, rows[1].mode), (Stage::Model, Mode::Reused));
        let row = &rows[0];
        assert_eq!(row.messages, 3);
        assert_eq!(row.results.len(), contenders.len());
        let ox = &row.results[0];
        assert_eq!(ox.parser, "ox");
        assert!(ox.ns_per_msg.median > 0.0);
        assert!(ox.allocs > 0 && ox.alloc_bytes > 0);
        assert!(ox.diagnostics >= 1);
        assert!(row.passes_per_sample >= 1);
    }
}
