//! Errors of the build-side locale data.

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
    #[error("a modulus or value ≥ 10^18 (plans/02-catalog-format.md §4.1 operand contract)")]
    TooLarge,
    #[error("bad sample {0:?}")]
    Sample(String),
    #[error("sample range {0:?}: endpoints differ in fraction digits or use an exponent")]
    SampleRange(String),
}

/// A problem with the shipped rule table or the CLDR input.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Parse(#[from] ParseError),
    #[error("{kind} {locale} {category}: {source}")]
    Rule {
        kind: &'static str,
        locale: String,
        category: String,
        #[source]
        source: ParseError,
    },
    #[error("table line {line}: {message}")]
    Table { line: usize, message: String },
    #[cfg(feature = "extract")]
    #[error("CLDR JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "extract")]
    #[error("unexpected CLDR JSON shape: {0}")]
    Shape(String),
}
