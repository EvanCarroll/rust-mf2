//! Errors of the build-side locale data.

use thiserror::Error;

/// A syntax error in one `pluralRule-count-*` string.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// A character the rule grammar does not allow here.
    #[error("unexpected character {ch:?} at byte {at}")]
    UnexpectedChar {
        /// Its byte offset.
        at: usize,
        /// The character.
        ch: char,
    },
    /// Something else was expected.
    #[error("expected {expected} at byte {at}")]
    Expected {
        /// The byte offset.
        at: usize,
        /// What the grammar expected.
        expected: &'static str,
    },
    /// A number does not fit.
    #[error("number too large at byte {at}")]
    Overflow {
        /// Its byte offset.
        at: usize,
    },
    /// A range whose low end is above its high end.
    #[error("range {lo}..{hi} is empty")]
    EmptyRange {
        /// The low end.
        lo: u64,
        /// The high end.
        hi: u64,
    },
    /// `n % 0`.
    #[error("modulus 0")]
    ZeroModulus,
    /// The `other` category has a condition.
    #[error("the `other` rule must have no condition")]
    OtherWithCondition,
    /// A category other than `other` has none.
    #[error("a non-`other` rule has no condition")]
    MissingCondition,
    /// Not one of CLDR's plural categories.
    #[error("unknown category {0:?}")]
    UnknownCategory(String),
    /// More OR groups than the encoding holds.
    #[error("more than 31 OR groups in one rule")]
    TooManyGroups,
    /// A value beyond the operand contract.
    #[error("a modulus or value ≥ 10^18 (the plural operand contract)")]
    TooLarge,
    /// A malformed `@integer` / `@decimal` sample.
    #[error("bad sample {0:?}")]
    Sample(String),
    /// A sample range whose ends cannot be compared.
    #[error("sample range {0:?}: endpoints differ in fraction digits or use an exponent")]
    SampleRange(String),
}

/// A problem with the shipped rule table or the CLDR input.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A plural rule does not parse.
    #[error("{0}")]
    Parse(#[from] ParseError),
    /// A plural rule of the shipped table does not parse.
    #[error("{kind} {locale} {category}: {source}")]
    Rule {
        /// `cardinal` or `ordinal`.
        kind: &'static str,
        /// The locale.
        locale: String,
        /// The plural category.
        category: String,
        /// Why.
        #[source]
        source: ParseError,
    },
    /// A line of a shipped table is malformed.
    #[error("table line {line}: {message}")]
    Table {
        /// One-based line.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// A CLDR number pattern the encoder does not accept.
    #[error("CLDR number pattern {pattern:?}: {message}")]
    Pattern {
        /// The pattern.
        pattern: String,
        /// What is wrong.
        message: &'static str,
    },
    /// A locale's number data lacks a field.
    #[error("number data of {locale} ({system}): no {field}")]
    Missing {
        /// The locale.
        locale: String,
        /// The numbering system.
        system: String,
        /// The missing field.
        field: &'static str,
    },
    /// The catalog writer refused a LOCALE entry.
    #[error("LOCALE entry: {0}")]
    Entry(#[from] mf2_catalog::WriteError),
    /// The CLDR JSON does not parse (`extract`).
    #[cfg(feature = "extract")]
    #[error("CLDR JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The CLDR JSON is not shaped as expected (`extract`).
    #[cfg(feature = "extract")]
    #[error("unexpected CLDR JSON shape: {0}")]
    Shape(String),
    /// The CLDR input breaks an assumption the table format rests on (the
    /// extractor checks each one, so a CLDR update that changes them is
    /// noticed, not silently mis-encoded).
    #[cfg(feature = "extract")]
    #[error("CLDR data: {0}")]
    Assumption(String),
    /// A file could not be read or written (`extract`).
    #[cfg(feature = "extract")]
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: std::path::PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },
    /// ICU4X data for `icu.blob` (`icu-blob`).
    #[cfg(feature = "icu-blob")]
    #[error("icu.blob: {0}")]
    IcuData(#[from] icu_provider::DataError),
    /// Some formatter of the ICU4X backend does not build from
    /// ICU4X's data for the locale (`icu-blob`).
    #[cfg(feature = "icu-blob")]
    #[error("icu.blob for {locale}: {count} formatter(s) do not build, e.g. {first}")]
    IcuBlobBuild {
        /// The locale.
        locale: String,
        /// How many formatters fail.
        count: usize,
        /// The first that does.
        first: String,
    },
}
