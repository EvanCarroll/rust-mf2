//! `cargo xtask spec-sync [--rev <sha>] [--check]`: vendor
//! `unicode-org/message-format-wg` `spec/`, `test/` and `LICENSE` verbatim into
//! `third_party/message-format-wg/` (plans/01-conformance.md §1).
//!
//! The commit is fetched (blobless, with tags so the PIN can say which release
//! it follows) into `target/xtask-cache/message-format-wg`; files are read from
//! the object store byte-for-byte.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mf2_conformance::{Suite, SuiteTest, diff};

use crate::error::{Error, Result};
use crate::fsx;
use crate::git::{Repo, is_full_sha};
use crate::pin::Pin;

/// What is vendored, relative to both the upstream root and the vendor directory.
const VENDORED: &[&str] = &["spec", "test", "LICENSE"];
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
    let patterns: Vec<String> = VENDORED.iter().map(|p| format!("/{p}")).collect();
    repo.sparse_checkout(&commit, &patterns)?;
    let entries = repo.ls_files(&commit, VENDORED)?;
    let upstream: Tree = repo.read_blobs(&entries)?;
    for want in VENDORED {
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

    let mut local = Tree::new();
    for sub in VENDORED {
        local.extend(fsx::read_tree(&dir, sub)?);
    }
    // Anything else next to PIN is not upstream's either.
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

    for sub in VENDORED {
        fsx::remove(&dir.join(sub))?;
    }
    for (path, bytes) in &upstream {
        fsx::write(&dir.join(path), bytes)?;
    }

    // Re-syncing the pinned commit leaves the PIN alone (its prose stays).
    if !commit.eq_ignore_ascii_case(&pinned) {
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
        "spec-sync: vendored {commit} ({}): {} files, {} tests in {} suite files",
        pin.get("relation").unwrap_or("?"),
        upstream.len(),
        new_suite.tests().len(),
        new_suite.files().len()
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
    use super::{Tree, compare};
    use crate::error::Error;

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
