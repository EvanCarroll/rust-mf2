//! `cargo xtask api [--check]`: the public API of the 16 published crates,
//! listed and committed (`plans/17-phase-9-work-order.md` A2;
//! `docs/versioning.md`): as `crates/<name>/api.txt`, or, for a crate whose
//! features select what it offers, once per mode, as
//! `crates/<name>/api/<mode>.txt` (`mf2`; `plans/18-phase-10-work-order.md`
//! B5).
//!
//! A library crate's listing is `cargo public-api -ss`'s (no blanket or
//! auto-trait impls; derived ones stay, since removing a derive breaks a
//! caller), made in-process with the
//! `public-api` and `rustdoc-json` crates from the rustdoc JSON of a pinned
//! nightly (installed through rustup when missing; `#[doc(hidden)]` items —
//! which the version policy does not promise — are not listed). Each crate
//! is listed with the features and target its `[package.metadata.docs.rs]`
//! gives docs.rs (`cargo xtask docs-rs`), so the published documentation
//! shows what the listing promises: the feature set an application turns on
//! for its server, and `mf2-host-web` for `wasm32-unknown-unknown`, the only
//! target it has.
//!
//! A crate with a `[package.metadata.api]` table is listed per mode
//! instead: `modes` names each mode's features (`mf2`'s `core`, `ssr`,
//! `hydrate`, `csr`, `native`, `ratatui`), since no one feature set shows
//! them all — the Leptos modes exclude each other — and an item only one
//! mode has would otherwise change unseen. `baseline` spells the modes as an
//! earlier release did, for `cargo xtask release`'s comparison with it.
//!
//! Two things rustdoc's JSON leaves out are put back before the listing is
//! made, each read from a second build that documents hidden items too:
//! * a public re-export of a hidden item, which rustdoc drops with the item
//!   (`mf2::leptos::islands_gate!`, whose macro is exported, hidden, at the
//!   crate's root): it is listed at the re-export's path;
//! * a glob re-export of another crate's module, which the JSON cannot look
//!   into, so that public-api prints the glob alone (1.x's `leptos-mf2`
//!   shim had `pub use mf2::leptos::*`; none is left, and the step stays
//!   as the guard for the next one): it is listed as one re-export per name it
//!   brings, read from that crate's own JSON built with the features the
//!   listed crate turns on in it (`cargo tree`). What each name is, that
//!   crate's own listing holds.
//!
//! `mf2-cli` is a binary: its promise is the command tree, and its listing
//! is the commands and their arguments as clap declares them, written and
//! checked by its own test (`listing::api_txt`, which `cargo test` runs
//! too).
//!
//! Without `--check`, the listings are written; with it, each is compared
//! with the committed file and any difference fails, naming the lines — a
//! change to the public API is committed together with its listing.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::cmd::{cargo, run_capture, run_inherit_env};
use crate::docs_rs::Presented;
use crate::error::{Error, Result};
use crate::fsx;

/// The nightly whose rustdoc JSON `public-api` reads. Pinned, as the JSON
/// format changes between nightlies (this one writes format 61, which
/// `public-api` 0.52.2 reads).
pub(crate) const NIGHTLY: &str = "nightly-2026-09-24";

const WASM: &str = "wasm32-unknown-unknown";

/// The first line of every listing.
const HEADER: &str = "# The public API that 2.x promises (docs/versioning.md). Written by \
                      `cargo xtask api`, checked by `cargo xtask ci`; commit it with the change.";

/// What the second build gives rustdoc, so that the JSON has the hidden items
/// too.
const HIDDEN: &str = "-Z unstable-options --document-hidden-items";

/// The keys of `[package.metadata.api]`. Any other is refused rather than
/// ignored.
const KEYS: [&str; 2] = ["modes", "baseline"];

/// A crate's public API per mode: its `[package.metadata.api]`.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Modes {
    /// Each mode's features (the crate's default features are on too).
    pub(crate) modes: BTreeMap<String, Vec<String>>,
    /// For a release that spelled the modes otherwise, by version: each mode
    /// it had, with the features it needed. A mode it does not name, it did
    /// not have.
    pub(crate) baselines: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// The features one listing is made with.
