//! `cargo xtask spec-sync [--rev <sha>] [--check]`: vendor
//! `unicode-org/message-format-wg` `test/` and `LICENSE` verbatim into
//! `third_party/message-format-wg/`, and fetch `spec/` into the git-ignored
//! cache the conformance crate reads (plans/01-conformance.md §1).
//!
//! The specification text is not vendored: since upstream #1112 it may not be
//! distributed publicly without Unicode's permission (D13;
//! plans/17-phase-9-work-order.md A0). It is written to
//! [`mf2_conformance::spec::SPEC_DIR`] only after every file matches the
//! SHA-256 the PIN's `digests` records, and [`SPEC_STAMP`] names the commit.
//!
//! The commit is fetched (blobless, with tags so the PIN can say which release
//! it follows) into `target/xtask-cache/message-format-wg`; files are read from
//! the object store byte-for-byte.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mf2_conformance::spec::{SPEC_DIR, SPEC_STAMP};
use mf2_conformance::{Suite, SuiteTest, diff};

use crate::error::{Error, Result};
use crate::fsx;
use crate::git::{Repo, is_full_sha};
use crate::pin::{Pin, digest_listing, digest_problems};

/// What is vendored, relative to both the upstream root and the vendor directory.
const VENDORED: &[&str] = &["test", "LICENSE"];
/// What is fetched into the cache and never vendored.
const FETCHED: &str = "spec";
/// Where the suite lives inside `test/`.
const SUITE_PREFIX: &str = "test/tests/";

type Tree = BTreeMap<String, Vec<u8>>;

pub(crate) fn run(root: &Path, rev: Option<&str>, check: bool) -> Result<()> {
    let dir = root.join("third_party").join("message-format-wg");
    let mut pin = Pin::load(&dir.join("PIN"))?;
    let url = pin.get("upstream")?.to_owned();
    let pinned = pin.get("commit")?.to_owned();
    let rev = rev.unwrap_or(&pinned).to_owned();
    if !is_full_sha(&rev) {
        return Err(Error::BadRevision(rev));
    }

    let cache = root
        .join("target")
        .join("xtask-cache")
        .join("message-format-wg");
    eprintln!("spec-sync: fetching {url} @ {rev}");
    let repo = Repo::open_or_init(&cache, &url)?;
    repo.fetch_blobless(&[&rev], None, true)?;
    let commit = repo.commit_of(&rev)?;
    if !commit.eq_ignore_ascii_case(&rev) {
        return Err(Error::BadRevision(rev));
    }
    let wanted: Vec<&str> = VENDORED.iter().copied().chain([FETCHED]).collect();
    let patterns: Vec<String> = wanted.iter().map(|p| format!("/{p}")).collect();
    repo.sparse_checkout(&commit, &patterns)?;
    let entries = repo.ls_files(&commit, &wanted)?;
    let mut upstream: Tree = repo.read_blobs(&entries)?;
    for want in &wanted {
        if !upstream
            .keys()
            .any(|k| k == want || k.starts_with(&format!("{want}/")))
        {
            return Err(Error::UpstreamMissing {
                commit: commit.clone(),
                path: (*want).to_owned(),
            });
        }
    }

    let prefix = format!("{FETCHED}/");
    let (spec, vendored): (Tree, Tree) = std::mem::take(&mut upstream)
        .into_iter()
        .partition(|(path, _)| path.starts_with(&prefix));
    upstream = vendored;

    // The spec text is checked against the PIN before it goes anywhere. At
    // the pinned commit the digests must be there and agree (the first sync
    // after the text left the tree records them); a new --rev re-pins them.
    let listing = digest_listing(&spec);
    let repin = !commit.eq_ignore_ascii_case(&pinned);
    match pin.digests()? {
        Some(want) if !repin => verify_digests(&commit, &spec, &want)?,
        None if check => {
            return Err(Error::SpecDigest {
                commit,
                detail: "the PIN records none; run `cargo xtask spec-sync` to record them"
                    .to_owned(),
            });
        }
        _ => {}
    }
    write_cache(root, &commit, &spec)?;

    let mut local = Tree::new();
    for sub in VENDORED {
        local.extend(fsx::read_tree(&dir, sub)?);
    }
    // Anything else next to PIN is not upstream's either — `spec/` included.
    for entry in std::fs::read_dir(&dir).map_err(|source| Error::IoAt {
        path: dir.clone(),
        source,
    })? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name != "PIN" && !VENDORED.contains(&name.as_str()) {
            local.extend(fsx::read_tree(&dir, &name)?);
        }
    }

    if check {
        return compare(&commit, &upstream, &local);
    }

    // Report the suite change by ledger key before replacing anything.
    let old_suite = suite_of(&local)?;
    let new_suite = suite_of(&upstream)?;
    let d = diff(&old_suite, &new_suite);

    for sub in VENDORED.iter().chain([&FETCHED]) {
        fsx::remove(&dir.join(sub))?;
    }
    for (path, bytes) in &upstream {
        fsx::write(&dir.join(path), bytes)?;
    }

    // Re-syncing the pinned commit leaves the PIN alone (its prose stays),
    // unless the digests were never recorded.
    if pin.digests()?.is_none() || repin {
        pin.set("digests", &listing, Some("contents"));
        pin.save()?;
    }
    if repin {
        let date = repo.commit_date(&commit)?;
        let relation = repo.describe(&commit).map_or_else(
            || "no release tag reachable".to_owned(),
            |d| format!("{d} (git describe)"),
        );
        pin.set("commit", &commit, Some("upstream"));
        pin.set("date", &date, Some("commit"));
        pin.set("relation", &relation, Some("date"));
        pin.save()?;
    }

    println!(
        "spec-sync: vendored {commit} ({}): {} files, {} tests in {} suite files; \
         {} spec files in {SPEC_DIR} (not vendored)",
        pin.get("relation").unwrap_or("?"),
        upstream.len(),
        new_suite.tests().len(),
        new_suite.files().len(),
        spec.len()
    );
    let show = |sign: &str, t: &SuiteTest| {
        println!("  {sign} {} #{}  {:?}", t.key, t.index, t.src);
    };
    for t in &d.added {
        show("+", t);
    }
    for t in &d.removed {
        show("-", t);
    }
    for (old, new) in &d.changed {
        println!(
            "  ~ {} #{} -> #{}  {:?}",
            new.key, old.index, new.index, new.src
        );
    }
    println!(
        "spec-sync: tests added {}, removed {}, changed {}, moved {}",
        d.added.len(),
        d.removed.len(),
        d.changed.len(),
        d.moved.len()
    );
    if !d.is_empty() {
        println!(
            "spec-sync: update conformance/ledger.toml in the same commit \
             (`cargo xtask conformance-report` lists what no longer matches)"
        );
    }
    Ok(())
}

