//! `cargo xtask refusals`: each combination of `mf2`'s features that
//! `plans/19-native-and-terminal.md` §3 refuses is a compile error that says
//! what to write, and the only error the user reads (Phase 10, B1's review
//! fixes).
//!
//! Each case runs `cargo check` and must fail. Its one error must carry
//! `mf2`'s sentence, whichever crate cargo compiles first: with two modes
//! and a Leptos line, that is the line's helper crate (`mf2-leptos-ui-0-9`
//! or `-0-8`), which `mf2` depends on and which therefore says `mf2`'s
//! words. Every other `error` line must be cargo's own "could not compile".
//!
//! `cargo xtask ci` runs it. A feature that joins the refusals adds a case:
//! `ratatui` inherits `native`'s (Phase 10 B3), and `axum` adds its own (D1).

use std::path::Path;
use std::process::Command;

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
const NATIVE: &str = "mf2: `native` is on beside `hydrate` or `csr`";

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
];

fn fail(message: impl Into<String>) -> Error {
    Error::Refusals(message.into())
}

pub(crate) fn run(root: &Path) -> Result<()> {
    for case in CASES {
        let shown = format!("cargo check {}", case.args.join(" "));
        eprintln!("==> refused: {} ({shown})", case.what);
        let output = Command::new(cargo())
            .arg("check")
            .args(case.args)
            .args(["--color", "never"])
            .current_dir(root)
            .output()
            .map_err(|source| Error::Spawn {
                program: shown.clone(),
                source,
            })?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            return Err(fail(format!("`{shown}` compiled; {} must not", case.what)));
        }
        let errors: Vec<&str> = stderr
            .lines()
            .filter(|line| line.starts_with("error"))
            .filter(|line| !line.starts_with("error: could not compile"))
            .collect();
        match errors.as_slice() {
            [only] if only.contains(case.says) => eprintln!("    {only}"),
            _ => {
                return Err(fail(format!(
                    "`{shown}` fails, but not with {:?} alone; its errors:\n{}",
                    case.says,
                    errors.join("\n")
                )));
            }
        }
    }
    eprintln!(
        "==> refusals: each of the {} is mf2's one sentence",
        CASES.len()
    );
    Ok(())
}