#[derive(Debug, Clone, Default)]
struct FeatureSet {
    features: Vec<String>,
    all_features: bool,
    no_default_features: bool,
}

impl FeatureSet {
    /// The feature arguments of a cargo command.
    fn args(&self) -> Vec<String> {
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

    /// How a listing's header names them.
    fn shown(&self) -> String {
        if self.all_features {
            "(all)".to_owned()
        } else if self.features.is_empty() {
            "(default)".to_owned()
        } else {
            self.features.join(",")
        }
    }
}

/// One listing to write or check.
struct Listing<'a> {
    /// The crate, as docs.rs presents it (its target).
    listed: &'a Presented,
    /// Its path below the repository root.
    file: String,
    mode: Option<&'a str>,
    features: FeatureSet,
}

fn fail(message: impl Into<String>) -> Error {
    Error::Api(message.into())
}

pub(crate) fn run(root: &Path, check: bool) -> Result<()> {
    install(root)?;
    let metadata = crate::packages::metadata(root)?;
    let modes = modes(&metadata)?;
    let presented = crate::docs_rs::presented(root)?;
    let mut stale = Vec::new();
    let mut count = 0;
    for listed in &presented {
        let per_mode = modes.get(&listed.name);
        stale.extend(strays(root, &listed.name, per_mode, check)?);
        for listing in listings(listed, per_mode) {
            eprintln!("==> api: {}", listing.file);
            let text = list(root, &metadata, &listing)?;
            if let Some(diff) = compare(root, &listing.file, &text, check)? {
                stale.push(diff);
            }
            count += 1;
        }
    }
    eprintln!("==> api: mf2-cli (its command tree)");
    cli(root, check)?;
    if stale.is_empty() {
        eprintln!(
            "api: {} listings {}",
            count + 1,
            if check { "unchanged" } else { "written" }
        );
        Ok(())
    } else {
        Err(fail(format!(
            "the public API differs from the committed listings; run `cargo xtask api` and \
             commit them with the change\n{}",
            stale.join("\n")
        )))
    }
}

/// `listed`'s listings: one per mode, or the one docs.rs's features give.
fn listings<'a>(listed: &'a Presented, per_mode: Option<&'a Modes>) -> Vec<Listing<'a>> {
    let dir = format!("crates/{}", listed.name);
    match per_mode {
        Some(modes) => modes
            .modes
            .iter()
            .map(|(mode, features)| Listing {
                listed,
                file: format!("{dir}/api/{mode}.txt"),
                mode: Some(mode),
                features: FeatureSet {
                    features: features.clone(),
                    ..FeatureSet::default()
                },
            })
            .collect(),
        None => vec![Listing {
            listed,
            file: format!("{dir}/api.txt"),
            mode: None,
            features: FeatureSet {
                features: listed.features.clone(),
                all_features: listed.all_features,
                no_default_features: listed.no_default_features,
            },
        }],
    }
}

/// The listing's text, as the committed file holds it.
fn list(root: &Path, metadata: &Value, listing: &Listing<'_>) -> Result<String> {
    let listed = listing.listed;
    // Documented for another target than the host's: listed for it too.
    let target = listed.default_target.as_deref();
    let manifest = manifest_path(metadata, &listed.name)?;
    let mut shown = rustdoc(root, &manifest, &listing.features, target, false)?;
    let hidden = rustdoc(root, &manifest, &listing.features, target, true)?;
    restore_hidden_reexports(&listed.name, &mut shown, &hidden)?;
    expand_foreign_globs(root, metadata, listing, &mut shown, &hidden)?;
    let json = root
        .join("target")
        .join("api")
        .join("listed")
        .join(match listing.mode {
            Some(mode) => format!("{}.{mode}.json", listed.name),
            None => format!("{}.json", listed.name),
        });
    let bytes = serde_json::to_vec(&shown).map_err(|e| Error::Json {
        path: json.clone(),
        message: e.to_string(),
    })?;
    fsx::write(&json, &bytes)?;
    let api = public_api::Builder::from_rustdoc_json(&json)
        .omit_blanket_impls(true)
        .omit_auto_trait_impls(true)
        .build()
        .map_err(|e| fail(format!("{}: {e}", listing.file)))?;
    let mode = listing
        .mode
        .map(|m| format!("mode: {m}; "))
        .unwrap_or_default();
    let target = target.map(|t| format!("; target: {t}")).unwrap_or_default();
    let mut text = format!(
        "{HEADER}\n# {NIGHTLY}; {mode}features: {}{target}\n",
        listing.features.shown()
    );
    for item in api.items() {
        let _ = writeln!(text, "{item}");
    }
    Ok(text)
}

