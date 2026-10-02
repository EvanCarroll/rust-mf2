//! `cargo xtask msrv`: the 16 published crates built on the oldest Rust they
//! claim (Phase 9 A3; `docs/versioning.md`).
//!
//! The MSRV is `rust-version` in `[workspace.package]`, which the 16 inherit
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
//! The steps are the `msrv` rows of `feature_sets::SETS` (`plan/01` §6.3):
//!
//! 1. natively on Leptos 0.9, all 16 but the web host and the 0.8 helper, with every feature an application can
//!    turn on at once for its server (`ssr`, `axum`, the native and Ratatui
//!    modules, both function crates with the ICU4X date backend, the
//!    compiler, the CLI's `icu-blob`, `serde`, …);
//! 2. natively on the Leptos 0.8 opt-in, `mf2` (with `axum`) and its 0.8
//!    helper;
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

use crate::feature_sets::{self, WASM};

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
    let sets = feature_sets::used_by(|set| set.msrv().is_some());
    let sets = if below { &sets[..1] } else { &sets[..] };
    for set in sets {
        eprintln!(
            "==> msrv: Rust {toolchain}, {}",
            set.msrv().unwrap_or_default()
        );
        let mut args: Vec<&str> = vec!["run", toolchain.as_str(), "cargo", "check"];
        if let Some(triple) = set.target.triple() {
            args.push("--target");
            args.push(triple);
        }
        args.extend(set.selection());
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
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
                    "Rust {toolchain} builds the 16: `rust-version = \"{msrv}\"` is not \
                     the oldest (measure again and lower it)"
                )));
            }
        }
    }
    eprintln!("msrv: the 16 build on Rust {toolchain}");
    Ok(())
}

/// `rust-version` as the 16 state it (`mf2`'s; the metadata test holds
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

/// The toolchain and the wasm target (rustup, only when either is missing).
fn install(root: &Path, toolchain: &str) -> Result<()> {
    crate::cmd::rustup_install(root, toolchain, WASM)
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
