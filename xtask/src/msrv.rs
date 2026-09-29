//! `cargo xtask msrv`: the 20 published crates built on the oldest Rust they
//! claim (`plans/17-phase-9-work-order.md` A3; `docs/versioning.md`).
//!
//! The MSRV is `rust-version` in `[workspace.package]`, which the 20 inherit
//! (A1's metadata test holds them to it). It was measured, not assumed:
//! Rust 1.88, since Leptos 0.9.0-beta, its 0.8 line and several of their
//! dependencies declare 1.88, and our own code uses `let` chains and
//! `<[T]>::as_chunks`, both stable from 1.88.
//!
//! The toolchain is installed through rustup if it is missing, with
//! `wasm32-unknown-unknown`, and each step is a `cargo check` of the library
//! targets — what an application compiles; tests and dev-dependencies may
//! need a newer Rust — into `target/msrv/<toolchain>`, against the working
//! tree's lock file:
//!
//! 1. natively on Leptos 0.9, all 20 with every feature an application can
//!    turn on at once for its server (`ssr`, the native module, both
//!    function crates with the ICU4X date backend, the compiler, the CLI's
//!    `icu-blob`, `serde`, …);
//! 2. natively on the Leptos 0.8 opt-in, `mf2`, its 0.8 helper, the
//!    `leptos-mf2` shim and `mf2-axum`;
//! 3. `wasm32-unknown-unknown`, `hydrate` with the ICU4X date backend;
//! 4. `wasm32-unknown-unknown`, `csr` with the `intl` option and the `Intl`
//!    date backend;
//! 5. `wasm32-unknown-unknown`, `hydrate` on Leptos 0.8.
//!
//! `--below` is the negative control: the release before the MSRV must fail
//! step 1 (it did, on 1.87, 2026-09-25: dependencies requiring 1.88 refused
//! up front; with `--ignore-rust-version`, our crates' own `let` chains).

use std::ffi::OsStr;
use std::path::Path;

use crate::cmd::{cargo, run_capture, run_inherit_env};
use crate::error::{Error, Result};

const WASM: &str = "wasm32-unknown-unknown";

/// Every feature an application can turn on at once for its server, across
/// the 20 (step 1; also what `cargo xtask package --test` tests the unpacked
/// packages with).
pub(crate) const SERVER_FEATURES: &str = "mf2/compile,mf2/fn-number,mf2/datetime-icu,mf2/host-std,\
     mf2/leptos,mf2/ssr,mf2/static-locale,mf2/mark-fallback-lang,mf2/native,mf2-catalog/decode,mf2-catalog/static-bytes,\
     mf2-locale-data/extract,mf2-cli/icu-blob,mf2-model/serde,mf2-resource/serde,\
     mf2-runtime/fixed-decimal";

/// Every step's `cargo check` arguments, after `check`.
const STEPS: [(&str, &[&str]); 5] = [
    (
        "native, Leptos 0.9, every server feature",
        &[
            "-p",
            "mf2",
            "-p",
            "mf2-axum",
            "-p",
            "mf2-cli",
            "-p",
            "mf2-build",
            "-p",
            "mf2-catalog",
            "-p",
            "mf2-locale-data",
            "-p",
            "mf2-model",
            "-p",
            "mf2-resource",
            "-p",
            "mf2-runtime",
            "-p",
            "mf2-syntax",
            "-p",
            "mf2-macros",
            "-p",
            "mf2-host-std",
            "-p",
            "mf2-native",
            "-p",
            "mf2-ratatui",
            "-p",
            "mf2-fn-number",
            "-p",
            "mf2-fn-datetime",
            "-p",
            "leptos-mf2",
            "-p",
            "mf2-leptos-ui-0-9",
            "--features",
            SERVER_FEATURES,
        ],
    ),
    (
        "native, Leptos 0.8",
        &[
            "-p",
            "mf2",
            "-p",
            "mf2-leptos-ui-0-8",
            "-p",
            "leptos-mf2",
            "-p",
            "mf2-axum",
            "--no-default-features",
            "--features",
            "mf2/ssr,mf2/leptos-0-8,leptos-mf2/leptos-0-8,mf2-axum/leptos-0-8",
        ],
    ),
    (
        "wasm32, hydrate, ICU4X dates",
        &[
            "--target",
            WASM,
            "-p",
            "mf2",
            "-p",
            "mf2-host-web",
            "--features",
            "mf2/leptos,mf2/hydrate,mf2/fn-number,mf2/datetime-icu,mf2/intl",
        ],
    ),
    (
        "wasm32, csr, intl, Intl dates",
        &[
            "--target",
            WASM,
            "-p",
            "mf2",
            "--features",
            "mf2/leptos,mf2/csr,mf2/fn-number,mf2/datetime-intl,mf2/intl",
        ],
    ),
    (
        "wasm32, hydrate, Leptos 0.8",
        &[
            "--target",
            WASM,
            "-p",
            "mf2",
            "--features",
            "hydrate,leptos-0-8,fn-datetime",
        ],
    ),
];

