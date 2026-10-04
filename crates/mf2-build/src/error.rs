//! Everything that can stop a build.
//!
//! A *diagnostic* — a syntax error in a locale file, a lint — is not one of
//! these: it is a [`Report`](crate::Report) entry with a file, a line and a
//! column, and a build fails on the count of them. This type is for what
//! stops the build before it can report anything: I/O, a malformed
//! `mf2.toml`, a catalog the writer refuses.

use std::io;
use std::path::PathBuf;

/// The result of a build step.
pub type Result<T> = std::result::Result<T, Error>;

/// What stops a build.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: io::Error,
    },

    /// `mf2.toml` is not valid TOML, or a key in it is wrong.
    #[error("{path}: {message}")]
    Config {
        /// The configuration file.
        path: PathBuf,
        /// Which key, and what is wrong with it.
        message: String,
    },

    /// A flat JSON corpus is not an object of strings.
    #[error("{path}:{line}:{column}: {message}")]
    Json {
        /// The file.
        path: PathBuf,
        /// One-based line.
        line: u32,
        /// One-based column.
        column: u32,
        /// What is wrong.
        message: String,
    },

    /// The locale directory names a tag the catalog format cannot write.
    #[error("locale {locale:?}: {source}")]
    Locale {
        /// The tag.
        locale: String,
        /// What the locale data said.
        #[source]
        source: mf2_locale_data::Error,
    },

    /// The catalog writer refused a locale's messages.
    #[error("catalog for {locale}: {source}")]
    Write {
        /// The tag.
        locale: String,
        /// What the writer said.
        #[source]
        source: mf2_catalog::WriteError,
    },

    /// The locale data could not be sliced.
    #[error(transparent)]
    LocaleData(#[from] mf2_locale_data::Error),

    /// The manifest could not be written or read back.
    #[error(transparent)]
    Manifest(#[from] mf2_catalog::ManifestError),

    /// A catalog could not be written.
    #[error(transparent)]
    Catalog(#[from] mf2_catalog::WriteError),

    /// A catalog could not be compressed.
    ///
    /// Both encoders write into a `Vec`, so this is a compressor fault, not a
    /// full disk. It is still an error rather than a shrug: a truncated
    /// `.br` would be served immutable beside a `.mf2b` whose content hash
    /// says it is intact, and the client would see a corrupt catalog.
    #[error("catalog for {locale}: {format}: {source}")]
    Compress {
        /// The tag.
        locale: String,
        /// `brotli` or `gzip`.
        format: &'static str,
        /// What the encoder said.
        #[source]
        source: io::Error,
    },

    /// The layout on disk is not what a build needs.
    #[error("{0}")]
    Layout(String),

    /// Two locales' tags give the generated `Locale` one variant: `pt-BR`
    /// and `pt_br` are both `Locale::PtBr`.
    #[error(
        "locales {first:?} and {second:?} both give the generated `Locale::{variant}`: rename one"
    )]
    LocaleVariant {
        /// The first of the two tags.
        first: String,
        /// The second.
        second: String,
        /// The variant they share.
        variant: String,
    },

    /// Two markup names have one hash, so a style could not tell them
    /// apart.
    #[error("markup names {first:?} and {second:?} have one hash: rename one")]
    MarkupHash {
        /// The first of the two.
        first: String,
        /// The second.
        second: String,
    },

    /// The build found errors in the corpus; they are in the report.
    #[error("{errors} error{} in {locales} locale{}", plural(*errors), plural(*locales))]
    Corpus {
        /// How many errors.
        errors: usize,
        /// In how many locales.
        locales: usize,
    },

    /// [`run`](crate::run) cannot see `mf2`'s features: cargo passes them
    /// only to a crate that names `mf2` as a normal dependency.
    #[error(
        "mf2-build: this crate does not name `mf2` in its [dependencies], so its \
         build script cannot see mf2's features (DEP_MF2_V3_FEATURES): add it \
         there, as the crate that includes the generated module needs it anyway"
    )]
    NoMf2,

    /// `mf2` and `mf2-build` are not the same version.
    #[error(
        "mf2-build {build} builds for mf2 {build}, but this crate's mf2 is {mf2}: \
         name the same version of both"
    )]
    Mf2Version {
        /// What `mf2` says its version is.
        mf2: String,
        /// This crate's.
        build: String,
    },

    /// A date formatter of `mf2` is `icu` on one side or both, and this
    /// `mf2-build` was built without
    /// `icu-blob`, which writes ICU4X's date data into the catalogs. A
    /// build-dependency's features cannot come through `links`, so the
    /// message names the line to write (`plan/08` §3.5).
    #[error(
        "mf2-build: the date formatter of {features} is ICU4X, whose catalogs carry ICU4X's \
         date data, which this `mf2-build` cannot write. In this crate's Cargo.toml, \
         under [build-dependencies], write: mf2-build = {{ version = \"{version}\", \
         features = [\"icu-blob\"] }}. It is off by default because it adds about \
         16 s to a cold build",
        version = env!("CARGO_PKG_VERSION")
    )]
    IcuBlob {
        /// The `icu` features that are on, quoted: `` `native-datetime-icu` ``.
        features: String,
    },

    /// `unread-data` (`plan/08` §7): after slicing, a LOCALE entry was
    /// placed where none of its readers looks — in the catalog a browser
    /// downloads when only native code reads it, in the server-only table
    /// when native code does not read it, or anywhere when nothing does.
    /// The placement is the build's own, so this is a fault of `mf2-build`,
    /// never of the corpus.
    #[error(
        "unread-data: catalog for {locale}: the `{entry}` entry would go {place}, where none \
         of its readers ({readers}) looks. This is a fault of mf2-build's placement, not of \
         the corpus: please report it with the features this build was given"
    )]
    UnreadData {
        /// The BCP 47 tag of the catalog.
        locale: String,
        /// The entry's name (`icu.blob`), or its key when it has none.
        entry: String,
        /// Where it would go.
        place: &'static str,
        /// Who reads it: the browser, native code, both, or nothing.
        readers: &'static str,
    },
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

impl Error {
    /// An I/O error that names its file.
    #[doc(hidden)]
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Error {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
