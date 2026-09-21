//! Errors for `cargo xtask`.

use std::io;
use std::path::PathBuf;

/// Everything that can make an xtask command fail.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Conformance(#[from] mf2_conformance::Error),

    #[error("catalog writer: {0}")]
    CatalogWrite(#[from] mf2_catalog::WriteError),

    #[error("output of `{0}` is not UTF-8")]
    Utf8(String, #[source] std::string::FromUtf8Error),

    #[error("{path}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("could not run `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },

    #[error("`{command}` failed ({status}){stderr}")]
    CommandFailed {
        command: String,
        status: String,
        stderr: String,
    },

    #[error("{path}: invalid JSON: {message}")]
    Json { path: PathBuf, message: String },

    #[error("{path}: malformed PIN file: {message}")]
    Pin { path: PathBuf, message: String },

    #[error("invalid revision {0:?}: expected a full 40-hex-digit commit sha")]
    BadRevision(String),

    #[error("upstream {commit} does not contain {path}")]
    UpstreamMissing { commit: String, path: String },

    #[error("upstream {path} at {commit} is a {kind}, not a regular file")]
    UpstreamNotAFile {
        commit: String,
        path: String,
        kind: String,
    },

    #[error("tag {tag} resolves to {actual}, but the PIN says commit {expected}")]
    TagMismatch {
        tag: String,
        expected: String,
        actual: String,
    },

    #[error(
        "third_party/message-format-wg differs from upstream {commit} in {count} place(s); \
         run `cargo xtask spec-sync` to re-vendor it"
    )]
    SpecMismatch { commit: String, count: usize },

    #[error(
        "resource-sync is blocked: the W3C Message Resource draft states no license \
         (third_party/w3c-message-resource/PIN), so nothing may be vendored from it until \
         the owner confirms one; nothing was fetched"
    )]
    ResourceSyncBlocked,

    #[error(
        "size is not implemented yet: the size gate arrives in Phase 5 \
         (plans/06-size-and-perf.md); nothing was measured"
    )]
    SizeNotImplemented,

    #[error("ci step failed: {0}")]
    CiStepFailed(String),

    #[error("the conformance ledger has {0} violation(s)")]
    LedgerViolations(usize),

    #[error("{0} already exists; pass --force to overwrite it")]
    LedgerExists(PathBuf),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
