use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(String, #[source] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing CLDR field {0}")]
    Missing(String),
    #[error("too long for the probe encoding: {0}")]
    TooLong(String),
}
