//! The published packages' metadata:
//! exactly the 17 library crates are publishable, at one version, each with
//! what crates.io shows, and every dependency between two of them is an
//! exact requirement — the generated module, the macro and the runtime share
//! `#[doc(hidden)]` items that the version policy exempts from semver, so
//! only the same release of each is known to work with the others.
//!
//! A dev-dependency between two of ours may instead be path-only: cargo
//! strips it from the package, which is the only way to publish one that
//! closes a cycle (`mf2-fn-number` → `mf2` → `mf2-fn-number`). The tests
//! that use one are left out of the package and run in the workspace (A4;
//! `cargo xtask package`).
//!
//! Every one of the 17 states the one `rust-version` (A3): the MSRV
//! `[workspace.package]` records and `cargo xtask msrv` measures.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cmd::{cargo, run_capture};
use crate::error::{Error, Result};

/// The crates published to crates.io (D12), and nothing else: the 14 of
/// 2.x, the two helper crates of the Leptos components (D20), and 3.0's
/// browser-side ICU4X of the date functions (plan/08 §3.4); 1.x's
/// `leptos-mf2`, `mf2-axum`, `mf2-native` and `mf2-ratatui` are features
/// of `mf2` since 2.0.0.
pub(crate) const PUBLISHED: [&str; 17] = [
    "mf2",
    "mf2-build",
    "mf2-catalog",
    "mf2-cli",
    "mf2-fn-datetime",
    "mf2-fn-datetime-web-icu",
    "mf2-fn-number",
    "mf2-host-std",
    "mf2-host-web",
    "mf2-leptos-ui-0-8",
    "mf2-leptos-ui-0-9",
    "mf2-locale-data",
    "mf2-macros",
    "mf2-model",
    "mf2-resource",
    "mf2-runtime",
    "mf2-syntax",
];

/// Every published crate's version (`[workspace.package]`).
const VERSION: &str = "3.0.0";

/// The crates that ship data derived from CLDR, whose licence is
/// `MIT AND Unicode-3.0`: `mf2-locale-data`'s tables, and `mf2`'s
/// language-matching table (Phase 10 C3).
const CLDR_DATA: [&str; 2] = ["mf2", "mf2-locale-data"];

/// crates.io's limit on keywords and on categories.
const MAX_TERMS: usize = 5;

/// `cargo metadata --no-deps` of the workspace.
pub(crate) fn metadata(root: &Path) -> Result<Value> {
    let args = ["metadata", "--no-deps", "--format-version", "1"].map(OsStr::new);
    let out = run_capture(&cargo(), &args, root, &[])?;
    serde_json::from_slice(&out).map_err(|e| Error::Json {
        path: PathBuf::from("cargo metadata"),
        message: e.to_string(),
    })
}

/// The version the published crates are released at: `mf2`'s (A1's test
/// holds the 17 to one).
pub(crate) fn version(metadata: &Value) -> Option<&str> {
    metadata["packages"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == "mf2")?["version"]
        .as_str()
}

/// Every problem with `metadata` (`cargo metadata --no-deps`), one line each.
pub(crate) fn problems(metadata: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let empty = Vec::new();
    let packages = metadata["packages"].as_array().unwrap_or(&empty);
    let publishable: Vec<&str> = packages
        .iter()
        .filter(|p| !matches!(p["publish"].as_array(), Some(r) if r.is_empty()))
        .filter_map(|p| p["name"].as_str())
        .collect();
    for name in PUBLISHED {
        if !publishable.contains(&name) {
            out.push(format!("{name}: not publishable (`publish = true`)"));
        }
    }
    for name in &publishable {
        if !PUBLISHED.contains(name) {
            out.push(format!("{name}: publishable, but not one of the 17"));
        }
    }
    let msrv = packages
        .iter()
        .find(|p| p["name"] == "mf2")
        .and_then(|p| p["rust_version"].as_str());
    for package in packages {
        let Some(name) = package["name"].as_str() else {
            continue;
        };
        if PUBLISHED.contains(&name) {
            package_problems(name, package, msrv, &mut out);
        }
    }
    out
}

