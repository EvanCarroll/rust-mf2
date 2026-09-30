//! `cargo xtask refusals`: `plans/19-native-and-terminal.md` §3's refusals,
//! from both sides (Phase 10: B1's review fixes, and the browser-only
//! refusal, `plans/18-phase-10-work-order.md` question 24).
//!
//! **Refused.** Each combination of `mf2`'s features that §3 refuses is a
//! compile error that says what to write, and the only error the user
//! reads. Each case runs `cargo check` and must fail. Its one error must
//! carry `mf2`'s sentence, whichever crate cargo compiles first: with two
//! modes and a Leptos line, that is the line's helper crate
//! (`mf2-leptos-ui-0-9` or `-0-8`), which `mf2` depends on and which
//! therefore says `mf2`'s words. Every other `error` line must be cargo's
//! own "could not compile".
//!
//! **Compiled.** What §3 refuses only when compiling for the browser
//! (`wasm32`) compiles on the host: cargo unifies features across the
//! packages it builds together, so `cargo check --workspace` over a browser
//! client and a native application turns both on, as 1.x allowed. Each
//! host case runs `cargo check` natively and must pass.
//!
//! `cargo xtask ci` runs it. A feature that joins the refusals adds a case
//! on each side: `ratatui`, which implies `native`, has a sentence of its
//! own (Phase 10 B3), so that the one error names what an application
//! turned on, and `axum` adds its own (D1).

use std::path::Path;
use std::process::{Command, Output};

use crate::cmd::cargo;
use crate::error::{Error, Result};

/// Two modes, one of them the server's.
const TWO_MODES: &str = "mf2: turn on exactly one of `ssr`, `hydrate` and `csr`. cargo unifies \
                         features across a workspace";

/// The two client modes together.
const ONE_MODE: &str = "mf2: turn on exactly one of `ssr`, `hydrate` and `csr`.";

/// Both Leptos lines.
const BOTH_LINES: &str = "mf2: both Leptos lines are on, `leptos` (Leptos 0.9) and `leptos-0-8`";

/// `native` in a browser build.
const NATIVE: &str = "mf2: `native` is on beside `hydrate` or `csr` in a build for the browser";

/// `ratatui` in a browser build: its own sentence, not `native`'s, though
/// it implies `native`.
const RATATUI: &str = "mf2: `ratatui` is on beside `hydrate` or `csr` in a build for the browser";

/// `axum` in a browser build.
const AXUM: &str = "mf2: `axum` is on beside `hydrate` or `csr` in a build for the browser";

const WASM: &str = "wasm32-unknown-unknown";

/// A refused combination: what it is, its `cargo check` arguments, and what
/// its one error must say.
struct Case {
    what: &'static str,
    args: &'static [&'static str],
    says: &'static str,
}

const CASES: &[Case] = &[
    Case {
        what: "two modes, on Leptos 0.9",
        args: &["-p", "mf2", "--features", "leptos,ssr,hydrate"],
        says: TWO_MODES,
    },
    Case {
        what: "two modes, on Leptos 0.8",
        args: &["-p", "mf2", "--features", "leptos-0-8,ssr,hydrate"],
        says: TWO_MODES,
    },
    Case {
        what: "the two client modes",
        args: &["-p", "mf2", "--features", "leptos,hydrate,csr"],
        says: ONE_MODE,
    },
    Case {
        what: "both Leptos lines",
        args: &["-p", "mf2", "--features", "ssr,leptos,leptos-0-8"],
        says: BOTH_LINES,
    },
    Case {
        what: "`ssr` with no line",
        args: &["-p", "mf2", "--features", "ssr"],
        says: "mf2: `ssr` needs a Leptos line",
    },
    Case {
        what: "`hydrate` with no line",
        args: &["-p", "mf2", "--features", "hydrate"],
        says: "mf2: `hydrate` needs a Leptos line",
    },
    Case {
        what: "`csr` with no line",
        args: &["-p", "mf2", "--features", "csr"],
        says: "mf2: `csr` needs a Leptos line",
    },
    Case {
        what: "`native` beside `hydrate`, for the browser",
        args: &[
            "-p",
            "mf2",
            "--features",
            "native,leptos,hydrate",
            "--target",
            WASM,
        ],
        says: NATIVE,
    },
    Case {
        what: "`native` beside `csr`, for the browser",
        args: &[
            "-p",
            "mf2",
            "--features",
            "native,leptos,csr",
            "--target",
            WASM,
        ],
        says: NATIVE,
    },
    Case {
        what: "`ratatui` beside `hydrate`, for the browser",
        args: &[
            "-p",
            "mf2",
            "--features",
            "ratatui,leptos,hydrate",
            "--target",
            WASM,
        ],
        says: RATATUI,
    },
    Case {
        what: "`ratatui` beside `csr`, for the browser",
        args: &[
            "-p",
            "mf2",
            "--features",
            "ratatui,leptos,csr",
            "--target",
            WASM,
        ],
        says: RATATUI,
    },
    Case {
        what: "`axum` beside `hydrate`, for the browser",
        args: &[
            "-p",
            "mf2",
            "--features",
            "axum,leptos,hydrate",
            "--target",
            WASM,
        ],
        says: AXUM,
    },
    Case {
        what: "`axum` beside `csr`, for the browser",
        args: &[
            "-p",
            "mf2",
            "--features",
            "axum,leptos,csr",
            "--target",
            WASM,
        ],
        says: AXUM,
    },
];

