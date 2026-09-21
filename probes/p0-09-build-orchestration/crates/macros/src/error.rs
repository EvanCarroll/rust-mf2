//! Why the manifest could not be used.

/// A manifest problem, reported as a `compile_error!` at the call site.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The baked path does not exist or cannot be read.
    #[error(
        "mf2: cannot read the i18n manifest at `{path}` ({source}), nor at the same place under this \
         compilation's target directory. The i18n crate's build script writes it to its OUT_DIR; rebuild the \
         i18n crate (`cargo clean -p <i18n crate>`) or use P09_MANIFEST_MODE=inline"
    )]
    Read {
        /// The baked path.
        path: String,
        /// The I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The inline manifest literal is not a byte string.
    #[error("mf2: bad inline manifest literal: {0}")]
    Literal(String),
    /// The file is not a manifest.
    #[error("mf2: corrupt i18n manifest: {0}")]
    Decode(#[from] p09_catalog::DecodeError),
    /// The file changed after the wrapper was generated.
    #[error(
        "mf2: stale i18n manifest `{path}`: it has hash {found:016x}, but the i18n crate was compiled against \
         {expected:016x}; rebuild the i18n crate"
    )]
    Stale {
        /// Path or cache key.
        path: String,
        /// Hash in the file.
        found: u64,
        /// Hash baked into the wrapper.
        expected: u64,
    },
}
