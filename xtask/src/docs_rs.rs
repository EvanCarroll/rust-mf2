//! `cargo xtask docs-rs`: the 18 published crates' documentation built as
//! docs.rs builds it (`plans/17-phase-9-work-order.md` A5).
//!
//! Each library crate's `[package.metadata.docs.rs]` is the one statement
//! of how it is documented: its features (`features`, `all-features`,
//! `no-default-features`) and its targets (`targets`, first
//! `default-target`). Every crate must list its `targets`, so docs.rs
//! builds only those instead of its default five. The same table gives
//! `cargo xtask api` the feature set and target each public API is listed
//! with, so the published documentation shows what `api.txt` promises.
//!
//! For each crate and target, as docs.rs does: `cargo rustdoc --lib` on a
//! nightly (the one `cargo xtask api` pins, rather than docs.rs's latest,
//! so that a run is repeatable) with `--cfg docsrs` given to rustdoc and
//! `DOCS_RS=1` set, into `target/docs-rs`. Stricter than docs.rs, which
//! caps lints at warnings: every rustdoc and rustc warning is an error —
//! among them `rustdoc::broken_intra_doc_links` — and so is a crate with no
//! front page (`rustdoc::missing_crate_level_docs`). Each front page must
//! also point to the user guide.
//!
//! `mf2-cli` has no library, and docs.rs documents none for it: it is
//! skipped, and its table, if it had one, would be refused.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cmd::{cargo, run_capture, run_inherit_env};
use crate::error::{Error, Result};

/// The keys of `[package.metadata.docs.rs]` this command reproduces. Any
/// other (`rustc-args`, `cargo-args`, …) is refused rather than ignored,
/// so the build here cannot silently differ from docs.rs's.
const KEYS: [&str; 5] = [
    "features",
    "all-features",
    "no-default-features",
    "default-target",
    "targets",
];

/// What every front page says, somewhere: where the user guide is.
const GUIDE: &str = "user guide";

/// A library crate as docs.rs presents it.
pub(crate) struct Presented {
    pub(crate) name: String,
    pub(crate) features: Vec<String>,
    pub(crate) all_features: bool,
    pub(crate) no_default_features: bool,
    /// The table's `default-target`, when it names one: the crate is
    /// documented for another target than the host's.
    pub(crate) default_target: Option<String>,
    /// The targets, the default first.
    pub(crate) targets: Vec<String>,
    /// A proc-macro crate, built for the host whatever the target: its
    /// documentation is not under the target's directory.
    pub(crate) proc_macro: bool,
}

impl Presented {
    /// The feature arguments of a cargo command, as docs.rs passes them.
    fn feature_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.no_default_features {
            args.push("--no-default-features".to_owned());
        }
        if self.all_features {
            args.push("--all-features".to_owned());
        }
        if !self.features.is_empty() {
            args.push("--features".to_owned());
            args.push(self.features.join(","));
        }
        args
    }
}

fn fail(message: impl Into<String>) -> Error {
    Error::DocsRs(message.into())
}

pub(crate) fn run(root: &Path) -> Result<()> {
    crate::api::install(root)?;
    let presented = presented(root)?;
    let target_dir = root.join("target").join("docs-rs");
    let one = OsStr::new("1");
    let envs = [
        ("CARGO_TARGET_DIR", target_dir.as_os_str()),
        ("DOCS_RS", one),
    ];
    let mut failed = Vec::new();
    let mut unguided = Vec::new();
    for crate_ in &presented {
        for target in &crate_.targets {
            eprintln!("==> docs-rs: {} for {target}", crate_.name);
            let mut args: Vec<String> = [
                "run",
                crate::api::NIGHTLY,
                "cargo",
                "rustdoc",
                "-p",
                &crate_.name,
                "--lib",
                "--target",
                target,
            ]
            .map(str::to_owned)
            .into();
            args.extend(crate_.feature_args());
            args.extend(
                [
                    "--",
                    "--cfg",
                    "docsrs",
                    "-D",
                    "warnings",
                    "-D",
                    "rustdoc::missing_crate_level_docs",
                ]
                .map(str::to_owned),
            );
            let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
            if run_inherit_env(OsStr::new("rustup"), &args, root, &envs).is_err() {
                failed.push(format!("{} ({target})", crate_.name));
                continue;
            }
            let page = front_page(
                &target_dir,
                (!crate_.proc_macro).then_some(target),
                &crate_.name,
            );
            let html = crate::fsx::read_to_string(&page)?;
            if !html.to_lowercase().contains(GUIDE) {
                unguided.push(format!("{} ({target})", crate_.name));
            }
        }
    }
    if !failed.is_empty() {
        return Err(fail(format!(
            "the documentation does not build, or warns (above): {}",
            failed.join(", ")
        )));
    }
    if !unguided.is_empty() {
        return Err(fail(format!(
            "a front page does not say where the {GUIDE} is: {}",
            unguided.join(", ")
        )));
    }
    eprintln!(
        "docs-rs: {} crates documented as docs.rs does, no warnings",
        presented.len()
    );
    Ok(())
}