/// The rustdoc JSON of the crate at `manifest`, from the pinned nightly;
/// with `hidden`, documenting hidden items too.
fn rustdoc(
    root: &Path,
    manifest: &Path,
    features: &FeatureSet,
    target: Option<&str>,
    hidden: bool,
) -> Result<Value> {
    let mut builder = rustdoc_json::Builder::default()
        .toolchain(NIGHTLY)
        .manifest_path(manifest)
        .target_dir(root.join("target").join("api"))
        .features(&features.features)
        .all_features(features.all_features)
        .no_default_features(features.no_default_features)
        // A listing built with one mode's features meets documentation that
        // links to another mode's items; the documentation's warnings are
        // `cargo xtask docs-rs`'s, not the listing's.
        .cap_lints(Some("allow"))
        .quiet(true);
    if hidden {
        builder = builder.env("RUSTDOCFLAGS", HIDDEN);
    }
    if let Some(target) = target {
        builder = builder.target(target.to_owned());
    }
    let path = builder
        .build()
        .map_err(|e| fail(format!("{}: rustdoc JSON: {e}", manifest.display())))?;
    let text = fsx::read_to_string(&path)?;
    serde_json::from_str(&text).map_err(|e| Error::Json {
        path,
        message: e.to_string(),
    })
}

// ----- what rustdoc's JSON leaves out -----

/// An item of `json`'s index.
fn item(json: &Value, id: u64) -> &Value {
    &json["index"][id.to_string()]
}

