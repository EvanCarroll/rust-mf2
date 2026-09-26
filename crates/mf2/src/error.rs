//! Errors of the facade's build-side helpers.

use alloc::vec::Vec;

use mf2_model::{Diagnostic, ErrorKind};

/// Why [`crate::compile_str`] refused a message.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CompileError {
    /// The source is not well-formed (syntax errors) or not valid
    /// (data-model errors): the build refuses to ship it, as the suite's
    /// error tests expect (`plans/01-conformance.md` §4).
    #[error("the message has {} syntax or data-model error(s)", .0.len())]
    Invalid(Vec<Diagnostic>),
    /// The catalog writer refused the model (e.g. a U+0000).
    #[error("catalog writer: {0}")]
    Write(#[from] mf2_catalog::WriteError),
    /// The written catalog did not load (a bug: the writer and the reader
    /// disagree).
    #[error("catalog reader: {0}")]
    Load(#[from] mf2_catalog::CatalogError),
    /// The locale data could not be read.
    #[error("locale data: {0}")]
    LocaleData(#[from] mf2_locale_data::Error),
}

impl CompileError {
    /// The MF2 error kinds of an [`CompileError::Invalid`] message, in the
    /// order reported (empty for the other variants).
    pub fn kinds(&self) -> Vec<ErrorKind> {
        match self {
            CompileError::Invalid(d) => d.iter().map(|d| d.kind).collect(),
            _ => Vec::new(),
        }
    }
}
