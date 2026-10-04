//! What cargo says `mf2`'s features are, as the crate that includes the
//! generated module depends on it — the set its build script reads through
//! `links` — so that `mf2 compile --site` builds the catalogs for the
//! functions the wasm is built with, and `mf2 check` checks what the build
//! checks (the tooling design §6, the native-and-terminal design §11).

use std::path::Path;
use std::process::Command;

use mf2_build::Features;
use serde_json::Value;

use crate::error::{Error, Result};

/// The crate in `dir`: its package name, and the features cargo resolves for
/// the `mf2` it names as a normal dependency, unified across its workspace as
/// `cargo metadata` reports them. `offline` keeps cargo off the network: it
/// answers from what it has already fetched, or fails. Offline, the resolve
/// is the host's (`--filter-platform`): a build fetches only its own
/// platform's packages, and without the filter cargo would want every
/// platform's, so a crate that has just built would still fail on a
/// dependency for Windows it never downloaded.
pub(crate) fn resolved_features(dir: &Path, offline: bool) -> Result<(String, Features)> {
    resolve(dir, offline).map(|resolved| (resolved.name, resolved.features))
}

/// What cargo says of the crate in `dir` and its `mf2`.
pub(crate) struct Resolved {
    /// The package name.
    pub(crate) name: String,
    /// `mf2`'s features as the build has them (see [`resolved_features`]).
    pub(crate) features: Features,
    /// The features the crate itself writes on its `mf2` dependency, in the
    /// order written: what `mf2 check` keeps of the modes.
    pub(crate) written: Vec<String>,
    /// Whether the crate's `mf2-build` build-dependency has `icu-blob`;
    /// `None` when it names no `mf2-build` there. The build needs it when
    /// a side's date formatter is ICU4X, and `mf2 check` says so too.
    pub(crate) icu_blob: Option<bool>,
}

/// [`resolved_features`], with the features the crate writes itself.
pub(crate) fn resolve(dir: &Path, offline: bool) -> Result<Resolved> {
    let fail = |message: String| Error::Cargo {
        dir: dir.to_owned(),
        message,
    };
    let manifest = dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(fail(
            "no Cargo.toml here; the functions are the features of its `mf2`".into(),
        ));
    }
    let manifest = manifest
        .canonicalize()
        .map_err(|source| Error::io(&manifest, source))?;
    // Under `cargo run` (a trunk hook) `CARGO` is the cargo that ran us.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    // In the crate, so that cargo reads its configuration as its build does.
    command
        .current_dir(dir)
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest);
    if offline {
        command.arg("--offline");
        if let Some(host) = host() {
            command.args(["--filter-platform", &host]);
        }
    }
    let output = command.output().map_err(|e| fail(e.to_string()))?;
    if !output.status.success() {
        return Err(fail(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let metadata: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| fail(e.to_string()))?;

    let package = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| {
            p["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path) == manifest)
        })
        .ok_or_else(|| fail("the crate is not in its own metadata".into()))?;
    let mut written: Vec<String> = Vec::new();
    // Every normal `mf2` entry, a platform's included: cargo unifies them.
    let entries = package["dependencies"].as_array().into_iter().flatten();
    for dep in entries.filter(|dep| dep["name"] == "mf2" && dep["kind"].is_null()) {
        for feature in dep["features"].as_array().into_iter().flatten() {
            if let Some(feature) = feature.as_str()
                && !written.iter().any(|w| w == feature)
            {
                written.push(feature.to_owned());
            }
        }
    }
    let id = &package["id"];
    let name = package["name"].as_str().unwrap_or_default().to_owned();
    let nodes = || {
        metadata["resolve"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
    };
    let node = nodes()
        .find(|node| &node["id"] == id)
        .ok_or_else(|| fail(format!("{name} is not in the resolve")))?;
    // `mf2` as a normal dependency (a `null` kind), under whatever name.
    let is_package = |pkg: &Value, wanted: &str| {
        metadata["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| &p["id"] == pkg && p["name"] == wanted)
    };
    let is_mf2 = |pkg: &Value| is_package(pkg, "mf2");
    let mf2 = node["deps"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|dep| {
            is_mf2(&dep["pkg"])
                && dep["dep_kinds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|kind| kind["kind"].is_null())
        })
        .ok_or_else(|| fail(format!("{name} does not name `mf2` in its [dependencies]")))?;
    // `mf2-build` as a build-dependency, and whether it has `icu-blob`.
    let icu_blob = node["deps"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|dep| {
            is_package(&dep["pkg"], "mf2-build")
                && dep["dep_kinds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|kind| kind["kind"] == "build")
        })
        .map(|dep| {
            nodes()
                .find(|node| node["id"] == dep["pkg"])
                .and_then(|node| node["features"].as_array())
                .is_some_and(|features| features.iter().any(|f| f == "icu-blob"))
        });
    let node = nodes()
        .find(|node| node["id"] == mf2["pkg"])
        .ok_or_else(|| fail("mf2 is not in the resolve".into()))?;
    let features = node["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    Ok(Resolved {
        name,
        features: Features::from_names(features),
        written,
        icu_blob,
    })
}

/// The host's target triple, as `rustc -vV` names it — the platform a native
/// application or a server is built for.
fn host() -> Option<String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
}