/// The ids of a module's items.
fn items(module: &Value) -> Vec<u64> {
    module["inner"]["module"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect()
}

/// Whether an item says `#[doc(hidden)]`.
fn is_hidden(item: &Value) -> bool {
    item["attrs"].as_array().into_iter().flatten().any(|a| {
        a["other"]
            .as_str()
            .is_some_and(|s| s.starts_with("#[doc(") && s.contains("hidden"))
    })
}

/// The module at `path` (the crate's name first), walking module items down
/// from the root.
fn module_at(json: &Value, path: &[String]) -> Option<u64> {
    let mut id = json["root"].as_u64()?;
    let (first, rest) = path.split_first()?;
    if item(json, id)["name"].as_str() != Some(first) {
        return None;
    }
    for name in rest {
        id = items(item(json, id)).into_iter().find(|&child| {
            let child = item(json, child);
            child["inner"].get("module").is_some() && child["name"].as_str() == Some(name)
        })?;
    }
    Some(id)
}

/// The names a module gives: each item's, and each re-export's; and how
/// many glob re-exports it has.
fn names(json: &Value, module: u64) -> (BTreeSet<String>, usize) {
    let mut out = BTreeSet::new();
    let mut globs = 0;
    for id in items(item(json, module)) {
        let item = item(json, id);
        if let Some(use_) = item["inner"].get("use") {
            if use_["is_glob"].as_bool() == Some(true) {
                globs += 1;
            } else if let Some(name) = use_["name"].as_str() {
                out.insert(name.to_owned());
            }
        } else if let Some(name) = item["name"].as_str() {
            out.insert(name.to_owned());
        }
    }
    (out, globs)
}

/// The next id no item of `json` has.
fn fresh_id(json: &Value) -> u64 {
    ["index", "paths"]
        .iter()
        .filter_map(|key| json[*key].as_object())
        .flat_map(Map::keys)
        .filter_map(|k| k.parse::<u64>().ok())
        .max()
        .map_or(0, |max| max + 1)
}

/// Adds `item` to `json`'s index under a fresh id, and returns the id.
fn insert(json: &mut Value, mut item: Value) -> u64 {
    let id = fresh_id(json);
    item["id"] = json!(id);
    // What its documentation links to has other ids here; nothing lists it.
    item["links"] = json!({});
    json["index"][id.to_string()] = item;
    id
}

/// Adds `id` to a module's items.
fn push_item(json: &mut Value, module: u64, id: u64) {
    if let Some(items) =
        json["index"][module.to_string()]["inner"]["module"]["items"].as_array_mut()
    {
        items.push(json!(id));
    }
}

/// The public re-exports of a hidden item in `hidden` (a build that
/// documents hidden items): each re-export, itself not hidden, in a module
/// that is not, of an item of this crate that is. Each with its module's
/// path.
fn hidden_reexports(hidden: &Value) -> Vec<(Vec<String>, Value, Value)> {
    let mut out = Vec::new();
    let Some(root) = hidden["root"].as_u64() else {
        return out;
    };
    let name = item(hidden, root)["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let mut stack = vec![(root, vec![name])];
    while let Some((module, path)) = stack.pop() {
        for id in items(item(hidden, module)) {
            let child = item(hidden, id);
            if is_hidden(child) {
                continue;
            }
            if child["inner"].get("module").is_some() {
                let mut sub = path.clone();
                sub.push(child["name"].as_str().unwrap_or_default().to_owned());
                stack.push((id, sub));
            } else if let Some(use_) = child["inner"].get("use") {
                let target = use_["id"].as_u64().map(|t| item(hidden, t));
                if use_["is_glob"].as_bool() != Some(true)
                    && let Some(target) = target.filter(|t| !t.is_null() && is_hidden(t))
                {
                    out.push((path.clone(), child.clone(), target.clone()));
                }
            }
        }
    }
    out
}

/// Puts back into `shown` each public re-export of a hidden item that rustdoc
/// dropped with the item ([`hidden_reexports`]), so that the listing names it
/// at the re-export's path. A macro can be put back as it is, since it
/// refers to no other item; another kind is refused, so that nothing is left
/// out unseen.
fn restore_hidden_reexports(crate_: &str, shown: &mut Value, hidden: &Value) -> Result<()> {
    for (path, use_, target) in hidden_reexports(hidden) {
        let name = use_["inner"]["use"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let at = format!("{}::{name}", path.join("::"));
        let module = module_at(shown, &path)
            .ok_or_else(|| fail(format!("{crate_}: `{at}`'s module is not in the listing")))?;
        if names(shown, module).0.contains(&name) {
            continue;
        }
        if target["inner"].get("macro").is_none() {
            let kind = target["inner"]
                .as_object()
                .and_then(|o| o.keys().next().cloned())
                .unwrap_or_default();
            return Err(fail(format!(
                "{crate_}: `{at}` re-exports a hidden {kind}, which rustdoc leaves out; `cargo \
                 xtask api` puts back only a macro: list it by unhiding the item, or teach \
                 `cargo xtask api` its kind"
            )));
        }
        let target = insert(shown, target);
        let mut use_ = use_;
        use_["inner"]["use"]["id"] = json!(target);
        let use_ = insert(shown, use_);
        push_item(shown, module, use_);
    }
    Ok(())
}

/// Replaces in `shown` each glob re-export of another crate's module by one
/// re-export per name it brings (as public-api prints the crate's other
/// re-exports from that crate), so that the listing names them. The names
/// are read from that crate's own JSON, built with the features this build
/// turns on in it; a name the re-exporting module has of its own (hidden
/// ones too) shadows the glob's.
fn expand_foreign_globs(
    root: &Path,
    metadata: &Value,
    listing: &Listing<'_>,
    shown: &mut Value,
    hidden: &Value,
) -> Result<()> {
    let crate_ = &listing.listed.name;
    let target = listing.listed.default_target.as_deref();
    let Some(root_id) = shown["root"].as_u64() else {
        return Ok(());
    };
    let root_name = item(shown, root_id)["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    // Every module of the listing, with its path, and each glob in it that
    // public-api cannot see into.
    let mut globs = Vec::new();
    let mut stack = vec![(root_id, vec![root_name])];
    while let Some((module, path)) = stack.pop() {
        for id in items(item(shown, module)) {
            let child = item(shown, id);
            if child["inner"].get("module").is_some() {
                let mut sub = path.clone();
                sub.push(child["name"].as_str().unwrap_or_default().to_owned());
                stack.push((id, sub));
            } else if let Some(use_) = child["inner"].get("use")
                && use_["is_glob"].as_bool() == Some(true)
                && use_["id"]
                    .as_u64()
                    .is_none_or(|target| item(shown, target).is_null())
            {
                globs.push((path.clone(), module, id, use_.clone()));
            }
        }
    }
    for (path, module, glob, use_) in globs {
        let source = use_["source"].as_str().unwrap_or_default().to_owned();
        let at = format!("{}::{source}::*", path.join("::"));
        let summary = use_["id"].as_u64().map(|t| &shown["paths"][t.to_string()]);
        let Some(summary) = summary.filter(|s| !s.is_null()) else {
            return Err(fail(format!(
                "{crate_}: the glob `{at}` names nothing rustdoc can place"
            )));
        };
        let foreign_path: Vec<String> = summary["path"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_owned))
            .collect();
        let foreign = shown["external_crates"][summary["crate_id"].to_string()]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let package = package_of(metadata, &foreign).ok_or_else(|| {
            fail(format!(
                "{crate_}: the glob `{at}` re-exports `{foreign}`, which is not a crate of this \
                 workspace, so its names cannot be listed"
            ))
        })?;
        let features = forwarded(root, crate_, &listing.features, target, &package)?;
        let manifest = manifest_path(metadata, &package)?;
        let mut theirs = rustdoc(root, &manifest, &features, target, false)?;
        let their_hidden = rustdoc(root, &manifest, &features, target, true)?;
        restore_hidden_reexports(&package, &mut theirs, &their_hidden)?;
        let their_module = module_at(&theirs, &foreign_path).ok_or_else(|| {
            fail(format!(
                "{crate_}: the glob `{at}`: `{}` has no module there",
                foreign_path.join("::")
            ))
        })?;
        let (brought, nested) = names(&theirs, their_module);
        if nested > 0 {
            return Err(fail(format!(
                "{crate_}: the glob `{at}` brings a glob of its own, which `cargo xtask api` \
                 does not follow"
            )));
        }
        let own = module_at(hidden, &path)
            .map(|m| names(hidden, m).0)
            .unwrap_or_default();
        if let Some(items) =
            shown["index"][module.to_string()]["inner"]["module"]["items"].as_array_mut()
        {
            items.retain(|id| id.as_u64() != Some(glob));
        }
        for name in brought.difference(&own) {
            let reexport = json!({
                "crate_id": 0,
                "name": null,
                "span": null,
                "visibility": "public",
                "docs": null,
                "attrs": [],
                "deprecation": null,
                "inner": {"use": {
                    "source": format!("{source}::{name}"),
                    "name": name,
                    "id": null,
                    "is_glob": false,
                }},
            });
            let id = insert(shown, reexport);
            push_item(shown, module, id);
        }
    }
    Ok(())
}

/// The workspace package whose library is the crate `name`.
fn package_of(metadata: &Value, name: &str) -> Option<String> {
    metadata["packages"].as_array()?.iter().find(|p| {
        p["targets"].as_array().into_iter().flatten().any(|t| {
            let lib = t["kind"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|k| k != "bin" && k != "test" && k != "example" && k != "bench");
            lib && t["name"].as_str().map(|n| n.replace('-', "_")) == Some(name.to_owned())
        })
    })?["name"]
        .as_str()
        .map(str::to_owned)
}

/// A workspace package's manifest.
pub(crate) fn manifest_path(metadata: &Value, package: &str) -> Result<PathBuf> {
    metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == package)
        .and_then(|p| p["manifest_path"].as_str())
        .map(PathBuf::from)
        .ok_or_else(|| fail(format!("cargo metadata has no package {package}")))
}

/// The features `crate_`, built with `features` alone, turns on in its
/// dependency `dep`, as cargo resolves them (`cargo tree`): what its
/// `[features]` forward, and what its dependencies unify. `default` among
/// them when the defaults are on.
fn forwarded(
    root: &Path,
    crate_: &str,
    features: &FeatureSet,
    target: Option<&str>,
    dep: &str,
) -> Result<FeatureSet> {
    let mut args: Vec<String> = [
        "tree", "-p", crate_, "-e", "normal", "-i", dep, "--depth", "0", "--prefix", "none",
        "--format", "{f}",
    ]
    .map(str::to_owned)
    .into();
    args.extend(features.args());
    if let Some(target) = target {
        args.extend(["--target".to_owned(), target.to_owned()]);
    }
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let out = run_capture(&cargo(), &args, root, &[])?;
    let out = String::from_utf8_lossy(&out);
    Ok(FeatureSet {
        features: out
            .lines()
            .next()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(str::to_owned)
            .collect(),
        no_default_features: true,
        all_features: false,
    })
}

// ----- the table -----

/// Every workspace package's `[package.metadata.api]`, read and checked, by
/// package.
pub(crate) fn modes(metadata: &Value) -> Result<BTreeMap<String, Modes>> {
    let mut out = BTreeMap::new();
    for p in metadata["packages"].as_array().into_iter().flatten() {
        let table = &p["metadata"]["api"];
        if table.is_null() {
            continue;
        }
        let name = p["name"].as_str().unwrap_or_default();
        out.insert(name.to_owned(), read_modes(name, table)?);
    }
    Ok(out)
}

/// One crate's `[package.metadata.api]`, as `cargo metadata` gives it.
fn read_modes(name: &str, table: &Value) -> Result<Modes> {
    let Some(table) = table.as_object() else {
        return Err(fail(format!(
            "{name}: [package.metadata.api] is not a table"
        )));
    };
    if let Some(key) = table.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(fail(format!(
            "{name}: [package.metadata.api] `{key}` is not one `cargo xtask api` reads ({})",
            KEYS.join(", ")
        )));
    }
    let modes = lists(name, "modes", table.get("modes"))?;
    if modes.is_empty() {
        return Err(fail(format!(
            "{name}: [package.metadata.api] names no mode in `modes`"
        )));
    }
    for (mode, features) in &modes {
        if mode.is_empty()
            || !mode
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(fail(format!(
                "{name}: the mode `{mode}` is not a name its listing's file can have \
                 (a-z, 0-9, -)"
            )));
        }
        if let Some(f) = features.iter().find(|f| f.contains('/')) {
            return Err(fail(format!(
                "{name}: the mode `{mode}` names `{f}`, a dependency's feature: a mode is \
                 this crate's own features"
            )));
        }
    }
    let mut baselines = BTreeMap::new();
    if let Some(baseline) = table.get("baseline") {
        let Some(baseline) = baseline.as_object() else {
            return Err(fail(format!(
                "{name}: [package.metadata.api] `baseline` is not a table of versions"
            )));
        };
        for (version, spelled) in baseline {
            let key = format!("baseline.\"{version}\"");
            let spelled = lists(name, &key, Some(spelled))?;
            if let Some(mode) = spelled.keys().find(|m| !modes.contains_key(*m)) {
                return Err(fail(format!(
                    "{name}: `{key}` spells `{mode}`, which `modes` does not name"
                )));
            }
            baselines.insert(version.clone(), spelled);
        }
    }
    Ok(Modes { modes, baselines })
}

