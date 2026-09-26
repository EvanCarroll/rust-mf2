//! Errors of `mf2-syntax`'s fallible operations.
//!
//! Syntax and data-model errors are not Rust errors: they are
//! [`mf2_model::Diagnostic`]s, reported alongside a best-effort result. This
//! type is for operations that cannot produce a result at all.

/// A data model that MF2 syntax cannot represent (see [`crate::serialize`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Text or a literal contains U+0000, which MF2 syntax cannot express
    /// (the grammar excludes it everywhere and has no escape for it).
    #[error("text or literal contains U+0000, which MF2 syntax cannot represent")]
    Nul,
    /// A variable, function, option, markup or attribute name is not an MF2
    /// name (or identifier).
    #[error("{0} is not a valid MF2 name")]
    InvalidName(NameRole),
    /// An input declaration's name differs from its variable's name.
    #[error("an input declaration's name differs from its variable's name")]
    InputNameMismatch,
    /// A select message has no selector (`.match` needs one).
    #[error("a select message needs at least one selector")]
    NoSelectors,
    /// A select message has no variant.
    #[error("a select message needs at least one variant")]
    NoVariants,
    /// A variant has no key.
    #[error("a variant needs at least one key")]
    NoKeys,
    /// A message, declaration, key, option value, pattern part or
    /// expression of a kind this version does not know (the model's enums
    /// are not exhaustive).
    #[error("a part of the message of an unknown kind")]
    UnknownNode,
}

/// Which kind of name was invalid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum NameRole {
    /// A variable (reference or declaration).
    Variable,
    /// A function identifier.
    Function,
    /// An option identifier.
    Option,
    /// A markup identifier.
    Markup,
    /// An attribute identifier.
    Attribute,
}

impl core::fmt::Display for NameRole {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            NameRole::Variable => "a variable name",
            NameRole::Function => "a function identifier",
            NameRole::Option => "an option identifier",
            NameRole::Markup => "a markup identifier",
            NameRole::Attribute => "an attribute identifier",
        })
    }
}
