//! Error types.
//!
//! [`CatalogError`] is on the client path: a plain `#[repr(u8)]` enum that
//! the reader returns and never formats. Its `Display` impl (thiserror)
//! exists for servers and tools and is unreachable from the reader (B12,
//! checked in CI). The build-side errors live behind the features that
//! produce them.

use thiserror::Error;

/// Why [`crate::Catalog::new`] rejected a buffer (F4, F6, F9).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
#[repr(u8)]
#[non_exhaustive]
pub enum CatalogError {
    /// Not an `.mf2b` file: the magic bytes are missing or wrong.
    #[error("not an .mf2b catalog")]
    Magic,
    /// A major format version this reader does not know (F9).
    #[error("unsupported .mf2b format version")]
    Version,
    /// Compiled against another manifest: the wasm and the catalog disagree
    /// on ids, slots, markup or functions (F6). Reload.
    #[error("catalog was compiled against a different manifest")]
    ManifestMismatch,
    /// The buffer ends inside the header or the section table.
    #[error("catalog is truncated")]
    Truncated,
    /// A header field is invalid: message count, direction or the locale tag.
    #[error("invalid catalog header")]
    Header,
    /// Sections out of bounds, out of order or overlapping, a known kind
    /// twice, or STRINGS not last.
    #[error("invalid section table")]
    SectionTable,
    /// A required section is missing.
    #[error("a required section is missing")]
    MissingSection,
    /// INDEX has the wrong size or an entry out of bounds or out of order.
    #[error("invalid INDEX section")]
    Index,
    /// The LOCALE container or one of its plural entries is malformed.
    #[error("invalid LOCALE section")]
    Locale,
    /// FUNCS is malformed.
    #[error("invalid FUNCS section")]
    Funcs,
    /// FALLBACK is malformed.
    #[error("invalid FALLBACK section")]
    Fallback,
    /// NAMES is malformed.
    #[error("invalid NAMES section")]
    Names,
    /// The NFC section is not a whole canonical-equivalence map.
    #[error("invalid NFC section")]
    Nfc,
    /// IDS is malformed.
    #[error("invalid IDS section")]
    Ids,
    /// STRINGS does not end with a NUL byte.
    #[error("invalid STRINGS section")]
    Strings,
}

/// Why the writer refused a catalog.
#[cfg(feature = "writer")]
#[derive(Clone, PartialEq, Eq, Debug, Error)]
#[non_exhaustive]
pub enum WriteError {
    /// A string contains U+0000, which has no syntax form and no catalog form.
    #[error("string contains U+0000: {0:?}")]
    Nul(alloc::string::String),
    /// A variable is neither a local in scope nor in the message's slot list.
    #[error("message {message}: variable ${name} is not in its slot list")]
    UnknownVariable {
        /// The message's index (its `MsgId` index).
        message: usize,
        /// The variable's name as written.
        name: alloc::string::String,
    },
    /// A function is not in the manifest's function set.
    #[error("message {message}: function :{name} is not in the manifest")]
    UnknownFunction {
        /// The message's index.
        message: usize,
        /// The function's identifier as written.
        name: alloc::string::String,
    },
    /// An `.input` declaration whose name differs from its variable's.
    #[error("message {message}: .input declares {name:?} for variable {variable:?}")]
    InputName {
        /// The message's index.
        message: usize,
        /// The declaration's name.
        name: alloc::string::String,
        /// Its variable's name.
        variable: alloc::string::String,
    },
    /// A data-model node this format version cannot represent.
    #[error("message {message}: unsupported data-model node")]
    Unsupported {
        /// The message's index.
        message: usize,
    },
    /// The manifest and the messages do not fit together, or the manifest's
    /// lists are not strictly ascending.
    #[error("manifest: {0}")]
    Manifest(&'static str),
    /// The header direction must be `ltr` or `rtl`.
    #[error("catalog direction must be ltr or rtl")]
    Dir,
    /// A LOCALE entry is duplicated or malformed.
    #[error("LOCALE entry {0}: duplicate or malformed")]
    LocaleEntry(u32),
    /// A fallback entry names a message twice or out of range, or there are
    /// more than 256 fallback locales.
    #[error("fallback: {0}")]
    Fallback(&'static str),
    /// Beyond the format's limits (2²⁴ messages, 2³⁰ bytes of MESSAGES or
    /// STRINGS, 2³² for counts).
    #[error("too large: {0}")]
    TooLarge(&'static str),
    /// A writer bug: the two passes disagreed.
    #[error("internal writer error: {0}")]
    Internal(&'static str),
}

/// Why the decoder could not rebuild a message's model.
#[cfg(feature = "decode")]
#[derive(Clone, PartialEq, Eq, Debug, Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The id is out of range or the message is absent from the catalog.
    #[error("message is absent from the catalog")]
    Absent,
    /// The message's MESSAGES record is malformed.
    #[error("malformed MESSAGES record")]
    Malformed,
    /// A string reference is out of bounds or not valid UTF-8.
    #[error("invalid string reference")]
    String,
    /// The message's NAMES entry is missing or malformed, or a variable's
    /// index is outside it.
    #[error("invalid NAMES entry")]
    Names,
    /// A function index is outside FUNCS.
    #[error("function index out of range")]
    Function,
    /// The message's COLD record is malformed.
    #[error("malformed COLD record")]
    Cold,
}

/// Why a manifest file (`manifest.mf2m`) or value was rejected.
#[cfg(feature = "manifest")]
#[derive(Clone, PartialEq, Eq, Debug, Error)]
#[non_exhaustive]
pub enum ManifestError {
    /// Not a manifest file.
    #[error("not an .mf2m manifest")]
    Magic,
    /// A major version this reader does not know.
    #[error("unsupported manifest version")]
    Version,
    /// The file ends early, or a varint is malformed.
    #[error("manifest is truncated or malformed")]
    Truncated,
    /// A name is not valid UTF-8.
    #[error("manifest string is not UTF-8")]
    Utf8,
    /// The stored hash does not match the content.
    #[error("manifest hash does not match its content")]
    Hash,
    /// Bytes after the end of the manifest.
    #[error("trailing bytes after the manifest")]
    Trailing,
    /// A list is not strictly ascending, or the per-message lists do not
    /// match the ids.
    #[error("invalid manifest: {0}")]
    Invalid(&'static str),
}
