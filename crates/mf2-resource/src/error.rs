//! Errors of `mf2-resource`'s fallible operations.
//!
//! Syntax errors are not Rust errors: they are [`crate::Diagnostic`]s, reported
//! alongside a best-effort result, exactly as `mf2-syntax` reports the
//! message-level ones. This type is for operations that cannot produce a
//! result at all — serializing a model the resource syntax cannot write.

/// A model the resource syntax cannot represent (see `serialize`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A value contains `\` before a character that is not one of the four
    /// MF2 escapes the container passes through. Every other `\` in a value
    /// is a container escape, so such a value cannot be written back.
    #[error(
        "a value contains `\\` before {0:?}; only `\\\\`, `\\{{`, `\\|` and `\\}}` pass through to the message"
    )]
    LoneBackslash(char),
    /// A value ends in `\`, which would read back as an escaped line break.
    #[error("a value ends in `\\`, which the resource syntax cannot represent")]
    TrailingBackslash,
    /// A character no escape can represent in this position: a raw control
    /// character or a line terminator in an id, a property name or a comment.
    #[error("{0} cannot contain {1:?}: the resource syntax has no escape for it there")]
    Unrepresentable(Role, char),
    /// An id with no parts, or a part with no characters.
    #[error("{0} is empty")]
    Empty(Role),
}

/// Which part of a resource could not be written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Role {
    /// An entry's or a section's id.
    Id,
    /// One dot-separated part of an id.
    IdPart,
    /// A property name (`@name`).
    PropertyName,
    /// A comment's text.
    Comment,
}

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Role::Id => "an id",
            Role::IdPart => "an id part",
            Role::PropertyName => "a property name",
            Role::Comment => "a comment",
        })
    }
}
