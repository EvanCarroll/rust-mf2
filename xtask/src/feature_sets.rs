//! One table of the feature sets this workspace compiles (`plan/01` §6.3).
//!
//! `ci`, `codegen-matrix`, `msrv` and `refusals` each used to hand-list the
//! sets they build, so the same combination was written in two or three
//! places and a new feature was added to one list and forgotten in the
//! others. Every set is now written here once, with the target it is
//! compiled for, whether it must build or must be refused, and which
//! commands use it. The four commands read the table and add only their own
//! shape of invocation (clippy with `-D warnings`, a `cargo check` of the
//! fixture, `rustup run <msrv> cargo check`, a check that must fail).
//!
//! A powerset is not practical: the modes exclude each other, several sets
//! exist for one target only, and a few carry the test targets that only
//! they build. What the table does give is one place to add a feature to,
//! and `cargo xtask feature-sets`, which compiles each of `mf2`'s features
//! alone — with the Leptos line or the target that feature needs, which the
//! table states — the way a dependent that turns on one thing does.

use crate::error::Result;

/// The browser target.
pub(crate) const WASM: &str = "wasm32-unknown-unknown";

/// Which target a set is compiled for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// The machine the command runs on.
    Host,
    /// `wasm32-unknown-unknown`.
    Wasm,
}

impl Target {
    /// The `--target` triple, or `None` for the host.
    pub(crate) fn triple(self) -> Option<&'static str> {
        match self {
            Target::Host => None,
            Target::Wasm => Some(WASM),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Target::Host => "host",
            Target::Wasm => "wasm",
        }
    }
}

/// Whether a set must compile, or must be refused by one sentence of `mf2`'s.
pub(crate) enum Outcome {
    Builds,
    /// The whole of the one error the user must read.
    Refused(&'static str),
}

/// What `cargo xtask ci` runs for a set, in order.
pub(crate) enum CiStep {
    /// `cargo clippy … -- -D warnings` (`--all-targets` on the host).
    Clippy,
    /// `cargo test …` with these arguments after the features.
    Test(&'static [&'static str]),
}

/// Which list of `cargo xtask codegen-matrix` a set belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    /// What an application's `ssr` build forwards.
    Server,
    /// What a native application's build forwards (Phase 10 C4).
    Native,
    /// What its `hydrate` build forwards.
    Client,
}

/// A command that uses a set, with what that command needs of it.
pub(crate) enum Use {
    /// `cargo xtask ci`, with the steps it runs.
    Ci(&'static [CiStep]),
    /// `cargo xtask codegen-matrix`, in one of its three lists.
    CodegenMatrix(Group),
    /// `cargo xtask msrv`, as a step with this name.
    Msrv(&'static str),
    /// `cargo xtask refusals`, on whichever side `outcome` says.
    Refusals,
    /// `cargo xtask feature-sets`: one `mf2` feature alone (nightly).
    Alone,
}

impl Use {
    fn label(&self) -> &'static str {
        match self {
            Use::Ci(_) => "ci",
            Use::CodegenMatrix(_) => "codegen-matrix",
            Use::Msrv(_) => "msrv",
            Use::Refusals => "refusals",
            Use::Alone => "feature-sets",
        }
    }
}

/// One feature set: the packages cargo selects, the features turned on, the
/// target, what must happen, and the commands that use it.
pub(crate) struct Set {
    /// What the set is, in a line.
    pub(crate) what: &'static str,
    pub(crate) packages: &'static [&'static str],
    pub(crate) features: &'static str,
    pub(crate) target: Target,
    pub(crate) no_default_features: bool,
    pub(crate) outcome: Outcome,
    pub(crate) uses: &'static [Use],
}

impl Set {
    /// `-p P…`, `--no-default-features` when the set asks for it, and
    /// `--features F`: the arguments every command shares.
    pub(crate) fn selection(&self) -> Vec<&'static str> {
        let mut args = Vec::with_capacity(self.packages.len() * 2 + 3);
        for package in self.packages {
            args.push("-p");
            args.push(package);
        }
        if self.no_default_features {
            args.push("--no-default-features");
        }
        args.push("--features");
        args.push(self.features);
        args
    }

    /// The steps `ci` runs for this set, if it uses it.
    pub(crate) fn ci(&self) -> Option<&'static [CiStep]> {
        self.uses.iter().find_map(|u| match u {
            Use::Ci(steps) => Some(*steps),
            _ => None,
        })
    }

    /// The `codegen-matrix` list this set is in, if any.
    pub(crate) fn group(&self) -> Option<Group> {
        self.uses.iter().find_map(|u| match u {
            Use::CodegenMatrix(group) => Some(*group),
            _ => None,
        })
    }

    /// The name of the `msrv` step this set is, if it is one.
    pub(crate) fn msrv(&self) -> Option<&'static str> {
        self.uses.iter().find_map(|u| match u {
            Use::Msrv(what) => Some(*what),
            _ => None,
        })
    }

    fn has(&self, label: &str) -> bool {
        self.uses.iter().any(|u| u.label() == label)
    }

    /// Whether `refusals` uses this set.
    pub(crate) fn refusals(&self) -> bool {
        self.has("refusals")
    }

    /// The sentence a refused set's one error must carry.
    pub(crate) fn refused(&self) -> Option<&'static str> {
        match self.outcome {
            Outcome::Refused(says) => Some(says),
            Outcome::Builds => None,
        }
    }

    /// Whether this set is one `mf2` feature alone.
    pub(crate) fn alone(&self) -> bool {
        self.has("feature-sets")
    }
}

