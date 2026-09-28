//! `cargo xtask cldr-sync`: vendor the CLDR JSON subset named in
//! `third_party/cldr-json/PIN` (Phase 0 task A4), and materialise every
//! locale's number, currency and unit files in the cache for the all-locale
//! tables of `mf2-locale-data` (Phase 4 task A2; `plans/05-tooling.md` §7).
//!
//! Upstream is very large, so the pinned commit is fetched as a shallow,
//! blobless partial clone and only the needed blobs are materialised (sparse
//! checkout). Vendored files are copied byte-for-byte from the object store;
//! the cache-only files stay in the cache's working tree
//! (`target/xtask-cache/cldr-json`, never vendored), each checked against
//! its blob's object name (`git hash-object --no-filters`), so they too are
//! the pinned bytes exactly.

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

/// Package-relative files materialised only in the cache, never vendored:
/// the core files that resolve a locale tag to a CLDR locale (Phase 4, A2).
const CACHE_CORE: &[&str] = &[
    "cldr-core/availableLocales.json",
    "cldr-core/defaultContent.json",
    "cldr-core/supplemental/parentLocales.json",
];

/// Per-locale files materialised only in the cache, for **every** locale:
/// `(package main directory, file name)` (Phase 4, A2; the currency and unit
/// files feed A4).
const CACHE_PER_LOCALE: &[(&str, &str)] = &[
    ("cldr-numbers-full/main", "numbers.json"),
    ("cldr-numbers-full/main", "currencies.json"),
    ("cldr-units-full/main", "units.json"),
];

/// The cache directory (a git working tree) under the repository root.
pub(crate) fn cache_dir(root: &Path) -> std::path::PathBuf {
    root.join("target").join("xtask-cache").join("cldr-json")
}

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
        // The one locale matcher (Phase 10 C3, master plan D21): the distance
        // between a requested and a supported locale, and which other script
        // a reader accepts; its `$americas` match variable is a macro-region
        // (`019`), so the matcher needs the region containment too.
        "cldr-core/supplemental/languageMatching.json".to_owned(),
        "cldr-core/supplemental/territoryContainment.json".to_owned(),
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

    let cache = cache_dir(root);
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

    let mut patterns: Vec<String> = upstream_paths.iter().map(|p| format!("/{p}")).collect();
    patterns.extend(CACHE_CORE.iter().map(|p| format!("/{prefix}{p}")));
    patterns.extend(
        CACHE_PER_LOCALE
            .iter()
            .map(|(dir, file)| format!("/{prefix}{dir}/*/{file}")),
    );
    eprintln!(
        "cldr-sync: materialising {} vendored files and every locale's number, currency and \
         unit files (sparse checkout)",
        upstream_paths.len()
    );
    repo.sparse_checkout(&commit, &patterns)?;
    let cached = verify_cache(&repo, &commit, prefix)?;

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
         cldr-core/supplemental/languageMatching.json\n\
         cldr-core/supplemental/territoryContainment.json\n\
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
         likelySubtags and scriptMetadata for text direction; Phase 10 languageMatching and\n\
         territoryContainment for the one locale matcher):\n\
         cldr-core/supplemental/{{plurals,ordinals,numberingSystems,currencyData,likelySubtags}}.json;\n\
         cldr-core/supplemental/{{languageMatching,territoryContainment}}.json;\n\
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
    pin.set("cache", &cached.describe(prefix), Some("vendored"));
    pin.save()?;
    eprintln!(
        "cldr-sync: vendored {} files into {}; PIN updated",
        blobs.len(),
        dir.display()
    );
    Ok(())
}

/// What `verify_cache` found in the cache.
struct Cached {
    locales: usize,
    files: usize,
    bytes: u64,
}

impl Cached {
    fn describe(&self, prefix: &str) -> String {
        format!(
            "{} files ({} bytes) materialised by `cargo xtask cldr-sync` in\n\
             target/xtask-cache/cldr-json/{prefix} (the cache; never vendored), each checked\n\
             byte-for-byte against the pinned commit's blob (Phase 4 task A2):\n\
             cldr-core/{{availableLocales,defaultContent}}.json;\n\
             cldr-core/supplemental/parentLocales.json;\n\
             cldr-numbers-full/main/*/{{numbers,currencies}}.json and\n\
             cldr-units-full/main/*/units.json for every locale ({} locales).\n\
             The -full locale files are resolved against their parents: every field a\n\
             locale inherits (from a parent locale or root, `und`) is written out in it;\n\
             no default-content locale (en-US, ar-001, ...) has files of its own; root has\n\
             only the latn numbering system. `cargo xtask locale-data` checks all three\n\
             and reads these files together with the vendored supplemental ones.",
            self.files, self.bytes, self.locales
        )
    }
}

/// Checks that every cache-only file is in the working tree with exactly its
/// blob's bytes, and that each per-locale file exists for the same locales.
fn verify_cache(repo: &Repo, commit: &str, prefix: &str) -> Result<Cached> {
    let core: Vec<String> = CACHE_CORE.iter().map(|p| format!("{prefix}{p}")).collect();
    let mut dirs: Vec<String> = CACHE_PER_LOCALE
        .iter()
        .map(|(dir, _)| format!("{prefix}{dir}"))
        .collect();
    dirs.dedup();
    let mut query: Vec<&str> = core.iter().map(String::as_str).collect();
    query.extend(dirs.iter().map(String::as_str));
    let listed = repo.ls_files(commit, &query)?;
    for p in &core {
        if !listed.iter().any(|e| &e.path == p) {
            return Err(Error::UpstreamMissing {
                commit: commit.to_owned(),
                path: p.clone(),
            });
        }
    }
    let mut locales_per_file: Vec<Vec<&str>> = vec![Vec::new(); CACHE_PER_LOCALE.len()];
    let mut entries = Vec::new();
    for e in &listed {
        if core.contains(&e.path) {
            entries.push(e);
            continue;
        }
        for (k, (dir, file)) in CACHE_PER_LOCALE.iter().enumerate() {
            let Some(rest) = e.path.strip_prefix(&format!("{prefix}{dir}/")) else {
                continue;
            };
            if let Some((locale, name)) = rest.split_once('/')
                && name == *file
                && let Some(list) = locales_per_file.get_mut(k)
            {
                list.push(locale);
                entries.push(e);
            }
        }
    }
    let first = locales_per_file.first().cloned().unwrap_or_default();
    if first.is_empty() || locales_per_file.iter().any(|l| *l != first) {
        return Err(Error::UpstreamMissing {
            commit: commit.to_owned(),
            path: "the same locales in every per-locale package".to_owned(),
        });
    }
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let hashes = repo.hash_files(&paths)?;
    if hashes.len() != entries.len() {
        return Err(Error::CacheMismatch(format!(
            "hashed {} of {} files",
            hashes.len(),
            entries.len()
        )));
    }
    let mut bytes = 0u64;
    for (e, h) in entries.iter().zip(&hashes) {
        if *h != e.oid {
            return Err(Error::CacheMismatch(e.path.clone()));
        }
        let path = repo.dir().join(&e.path);
        bytes += std::fs::metadata(&path)
            .map_err(|source| Error::IoAt { path, source })?
            .len();
    }
    eprintln!(
        "cldr-sync: cache holds {} files ({bytes} bytes) for {} locales, byte-for-byte the pinned blobs",
        entries.len(),
        first.len()
    );
    Ok(Cached {
        locales: first.len(),
        files: entries.len(),
        bytes,
    })
}
