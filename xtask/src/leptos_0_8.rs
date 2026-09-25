//! `cargo xtask leptos-0-8`: the Leptos 0.8 opt-in, built and tested
//! (`plans/16-phase-8-work-order.md` A0; `plans/04-leptos-integration.md`
//! §10).
//!
//! Leptos 0.9 is the default line; 0.8 is `default-features = false,
//! features = ["leptos-0-8"]` on `leptos-mf2` and `mf2-axum`, which then
//! depend on the 0.8 crates under renamed names. Both lines are in the one
//! lock file, so this runs in the working tree.
//!
//! 1. **Both lines at once is an error that says what to write:** `leptos-0-8`
//!    beside the default features must fail, naming `default-features =
//!    false`.
//! 2. **On 0.8:** `leptos-mf2` linted for `ssr` natively (every target, with
//!    `mark-fallback-lang`) and for `hydrate` and `csr` on
//!    `wasm32-unknown-unknown`; its `render`, `churn` and `fallback_lang`
//!    tests; `mf2-axum`'s tests; and conformance layer L6 (the `layers` and
//!    `l6` tests of `mf2-conformance`).
//!
//! `--negative-control` runs step 2 on a copy of the tracked tree (under
//! `target/leptos-0-8/tree`) whose glue gives the two `to_html_with_buf`
//! impls their 0.9 form under `leptos-0-8`: it must fail.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cmd::{cargo, run_capture, run_inherit_env};
use crate::error::{Error, Result};
use crate::fsx;

/// What the error for both lines at once must say.
const BOTH: &str = "`default-features = false` beside `features = [\"leptos-0-8\"]`";

/// The glue whose line-switched impls the negative control swaps.
const GLUE: &str = "crates/leptos-mf2/src/glue/view.rs";

fn fail(message: impl Into<String>) -> Error {
    Error::Leptos08(message.into())
}

pub(crate) fn run(root: &Path, negative_control: bool) -> Result<()> {
    let (tree, envs) = if negative_control {
        let base = root.join("target").join("leptos-0-8");
        let tree = base.join("tree");
        eprintln!(
            "==> negative control: the tracked tree copied to {}",
            tree.display()
        );
        copy_tracked(root, &tree)?;
        swap_glue(&tree.join(GLUE))?;
        (tree, Some(base.join("target")))
    } else {
        eprintln!("==> both lines at once must be a compile error that says what to write");
        refuses_both(root)?;
        (root.to_path_buf(), None)
    };
    let envs: Vec<(&str, &OsStr)> = envs
        .as_ref()
        .map(|t| vec![("CARGO_TARGET_DIR", t.as_os_str())])
        .unwrap_or_default();

    let wasm = "wasm32-unknown-unknown";
    let deny = ["--", "-D", "warnings"];
    let mf2 = ["-p", "leptos-mf2", "--no-default-features", "--features"];
    let steps: [Vec<&str>; 9] = [
        [
            &["clippy"][..],
            &mf2,
            &["ssr,leptos-0-8,mark-fallback-lang", "--all-targets"],
            &deny,
        ]
        .concat(),
        [
            &["clippy", "--target", wasm][..],
            &mf2,
            &["hydrate,leptos-0-8"],
            &deny,
        ]
        .concat(),
        [
            &["clippy", "--target", wasm][..],
            &mf2,
            &["hydrate,leptos-0-8,mark-fallback-lang"],
            &deny,
        ]
        .concat(),
        [
            &["clippy", "--target", wasm][..],
            &mf2,
            &["csr,leptos-0-8,static-locale"],
            &deny,
        ]
        .concat(),
        [&["test"][..], &mf2, &["ssr,leptos-0-8", "--test", "render"]].concat(),
        [&["test"][..], &mf2, &["csr,leptos-0-8", "--test", "churn"]].concat(),
        [
            &["test"][..],
            &mf2,
            &[
                "ssr,leptos-0-8,mark-fallback-lang",
                "--test",
                "fallback_lang",
            ],
        ]
        .concat(),
        vec![
            "test",
            "-p",
            "mf2-axum",
            "--no-default-features",
            "--features",
            "leptos-0-8",
        ],
        vec![
            "test",
            "-p",
            "mf2-conformance",
            "--no-default-features",
            "--features",
            "leptos-0-8",
            "--test",
            "layers",
            "--test",
            "l6",
        ],
    ];
    let cargo = cargo();
    for (i, step) in steps.iter().enumerate() {
        let shown = format!("cargo {}", step.join(" "));
        eprintln!("==> (0.8) {shown}");
        let args: Vec<&OsStr> = step.iter().map(OsStr::new).collect();
        run_inherit_env(&cargo, &args, &tree, &envs)
            .map_err(|_| fail(format!("on Leptos 0.8: `{shown}` failed")))?;
        if i == 0 {
            report_resolved(&tree)?;
        }
    }
    if negative_control {
        return Err(fail(
            "the negative control passed: the 0.9 form of `to_html_with_buf` built on Leptos 0.8",
        ));
    }
    eprintln!(
        "==> leptos-0-8: leptos-mf2 lints for ssr, hydrate and csr and passes render, churn and \
         fallback_lang; mf2-axum's tests and layer L6 pass — all on Leptos 0.8"
    );
    Ok(())
}

