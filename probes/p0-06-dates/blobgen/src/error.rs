use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("ICU4X data: {0}")]
    Data(#[from] icu_provider::DataError),
    #[error("locale: {0}")]
    Locale(#[from] icu::locale::ParseError),
    #[error("marker {0} is requested by the binary but not wired into blobgen's source")]
    MissingMarker(String),
}
