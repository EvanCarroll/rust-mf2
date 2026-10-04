//! The adapter boundary: what a parser must provide to be measured.
//!
//! This is **not** the product `Frontend` trait of `mf2-model` (frozen in
//! Phase 1). It is the bench's own, finer-grained
//! boundary: two stages × two state modes, one message per call, so that the
//! rows of the D1 gate compare like with like.
//!
//! * Implement [`Adapter`] for a parser (per-message methods, monomorphic).
//! * Every `Adapter` is a [`Contender`] (object-safe, one whole pass per call),
//!   which is what the measurement loop interleaves. The hot loop over the
//!   messages is inside [`Contender::pass`], generic over the adapter, so the
//!   dynamic dispatch happens once per pass, never per message.

use std::collections::BTreeSet;
use std::hint::black_box;

use serde::Serialize;

/// What a row measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Source → lossless CST with diagnostics (ox: `parse_source`).
    Cst,
    /// Source → data model (or ox's semantic model) plus the Data Model
    /// validation (ox: `parse_source` + `build_semantic_model` +
    /// `validate_semantics`).
    Model,
}

impl Stage {
    /// Both stages, in report order.
    pub const ALL: [Self; 2] = [Self::Cst, Self::Model];

    /// Label used in the tables.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cst => "parse → CST",
            Self::Model => "+ model + validation",
        }
    }
}

/// How parser state is handled across the messages of a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// All state is created and dropped per message (ox: one `SourceStore`
    /// per message).
    Fresh,
    /// State is created once per pass (one pass = one corpus, e.g. one
    /// locale's messages), reused by every message, and dropped at the end of
    /// the pass. Its creation and growth are part of the measurement.
    Reused,
}

impl Mode {
    /// Both modes, in report order.
    pub const ALL: [Self; 2] = [Self::Fresh, Self::Reused];

    /// Label used in the tables.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Reused => "reused",
        }
    }
}

/// What a pass is about to parse, so reused state may be pre-sized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hint {
    /// Number of messages in the pass.
    pub messages: usize,
    /// Total UTF-8 bytes of those messages.
    pub bytes: usize,
}

impl Hint {
    /// The hint for `messages`.
    pub fn of(messages: &[String]) -> Self {
        Self {
            messages: messages.len(),
            bytes: messages.iter().map(String::len).sum(),
        }
    }
}

/// Errors a parser reports for one message, in the WG suite's vocabulary:
/// `syntax-error` and the six Data Model Error names. Anything else (an
/// internal failure of the parser) is recorded as `internal:<detail>`, which
/// never matches an expectation.
pub type ErrorSet = BTreeSet<String>;

/// A parser under measurement, one message per call.
///
/// Every method returns the number of diagnostics the parser reported for the
/// message (syntax diagnostics; or, when there are none and the stage is
/// [`Stage::Model`], the Data Model errors; an internal failure counts as 1).
/// The value keeps the optimiser from discarding work, and lets tests check
/// that the fresh and reused paths agree. Adapters should `black_box` their
/// parse result before counting.
pub trait Adapter {
    /// Reused state (see [`Mode::Reused`]).
    type State;

    /// Stable key used in reports and on the command line, e.g. `ox`.
    fn key(&self) -> &'static str;

    /// Human-readable name with version, e.g. `ox_mf2_parser 0.14.0-alpha.12`.
    fn name(&self) -> &'static str;

    /// The API calls behind one row, for the report.
    fn api(&self, stage: Stage, mode: Mode) -> &'static str;

    /// Creates the state one pass of `stage` reuses.
    fn new_state(&self, stage: Stage, hint: Hint) -> Self::State;

    /// [`Stage::Cst`], [`Mode::Fresh`].
    fn cst_fresh(&self, src: &str) -> usize;

    /// [`Stage::Model`], [`Mode::Fresh`].
    fn model_fresh(&self, src: &str) -> usize;

    /// [`Stage::Cst`], [`Mode::Reused`]; `state` came from
    /// `new_state(Stage::Cst, …)`.
    fn cst_reused(&self, state: &mut Self::State, src: &str) -> usize;

    /// [`Stage::Model`], [`Mode::Reused`]; `state` came from
    /// `new_state(Stage::Model, …)`.
    fn model_reused(&self, state: &mut Self::State, src: &str) -> usize;

    /// The errors reported for `src` (syntax, then — if none — Data Model
    /// validation), for the correctness check. Not timed.
    fn classify(&self, src: &str) -> ErrorSet;
}

/// The object-safe view of an [`Adapter`] that the measurement loop drives.
pub trait Contender {
    /// See [`Adapter::key`].
    fn key(&self) -> &'static str;
    /// See [`Adapter::name`].
    fn name(&self) -> &'static str;
    /// See [`Adapter::api`].
    fn api(&self, stage: Stage, mode: Mode) -> &'static str;
    /// Parses every message once in the given stage and mode; returns the sum
    /// of the per-message diagnostic counts.
    fn pass(&self, messages: &[String], stage: Stage, mode: Mode) -> usize;
    /// See [`Adapter::classify`].
    fn classify(&self, src: &str) -> ErrorSet;
}

impl<A: Adapter> Contender for A {
    fn key(&self) -> &'static str {
        Adapter::key(self)
    }

    fn name(&self) -> &'static str {
        Adapter::name(self)
    }

    fn api(&self, stage: Stage, mode: Mode) -> &'static str {
        Adapter::api(self, stage, mode)
    }

    fn pass(&self, messages: &[String], stage: Stage, mode: Mode) -> usize {
        let mut sum = 0usize;
        match (stage, mode) {
            (Stage::Cst, Mode::Fresh) => {
                for src in messages {
                    sum = sum.wrapping_add(black_box(self.cst_fresh(black_box(src))));
                }
            }
            (Stage::Model, Mode::Fresh) => {
                for src in messages {
                    sum = sum.wrapping_add(black_box(self.model_fresh(black_box(src))));
                }
            }
            (Stage::Cst, Mode::Reused) => {
                let mut state = self.new_state(stage, Hint::of(messages));
                for src in messages {
                    sum = sum.wrapping_add(black_box(self.cst_reused(&mut state, black_box(src))));
                }
                drop(black_box(state));
            }
            (Stage::Model, Mode::Reused) => {
                let mut state = self.new_state(stage, Hint::of(messages));
                for src in messages {
                    sum =
                        sum.wrapping_add(black_box(self.model_reused(&mut state, black_box(src))));
                }
                drop(black_box(state));
            }
        }
        sum
    }

    fn classify(&self, src: &str) -> ErrorSet {
        Adapter::classify(self, src)
    }
}
