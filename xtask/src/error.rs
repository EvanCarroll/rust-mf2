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
    /// A sample in the user documentation is malformed or does not compile.
    #[error("docs: {0}")]
    Docs(String),
    /// The converted reference application is not what it must be.
    #[error("fluent-migrate: {0}")]
    FluentMigrate(String),
    /// The `leptos-fluent` A/B could not be built or measured.
    #[error("fluent-ab: {0}")]
    FluentAb(String),
    /// `mf2`'s Leptos layer on the Leptos 0.8 line, or the refusal of both
    /// lines at once, failed.
    #[error("leptos-0-8: {0}")]
    Leptos08(String),
    /// A combination of `mf2`'s features that must be refused compiled, or
    /// was refused with something other than `mf2`'s one sentence.
    #[error("refusals: {0}")]
    Refusals(String),
    /// The 20 do not build on their `rust-version`, or the release before
    /// it builds them too.
    #[error("msrv: {0}")]
    Msrv(String),
    /// A published crate's public API differs from its committed `api.txt`,
    /// or could not be listed.
    #[error("api: {0}")]
    Api(String),
    /// A published crate's package ships a file it must not, or differs
    /// from its committed `package.txt`, or its own tests fail.
    #[error("package: {0}")]
    Package(String),
    /// A published crate's documentation does not build as docs.rs builds
    /// it, warns, or has no front page pointing to the user guide.
    #[error("docs-rs: {0}")]
    DocsRs(String),
    /// A release check failed, or the publish was refused.
    #[error("release: {0}")]
    Release(String),
    /// The terminal UI's measurement could not be made, or did not repeat.
    #[error("tui-gate: {0}")]
    TuiGate(String),
    /// A feature set links a crate a row forbids, or none of a crate a row
    /// requires — or the canary application could not be linked or read.
    #[error("native-canaries: {0}")]
    NativeCanaries(String),
    /// A fresh measurement of a feature's cost is off from the committed
    /// table, or a row is missing from it.
    #[error("feature-costs: {0}")]
    FeatureCosts(String),

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
        "the MF2 specification text fetched at {commit} does not match the digests in \
         third_party/message-format-wg/PIN: {detail}; nothing was written"
    )]
    SpecDigest { commit: String, detail: String },

    #[error(
        "resource-sync is blocked: the W3C Message Resource draft states no license \
         (third_party/w3c-message-resource/PIN), so nothing may be vendored from it until \
         the owner confirms one; nothing was fetched"
    )]
    ResourceSyncBlocked,

    /// `cargo xtask xliff-sync` could not fetch, check or vendor the standard.
    #[error("xliff-sync: {0}")]
    Xliff(String),

    /// `cargo xtask uts35-sync` could not fetch or check UTS #35's text, or
    /// its PIN no longer goes with the vendored CLDR data.
    #[error("uts35-sync: {0}")]
    Uts35(String),

    #[error("ci step failed: {0}")]
    CiStepFailed(String),

    #[error("{} ci step(s) failed: {}", .0.len(), .0.join("; "))]
    CiStepsFailed(Vec<String>),

    #[error("the conformance ledger has {0} violation(s)")]
    LedgerViolations(usize),

    #[error("the spec coverage matrix has {0} gap(s) (conformance/COVERAGE.md)")]
    CoverageGaps(usize),

    #[error("{0} already exists; pass --force to overwrite it")]
    LedgerExists(PathBuf),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
