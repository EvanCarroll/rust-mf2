//! `cargo xtask cldr-sync`: vendor the CLDR JSON subset named in
//! `third_party/cldr-json/PIN` (Phase 0 task A4).
//!
//! Upstream is very large, so the pinned commit is fetched as a shallow,
//! blobless partial clone and only the needed blobs are materialised (sparse
//! checkout). Files are copied byte-for-byte from the object store.

use std::path::Path;

use crate::error::{Error, Result};
use crate::fsx;
use crate::git::{Repo, is_full_sha};
use crate::pin::Pin;

/// The probe locale panel (plans/01-conformance.md §5, plans/07 A4).
pub(crate) const LOCALES: &[&str] = &[
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
];

/// Top-level vendored entries (removed before re-vendoring; `PIN` is kept).
const VENDORED_ROOTS: &[&str] = &[
    "cldr-core",
    "cldr-numbers-full",
    "cldr-units-full",
    "LICENSE",
];

/// Package-relative paths to vendor.
fn wanted_paths() -> Vec<String> {
    let mut paths = vec![
        "cldr-core/supplemental/plurals.json".to_owned(),
        "cldr-core/supplemental/ordinals.json".to_owned(),
        // Digit sets for `numberingSystem` and currency fraction digits (P0.5).
        "cldr-core/supplemental/numberingSystems.json".to_owned(),
        "cldr-core/supplemental/currencyData.json".to_owned(),
        // Text direction of every locale (Phase 3, `mf2-locale-data`): the
        // likely script of a language (and region), and which scripts are RTL.
        "cldr-core/supplemental/likelySubtags.json".to_owned(),
        "cldr-core/scriptMetadata.json".to_owned(),
    ];
    for loc in LOCALES {
        paths.push(format!("cldr-numbers-full/main/{loc}/numbers.json"));
        paths.push(format!("cldr-numbers-full/main/{loc}/currencies.json"));
        paths.push(format!("cldr-units-full/main/{loc}/units.json"));
    }
    paths
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let dir = root.join("third_party").join("cldr-json");
    let mut pin = Pin::load(&dir.join("PIN"))?;
    let url = pin.get("upstream")?.to_owned();
    let tag = pin.get("tag")?.to_owned();
    let commit = pin.get("commit")?.to_owned();
    if !is_full_sha(&commit) {
        return Err(Error::BadRevision(commit));
    }

    let cache = root.join("target").join("xtask-cache").join("cldr-json");
    eprintln!("cldr-sync: {url} tag {tag} = {commit}");
    let repo = Repo::open_or_init(&cache, &url)?;

    // The tag must still name the pinned commit.
    let tagged = repo.remote_tag_commit(&tag)?;
    match tagged {
        Some(actual) if actual == commit => {}
        Some(actual) => {
            return Err(Error::TagMismatch {
                tag,
                expected: commit,
                actual,
            });
        }
        None => {
            return Err(Error::TagMismatch {
                tag,
                expected: commit,
                actual: "nothing (no such tag upstream)".to_owned(),
            });
        }
    }

    eprintln!(
        "cldr-sync: fetching {commit} (shallow, blobless) into {}",
        cache.display()
    );
    repo.fetch_blobless(&[&commit], Some(1), false)?;
    let resolved = repo.commit_of(&commit)?;
    if resolved != commit {
        return Err(Error::BadRevision(commit));
    }

    // Upstream keeps its packages either at the root or under `cldr-json/`.
    let names = repo.root_names(&commit)?;
    let prefix = if names.iter().any(|n| n == "cldr-core") {
        ""
    } else {
        "cldr-json/"
    };
    let license_upstream = ["LICENSE", "LICENSE.md", "LICENSE.txt"]
        .into_iter()
        .find(|l| names.iter().any(|n| n == l))
        .map_or_else(|| format!("{prefix}cldr-core/LICENSE"), str::to_owned);

    let wanted = wanted_paths();
    let mut upstream_paths: Vec<String> = wanted.iter().map(|p| format!("{prefix}{p}")).collect();
    upstream_paths.push(license_upstream.clone());

    let patterns: Vec<String> = upstream_paths.iter().map(|p| format!("/{p}")).collect();
    eprintln!(
        "cldr-sync: materialising {} files (sparse checkout)",
        upstream_paths.len()
    );
    repo.sparse_checkout(&commit, &patterns)?;

    let refs: Vec<&str> = upstream_paths.iter().map(String::as_str).collect();
    let entries = repo.ls_files(&commit, &refs)?;
    for p in &upstream_paths {
        if !entries.iter().any(|e| &e.path == p) {
            return Err(Error::UpstreamMissing {
                commit: commit.clone(),
                path: p.clone(),
            });
        }
    }
    let blobs = repo.read_blobs(&entries)?;

    for root_entry in VENDORED_ROOTS {
        fsx::remove(&dir.join(root_entry))?;
    }
    let mut total = 0usize;
    for (path, bytes) in &blobs {
        let local = if *path == license_upstream {
            "LICENSE".to_owned()
        } else {
            path.strip_prefix(prefix).unwrap_or(path).to_owned()
        };
        fsx::write(&dir.join(&local), bytes)?;
        total += bytes.len();
        println!("  {local}  ({} bytes)", bytes.len());
    }

    let layout = format!(
        "each file keeps its upstream path relative to upstream's {},\n\
         copied byte-for-byte from the pinned commit:\n\
         cldr-core/supplemental/plurals.json\n\
         cldr-core/supplemental/ordinals.json\n\
         cldr-core/supplemental/numberingSystems.json\n\
         cldr-core/supplemental/currencyData.json\n\
         cldr-core/supplemental/likelySubtags.json\n\
         cldr-core/scriptMetadata.json\n\
         cldr-numbers-full/main/<loc>/numbers.json\n\
         cldr-numbers-full/main/<loc>/currencies.json\n\
         cldr-units-full/main/<loc>/units.json\n\
         LICENSE  (upstream's /{license_upstream})",
        if prefix.is_empty() {
            "repository root".to_owned()
        } else {
            format!("`{prefix}` directory")
        },
    );
    let vendored = format!(
        "{} files ({total} bytes) by `cargo xtask cldr-sync` (Phase 0 task A4; Phase 3 added\n\
         likelySubtags and scriptMetadata for text direction):\n\
         cldr-core/supplemental/{{plurals,ordinals,numberingSystems,currencyData,likelySubtags}}.json;\n\
         cldr-core/scriptMetadata.json;\n\
         cldr-numbers-full/main/<loc>/{{numbers,currencies}}.json and\n\
         cldr-units-full/main/<loc>/units.json for the probe locale panel only;\n\
         LICENSE. The all-locales compact tables (plans/05-tooling.md §7) are in crates/mf2-locale-data/data/\n\
         (`cargo xtask locale-data`).",
        blobs.len(),
    );
    pin.set(
        "license",
        "Unicode-3.0 (upstream LICENSE copied verbatim to ./LICENSE)",
        None,
    );
    pin.set("locales", &LOCALES.join(" "), Some("note"));
    pin.set("layout", &layout, Some("locales"));
    pin.set("vendored", &vendored, Some("layout"));
    pin.save()?;
    eprintln!(
        "cldr-sync: vendored {} files into {}; PIN updated",
        blobs.len(),
        dir.display()
    );
    Ok(())
}