/// `mf2` alone: what most sets select.
const MF2: &[&str] = &["mf2"];

/// The fixture i18n crate `codegen-matrix` compiles.
pub(crate) const FIXTURE: &str = "mf2-i18n-fixture";

/// A set of `mf2`'s features that must build.
const fn mf2(
    target: Target,
    features: &'static str,
    uses: &'static [Use],
    what: &'static str,
) -> Set {
    Set {
        what,
        packages: MF2,
        features,
        target,
        no_default_features: false,
        outcome: Outcome::Builds,
        uses,
    }
}

/// A set of other packages' features that must build.
const fn pkgs(
    packages: &'static [&'static str],
    target: Target,
    features: &'static str,
    no_default_features: bool,
    uses: &'static [Use],
    what: &'static str,
) -> Set {
    Set {
        what,
        packages,
        features,
        target,
        no_default_features,
        outcome: Outcome::Builds,
        uses,
    }
}

/// A combination of `mf2`'s features that must not compile, and the whole of
/// the one error the user must read.
const fn refused(
    target: Target,
    features: &'static str,
    says: &'static str,
    what: &'static str,
) -> Set {
    Set {
        what,
        packages: MF2,
        features,
        target,
        no_default_features: false,
        outcome: Outcome::Refused(says),
        uses: &[Use::Refusals],
    }
}

/// One `mf2` feature alone, with the line or the target it needs to be valid.
const fn alone(target: Target, features: &'static str, what: &'static str) -> Set {
    mf2(target, features, &[Use::Alone], what)
}

/// Every feature an application can turn on at once for its server, across
/// the 17 published crates (the first `msrv` step; also what
/// `cargo xtask package --test` tests the unpacked packages with).
pub(crate) const SERVER_FEATURES: &str = "mf2/compile,mf2/leptos-server-number-builtin,mf2/axum-number-builtin,mf2/native-number-builtin,mf2/leptos-server-datetime-icu,mf2/axum-datetime-icu,mf2/native-datetime-icu,mf2/host-std,\
     mf2/leptos,mf2/ssr,mf2/axum,mf2/static-locale,mf2/mark-fallback-lang,mf2/native,mf2/ratatui,mf2-catalog/decode,mf2-catalog/static-bytes,\
     mf2-locale-data/extract,mf2-cli/icu-blob,mf2-model/serde,mf2-resource/serde,\
     mf2-runtime/fixed-decimal";

/// The 14 of the 17 published crates the first `msrv` step checks together
/// (the web host and the Leptos 0.8 helper are the second's; the browser's
/// ICU4X crate is a browser build's dependency only).
const MSRV_SERVER: &[&str] = &[
    "mf2",
    "mf2-cli",
    "mf2-build",
    "mf2-catalog",
    "mf2-locale-data",
    "mf2-model",
    "mf2-resource",
    "mf2-runtime",
    "mf2-syntax",
    "mf2-macros",
    "mf2-host-std",
    "mf2-fn-number",
    "mf2-fn-datetime",
    "mf2-leptos-ui-0-9",
];

use CiStep::{Clippy, Test};
use Group::{Client, Native, Server};
use Target::{Host, Wasm};
use Use::{CodegenMatrix, Msrv};

