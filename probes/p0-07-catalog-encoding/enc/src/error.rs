//! Errors of the P0.7 build side.

use thiserror::Error;

#[derive(Debug, Error)]
#[error("parse error at byte {at}: expected {expected}")]
pub struct ParseError {
    pub at: usize,
    pub expected: &'static str,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("plural rules: {0}")]
    Plural(#[from] plural_rules::Error),
    #[error("message `{id}`: {source}")]
    Parse {
        id: String,
        #[source]
        source: ParseError,
    },
    #[error("message `{id}` uses variable `${var}`, which the source message lacks")]
    UnknownVariable { id: String, var: String },
    #[error("message `{id}` is not in the manifest")]
    UnknownId { id: String },
    #[error("string contains U+0000, not representable in the NUL-terminated layout")]
    NulInString,
    #[error("string too long for the chosen layout ({0} bytes)")]
    StringTooLong(usize),
    #[error("offset does not fit the INDEX entry ({0})")]
    OffsetTooLarge(usize),
    #[error("decode: {0}")]
    Decode(&'static str),
    #[error("round trip mismatch in `{id}` ({locale}, {variant})")]
    RoundTrip { id: String, locale: String, variant: String },
    #[error("{0}")]
    Other(String),
}
