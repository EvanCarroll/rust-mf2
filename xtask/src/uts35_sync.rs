//! `cargo xtask uts35-sync [--tag <release tag>] [--list]`: fetch UTS #35
//! Part 1 (Core) — the text of the language-matching rules the one locale
//! matcher follows (master plan D21) — from the CLDR repository into the
//! git-ignored cache, at the release that goes with the vendored CLDR data.
//!
//! Nothing is vendored (owner, 2026-09-28; D13): as `spec-sync` does for the
//! MF2 specification, the
//! text is written to [`TEXT_DIR`] only after every file matches the SHA-256
//! the PIN's `digests` records, with the commit in a `COMMIT` stamp written
//! last. Plans, code and tests paraphrase it and cite the section; nothing
//! in the tree quotes it.
//!
//! As `cldr-sync` requires of its own PIN, the `tag` must still name the
//! `commit` upstream (`git ls-remote`), and the PIN's `data` must be the
//! vendored CLDR data's release (`third_party/cldr-json/PIN`'s `tag`), so the
//! text cannot fall behind the data it describes. The commit is fetched
//! shallow and blobless into `target/xtask-cache/cldr`, and the files are
//! read from the object store byte for byte. `--tag` re-pins: it records the
//! tag's commit and date, the data's release and the digests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::fsx;
use crate::git::{Repo, is_full_sha};
use crate::pin::{Pin, digest_listing, digest_problems};

/// The PIN, relative to the repository root.
const PIN: &str = "third_party/uts35/PIN";

/// The CLDR data's PIN, whose `tag` this PIN's `data` must be.
const DATA_PIN: &str = "third_party/cldr-json/PIN";

/// Where the checked text is written (each file at its upstream path),
/// relative to the repository root.
const TEXT_DIR: &str = "target/xtask-cache/uts35";

/// The file in [`TEXT_DIR`] naming the commit the text was fetched at.
const STAMP: &str = "COMMIT";

type Tree = BTreeMap<String, Vec<u8>>;

fn fail(message: impl Into<String>) -> Error {
    Error::Uts35(message.into())
}

/// The clone of the CLDR repository (shallow, blobless, sparse).
fn clone_dir(root: &Path) -> PathBuf {
    root.join("target").join("xtask-cache").join("cldr")
}

pub(crate) fn run(root: &Path, retag: Option<&str>, list: bool) -> Result<()> {
    let mut pin = Pin::load(&root.join(PIN))?;
    let url = pin.get("upstream")?.to_owned();
    let repo = Repo::open_or_init(&clone_dir(root), &url)?;
    if list {
        for (tag, commit) in repo.remote_tags()? {
            println!("{commit}  {tag}");
        }
        return Ok(());
    }
    let data = Pin::load(&root.join(DATA_PIN))?.get("tag")?.to_owned();
    let tag = match retag {
        Some(tag) => tag.to_owned(),
        None => pin
            .get("tag")
            .map_err(|_| {
                fail(format!(
                    "{PIN} names no tag: pin one with `--tag <release tag>` (`--list` shows them)"
                ))
            })?
            .to_owned(),
    };

    eprintln!("uts35-sync: {url} tag {tag}");
    let tagged = repo.remote_tag_commit(&tag)?.ok_or_else(|| {
        fail(format!(
            "upstream has no tag {tag} (`--list` shows the tags)"
        ))
    })?;
    if retag.is_none() {
        let pinned = pin.get("commit")?;
        if !pinned.eq_ignore_ascii_case(&tagged) {
            return Err(Error::TagMismatch {
                tag,
                expected: pinned.to_owned(),
                actual: tagged,
            });
        }
        same_data(pin.get("data")?, &data)?;
    }
    if !is_full_sha(&tagged) {
        return Err(Error::BadRevision(tagged));
    }
    eprintln!(
        "uts35-sync: fetching {tagged} (shallow, blobless) into {}",
        repo.dir().display()
    );
    repo.fetch_blobless(&[&tagged], Some(1), false)?;
    let commit = repo.commit_of(&tagged)?;
    if !commit.eq_ignore_ascii_case(&tagged) {
        return Err(Error::BadRevision(tagged));
    }

    let wanted: Vec<String> = pin
        .get("files")?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let patterns: Vec<String> = wanted.iter().map(|p| format!("/{p}")).collect();
    repo.sparse_checkout(&commit, &patterns)?;
    let refs: Vec<&str> = wanted.iter().map(String::as_str).collect();
    let entries = repo.ls_files(&commit, &refs)?;
    for path in &wanted {
        if !entries.iter().any(|e| &e.path == path) {
            return Err(Error::UpstreamMissing {
                commit: commit.clone(),
                path: path.clone(),
            });
        }
    }
    let text: Tree = repo.read_blobs(&entries)?;

    // Checked before anything is written. A re-pin records the digests
    // instead, and says whether the text moved.
    let recorded = pin.digests()?;
    if retag.is_none() {
        let want = recorded.ok_or_else(|| {
            fail(format!(
                "{PIN} records no digests: re-pin with `--tag {tag}`"
            ))
        })?;
        verify(&commit, &text, &want)?;
    } else if let Some(old) = recorded
        && !digest_problems(&text, &old).is_empty()
    {
        eprintln!(
            "uts35-sync: the text differs from the one pinned before: re-read what the plans \
             cite from it"
        );
    }
    write_text(root, &commit, &text)?;

    if retag.is_some() {
        let date = repo.commit_date(&commit)?;
        pin.set("tag", &tag, Some("upstream"));
        pin.set("commit", &commit, Some("tag"));
        pin.set("date", &date, Some("commit"));
        pin.set("data", &data, Some("date"));
        pin.set("digests", &digest_listing(&text), Some("files"));
        pin.save()?;
    }
    let bytes: usize = text.values().map(Vec::len).sum();
    println!(
        "uts35-sync: {tag} = {commit}: {} file(s), {bytes} bytes, in {TEXT_DIR}/ (not \
         vendored){}",
        text.len(),
        if retag.is_some() {
            "; the PIN re-pinned"
        } else {
            ""
        }
    );
    Ok(())
}