/// Step 1: `leptos-0-8` with the default `leptos-0-9` still on.
fn refuses_both(root: &Path) -> Result<()> {
    let output = Command::new(cargo())
        .args([
            "check",
            "-p",
            "leptos-mf2",
            "--features",
            "ssr,leptos-0-8",
            "--color",
            "never",
        ])
        .current_dir(root)
        .output()
        .map_err(|source| Error::Spawn {
            program: "cargo check".to_owned(),
            source,
        })?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        return Err(fail(
            "`leptos-0-8` beside the default `leptos-0-9` compiled; it must be a compile error",
        ));
    }
    // From the first line that starts an error to the blank line ending it.
    let first = stderr
        .match_indices("error")
        .map(|(at, _)| at)
        .find(|&at| at == 0 || stderr[..at].ends_with('\n'))
        .map_or("", |at| {
            let rest = &stderr[at..];
            rest.find("\n\n").map_or(rest, |end| &rest[..end])
        });
    if !first.contains(BOTH) {
        return Err(fail(format!(
            "both lines at once fail, but the first error does not say {BOTH:?}:\n{first}"
        )));
    }
    eprintln!("{first}");
    Ok(())
}

/// The negative control's mutation: the `cfg` of each `to_html_with_buf`
/// impl negated, so that `leptos-0-8` compiles the 0.9 form. Fails if there
/// are not exactly the two pairs the glue is known to have.
fn swap_glue(path: &Path) -> Result<()> {
    const OLD: &str = "#[cfg(feature = \"leptos-0-8\")]";
    const NEW: &str = "#[cfg(not(feature = \"leptos-0-8\"))]";
    let text = fsx::read_to_string(path)?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut swapped = 0_usize;
    for (i, line) in lines.iter().enumerate() {
        let before_impl = lines
            .get(i + 1)
            .is_some_and(|next| next.trim_start().starts_with("fn to_html_with_buf("));
        let attr = line.trim();
        if before_impl && (attr == OLD || attr == NEW) {
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push_str(indent);
            out.push_str(if attr == OLD { NEW } else { OLD });
            out.push('\n');
            swapped += 1;
        } else {
            out.push_str(line);
        }
    }
    if swapped != 4 {
        return Err(fail(format!(
            "{}: expected the two `to_html_with_buf` impls in both forms (4 `cfg`s), found {swapped}",
            path.display()
        )));
    }
    fsx::write(path, out.as_bytes())
}

/// Copies every tracked file that exists in the working tree to `to`, which
/// is emptied first.
fn copy_tracked(root: &Path, to: &Path) -> Result<()> {
    fsx::remove(to)?;
    let listed = run_capture(
        OsStr::new("git"),
        &[OsStr::new("ls-files"), OsStr::new("-z")],
        root,
        &[],
    )?;
    let mut copied = 0_usize;
    for rel in listed.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let rel = String::from_utf8_lossy(rel);
        let from = root.join(rel.as_ref());
        // A tracked file deleted in the working tree is deleted in the copy.
        if !from.is_file() {
            continue;
        }
        let dest = to.join(rel.as_ref());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::IoAt {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::copy(&from, &dest).map_err(|source| Error::IoAt { path: from, source })?;
        copied += 1;
    }
    eprintln!("    {copied} files");
    Ok(())
}

/// Prints the versions of the Leptos crates the lock file holds — both
/// lines, since both are resolved.
fn report_resolved(tree: &Path) -> Result<()> {
    let lock = fsx::read_to_string(&tree.join("Cargo.lock"))?;
    for name in ["leptos", "tachys", "reactive_graph", "leptos_axum"] {
        let versions: Vec<&str> = lock
            .split("[[package]]")
            .filter(|p| p.lines().any(|l| l.trim() == format!("name = \"{name}\"")))
            .filter_map(|p| {
                p.lines()
                    .find_map(|l| l.trim().strip_prefix("version = \""))
                    .and_then(|v| v.strip_suffix('"'))
            })
            .collect();
        eprintln!("    {name} {}", versions.join(", "));
    }
    Ok(())
}