/// Every set, once. `ci` runs the browser sets, then the workspace's tests,
/// then the host sets, in this order.
pub(crate) const SETS: &[Set] = &[
    // The sets only a browser build compiles, which `--workspace` never
    // reaches: it unifies `ssr` in (the conformance crate turns it on), so
    // nothing else ever compiles the client half — the hydration cursor,
    // the boot, the fetch — or the client-only options.
    pkgs(
        &["mf2-runtime", "mf2-fn-number", "mf2-host-web"],
        Wasm,
        "mf2-runtime/web-number-intl,mf2-host-web/number-intl",
        false,
        &[Use::Ci(&[Clippy])],
        "the browser's `intl` number formatter, whose code compiles for the browser target only",
    ),
    pkgs(
        &["mf2-runtime", "mf2-fn-number", "mf2-host-web"],
        Wasm,
        "mf2-runtime/web-number-intl,mf2-runtime/web-number-builtin,mf2-host-web/number-intl",
        false,
        &[Use::Ci(&[Clippy])],
        "both browser number backends, where our own code must win (a compile-time check)",
    ),
    pkgs(
        &["mf2-fn-datetime"],
        Wasm,
        "mf2-fn-datetime/web-icu,mf2-fn-datetime/web-intl",
        false,
        &[Use::Ci(&[Clippy])],
        "both browser date formatters, where ICU4X must win (a compile-time check)",
    ),
    mf2(
        Wasm,
        "leptos,hydrate",
        &[Use::Ci(&[Clippy])],
        "the client half of the Leptos layer",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,static-locale",
        &[Use::Ci(&[Clippy])],
        "`static-locale` (the islands default), which changes what the registry and the glue compile",
    ),
    mf2(
        Wasm,
        "leptos,csr",
        &[Use::Ci(&[Clippy])],
        "`csr`, whose boot is its own: the index, the stored locale, `navigator.languages`",
    ),
    mf2(
        Wasm,
        "leptos,csr,static-locale",
        &[Use::Ci(&[Clippy])],
        "`csr` with `static-locale`, where a switch reloads rather than writing a cookie",
    ),
    // Both sides' date features at once, as an application writes them on
    // its one `mf2` line: a server's feature must change nothing in the
    // browser's build, which `mf2`'s own compile-time checks hold
    // (`crates/mf2/src/agreement.rs`).
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-datetime-icu,leptos-server-datetime-icu",
        &[Use::Ci(&[Clippy])],
        "ICU4X dates on both sides: the server's formatter cache stays out of the browser",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-datetime-intl,leptos-server-datetime-icu",
        &[Use::Ci(&[Clippy])],
        "`Intl` dates beside an ICU4X server: the server's load number stays out of the browser",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-datetime-icu-cached,leptos-server-datetime-icu",
        &[Use::Ci(&[Clippy])],
        "ICU4X dates in the browser with the formatter cache it asked for",
    ),
    // Both sides' number features at once, the same way: each build formats
    // with its own side's, and `mf2` checks at compile time that the runtime
    // asks the browser's `Intl` exactly where the browser's formatter is
    // `intl`.
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-number-intl,leptos-server-number-builtin",
        &[Use::Ci(&[Clippy])],
        "`Intl` numbers in the browser beside a server on mf2's own number code",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-number-builtin,leptos-client-number-intl,leptos-server-number-plain",
        &[Use::Ci(&[Clippy])],
        "both of the browser's number formatters on: mf2's own code formats, and `Intl` is not asked",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-number-plain,leptos-server-number-builtin",
        &[Use::Ci(&[Clippy])],
        "plain digits in the browser beside a server on mf2's own number code",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,mark-fallback-lang",
        &[Use::Ci(&[Clippy])],
        "`mark-fallback-lang`'s second shape of the text glue: the adopted wrapper, the fitted span",
    ),
    mf2(
        Wasm,
        "leptos,csr,static-locale,mark-fallback-lang",
        &[Use::Ci(&[Clippy])],
        "`mark-fallback-lang` where unregistered nodes fit the wrapper through a rebuild",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-datetime-iso",
        &[Use::Ci(&[Clippy])],
        "the reader's time zone: the boot's correction and the glue's hydration queue",
    ),
    mf2(
        Wasm,
        "leptos,hydrate,leptos-client-datetime-iso,static-locale,mark-fallback-lang",
        &[Use::Ci(&[Clippy])],
        "the reader's time zone where the queue, not the registry, holds the nodes",
    ),
    mf2(
        Wasm,
        "leptos,csr,leptos-client-datetime-iso",
        &[Use::Ci(&[Clippy])],
        "the mount's zone, for `csr`",
    ),
    mf2(
        Wasm,
        "host-web",
        &[Use::Ci(&[Clippy])],
        "the call-site core with no mode: what a Leptos-free client compiles",
    ),
    // The host sets. Each is one a plain `--workspace` build does not
    // compile, because cargo unifies `ssr` into `mf2` across the workspace.
    pkgs(
        &["mf2-fn-datetime"],
        Host,
        "mf2-fn-datetime/compiled-data",
        false,
        &[Use::Ci(&[Clippy, Test(&["--test", "icu"])])],
        "ICU4X's compiled-in data, which no feature of `mf2` turns on: the blob must equal it byte for byte",
    ),
    mf2(
        Host,
        "leptos,csr,compile,host-std",
        &[Use::Ci(&[Test(&["--test", "churn"])])],
        "the client half's conversions under churn, natively (the browser's measurement is `cargo xtask churn`)",
    ),
    mf2(
        Host,
        "leptos,ssr,compile,mark-fallback-lang",
        &[Use::Ci(&[Clippy])],
        "the server's half of `mark-fallback-lang`",
    ),
    mf2(
        Host,
        "leptos,ssr,mark-fallback-lang",
        &[Use::Ci(&[Test(&["--test", "fallback_lang"])])],
        "`mark-fallback-lang`'s server test and its catalogs that borrow (the browser's is `demo.mjs`)",
    ),
    mf2(
        Host,
        "native,compile",
        &[Use::Ci(&[
            Clippy,
            Test(&[
                "--lib",
                "--test",
                "native",
                "--test",
                "ambient",
                "--test",
                "uninstalled",
                "--test",
                "from_directory",
                "--test",
                "locale_allocations",
            ]),
        ])],
        "the native module as a native application builds it: the store before and after `install`, and a language chosen with no allocation",
    ),
    mf2(
        Host,
        "native,compile,native-datetime-iso",
        &[Use::Ci(&[Test(&["--test", "system_zone"])])],
        "the system's zone as a POSIX rule, in a child process whose `TZ` is one",
    ),
    mf2(
        Host,
        "native,native-datetime-iso",
        &[Use::Ci(&[Clippy, Test(&["--test", "zone_db"])])],
        "a native application's zone lookups, which read the machine's IANA database",
    ),
    mf2(
        Host,
        "native,native-datetime-iso,tzdb-bundled",
        &[Use::Ci(&[Clippy, Test(&["--test", "zone_db"])])],
        "`tzdb-bundled`, where they read jiff's bundled database instead (what `ssr` and `axum` turn on)",
    ),
    mf2(
        Host,
        "leptos,csr,native,compile",
        &[Use::Ci(&[
            Clippy,
            Test(&["--test", "lookup", "--test", "ambient"]),
        ])],
        "the one lookup with a client mode beside `native`, on the host, where the two compile together",
    ),
    mf2(
        Host,
        "ratatui,compile",
        &[Use::Ci(&[Clippy, Test(&["--test", "ratatui"])])],
        "the Ratatui module as a terminal UI builds it, with no Leptos layer beside it",
    ),
    mf2(
        Host,
        "axum,compile",
        &[Use::Ci(&[Clippy, Test(&["--lib"])])],
        "the Axum module as a plain Axum server builds it, with no Leptos layer beside it",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "axum",
        false,
        &[Use::Ci(&[Clippy, Test(&["--test", "axum"])])],
        "a plain Axum application answering per `Accept-Language` through the generated `Locale`",
    ),
    pkgs(
        &["mf2-resource"],
        Host,
        "serde",
        false,
        &[Use::Ci(&[Test(&[])])],
        "`mf2-resource`'s optional `serde`, which nothing in the workspace turns on",
    ),
    // `codegen-matrix`: the module `mf2-build` generates, compiled in every
    // combination of the facade's features. The server and native lists are
    // compiled for the host, the client list for the browser; the last rows
    // of each also turn on `mf2`'s Leptos layer, on each line.
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-plain",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, numbers in plain digits",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-builtin",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, numbers by mf2's own code",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-plain,host-std-datetime-iso",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, dates",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-plain,host-std-datetime-icu",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, dates on ICU4X",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-builtin,host-std-datetime-iso",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, both function crates",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-builtin,host-std-datetime-icu",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, both with ICU4X dates",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-builtin,host-std-datetime-iso,mf2/leptos,mf2/ssr",
        true,
        &[CodegenMatrix(Server)],
        "the generated module beside the description types' Leptos impls, 0.9",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-plain,host-std-datetime-iso,mf2/leptos-0-8,mf2/ssr,mf2/mark-fallback-lang",
        true,
        &[CodegenMatrix(Server)],
        "the generated module beside the Leptos layer, 0.8, marking borrowed text",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "native,host-std-number-plain",
        true,
        &[CodegenMatrix(Native)],
        "the module a native application includes (`Emit::Native`)",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "native,host-std-number-builtin,mf2/clap",
        true,
        &[CodegenMatrix(Native)],
        "the native module with a command line's value parser",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "native,host-std-number-builtin,host-std-datetime-icu,mf2/ratatui",
        true,
        &[CodegenMatrix(Native)],
        "the native module for a terminal UI, with every function",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "native,ssr,host-std-number-builtin,mf2/leptos,mf2/ssr",
        true,
        &[CodegenMatrix(Native)],
        "the native module beside the Leptos layer's server, from one table of embedded bytes",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-plain,host-std-datetime-iso,mf2/leptos,mf2/ssr,mf2/native",
        true,
        &[CodegenMatrix(Native)],
        "a web module with `native` turned on beside `ssr`",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, numbers in plain digits",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-builtin",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, numbers by mf2's own code",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-intl",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, numbers through the browser's Intl",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,host-web-datetime-iso",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, dates",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,host-web-datetime-icu",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, dates on ICU4X",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,host-web-datetime-icu-cached",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, dates on ICU4X with its formatter cache",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,host-web-datetime-intl",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, dates through the browser's Intl",
    ),
    pkgs(
        &[FIXTURE],
        Host,
        "ssr,host-std-number-builtin,host-web-number-intl",
        true,
        &[CodegenMatrix(Server)],
        "the generated module, server, with the browser's number formatter on beside its own",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-intl,host-std-number-builtin",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, with the server's number formatter on beside its own",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,host-std-number-builtin",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client in plain digits beside a server's own code",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-intl,host-web-datetime-intl",
        true,
        &[CodegenMatrix(Client)],
        "the generated module, client, every function through the browser",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-builtin,host-web-datetime-iso,mf2/leptos,mf2/hydrate",
        true,
        &[CodegenMatrix(Client)],
        "the generated module beside the Leptos layer's client, 0.9",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,mf2/leptos,mf2/csr,mf2/static-locale",
        true,
        &[CodegenMatrix(Client)],
        "the generated module in a `csr` application with no live update",
    ),
    pkgs(
        &[FIXTURE],
        Wasm,
        "hydrate,host-web-number-plain,host-web-datetime-iso,mf2/leptos-0-8,mf2/hydrate",
        true,
        &[CodegenMatrix(Client)],
        "the generated module beside the Leptos layer's client, 0.8",
    ),
    // `msrv`: the published crates' library targets on the oldest Rust they
    // claim — what an application compiles, against the working tree's lock
    // file.
    pkgs(
        MSRV_SERVER,
        Host,
        SERVER_FEATURES,
        false,
        &[Msrv("native, Leptos 0.9, every server feature")],
        "every feature an application can turn on at once for its server",
    ),
    pkgs(
        &["mf2", "mf2-leptos-ui-0-8"],
        Host,
        "mf2/ssr,mf2/axum,mf2/leptos-0-8",
        true,
        &[Msrv("native, Leptos 0.8")],
        "the Leptos 0.8 opt-in and its helper crate",
    ),
    pkgs(
        &["mf2", "mf2-host-web"],
        Wasm,
        "mf2/leptos,mf2/hydrate,mf2/leptos-client-number-intl,mf2/leptos-client-datetime-icu",
        false,
        &[Msrv("wasm32, hydrate, ICU4X dates")],
        "the client with ICU4X dates and numbers through the browser's `Intl`",
    ),
    pkgs(
        MF2,
        Wasm,
        "mf2/leptos,mf2/csr,mf2/leptos-client-number-intl,mf2/leptos-client-datetime-intl",
        false,
        &[Msrv("wasm32, csr, Intl numbers and dates")],
        "the client built in the browser, formatting through the browser",
    ),
    pkgs(
        MF2,
        Wasm,
        "hydrate,leptos-0-8,leptos-client-datetime-iso",
        false,
        &[Msrv("wasm32, hydrate, Leptos 0.8")],
        "the client on the Leptos 0.8 opt-in",
    ),
    // `refusals`: each combination `mf2` refuses is one compile error that
    // says what to write, and the only error the user reads. With two modes
    // and a Leptos line that error comes from the line's helper crate, which
    // `mf2` depends on and which therefore says `mf2`'s words.
    refused(
        Host,
        "leptos,ssr,hydrate",
        TWO_MODES,
        "two modes, on Leptos 0.9",
    ),
    refused(
        Host,
        "leptos-0-8,ssr,hydrate",
        TWO_MODES,
        "two modes, on Leptos 0.8",
    ),
    refused(Host, "leptos,hydrate,csr", ONE_MODE, "the two client modes"),
    refused(
        Host,
        "ssr,leptos,leptos-0-8",
        BOTH_LINES,
        "both Leptos lines",
    ),
    refused(
        Host,
        "ssr",
        "mf2: `ssr` needs a Leptos line",
        "`ssr` with no line",
    ),
    refused(
        Host,
        "hydrate",
        "mf2: `hydrate` needs a Leptos line",
        "`hydrate` with no line",
    ),
    refused(
        Host,
        "csr",
        "mf2: `csr` needs a Leptos line",
        "`csr` with no line",
    ),
    refused(
        Wasm,
        "native,leptos,hydrate",
        NATIVE,
        "`native` beside `hydrate`, for the browser",
    ),
    refused(
        Wasm,
        "native,leptos,csr",
        NATIVE,
        "`native` beside `csr`, for the browser",
    ),
    refused(
        Wasm,
        "ratatui,leptos,hydrate",
        RATATUI,
        "`ratatui` beside `hydrate`, for the browser",
    ),
    refused(
        Wasm,
        "ratatui,leptos,csr",
        RATATUI,
        "`ratatui` beside `csr`, for the browser",
    ),
    refused(
        Wasm,
        "axum,leptos,hydrate",
        AXUM,
        "`axum` beside `hydrate`, for the browser",
    ),
    refused(
        Wasm,
        "axum,leptos,csr",
        AXUM,
        "`axum` beside `csr`, for the browser",
    ),
    // What `mf2` refuses only for the browser compiles on the host: cargo
    // unifies features across the packages it builds together, so a check
    // over a browser client and a native application turns both on.
    mf2(
        Host,
        "native,leptos,hydrate",
        &[Use::Refusals],
        "`native` beside `hydrate`, on the host",
    ),
    mf2(
        Host,
        "native,leptos,csr",
        &[Use::Refusals],
        "`native` beside `csr`, on the host",
    ),
    mf2(
        Host,
        "ratatui,leptos,hydrate",
        &[Use::Refusals],
        "`ratatui` beside `hydrate`, on the host",
    ),
    mf2(
        Host,
        "ratatui,leptos,csr",
        &[Use::Refusals],
        "`ratatui` beside `csr`, on the host",
    ),
    mf2(
        Host,
        "axum,leptos,hydrate",
        &[Use::Refusals],
        "`axum` beside `hydrate`, on the host",
    ),
    mf2(
        Host,
        "axum,leptos,csr",
        &[Use::Refusals],
        "`axum` beside `csr`, on the host",
    ),
    // `cargo xtask feature-sets` (nightly): each of `mf2`'s features alone,
    // as a dependent that turns on one thing compiles it. A mode needs a
    // Leptos line, and the browser's host and the client-only options need
    // the browser target; everything else is valid by itself.
    alone(Host, "compile", "`compile`: the one-message compiler"),
    // The number formatters: the framework-free families, of which each
    // framework's is another name.
    alone(
        Host,
        "host-std-number-plain",
        "`host-std-number-plain`: native numbers in plain digits",
    ),
    alone(
        Host,
        "host-std-number-builtin",
        "`host-std-number-builtin`: native numbers by mf2's own code",
    ),
    alone(
        Wasm,
        "host-web-number-plain",
        "`host-web-number-plain`: the browser's numbers in plain digits",
    ),
    alone(
        Wasm,
        "host-web-number-intl",
        "`host-web-number-intl`: the browser's numbers through `Intl`",
    ),
    alone(
        Wasm,
        "host-web-number-builtin",
        "`host-web-number-builtin`: the browser's numbers by mf2's own code",
    ),
    // The date formatters: the framework-free families, of which each
    // framework's is another name (`plan/08` §3.4).
    alone(
        Host,
        "host-std-datetime-iso",
        "`host-std-datetime-iso`: native dates, the ISO stand-in",
    ),
    alone(
        Host,
        "host-std-datetime-icu",
        "`host-std-datetime-icu`: native dates over ICU4X",
    ),
    alone(
        Wasm,
        "host-web-datetime-iso",
        "`host-web-datetime-iso`: the browser's dates, the ISO stand-in",
    ),
    alone(
        Wasm,
        "host-web-datetime-intl",
        "`host-web-datetime-intl`: the browser's dates through `Intl`",
    ),
    alone(
        Wasm,
        "host-web-datetime-icu",
        "`host-web-datetime-icu`: the browser's dates over ICU4X",
    ),
    alone(
        Wasm,
        "host-web-datetime-icu-cached",
        "`host-web-datetime-icu-cached`: the same, with ICU4X's formatter cache",
    ),
    // An application's whole line, for each kind of application: what
    // `mf2 init` and `mf2 check` write, and the same application on the
    // smallest formatters. With the rows above, every family's feature for
    // every formatter is compiled under its own name (a test below holds
    // the table to that).
    alone(
        Host,
        "leptos,ssr,leptos-client-number-intl,leptos-server-number-builtin,leptos-client-datetime-intl,leptos-server-datetime-icu",
        "a server-rendered Leptos application's line, on the server",
    ),
    alone(
        Host,
        "leptos,ssr,leptos-client-number-plain,leptos-server-number-plain,leptos-client-datetime-iso,leptos-server-datetime-iso",
        "a server-rendered Leptos application on the smallest formatters, on the server",
    ),
    alone(
        Wasm,
        "leptos,csr,leptos-client-number-builtin,leptos-client-datetime-icu",
        "a client-only Leptos application with the same text as a server on mf2's own code and ICU4X",
    ),
    alone(
        Host,
        "native,native-number-builtin,native-datetime-icu",
        "a command-line tool's line",
    ),
    alone(
        Host,
        "native,native-number-plain,native-datetime-iso",
        "a command-line tool on the smallest formatters",
    ),
    alone(
        Host,
        "axum,axum-number-builtin,axum-datetime-icu",
        "an Axum server's line",
    ),
    alone(
        Host,
        "axum,axum-number-plain,axum-datetime-iso",
        "an Axum server on the smallest formatters",
    ),
    alone(Host, "host-std", "`host-std`: the native host"),
    alone(
        Wasm,
        "host-web",
        "`host-web`: the browser's host, which needs the browser target",
    ),
    alone(
        Host,
        "native",
        "`native`: a native application's catalogs, languages and zone",
    ),
    alone(
        Host,
        "ratatui",
        "`ratatui`: a terminal UI's text (implies `native`)",
    ),
    alone(Host, "axum", "`axum`: an Axum server"),
    alone(
        Host,
        "clap",
        "`clap`: a value parser for the generated `Locale`",
    ),
    alone(
        Host,
        "leptos",
        "`leptos`: the Leptos 0.9 line, with no mode yet",
    ),
    alone(
        Host,
        "leptos-0-8",
        "`leptos-0-8`: the Leptos 0.8 line, with no mode yet",
    ),
    alone(Host, "ssr,leptos", "`ssr`, which needs a Leptos line"),
    alone(
        Wasm,
        "hydrate,leptos",
        "`hydrate`, which needs a Leptos line and the browser target",
    ),
    alone(
        Wasm,
        "csr,leptos",
        "`csr`, which needs a Leptos line and the browser target",
    ),
    alone(Host, "static-locale", "`static-locale`: no live update"),
    alone(
        Host,
        "mark-fallback-lang",
        "`mark-fallback-lang`: borrowed text inside a `<span lang>`",
    ),
];