/// The PIN's `data` is the vendored CLDR data's release.
fn same_data(described: &str, vendored: &str) -> Result<()> {
    if described == vendored {
        Ok(())
    } else {
        Err(fail(format!(
            "the text is pinned for CLDR data {described}, but {DATA_PIN} is at {vendored}: \
             re-pin the text at the matching release (`--tag`; `--list` shows the tags) and \
             re-read what the plans cite from it"
        )))
    }
}

/// Every fetched file has the digest the PIN records, and every recorded
/// file was fetched.
fn verify(commit: &str, text: &Tree, want: &BTreeMap<String, String>) -> Result<()> {
    let problems = digest_problems(text, want);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(fail(format!(
            "the text fetched at {commit} does not match the digests in {PIN}: {}; nothing was \
             written",
            problems.join("; ")
        )))
    }
}

/// Replaces [`TEXT_DIR`] with `text`, the stamp last, so a partial write is
/// never taken for a complete one.
fn write_text(root: &Path, commit: &str, text: &Tree) -> Result<()> {
    let dir = root.join(TEXT_DIR);
    fsx::remove(&dir)?;
    for (path, bytes) in text {
        fsx::write(&dir.join(path), bytes)?;
    }
    fsx::write(&dir.join(STAMP), format!("{commit}\n").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{DATA_PIN, PIN, Tree, same_data, verify};
    use crate::error::Error;
    use crate::fsx;
    use crate::pin::{Pin, digest_listing};

    /// The committed PINs agree: the text was pinned for the CLDR data the
    /// tree vendors. A new `cldr-sync` tag fails here until the text is
    /// re-pinned with it (and what the plans cite from it re-read).
    #[test]
    fn the_text_goes_with_the_vendored_data() {
        let root = fsx::repo_root();
        let text = Pin::load(&root.join(PIN)).unwrap();
        let data = Pin::load(&root.join(DATA_PIN)).unwrap();
        same_data(text.get("data").unwrap(), data.get("tag").unwrap()).unwrap();
        assert!(
            same_data("48.2.1", "49.0.0").is_err(),
            "another release must be refused"
        );
    }

    #[test]
    fn an_altered_digest_is_refused() {
        let text: Tree = [("docs/ldml/tr35.md", b"text\n")]
            .into_iter()
            .map(|(p, b)| (p.to_owned(), b.to_vec()))
            .collect();
        let digests = |listing: &str| {
            listing
                .lines()
                .map(|l| {
                    let (d, p) = l.split_once("  ").unwrap();
                    (p.to_owned(), d.to_owned())
                })
                .collect()
        };
        let good = digest_listing(&text);
        assert!(verify("c", &text, &digests(&good)).is_ok());
        let first = good.chars().next().unwrap();
        let flipped = if first == '0' { '1' } else { '0' };
        let altered = format!("{flipped}{}", &good[1..]);
        for listing in [altered, String::new()] {
            let refused = verify("c", &text, &digests(&listing));
            assert!(
                matches!(&refused, Err(Error::Uts35(m)) if m.contains("nothing was written")),
                "{refused:?}"
            );
        }
    }
}