/// A combination §3 refuses only for the browser: what it is, and its
/// `cargo check` arguments on the host, where it must compile.
struct Host {
    what: &'static str,
    args: &'static [&'static str],
}

const HOST: &[Host] = &[
    Host {
        what: "`native` beside `hydrate`, on the host",
        args: &["-p", "mf2", "--features", "native,leptos,hydrate"],
    },
    Host {
        what: "`native` beside `csr`, on the host",
        args: &["-p", "mf2", "--features", "native,leptos,csr"],
    },
    Host {
        what: "`ratatui` beside `hydrate`, on the host",
        args: &["-p", "mf2", "--features", "ratatui,leptos,hydrate"],
    },
    Host {
        what: "`ratatui` beside `csr`, on the host",
        args: &["-p", "mf2", "--features", "ratatui,leptos,csr"],
    },
    Host {
        what: "`axum` beside `hydrate`, on the host",
        args: &["-p", "mf2", "--features", "axum,leptos,hydrate"],
    },
    Host {
        what: "`axum` beside `csr`, on the host",
        args: &["-p", "mf2", "--features", "axum,leptos,csr"],
    },
    // 1.x's workspace: a browser client on `leptos-mf2` and a native
    // application on `mf2-native`, checked together as `--workspace` does.
    Host {
        what: "`leptos-mf2` in `csr` beside `mf2-native`, on the host",
        args: &[
            "-p",
            "leptos-mf2",
            "-p",
            "mf2-native",
            "--features",
            "leptos-mf2/csr",
        ],
    },
    // …and with a terminal UI on `mf2-ratatui` beside the client.
    Host {
        what: "`leptos-mf2` in `csr` beside `mf2-ratatui`, on the host",
        args: &[
            "-p",
            "leptos-mf2",
            "-p",
            "mf2-ratatui",
            "--features",
            "leptos-mf2/csr",
        ],
    },
];

fn fail(message: impl Into<String>) -> Error {
    Error::Refusals(message.into())
}

/// `cargo check` with `args`, in `root`; `shown` names it in an error.
fn check(root: &Path, args: &[&str], shown: &str) -> Result<Output> {
    Command::new(cargo())
        .arg("check")
        .args(args)
        .args(["--color", "never"])
        .current_dir(root)
        .output()
        .map_err(|source| Error::Spawn {
            program: shown.to_owned(),
            source,
        })
}

/// The `error` lines of a cargo run, but cargo's own "could not compile".
fn errors(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| line.starts_with("error"))
        .filter(|line| !line.starts_with("error: could not compile"))
        .map(str::to_owned)
        .collect()
}

pub(crate) fn run(root: &Path) -> Result<()> {
    for case in CASES {
        let shown = format!("cargo check {}", case.args.join(" "));
        eprintln!("==> refused: {} ({shown})", case.what);
        let output = check(root, case.args, &shown)?;
        if output.status.success() {
            return Err(fail(format!("`{shown}` compiled; {} must not", case.what)));
        }
        let found = errors(&output);
        match found.as_slice() {
            [only] if only.contains(case.says) => eprintln!("    {only}"),
            _ => {
                return Err(fail(format!(
                    "`{shown}` fails, but not with {:?} alone; its errors:\n{}",
                    case.says,
                    found.join("\n")
                )));
            }
        }
    }
    for case in HOST {
        let shown = format!("cargo check {}", case.args.join(" "));
        eprintln!("==> compiles: {} ({shown})", case.what);
        let output = check(root, case.args, &shown)?;
        if !output.status.success() {
            return Err(fail(format!(
                "`{shown}` fails; {} must compile, as only a browser build refuses it; \
                 its errors:\n{}",
                case.what,
                errors(&output).join("\n")
            )));
        }
    }
    eprintln!(
        "==> refusals: each of the {} is mf2's one sentence, and the {} host combinations compile",
        CASES.len(),
        HOST.len()
    );
    Ok(())
}
