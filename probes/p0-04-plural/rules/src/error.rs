//! Errors of the build-side plural tooling.

use thiserror::Error;

/// A syntax error in one `pluralRule-count-*` string.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("unexpected character {ch:?} at byte {at}")]
    UnexpectedChar { at: usize, ch: char },
    #[error("expected {expected} at byte {at}")]
    Expected { at: usize, expected: &'static str },
    #[error("number too large at byte {at}")]
    Overflow { at: usize },
    #[error("range {lo}..{hi} is empty")]
    EmptyRange { lo: u64, hi: u64 },
    #[error("modulus 0")]
    ZeroModulus,
    #[error("the `other` rule must have no condition")]
    OtherWithCondition,
    #[error("a non-`other` rule has no condition")]
    MissingCondition,
    #[error("unknown category {0:?}")]
    UnknownCategory(String),
    #[error("more than 31 OR groups in one rule")]
    TooManyGroups,
    #[error("bad sample {0:?}")]
    Sample(String),
    #[error("sample range {0:?}: endpoints differ in fraction digits or use an exponent")]
    SampleRange(String),
}

/// Anything that can go wrong loading and checking the CLDR inputs.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Parse(#[from] ParseError),
    #[error("{locale} {kind} {key}: {source}")]
    Rule {
        locale: String,
        kind: &'static str,
        key: String,
        #[source]
        source: ParseError,
    },
    #[error("unexpected JSON shape: {0}")]
    Shape(String),
}
