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
    #[error("CLDR number pattern {pattern:?}: {message}")]
    Pattern {
        pattern: String,
        message: &'static str,
    },
    #[error("number data of {locale} ({system}): no {field}")]
    Missing {
        locale: String,
        system: String,
        field: &'static str,
    },
    #[error("LOCALE entry: {0}")]
    Entry(#[from] mf2_catalog::WriteError),
    #[cfg(feature = "extract")]
    #[error("CLDR JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "extract")]
    #[error("unexpected CLDR JSON shape: {0}")]
    Shape(String),
    /// The CLDR input breaks an assumption the table format rests on (the
    /// extractor checks each one, so a CLDR update that changes them is
    /// noticed, not silently mis-encoded).
    #[cfg(feature = "extract")]
    #[error("CLDR data: {0}")]
    Assumption(String),
    #[cfg(feature = "extract")]
    #[error("{path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// ICU4X data for `icu.blob` (`icu-blob`).
    #[cfg(feature = "icu-blob")]
    #[error("icu.blob: {0}")]
    IcuData(#[from] icu_provider::DataError),
    /// Some formatter of the `datetime-icu` backend does not build from
    /// ICU4X's data for the locale (`icu-blob`).
    #[cfg(feature = "icu-blob")]
    #[error("icu.blob for {locale}: {count} formatter(s) do not build, e.g. {first}")]
    IcuBlobBuild {
        locale: String,
        count: usize,
        first: String,
    },
}