/// A table of feature lists.
fn lists(name: &str, key: &str, value: Option<&Value>) -> Result<BTreeMap<String, Vec<String>>> {
    let refused = || fail(format!("{name}: `{key}` is not a table of feature lists"));
    let table = value.and_then(Value::as_object).ok_or_else(refused)?;
    table
        .iter()
        .map(|(k, v)| {
            let list = v
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .map(|s| s.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(refused)?;
            Ok((k.clone(), list))
        })
        .collect()
}

// ----- the committed files -----

/// Listings that no longer belong beside the ones `listings` makes: an
/// `api.txt` beside per-mode listings, an `api/` for a crate listed once, a
/// mode the table no longer names. Removed when writing; each named when
/// checking.
fn strays(root: &Path, name: &str, per_mode: Option<&Modes>, check: bool) -> Result<Vec<String>> {
    let dir = root.join("crates").join(name);
    let mut stray = Vec::new();
    match per_mode {
        Some(modes) => {
            if dir.join("api.txt").exists() {
                stray.push("api.txt".to_owned());
            }
            let listed = dir.join("api");
            if listed.is_dir() {
                for entry in std::fs::read_dir(&listed).map_err(|source| Error::IoAt {
                    path: listed.clone(),
                    source,
                })? {
                    let file = entry?.file_name().to_string_lossy().into_owned();
                    let mode = file.strip_suffix(".txt");
                    if !mode.is_some_and(|m| modes.modes.contains_key(m)) {
                        stray.push(format!("api/{file}"));
                    }
                }
            }
        }
        None => {
            if dir.join("api").exists() {
                stray.push("api".to_owned());
            }
        }
    }
    stray.sort();
    if check {
        return Ok(stray
            .iter()
            .map(|s| {
                format!("crates/{name}/{s}: not a listing `cargo xtask api` writes (remove it)")
            })
            .collect());
    }
    for s in &stray {
        fsx::remove(&dir.join(s))?;
    }
    Ok(Vec::new())
}

/// Writes `text` as `file` (a path below the root), or with `check` compares
/// the two: `Some(the difference)` when they differ.
fn compare(root: &Path, file: &str, text: &str, check: bool) -> Result<Option<String>> {
    let path = root.join(file);
    if !check {
        fsx::write(&path, text.as_bytes())?;
        return Ok(None);
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(diff(file, &committed, text))
}

/// The lines only one side has, `-` committed and `+` built, under the
/// file's name; `None` when the two are the same.
pub(crate) fn diff(file: &str, committed: &str, built: &str) -> Option<String> {
    if committed == built {
        return None;
    }
    let old: BTreeSet<&str> = committed.lines().collect();
    let new: BTreeSet<&str> = built.lines().collect();
    let mut out = format!("{file}:");
    for line in old.difference(&new) {
        let _ = write!(out, "\n  - {line}");
    }
    for line in new.difference(&old) {
        let _ = write!(out, "\n  + {line}");
    }
    if old == new {
        out.push_str("\n  (the same lines in another order)");
    }
    Some(out)
}

/// `mf2-cli`'s listing, through its test: written when `MF2_CLI_API_WRITE`
/// is set, compared otherwise.
fn cli(root: &Path, check: bool) -> Result<()> {
    let args = [
        "test",
        "-q",
        "-p",
        "mf2-cli",
        "--bin",
        "mf2",
        "--",
        "listing::",
    ]
    .map(OsStr::new);
    let write = OsStr::new("1");
    let envs: &[(&str, &OsStr)] = if check {
        &[]
    } else {
        &[("MF2_CLI_API_WRITE", write)]
    };
    run_inherit_env(&cargo(), &args, root, envs)
        .map_err(|_| fail("crates/mf2-cli/api.txt differs from the command tree (above)"))
}

/// The pinned nightly, with the wasm target `mf2-host-web` needs
/// (rustup, only when either is missing).
pub(crate) fn install(root: &Path) -> Result<()> {
    crate::cmd::rustup_install(root, NIGHTLY, WASM)
}

#[cfg(test)]
mod tests {
    use super::{Modes, diff, hidden_reexports, is_hidden, names, read_modes};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn a_difference_names_its_lines() {
        assert_eq!(diff("x", "a\nb\n", "a\nb\n"), None);
        let d = diff("x", "a\nb\n", "a\nc\n").unwrap();
        assert!(
            d.starts_with("x:") && d.contains("- b") && d.contains("+ c") && !d.contains("- a"),
            "{d}"
        );
        let d = diff("x", "a\nb\n", "b\na\n").unwrap();
        assert!(d.contains("another order"), "{d}");
    }

    #[test]
    fn a_table_of_modes_is_read() {
        let got = read_modes(
            "x",
            &json!({
                "modes": {"core": ["a"], "web": ["a", "b"], "new": ["a", "c"]},
                "baseline": {"1.0.0": {"core": ["a"], "web": ["a", "dep/b"]}},
            }),
        )
        .unwrap();
        let list = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            got,
            Modes {
                modes: BTreeMap::from([
                    ("core".to_owned(), list(&["a"])),
                    ("new".to_owned(), list(&["a", "c"])),
                    ("web".to_owned(), list(&["a", "b"])),
                ]),
                baselines: BTreeMap::from([(
                    "1.0.0".to_owned(),
                    BTreeMap::from([
                        ("core".to_owned(), list(&["a"])),
                        ("web".to_owned(), list(&["a", "dep/b"])),
                    ])
                )]),
            }
        );
    }

    // Negative controls: what the table must not say.
    #[test]
    fn what_the_listing_cannot_follow_is_refused() {
        let refused = |table| read_modes("x", &table).err().map(|e| e.to_string());
        for (table, says) in [
            (json!(["core"]), "is not a table"),
            (
                json!({"modes": {"a": []}, "features": []}),
                "`features` is not one",
            ),
            (json!({"baseline": {}}), "is not a table of feature lists"),
            (json!({"modes": {}}), "names no mode"),
            (
                json!({"modes": {"a": "b"}}),
                "is not a table of feature lists",
            ),
            (
                json!({"modes": {"A b": []}}),
                "is not a name its listing's file",
            ),
            (json!({"modes": {"a": ["dep/b"]}}), "a dependency's feature"),
            (
                json!({"modes": {"a": []}, "baseline": {"1.0.0": {"b": []}}}),
                "which `modes` does not name",
            ),
        ] {
            let got = refused(table).unwrap_or_default();
            assert!(got.contains(says), "{got:?} should say {says:?}");
        }
    }

    /// A crate `c` with a module `m`, a hidden module `h`, and in `m`: a
    /// macro re-exported under another name (`gate`, of the hidden
    /// `__gate`), a hidden re-export of it, and a plain function.
    fn crate_json() -> serde_json::Value {
        let hidden = json!([{"other": "#[doc(hidden)]"}]);
        json!({
            "root": 0,
            "index": {
                "0": {"name": "c", "attrs": [], "inner": {"module": {"items": [1, 2, 3]}}},
                "1": {"name": "m", "attrs": [], "inner": {"module": {"items": [4, 5, 6]}}},
                "2": {"name": "h", "attrs": hidden, "inner": {"module": {"items": [7]}}},
                "3": {"name": "__gate", "attrs": hidden, "inner": {"macro": "macro_rules! __gate { () => { ... }; }"}},
                "4": {"name": null, "attrs": [], "inner": {"use": {"source": "crate::__gate", "name": "gate", "id": 3, "is_glob": false}}},
                "5": {"name": null, "attrs": hidden, "inner": {"use": {"source": "crate::__gate", "name": "also", "id": 3, "is_glob": false}}},
                "6": {"name": "f", "attrs": [], "inner": {"function": {}}},
                "7": {"name": null, "attrs": [], "inner": {"use": {"source": "crate::__gate", "name": "in_h", "id": 3, "is_glob": false}}},
            },
            "paths": {},
        })
    }

    #[test]
    fn a_public_reexport_of_a_hidden_item_is_found() {
        let json = crate_json();
        assert!(is_hidden(&json["index"]["3"]) && !is_hidden(&json["index"]["4"]));
        let found = hidden_reexports(&json);
        // Not the hidden re-export, and nothing inside the hidden module.
        assert_eq!(found.len(), 1, "{found:?}");
        let (path, use_, target) = &found[0];
        assert_eq!(path, &["c", "m"]);
        assert_eq!(use_["inner"]["use"]["name"], "gate");
        assert_eq!(target["name"], "__gate");
        assert_eq!(
            names(&json, 1).0.into_iter().collect::<Vec<_>>(),
            ["also", "f", "gate"]
        );
    }
}
