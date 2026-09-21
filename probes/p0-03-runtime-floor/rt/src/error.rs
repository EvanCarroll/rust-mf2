//! Error values. Plain enums: the client never formats them (B12) — their
//! `Display` impls (thiserror) exist for server/dev use and are unreachable
//! from the formatting paths.

use thiserror::Error;

/// Why `Catalog::new` rejected a buffer (F4, F6, F9).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
pub enum CatalogError {
    #[error("not a .mf2b file")]
    Magic,
    #[error("unsupported format version")]
    Version,
    #[error("unsupported layout flags")]
    Layout,
    #[error("catalog compiled against a different manifest")]
    ManifestMismatch,
    #[error("header or section table out of bounds")]
    Header,
    #[error("sections overlap, are unordered, or the pool is not last")]
    Sections,
    #[error("a required section is missing")]
    MissingSection,
    #[error("INDEX size or entries invalid")]
    Index,
    #[error("LOCALE or FUNCS section malformed")]
    Tables,
    #[error("string pool is not valid UTF-8")]
    Utf8,
    #[error("allocation failed")]
    Alloc,
}

/// Formatting errors (the suite's error kinds used here). Formatting never
/// fails: these are reported alongside the output.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
pub enum FormatError {
    #[error("unresolved variable")]
    UnresolvedVariable,
    #[error("unknown function")]
    UnknownFunction,
    #[error("bad operand")]
    BadOperand,
    #[error("bad option")]
    BadOption,
    #[error("bad selector")]
    BadSelector,
    #[error("unsupported operation")]
    UnsupportedOperation,
    #[error("message is absent from the catalog")]
    MissingMessage,
    #[error("catalog data is corrupt at this message")]
    Corrupt,
}
