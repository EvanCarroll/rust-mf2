//! Errors of the probe server binary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[cfg(feature = "ssr")]
    #[error("leptos configuration: {0}")]
    Config(#[from] leptos::config::errors::LeptosConfigError),
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
}
