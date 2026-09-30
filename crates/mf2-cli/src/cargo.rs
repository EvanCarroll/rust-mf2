//! What cargo says `mf2`'s features are, as the crate that includes the
//! generated module depends on it — the set its build script reads through
//! `links` — so that `mf2 compile --site` builds the catalogs for the
//! functions the wasm is built with, and `mf2 check` checks what the build
//! checks (`plans/05-tooling.md` §6, `plans/19-native-and-terminal.md` §11).

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
    let is_mf2 = |pkg: &Value| {
        metadata["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| &p["id"] == pkg && p["name"] == "mf2")
    };
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
    let node = nodes()
        .find(|node| node["id"] == mf2["pkg"])
        .ok_or_else(|| fail("mf2 is not in the resolve".into()))?;
    let features = node["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    Ok((name, Features::from_names(features)))
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