/// `target_dir/[<target>/]doc/<crate>/index.html`.
fn front_page(target_dir: &Path, target: Option<&str>, name: &str) -> PathBuf {
    target_dir
        .join(target.unwrap_or_default())
        .join("doc")
        .join(name.replace('-', "_"))
        .join("index.html")
}

/// Every publishable library crate, with its docs.rs table read and
/// checked.
pub(crate) fn presented(root: &Path) -> Result<Vec<Presented>> {
    let args = ["metadata", "--format-version", "1", "--no-deps"].map(OsStr::new);
    let bytes = run_capture(&cargo(), &args, root, &[])?;
    let metadata: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Json {
        path: PathBuf::from("cargo metadata"),
        message: e.to_string(),
    })?;
    let mut out = Vec::new();
    for p in metadata["packages"].as_array().into_iter().flatten() {
        if matches!(p["publish"].as_array(), Some(r) if r.is_empty()) {
            continue;
        }
        let name = p["name"].as_str().unwrap_or_default();
        let kinds: Vec<&str> = p["targets"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|t| t["kind"].as_array().into_iter().flatten())
            .filter_map(Value::as_str)
            .collect();
        let proc_macro = kinds.contains(&"proc-macro");
        let library = proc_macro || kinds.contains(&"lib");
        let table = &p["metadata"]["docs"]["rs"];
        if !library {
            if !table.is_null() {
                return Err(fail(format!(
                    "{name} has no library, so docs.rs documents nothing: remove its \
                     [package.metadata.docs.rs]"
                )));
            }
            continue;
        }
        out.push(Presented {
            proc_macro,
            ..read_table(name, table)?
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// One crate's `[package.metadata.docs.rs]`, as `cargo metadata` gives it.
fn read_table(name: &str, table: &Value) -> Result<Presented> {
    let Some(table) = table.as_object() else {
        return Err(fail(format!(
            "{name} has no [package.metadata.docs.rs] (it must list its `targets`)"
        )));
    };
    if let Some(key) = table.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(fail(format!(
            "{name}: [package.metadata.docs.rs] `{key}` is not one this command \
             reproduces ({})",
            KEYS.join(", ")
        )));
    }
    let strings = |key: &str| -> Result<Vec<String>> {
        match table.get(key) {
            None => Ok(Vec::new()),
            Some(v) => v
                .as_array()
                .and_then(|a| a.iter().map(|s| s.as_str().map(str::to_owned)).collect())
                .ok_or_else(|| fail(format!("{name}: `{key}` is not a list of strings"))),
        }
    };
    let flag = |key: &str| -> Result<bool> {
        match table.get(key) {
            None => Ok(false),
            Some(v) => v
                .as_bool()
                .ok_or_else(|| fail(format!("{name}: `{key}` is not true or false"))),
        }
    };
    let mut targets = strings("targets")?;
    if targets.is_empty() {
        return Err(fail(format!(
            "{name}: [package.metadata.docs.rs] lists no `targets` (docs.rs would build \
             its default five)"
        )));
    }
    let default_target = match table.get("default-target") {
        None => None,
        Some(v) => {
            let Some(default) = v.as_str() else {
                return Err(fail(format!("{name}: `default-target` is not a string")));
            };
            let Some(at) = targets.iter().position(|t| t == default) else {
                return Err(fail(format!(
                    "{name}: `default-target` {default:?} is not among its `targets`"
                )));
            };
            let default = targets.remove(at);
            targets.insert(0, default.clone());
            Some(default)
        }
    };
    Ok(Presented {
        name: name.to_owned(),
        features: strings("features")?,
        all_features: flag("all-features")?,
        no_default_features: flag("no-default-features")?,
        default_target,
        targets,
        proc_macro: false,
    })
}

#[cfg(test)]
mod tests {
    use super::read_table;
    use serde_json::json;

    #[test]
    fn a_table_is_read_as_docs_rs_reads_it() {
        let p = read_table(
            "x",
            &json!({
                "features": ["a", "b"],
                "no-default-features": true,
                "default-target": "wasm32-unknown-unknown",
                "targets": ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"],
            }),
        )
        .unwrap();
        assert_eq!(
            p.targets,
            ["wasm32-unknown-unknown", "x86_64-unknown-linux-gnu"]
        );
        assert_eq!(p.default_target.as_deref(), Some("wasm32-unknown-unknown"));
        assert_eq!(
            p.feature_args(),
            ["--no-default-features", "--features", "a,b"]
        );
    }

    #[test]
    fn what_docs_rs_would_do_differently_is_refused() {
        let refused = |table| read_table("x", &table).err().map(|e| e.to_string());
        for (table, says) in [
            (json!(null), "no [package.metadata.docs.rs]"),
            (json!({"features": ["a"]}), "no `targets`"),
            (
                json!({"targets": ["t"], "rustc-args": ["--cfg", "x"]}),
                "`rustc-args` is not one",
            ),
            (
                json!({"targets": ["t"], "default-target": "u"}),
                "not among its `targets`",
            ),
            (
                json!({"targets": ["t"], "all-features": "yes"}),
                "not true or false",
            ),
        ] {
            let got = refused(table).unwrap_or_default();
            assert!(got.contains(says), "{got:?} should say {says:?}");
        }
    }
}
