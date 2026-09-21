//! Catalog installation errors.

/// An embedded catalog could not be installed.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// Undecodable bytes.
    #[error("catalog does not decode: {0}")]
    Decode(#[from] p09_catalog::DecodeError),
    /// Built against another manifest (deploy skew, plans/02 F6).
    #[error("catalog `{locale}` has manifest {found:016x}, the binary expects {expected:016x}")]
    Skew {
        /// Locale.
        locale: String,
        /// Hash in the catalog.
        found: u64,
        /// Hash compiled in.
        expected: u64,
    },
    /// Installed twice.
    #[error("catalogs already installed")]
    Twice,
}
