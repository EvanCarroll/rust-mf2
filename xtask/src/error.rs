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

    #[error("L4: {0}")]
    L4(String),
    /// Conformance layer L6 in the browser failed.
    #[error("{0}")]
    L6(String),
    /// Conformance layer L7 failed, or its ledger columns do not hold.
    #[error("l7-web: {0}")]
    L7(String),
    /// `leptos-mf2` on the Leptos 0.9 pre-release, or its 0.8 refusal, failed.
    #[error("leptos-beta: {0}")]
    LeptosBeta(String),

    #[error(
        "wasmtime {0} is not installed in target/tools; run \
         `cargo install --root target/tools wasmtime-cli --version {0} --locked`"
    )]
    WasmtimeMissing(&'static str),

    #[error("target/tools/bin/wasmtime is `{got}`, not the pinned {want}")]
    WasmtimeVersion { want: &'static str, got: String },

    #[error("locale data: {0}")]
    LocaleData(#[from] mf2_locale_data::Error),

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

    #[error(
        "the CLDR cache (target/xtask-cache/cldr-json) differs from the pinned blobs: {0}; \
         run `cargo xtask cldr-sync`"
    )]
    CacheMismatch(String),

    #[error(
        "the CLDR cache (target/xtask-cache/cldr-json) is {0}; the all-locale number tables \
         are generated from it: run `cargo xtask cldr-sync` first"
    )]
    CacheMissing(String),

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

    #[error("ci step failed: {0}")]
    CiStepFailed(String),

    #[error("the conformance ledger has {0} violation(s)")]
    LedgerViolations(usize),

    #[error("{0} already exists; pass --force to overwrite it")]
    LedgerExists(PathBuf),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