fn fail(message: impl Into<String>) -> Error {
    Error::Msrv(message.into())
}

pub(crate) fn run(root: &Path, below: bool) -> Result<()> {
    let msrv = workspace_msrv(root)?;
    let toolchain = if below {
        previous(&msrv)?
    } else {
        msrv.clone()
    };
    install(root, &toolchain)?;
    let target_dir = root.join("target").join("msrv").join(&toolchain);
    let envs = [("CARGO_TARGET_DIR", target_dir.as_os_str())];
    let steps: &[(&str, &[&str])] = if below { &STEPS[..1] } else { &STEPS };
    for (what, args) in steps {
        eprintln!("==> msrv: Rust {toolchain}, {what}");
        let head = ["run", toolchain.as_str(), "cargo", "check"];
        let args: Vec<&OsStr> = head.iter().chain(args.iter()).map(OsStr::new).collect();
        let outcome = run_inherit_env(OsStr::new("rustup"), &args, root, &envs);
        match (outcome, below) {
            (Ok(()), false) => {}
            (Err(e), false) => return Err(e),
            (Err(_), true) => {
                eprintln!(
                    "msrv: Rust {toolchain} fails, as it must: {msrv} is the oldest that builds"
                );
                return Ok(());
            }
            (Ok(()), true) => {
                return Err(fail(format!(
                    "Rust {toolchain} builds the 20: `rust-version = \"{msrv}\"` is not \
                     the oldest (measure again and lower it)"
                )));
            }
        }
    }
    eprintln!("msrv: the 20 build on Rust {toolchain}");
    Ok(())
}

/// `rust-version` as the 20 state it (`mf2`'s; the metadata test holds
/// the others to it).
fn workspace_msrv(root: &Path) -> Result<String> {
    let args = ["metadata", "--no-deps", "--format-version", "1"].map(OsStr::new);
    let out = run_capture(&cargo(), &args, root, &[])?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&out).map_err(|e| fail(format!("`cargo metadata` output: {e}")))?;
    metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "mf2")
        .and_then(|p| p["rust_version"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| fail("`mf2` states no `rust-version`"))
}

/// The stable release before `version` (`1.88` → `1.87`).
fn previous(version: &str) -> Result<String> {
    let mut parts = version.split('.');
    let (Some(major), Some(minor)) = (parts.next(), parts.next()) else {
        return Err(fail(format!("rust-version {version:?} is not MAJOR.MINOR")));
    };
    match minor.parse::<u32>() {
        Ok(m) if m > 0 => Ok(format!("{major}.{}", m - 1)),
        _ => Err(fail(format!("no release before {version}"))),
    }
}

/// The toolchain and the wasm target, through rustup (a no-op when present).
fn install(root: &Path, toolchain: &str) -> Result<()> {
    let args = [
        "toolchain",
        "install",
        toolchain,
        "--profile",
        "minimal",
        "--target",
        WASM,
    ]
    .map(OsStr::new);
    run_capture(OsStr::new("rustup"), &args, root, &[]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::previous;

    #[test]
    fn the_release_before() {
        assert_eq!(previous("1.88").unwrap(), "1.87");
        assert_eq!(previous("1.100.2").unwrap(), "1.99");
        assert!(previous("1.0").is_err());
        assert!(previous("1").is_err());
    }
}
