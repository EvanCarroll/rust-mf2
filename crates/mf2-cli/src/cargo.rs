//! What cargo says the i18n crate's features are, so that `mf2 compile
//! --site` builds the catalogs for the functions the wasm is built with,
//! and `mf2 check` checks what the build checks (`plans/05-tooling.md` §6).

use std::path::Path;
use std::process::Command;

use mf2_build::Features;
use serde_json::Value;

use crate::error::{Error, Result};

/// The i18n crate in `dir`: its package name and the features cargo resolves
/// for it, unified across its workspace as `cargo metadata` reports them.
/// `offline` keeps cargo off the network: it answers from what it has
/// already fetched, or fails.
pub(crate) fn resolved_features(dir: &Path, offline: bool) -> Result<(String, Features)> {
    let fail = |message: String| Error::Cargo {
        dir: dir.to_owned(),
        message,
    };
    let manifest = dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(fail(
            "no Cargo.toml here; the functions are the i18n crate's features".into(),
        ));
    }
    let manifest = manifest
        .canonicalize()
        .map_err(|source| Error::io(&manifest, source))?;
    // Under `cargo run` (a trunk hook) `CARGO` is the cargo that ran us.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest);
    if offline {
        command.arg("--offline");
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
    let id = &package["id"];
    let name = package["name"].as_str().unwrap_or_default().to_owned();
    let node = metadata["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|node| &node["id"] == id)
        .ok_or_else(|| fail(format!("{name} is not in the resolve")))?;
    let features = node["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    Ok((name, Features::from_names(features)))
}
