//! Formatting errors: small plain values (`plans/03-runtime.md` §2.2). They
//! derive `thiserror::Error` for server and test use; its `Display` is never
//! reached from the client path (B12).

use mf2_model::ErrorKind;

/// An error reported while formatting. Formatting still produces output.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, thiserror::Error)]
#[repr(u8)]
#[non_exhaustive]
pub enum FormatError {
    /// Unresolved Variable: no value for a variable.
    #[error("unresolved variable")]
    UnresolvedVariable,
    /// Unknown Function: the registry has no handler for a function.
    #[error("unknown function")]
    UnknownFunction,
    /// Bad Selector: a selector's value does not support selection.
    #[error("bad selector")]
    BadSelector,
    /// Bad Operand: a function cannot take its operand.
    #[error("bad operand")]
    BadOperand,
    /// Bad Option: an option or its value is not valid for the function.
    #[error("bad option")]
    BadOption,
    /// Bad Variant Key: a key a selector cannot compare against.
    #[error("bad variant key")]
    BadVariantKey,
    /// Unsupported Operation: valid, but not supported by this handler or
    /// configuration (or past an implementation limit).
    #[error("unsupported operation")]
    UnsupportedOperation,
    /// Any other failure of a function handler.
    #[error("message function error")]
    MessageFunctionError,
    /// No variant matched — only a catalog built from an invalid model (no
    /// fallback variant, or key counts unlike the selector count) has one.
    /// The message formats as `{�}`.
    #[error("no variant matched (missing fallback variant)")]
    MissingFallbackVariant,
    /// The id is not in this catalog. Nothing is written.
    #[error("missing message")]
    MissingMessage,
    /// The message's catalog record is malformed: `{�}` in its place, and
    /// the message ends there.
    #[error("malformed catalog record")]
    Malformed,
}

impl FormatError {
    /// The MF2 error kind (`mf2_model::ErrorKind`); `None` for
    /// [`FormatError::MissingMessage`] and [`FormatError::Malformed`], which
    /// are about the catalog, not the message.
    pub const fn kind(self) -> Option<ErrorKind> {
        Some(match self {
            FormatError::UnresolvedVariable => ErrorKind::UnresolvedVariable,
            FormatError::UnknownFunction => ErrorKind::UnknownFunction,
            FormatError::BadSelector => ErrorKind::BadSelector,
            FormatError::BadOperand => ErrorKind::BadOperand,
            FormatError::BadOption => ErrorKind::BadOption,
            FormatError::BadVariantKey => ErrorKind::BadVariantKey,
            FormatError::UnsupportedOperation => ErrorKind::UnsupportedOperation,
            FormatError::MessageFunctionError => ErrorKind::MessageFunctionError,
            FormatError::MissingFallbackVariant => ErrorKind::MissingFallbackVariant,
            FormatError::MissingMessage | FormatError::Malformed => return None,
        })
    }
}
