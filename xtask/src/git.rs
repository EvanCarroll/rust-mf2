//! A minimal wrapper around the `git` CLI for fetching one upstream commit into a
//! cache under `target/` and reading its files byte-for-byte.
//!
//! Files are read from the object store (`git cat-file blob`), never from a
//! working tree, so `.gitattributes` / eol conversion cannot alter them. A sparse
//! checkout is used only because it makes git fetch the needed blobs of a
//! blobless partial clone in one batch.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cmd::run_capture;
use crate::error::{Error, Result};

/// A non-interactive environment: never prompt for credentials.
const ENV: &[(&str, &str)] = &[("GIT_TERMINAL_PROMPT", "0")];

/// One entry of `git ls-tree -r`.
#[derive(Debug, Clone)]
pub(crate) struct TreeEntry {
    pub(crate) mode: String,
    pub(crate) kind: String,
    pub(crate) oid: String,
    pub(crate) path: String,
}

/// A cache repository with one remote, `origin`.
pub(crate) struct Repo {
    dir: PathBuf,
}

/// Whether `s` is a full hexadecimal SHA-1 object name.
pub(crate) fn is_full_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Repo {
    /// Opens the cache repository at `dir`, creating it if needed, with `origin` = `url`.
    pub(crate) fn open_or_init(dir: &Path, url: &str) -> Result<Self> {
        if !dir.join(".git").is_dir() {
            fs::create_dir_all(dir).map_err(|source| Error::IoAt {
                path: dir.to_path_buf(),
                source,
            })?;
            git_in(dir, &["init", "--quiet"])?;
        }
        let repo = Self {
            dir: dir.to_path_buf(),
        };
        if repo.git(&["remote", "get-url", "origin"]).is_ok() {
            repo.git(&["remote", "set-url", "origin", url])?;
        } else {
            repo.git(&["remote", "add", "origin", url])?;
        }
        Ok(repo)
    }

    /// Opens the cache repository at `dir` without creating or configuring
    /// anything; `None` when there is none.
    pub(crate) fn open(dir: &Path) -> Option<Self> {
        dir.join(".git").is_dir().then(|| Self {
            dir: dir.to_path_buf(),
        })
    }

    /// The directory of the working tree.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// The object names of the working-tree files `paths` (relative to the
    /// repository), hashed as blobs of their raw bytes (`git hash-object
    /// --no-filters`): equal to the tree entries' names exactly when the files
    /// are byte-for-byte the committed blobs.
    pub(crate) fn hash_files(&self, paths: &[&str]) -> Result<Vec<String>> {
        let mut out = Vec::with_capacity(paths.len());
        for chunk in paths.chunks(256) {
            let mut args = vec!["hash-object", "--no-filters", "--"];
            args.extend_from_slice(chunk);
            let text = self.git_text(&args)?;
            out.extend(text.lines().map(str::to_owned));
        }
        Ok(out)
    }

    /// Runs `git <args>` in the repository and returns stdout.
    pub(crate) fn git(&self, args: &[&str]) -> Result<Vec<u8>> {
        git_in(&self.dir, args)
    }

    fn git_text(&self, args: &[&str]) -> Result<String> {
        let out = self.git(args)?;
        String::from_utf8(out)
            .map(|s| s.trim_end().to_owned())
            .map_err(|e| Error::Utf8(format!("git {}", args.join(" ")), e))
    }

    /// Fetches `revs` from `origin` without blobs (they are fetched on checkout).
    pub(crate) fn fetch_blobless(
        &self,
        revs: &[&str],
        depth: Option<u32>,
        tags: bool,
    ) -> Result<()> {
        let depth_arg = depth.map(|d| format!("--depth={d}"));
        let mut args = vec![
            "fetch",
            "--quiet",
            "--filter=blob:none",
            "--no-recurse-submodules",
        ];
        if let Some(d) = &depth_arg {
            args.push(d);
        }
        args.push(if tags { "--tags" } else { "--no-tags" });
        args.push("origin");
        args.extend_from_slice(revs);
        self.git(&args).map(drop)
    }

    /// Resolves `rev` to a full commit sha.
    pub(crate) fn commit_of(&self, rev: &str) -> Result<String> {
        self.git_text(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
    }

    /// The committer date of `commit` as `YYYY-MM-DD`.
    pub(crate) fn commit_date(&self, commit: &str) -> Result<String> {
        self.git_text(&["show", "--no-patch", "--format=%cs", commit])
    }

    /// `git describe --tags --long`, rendered as "tag X" or "tag X + N commits".
    pub(crate) fn describe(&self, commit: &str) -> Option<String> {
        let d = self
            .git_text(&["describe", "--tags", "--long", commit])
            .ok()?;
        // <tag>-<count>-g<abbrev>; the tag itself may contain '-'.
        let mut it = d.rsplitn(3, '-');
        let _abbrev = it.next()?;
        let count: u32 = it.next()?.parse().ok()?;
        let tag = it.next()?;
        Some(match count {
            0 => format!("tag {tag}"),
            1 => format!("tag {tag} + 1 commit"),
            n => format!("tag {tag} + {n} commits"),
        })
    }

    /// The commit a remote tag points at (peeled), via `git ls-remote`.
    pub(crate) fn remote_tag_commit(&self, tag: &str) -> Result<Option<String>> {
        let plain = format!("refs/tags/{tag}");
        let peeled = format!("refs/tags/{tag}^{{}}");
        let out = self.git_text(&["ls-remote", "--tags", "origin", &plain])?;
        let mut direct = None;
        let mut deref = None;
        for line in out.lines() {
            let mut parts = line.split('\t');
            let (Some(oid), Some(name)) = (parts.next(), parts.next()) else {
                continue;
            };
            if name == peeled {
                deref = Some(oid.to_owned());
            } else if name == plain {
                direct = Some(oid.to_owned());
            }
        }
        Ok(deref.or(direct))
    }

    /// Lists the regular files under `paths` (files or directories) at `commit`.
    ///
    /// Fails on symlinks and submodules: vendored trees must be plain files.
    pub(crate) fn ls_files(&self, commit: &str, paths: &[&str]) -> Result<Vec<TreeEntry>> {
        let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", commit, "--"];
        args.extend_from_slice(paths);
        let out = self.git(&args)?;
        let mut entries = Vec::new();
        for record in out.split(|&b| b == 0).filter(|r| !r.is_empty()) {
            let record = String::from_utf8(record.to_vec())
                .map_err(|e| Error::Utf8("git ls-tree".to_owned(), e))?;
            let (meta, path) = record
                .split_once('\t')
                .ok_or_else(|| Error::CommandFailed {
                    command: "git ls-tree".to_owned(),
                    status: "unparseable output".to_owned(),
                    stderr: format!(": {record}"),
                })?;
            let mut meta = meta.split(' ');
            let entry = TreeEntry {
                mode: meta.next().unwrap_or_default().to_owned(),
                kind: meta.next().unwrap_or_default().to_owned(),
                oid: meta.next().unwrap_or_default().to_owned(),
                path: path.to_owned(),
            };
            if entry.kind != "blob" || !(entry.mode == "100644" || entry.mode == "100755") {
                return Err(Error::UpstreamNotAFile {
                    commit: commit.to_owned(),
                    path: entry.path,
                    kind: format!("{} (mode {})", entry.kind, entry.mode),
                });
            }
            entries.push(entry);
        }
        Ok(entries)
    }

    /// The names directly under the root tree of `commit`.
    pub(crate) fn root_names(&self, commit: &str) -> Result<Vec<String>> {
        let out = self.git_text(&["ls-tree", "--name-only", commit])?;
        Ok(out.lines().map(str::to_owned).collect())
    }

    /// Checks out `commit` (detached) restricted to the non-cone sparse `patterns`.
    ///
    /// In a blobless clone this fetches exactly the blobs those paths need, in
    /// one batch.
    pub(crate) fn sparse_checkout(&self, commit: &str, patterns: &[String]) -> Result<()> {
        let info = self.dir.join(".git").join("info");
        fs::create_dir_all(&info).map_err(|source| Error::IoAt {
            path: info.clone(),
            source,
        })?;
        let file = info.join("sparse-checkout");
        let mut text = patterns.join("\n");
        text.push('\n');
        fs::write(&file, text).map_err(|source| Error::IoAt { path: file, source })?;
        self.git(&["config", "core.sparseCheckout", "true"])?;
        self.git(&["config", "core.sparseCheckoutCone", "false"])?;
        self.git(&[
            "-c",
            "advice.detachedHead=false",
            "checkout",
            "--quiet",
            "--force",
            "--detach",
            commit,
        ])
        .map(drop)
    }

    /// Reads the raw bytes of each entry's blob, keyed by path.
    pub(crate) fn read_blobs(&self, entries: &[TreeEntry]) -> Result<BTreeMap<String, Vec<u8>>> {
        let mut files = BTreeMap::new();
        for e in entries {
            let bytes = self.git(&["cat-file", "blob", &e.oid])?;
            files.insert(e.path.clone(), bytes);
        }
        Ok(files)
    }
}

fn git_in(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    run_capture(OsStr::new("git"), &args, dir, ENV)
}
