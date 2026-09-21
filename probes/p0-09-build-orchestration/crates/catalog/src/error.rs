//! Decoding errors.

/// A manifest or catalog file could not be decoded.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Wrong magic bytes.
    #[error("bad magic: expected {expected}")]
    Magic {
        /// The expected magic.
        expected: &'static str,
    },
    /// Unsupported format version.
    #[error("unsupported version {0}")]
    Version(u8),
    /// The input ended early.
    #[error("truncated input")]
    Truncated,
    /// A string was not UTF-8.
    #[error("invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    /// An unknown tag byte.
    #[error("unknown tag {0}")]
    Tag(u8),
}