fn package_problems(name: &str, package: &Value, msrv: Option<&str>, out: &mut Vec<String>) {
    let text = |key: &str| package[key].as_str().filter(|s| !s.trim().is_empty());
    if text("version") != Some(VERSION) {
        out.push(format!("{name}: version is not {VERSION}"));
    }
    match (text("rust_version"), msrv) {
        (None, _) => out.push(format!("{name}: no `rust-version`")),
        (Some(v), Some(m)) if v != m => {
            out.push(format!("{name}: rust-version {v}, not the workspace's {m}"));
        }
        _ => {}
    }
    for key in ["description", "readme"] {
        if text(key).is_none() {
            out.push(format!("{name}: no `{key}`"));
        }
    }
    let license = if CLDR_DATA.contains(&name) {
        "MIT AND Unicode-3.0"
    } else {
        "MIT"
    };
    if text("license") != Some(license) {
        out.push(format!("{name}: license is not `{license}`"));
    }
    for key in ["keywords", "categories"] {
        let n = package[key].as_array().map_or(0, Vec::len);
        if !(1..=MAX_TERMS).contains(&n) {
            out.push(format!("{name}: {n} {key} (1 to {MAX_TERMS})"));
        }
    }
    for keyword in package["keywords"].as_array().into_iter().flatten() {
        let k = keyword.as_str().unwrap_or_default();
        let valid = k.len() <= 20
            && k.starts_with(|c: char| c.is_ascii_alphabetic())
            && k.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_+".contains(c));
        if !valid {
            out.push(format!(
                "{name}: keyword {k:?} is not one crates.io accepts"
            ));
        }
    }
    // The files the fields name, and the licence texts, beside the manifest.
    let Some(dir) = package["manifest_path"]
        .as_str()
        .and_then(|m| std::path::Path::new(m).parent())
    else {
        out.push(format!("{name}: no manifest path"));
        return;
    };
    let mut files = vec!["LICENSE"];
    if CLDR_DATA.contains(&name) {
        files.push("LICENSE-UNICODE");
    }
    files.extend(text("readme"));
    for file in files {
        if !dir.join(file).is_file() {
            out.push(format!("{name}: no {file} in the crate"));
        }
    }
    for dep in package["dependencies"].as_array().into_iter().flatten() {
        let Some(on) = dep["name"].as_str().filter(|d| PUBLISHED.contains(d)) else {
            continue;
        };
        let req = dep["req"].as_str().unwrap_or_default();
        let exact = format!("={VERSION}");
        let path_only_dev = dep["kind"] == "dev" && req == "*";
        if req != exact && !path_only_dev {
            out.push(format!(
                "{name} → {on}: `{req}`, not `{exact}` (or a path-only dev-dependency)"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{PUBLISHED, problems};
    use crate::fsx::repo_root;

    fn metadata() -> Value {
        super::metadata(&repo_root()).unwrap()
    }

    fn package<'m>(metadata: &'m mut Value, name: &str) -> &'m mut Value {
        metadata["packages"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["name"] == name)
            .unwrap()
    }

    #[test]
    fn the_twenty_are_published_and_complete() {
        let found = problems(&metadata());
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    // Negative controls: each kind of drift is caught.

    #[test]
    fn a_library_crate_left_unpublished_is_caught() {
        let mut m = metadata();
        package(&mut m, "mf2-runtime")["publish"] = serde_json::json!([]);
        assert_eq!(
            problems(&m),
            ["mf2-runtime: not publishable (`publish = true`)"]
        );
    }

    #[test]
    fn a_published_tool_is_caught() {
        let mut m = metadata();
        package(&mut m, "xtask")["publish"] = Value::Null;
        assert_eq!(problems(&m), ["xtask: publishable, but not one of the 17"]);
    }

    #[test]
    fn a_caret_requirement_between_two_of_ours_is_caught() {
        let mut m = metadata();
        let deps = package(&mut m, "mf2")["dependencies"]
            .as_array_mut()
            .unwrap();
        let dep = deps
            .iter_mut()
            .find(|d| d["name"] == "mf2-runtime")
            .unwrap();
        dep["req"] = "^3.0.0".into();
        assert_eq!(
            problems(&m),
            ["mf2 → mf2-runtime: `^3.0.0`, not `=3.0.0` (or a path-only dev-dependency)"]
        );
    }

    #[test]
    fn a_path_only_normal_dependency_is_caught() {
        let mut m = metadata();
        let deps = package(&mut m, "mf2")["dependencies"]
            .as_array_mut()
            .unwrap();
        let dep = deps
            .iter_mut()
            .find(|d| d["name"] == "mf2-runtime")
            .unwrap();
        dep["req"] = "*".into();
        assert_eq!(problems(&m).len(), 1);
    }

    #[test]
    fn missing_fields_and_wrong_licences_are_caught() {
        let mut m = metadata();
        let p = package(&mut m, "mf2-locale-data");
        p["license"] = "MIT".into();
        p["keywords"] = serde_json::json!([]);
        p["readme"] = Value::Null;
        let p = package(&mut m, "mf2-model");
        p["version"] = "0.1.0".into();
        p["keywords"] = serde_json::json!(["1st"]);
        package(&mut m, "mf2-syntax")["rust_version"] = Value::Null;
        package(&mut m, "mf2-macros")["rust_version"] = "1.85".into();
        let mut found = problems(&m);
        found.sort();
        assert_eq!(
            found,
            [
                "mf2-locale-data: 0 keywords (1 to 5)",
                "mf2-locale-data: license is not `MIT AND Unicode-3.0`",
                "mf2-locale-data: no `readme`",
                "mf2-macros: rust-version 1.85, not the workspace's 1.88",
                "mf2-model: keyword \"1st\" is not one crates.io accepts",
                "mf2-model: version is not 3.0.0",
                "mf2-syntax: no `rust-version`",
            ]
        );
    }

    #[test]
    fn the_list_is_sorted_and_has_no_duplicates() {
        assert!(PUBLISHED.windows(2).all(|w| w[0] < w[1]));
    }
}
