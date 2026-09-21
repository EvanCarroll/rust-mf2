//! Errors of the workload generator.

use std::path::PathBuf;

/// Everything that can go wrong while generating a workload.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reading or writing a file failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON input (the WG suite) could not be parsed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A template's `template.toml` could not be parsed.
    #[error("template TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// A knob combination the generator cannot honour.
    #[error("invalid knobs: {0}")]
    Knobs(String),

    /// A call-site template is incomplete or uses an unknown placeholder.
    #[error("template `{template}`: {message}")]
    Template {
        /// Template name.
        template: String,
        /// What is wrong with it.
        message: String,
    },

    /// A path that had to exist does not.
    #[error("not found: {}", .0.display())]
    NotFound(PathBuf),

    /// A WG suite file does not have the expected structure.
    #[error("suite file {file}: {message}")]
    Suite {
        /// Path relative to `test/tests`.
        file: String,
        /// What is wrong with it.
        message: String,
    },

    /// `corpora --check` found committed files that differ from fresh output.
    #[error("committed corpora are stale: {0}")]
    Stale(String),

    /// `stats` found the shape outside the tolerances of plans/06 §2.
    #[error("shape outside tolerance: {0}")]
    Shape(String),
}