/// Every fetched file has the digest the PIN records, and every recorded
/// file was fetched.
fn verify_digests(commit: &str, spec: &Tree, want: &BTreeMap<String, String>) -> Result<()> {
    let problems = digest_problems(spec, want);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::SpecDigest {
            commit: commit.to_owned(),
            detail: problems.join("; "),
        })
    }
}

/// Replaces the cache with `spec`, the stamp last, so a partial write is
/// never taken for a complete one.
fn write_cache(root: &Path, commit: &str, spec: &Tree) -> Result<()> {
    let stamp = root.join(SPEC_STAMP);
    let dir = root.join(SPEC_DIR);
    fsx::remove(&stamp)?;
    fsx::remove(&dir)?;
    let prefix = format!("{FETCHED}/");
    for (path, bytes) in spec {
        let rel = path.strip_prefix(&prefix).unwrap_or(path);
        fsx::write(&dir.join(rel), bytes)?;
    }
    fsx::write(&stamp, format!("{commit}\n").as_bytes())
}

fn suite_of(tree: &Tree) -> Result<Suite> {
    let mut sources = Vec::new();
    for (path, bytes) in tree {
        if let Some(rel) = path.strip_prefix(SUITE_PREFIX)
            && Path::new(rel)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            sources.push((rel.to_owned(), String::from_utf8_lossy(bytes).into_owned()));
        }
    }
    Ok(Suite::from_sources(sources)?)
}

fn compare(commit: &str, upstream: &Tree, local: &Tree) -> Result<()> {
    let paths: BTreeSet<&String> = upstream.keys().chain(local.keys()).collect();
    let mut differences = 0usize;
    for path in paths {
        match (upstream.get(path), local.get(path)) {
            (Some(u), Some(l)) if u == l => {}
            (Some(_), Some(_)) => {
                differences += 1;
                println!("  changed:  {path}");
            }
            (Some(_), None) => {
                differences += 1;
                println!("  missing:  {path}  (upstream has it)");
            }
            (None, Some(_)) => {
                differences += 1;
                println!("  extra:    {path}  (not upstream)");
            }
            (None, None) => {}
        }
    }
    if differences == 0 {
        println!(
            "spec-sync --check: third_party/message-format-wg matches upstream {commit} \
             byte-for-byte ({} files)",
            upstream.len()
        );
        Ok(())
    } else {
        Err(Error::SpecMismatch {
            commit: commit.to_owned(),
            count: differences,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Tree, compare, digest_listing, verify_digests};
    use crate::error::Error;

    #[test]
    fn an_altered_digest_is_refused() {
        let spec = tree(&[("spec/a.md", b"a\n"), ("spec/b.abnf", b"b\n")]);
        let digests = |listing: &str| {
            listing
                .lines()
                .map(|l| {
                    let (d, p) = l.split_once("  ").unwrap();
                    (p.to_owned(), d.to_owned())
                })
                .collect()
        };
        let good = digest_listing(&spec);
        assert!(verify_digests("c", &spec, &digests(&good)).is_ok());
        let first = good.chars().next().unwrap();
        let flipped = if first == '0' { '1' } else { '0' };
        let altered = format!("{flipped}{}", &good[1..]);
        let dropped = good.lines().next().unwrap().to_owned();
        for listing in [altered, dropped] {
            assert!(matches!(
                verify_digests("c", &spec, &digests(&listing)),
                Err(Error::SpecDigest { .. })
            ));
        }
    }

    fn tree(files: &[(&str, &[u8])]) -> Tree {
        files
            .iter()
            .map(|(p, b)| ((*p).to_owned(), b.to_vec()))
            .collect()
    }

    #[test]
    fn compare_is_byte_exact() {
        let upstream = tree(&[("LICENSE", b"L"), ("spec/a.md", b"a\n")]);
        assert!(compare("c", &upstream, &upstream.clone()).is_ok());
        for local in [
            tree(&[("LICENSE", b"L"), ("spec/a.md", b"a\r\n")]),
            tree(&[("LICENSE", b"L")]),
            tree(&[("LICENSE", b"L"), ("spec/a.md", b"a\n"), ("spec/b.md", b"")]),
        ] {
            assert!(matches!(
                compare("c", &upstream, &local),
                Err(Error::SpecMismatch { count: 1, .. })
            ));
        }
    }
}