/// Two modes, one of them the server's.
const TWO_MODES: &str = "mf2: turn on exactly one of `ssr`, `hydrate` and `csr`. cargo unifies \
                         features across a workspace";

/// The two client modes together.
const ONE_MODE: &str = "mf2: turn on exactly one of `ssr`, `hydrate` and `csr`.";

/// Both Leptos lines.
const BOTH_LINES: &str = "mf2: both Leptos lines are on, `leptos` (Leptos 0.9) and `leptos-0-8`";

/// `native` in a browser build.
const NATIVE: &str = "mf2: `native` is on beside `hydrate` or `csr` in a build for the browser";

/// `ratatui` in a browser build: its own sentence, not `native`'s, though it
/// implies `native`.
const RATATUI: &str = "mf2: `ratatui` is on beside `hydrate` or `csr` in a build for the browser";

/// `axum` in a browser build.
const AXUM: &str = "mf2: `axum` is on beside `hydrate` or `csr` in a build for the browser";

/// The sets one command uses, in the table's order.
pub(crate) fn used_by(mut keep: impl FnMut(&'static Set) -> bool) -> Vec<&'static Set> {
    SETS.iter().filter(|set| keep(set)).collect()
}

/// `cargo xtask feature-sets`: each of `mf2`'s features alone, checked on
/// the target it needs. Nightly, not `ci`: it is one `cargo check` per
/// feature.
pub(crate) fn check_each(root: &std::path::Path) -> Result<()> {
    let cargo = crate::cmd::cargo();
    let sets = used_by(Set::alone);
    for set in &sets {
        let mut args: Vec<&str> = vec!["check"];
        if let Some(triple) = set.target.triple() {
            args.push("--target");
            args.push(triple);
        }
        args.extend(set.selection());
        eprintln!("==> feature-sets: {}", set.what);
        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        crate::cmd::run_inherit(&cargo, &args, root)?;
    }
    eprintln!(
        "feature-sets: each of the {} features of `mf2` compiles alone",
        sets.len()
    );
    Ok(())
}

/// `cargo xtask feature-sets --list`: the whole table.
pub(crate) fn list() {
    for set in SETS {
        let uses: Vec<&str> = set.uses.iter().map(Use::label).collect();
        let outcome = match set.outcome {
            Outcome::Builds => "builds",
            Outcome::Refused(_) => "refused",
        };
        println!(
            "{:4}  {outcome}  {}\n      {}  [{}]",
            set.target.label(),
            set.selection().join(" "),
            set.what,
            uses.join(", ")
        );
    }
    println!("{} sets", SETS.len());
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use mf2_build::{
        Backend, Build, DateBackend, Emit, Features, NumberBackend, catalog, domain_features,
    };

    use super::{FIXTURE, Outcome, SETS, Set};

    /// The i18n fixture's corpora: its own, and its two size variants.
    fn corpora() -> [PathBuf; 3] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/i18n-fixture");
        [
            fixture.clone(),
            fixture.join("variants/plain"),
            fixture.join("variants/measures"),
        ]
    }

    /// The `mf2` features a set turns on, as `mf2-build` reads them: `mf2`'s
    /// own names, `mf2/<name>` from any package, and the fixture's features
    /// through its `[features]` table (`ssr` is `mf2/host-std`, `hydrate`
    /// `mf2/host-web`, the rest `mf2`'s of the same name). Other packages'
    /// features change no catalog.
    fn mf2_features(set: &Set) -> Vec<String> {
        let fixture = set.packages.contains(&FIXTURE);
        let mf2 = set.packages.contains(&"mf2");
        let mut names = Vec::new();
        for name in set.features.split(',').map(str::trim) {
            if let Some(feature) = name.strip_prefix("mf2/") {
                names.push(feature.to_owned());
            } else if name.is_empty() || name.contains('/') {
            } else if fixture {
                match name {
                    "ssr" => names.push("host-std".to_owned()),
                    "hydrate" => names.push("host-web".to_owned()),
                    // The corpus and where the catalogs go: every corpus
                    // and both kinds of build are checked anyway.
                    "split-catalogs" | "corpus-plain" | "corpus-measures" | "corpus-dates" => {}
                    other => names.push(other.to_owned()),
                }
            } else if mf2 {
                names.push(name.to_owned());
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// Every feature of the table of families and formatters is compiled
    /// somewhere: each is named in a set that must build, under its own
    /// name, and each framework-free one is also a feature of the i18n
    /// fixture, so that the generated module can be built with it. A
    /// feature added to `mf2` and to the table fails here until it has both.
    #[test]
    fn every_formatter_feature_has_a_row_and_a_forwarder() {
        let named: BTreeSet<&str> = SETS
            .iter()
            .filter(|set| matches!(set.outcome, Outcome::Builds))
            .flat_map(|set| set.features.split(','))
            .map(|feature| feature.trim().rsplit('/').next().unwrap_or(""))
            .collect();
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/i18n-fixture/Cargo.toml");
        let manifest: toml::Table = std::fs::read_to_string(&manifest)
            .expect("the fixture's manifest")
            .parse()
            .expect("a manifest");
        let forwarded = manifest["features"].as_table().expect("[features]");
        let features = domain_features::<NumberBackend>()
            .into_iter()
            .chain(domain_features::<DateBackend>())
            // A domain's own feature is turned on by each of the others.
            .filter(|f| f != NumberBackend::DOMAIN && f != DateBackend::DOMAIN);
        let mut missing = Vec::new();
        for feature in features {
            if !named.contains(feature.as_str()) {
                missing.push(format!("`{feature}` is in no set that must build"));
            }
            if feature.starts_with("host-") {
                let forwards = forwarded
                    .get(&feature)
                    .and_then(toml::Value::as_array)
                    .is_some_and(|on| {
                        on.iter()
                            .any(|f| f.as_str() == Some(&format!("mf2/{feature}")))
                    });
                if !forwards {
                    missing.push(format!(
                        "`{feature}` is not a feature of tools/i18n-fixture that turns on mf2's"
                    ));
                }
            }
        }
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }

    /// `unread-data` (`plan/08` §7) over every feature set of the table that
    /// must build and every corpus of the fixture, as a web build and as a
    /// native application's (the check reads the emit only for that): the
    /// build makes the check itself, and each catalog is checked again here.
    #[test]
    fn nothing_is_written_unread_in_any_feature_set() {
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/tmp/unread-data");
        std::fs::create_dir_all(&out).expect("mkdir");
        let mut seen = BTreeSet::new();
        let mut checked = 0usize;
        let mut tabled = 0usize;
        for set in SETS
            .iter()
            .filter(|set| matches!(set.outcome, Outcome::Builds))
        {
            let names = mf2_features(set);
            for (emit, native) in [(Emit::Both, false), (Emit::Native, true)] {
                if !seen.insert((names.join(","), native)) {
                    continue;
                }
                let features = Features::from_names(names.iter().cloned());
                let downloaded = !native && features.has_browser_side();
                for corpus in corpora() {
                    let outcome = Build::at(&corpus, &out)
                        .features(features.clone())
                        .emit(emit)
                        .check()
                        .unwrap_or_else(|e| {
                            panic!(
                                "{} ({}), {}: {e}",
                                set.what,
                                names.join(","),
                                corpus.display()
                            )
                        });
                    for built in &outcome.catalogs {
                        catalog::check_read(
                            &built.tag,
                            &built.locale_entries,
                            &built.server_entries,
                            &features,
                            downloaded,
                        )
                        .unwrap_or_else(|e| panic!("{} ({}): {e}", set.what, names.join(",")));
                        checked += 1;
                        if !built.server_entries.is_empty() {
                            tabled += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 0, "no feature set wrote a catalog");
        assert!(tabled > 0, "no feature set wrote a server-only table");
    }
}
