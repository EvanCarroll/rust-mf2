//! Errors of the ICU4X comparison tool.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("ICU4X data: {0}")]
    Data(#[from] icu_provider::DataError),
    #[error("locale: {0}")]
    Locale(#[from] icu_locale_core::ParseError),
    #[error("CLDR input: {0}")]
    Cldr(#[from] plural_rules::Error),
    #[error("CLDR sample: {0}")]
    Sample(#[from] plural_rules::ParseError),
    #[error("postcard: {0}")]
    Postcard(#[from] postcard::Error),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
}
