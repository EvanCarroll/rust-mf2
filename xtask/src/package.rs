//! `cargo xtask package [--check] [--test]`: the 18 published crates as
//! crates.io receives them (`plans/17-phase-9-work-order.md` A4).
//!
//! `cargo package --no-verify` writes each `.crate`; each is then read back
//! and audited, file by file:
//!
//! * every file comes from the crate's own directory — the two licence
//!   symlinks (`LICENSE` → the root's; `mf2-locale-data`'s `LICENSE-UNICODE`
//!   → `third_party/cldr-json/LICENSE`) are the only files packaged from
//!   outside it, and each must point where it says;
//! * no file is a copy of anything under `third_party/`, `plans/` or the
//!   specification cache (`target/xtask-cache/`; the text Unicode does not
//!   let us redistribute, D13), compared by SHA-256 — `LICENSE-UNICODE` is
//!   the one expected copy;
//! * the `.crate` is under crates.io's 10 MB limit.
//!
//! The file list is `crates/<name>/package.txt`: written without `--check`,
//! compared with it — naming the lines — with it, so a file that starts or
//! stops shipping is committed with the change. Sizes are printed, not
//! committed: they move with every edit.
//!
//! `--test` then unpacks every `.crate` into `target/package-test/`, joins
//! them in a workspace of their own whose `[patch.crates-io]` stands in for
//! the registry, and runs their tests with the server feature set
//! (`cargo xtask msrv`'s first step): what the packages' own `cargo test`
//! does against crates.io, the day they are published. A test that cannot
//! run from its package is excluded from it (`exclude` in its manifest) and
//! runs in the workspace.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cmd::{cargo, run_capture, run_inherit};
use crate::error::{Error, Result};
use crate::fsx;
use crate::pin::sha256_hex;

/// The first line of every `package.txt`.
const HEADER: &str = "# The files crates.io receives for this crate (plans/17-phase-9-work-order.md \
                      A4). Written by `cargo xtask package`, checked by `cargo xtask ci`; \
                      commit it with the change.";

/// crates.io's limit on a `.crate`.
const MAX_CRATE: u64 = 10 * 1000 * 1000;

/// Files cargo writes into a package rather than copies from the crate.
const GENERATED: [&str; 3] = ["Cargo.toml", "Cargo.lock", ".cargo_vcs_info.json"];

/// The packaged files that are symlinks out of their crate, and where each
/// must point (from the repository root). `None`: every crate.
const LINKS: [(Option<&str>, &str, &str); 2] = [
    (None, "LICENSE", "LICENSE"),
    (
        Some("mf2-locale-data"),
        "LICENSE-UNICODE",
        "third_party/cldr-json/LICENSE",
    ),
];

/// Where no packaged file may come from (by content).
const FORBIDDEN: [&str; 3] = ["third_party", "plans", "target/xtask-cache"];

fn fail(message: impl Into<String>) -> Error {
    Error::Package(message.into())
}

/// A publishable crate of the workspace.
struct Published {
    name: String,
    version: String,
    dir: PathBuf,
}

/// A packaged `.crate`, read back: each file by its path in the package,
/// and the compressed size.
struct Packaged {
    files: BTreeMap<String, Vec<u8>>,
    size: u64,
}

pub(crate) fn run(root: &Path, check: bool, test: bool) -> Result<()> {
    let published = publishable(root)?;
    let mut args = vec!["package", "--no-verify", "--allow-dirty"];
    for p in &published {
        args.extend(["-p", p.name.as_str()]);
    }
    eprintln!("==> cargo {}", args.join(" "));
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    run_inherit(&cargo(), &args, root)?;
    let out = root.join("target").join("package");
    let forbidden = forbidden(root)?;
    let mut problems = Vec::new();
    let mut stale = Vec::new();
    let mut sizes = String::new();
    for p in &published {
        let packaged = read_crate(&out.join(format!("{}-{}.crate", p.name, p.version)))?;
        problems.extend(audit(root, p, &packaged, &forbidden)?);
        let unpacked: usize = packaged.files.values().map(Vec::len).sum();
        let _ = writeln!(
            sizes,
            "  {:<16} {:>3} files {:>9} B unpacked {:>9} B .crate",
            p.name,
            packaged.files.len(),
            unpacked,
            packaged.size
        );
        let text = listing(&packaged);
        let path = p.dir.join("package.txt");
        if check {
            let committed = fs::read_to_string(&path).unwrap_or_default();
            if let Some(diff) = crate::api::diff(&p.name, &committed, &text) {
                stale.push(diff.replacen("/api.txt:", "/package.txt:", 1));
            }
        } else {
            fsx::write(&path, text.as_bytes())?;
        }
    }
    eprint!("package: sizes\n{sizes}");
    if !problems.is_empty() {
        return Err(fail(format!(
            "the packages ship what they must not:\n  {}",
            problems.join("\n  ")
        )));
    }
    if !stale.is_empty() {
        return Err(fail(format!(
            "the packaged files differ from the committed package.txt; run `cargo xtask \
             package` and commit the lists with the change\n{}",
            stale.join("\n")
        )));
    }
    eprintln!(
        "package: {} packages audited, lists {}",
        published.len(),
        if check { "unchanged" } else { "written" }
    );
    if test {
        test_unpacked(root, &published, &out)?;
    }
    Ok(())
}

/// The workspace's publishable crates (A1's metadata test holds them to the
/// 18), from `cargo metadata`.
fn publishable(root: &Path) -> Result<Vec<Published>> {
    let args = ["metadata", "--format-version", "1", "--no-deps"].map(OsStr::new);
    let bytes = run_capture(&cargo(), &args, root, &[])?;
    let metadata: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Json {
        path: PathBuf::from("cargo metadata"),
        message: e.to_string(),
    })?;
    let mut out = Vec::new();
    for p in metadata["packages"].as_array().into_iter().flatten() {
        if matches!(p["publish"].as_array(), Some(r) if r.is_empty()) {
            continue;
        }
        let (Some(name), Some(version), Some(manifest)) = (
            p["name"].as_str(),
            p["version"].as_str(),
            p["manifest_path"].as_str(),
        ) else {
            return Err(fail(
                "`cargo metadata`: a package without a name, version or path",
            ));
        };
        let dir = Path::new(manifest).parent().unwrap_or(root).to_path_buf();
        out.push(Published {
            name: name.to_owned(),
            version: version.to_owned(),
            dir,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// SHA-256 → the repository path of every file under [`FORBIDDEN`].
/// Every file under [`FORBIDDEN`], by content: one digest may name several
/// paths (the CLDR cache's upstream `LICENSE` is `third_party/cldr-json`'s,
/// byte for byte, once `cargo xtask cldr-sync` has filled the cache).
fn forbidden(root: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for sub in FORBIDDEN {
        for (path, bytes) in fsx::read_tree(root, sub)? {
            out.entry(sha256_hex(&bytes)).or_default().push(path);
        }
    }
    Ok(out)
}

/// What a `.crate` ships, as one SHA-256: every file's path and bytes but
/// the two cargo writes from the state of the tree rather than the crate —
/// `.cargo_vcs_info.json` (the commit packaged from) and `Cargo.lock` (which
/// records, among the dependencies resolved that day, the checksums of our
/// own crates' packages, and so moves with the commit too). The sources and
/// the normalized `Cargo.toml` are compared. Two packages with the same
/// digest ship the same crate.
pub(crate) fn content_digest(path: &Path) -> Result<String> {
    let packaged = read_crate(path)?;
    let mut all = Vec::new();
    for (rel, bytes) in packaged.files {
        if rel == ".cargo_vcs_info.json" || rel == "Cargo.lock" {
            continue;
        }
        all.extend_from_slice(rel.as_bytes());
        all.push(0);
        all.extend_from_slice(sha256_hex(&bytes).as_bytes());
        all.push(b'\n');
    }
    Ok(sha256_hex(&all))
}

fn read_crate(path: &Path) -> Result<Packaged> {
    let at = |source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    };
    let size = fs::metadata(path).map_err(at)?.len();
    let file = fs::File::open(path).map_err(at)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut files = BTreeMap::new();
    for entry in archive.entries().map_err(at)? {
        let mut entry = entry.map_err(at)?;
        let name = entry.path().map_err(at)?.to_string_lossy().into_owned();
        // Every path is under `<name>-<version>/`.
        let Some((_, rel)) = name.split_once('/') else {
            return Err(fail(format!(
                "{}: `{name}` is not in the package's directory",
                path.display()
            )));
        };
        let rel = rel.to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(at)?;
        files.insert(rel, bytes);
    }
    Ok(Packaged { files, size })
}

/// Every problem with one package, one line each.
fn audit(
    root: &Path,
    p: &Published,
    packaged: &Packaged,
    forbidden: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let at = |path: &Path| {
        let path = path.to_path_buf();
        move |source| Error::IoAt { path, source }
    };
    let dir = fs::canonicalize(&p.dir).map_err(at(&p.dir))?;
    if packaged.size > MAX_CRATE {
        out.push(format!(
            "{}: the .crate is {} B, over crates.io's {MAX_CRATE}",
            p.name, packaged.size
        ));
    }
    for (rel, bytes) in &packaged.files {
        if GENERATED.contains(&rel.as_str()) {
            continue;
        }
        let source = if rel == "Cargo.toml.orig" {
            p.dir.join("Cargo.toml")
        } else {
            p.dir.join(rel)
        };
        let link = LINKS
            .iter()
            .find(|(only, name, _)| *name == rel && only.is_none_or(|c| c == p.name))
            .map(|(_, _, target)| target);
        let Ok(real) = fs::canonicalize(&source) else {
            out.push(format!("{}: {rel} is not a file of the crate", p.name));
            continue;
        };
        match link {
            Some(target) => {
                let want = fs::canonicalize(root.join(target)).map_err(at(&root.join(target)))?;
                if real != want {
                    out.push(format!(
                        "{}: {rel} is {}, not {target}",
                        p.name,
                        real.display()
                    ));
                }
            }
            None if !real.starts_with(&dir) => {
                out.push(format!(
                    "{}: {rel} is {}, outside the crate",
                    p.name,
                    real.display()
                ));
            }
            None => {}
        }
        if fs::read(&real).map_err(at(&real))? != *bytes {
            out.push(format!(
                "{}: {rel} differs from {}",
                p.name,
                source.display()
            ));
        }
        if let Some(copies) = forbidden
            .get(&sha256_hex(bytes))
            .filter(|_| !bytes.is_empty())
            && link.is_none_or(|target| !copies.iter().any(|copy| copy == target))
        {
            out.push(format!(
                "{}: {rel} is a copy of {}",
                p.name,
                copies.join(", ")
            ));
        }
    }
    Ok(out)
}

fn listing(packaged: &Packaged) -> String {
    let mut text = format!("{HEADER}\n");
    for rel in packaged.files.keys() {
        text.push_str(rel);
        text.push('\n');
    }
    text
}

/// The packages' own tests, from their `.crate` files, in a workspace of
/// their own that patches crates.io with them.
fn test_unpacked(root: &Path, published: &[Published], out: &Path) -> Result<()> {
    let dir = root.join("target").join("package-test");
    let mut manifest = String::from(
        "# Written by `cargo xtask package --test`: the unpacked packages, with\n\
         # crates.io patched to them (they are not published yet).\n\
         [workspace]\nresolver = \"3\"\nmembers = [\n",
    );
    let mut patch = String::from("\n[patch.crates-io]\n");
    for p in published {
        let unpacked = format!("{}-{}", p.name, p.version);
        let at = dir.join(&unpacked);
        fsx::remove(&at)?;
        for (rel, bytes) in read_crate(&out.join(format!("{unpacked}.crate")))?.files {
            fsx::write(&at.join(rel), &bytes)?;
        }
        let _ = writeln!(manifest, "    \"{unpacked}\",");
        let _ = writeln!(patch, "{} = {{ path = \"{unpacked}\" }}", p.name);
    }
    manifest.push_str("]\n");
    manifest.push_str(&patch);
    fsx::write(&dir.join("Cargo.toml"), manifest.as_bytes())?;
    let args = [
        "test",
        "--no-fail-fast",
        "--workspace",
        "--features",
        crate::msrv::SERVER_FEATURES,
    ];
    eprintln!("==> (target/package-test) cargo {}", args.join(" "));
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    run_inherit(&cargo(), &args, &dir)
        .map_err(|_| fail("the unpacked packages' tests failed (target/package-test)"))?;
    eprintln!(
        "package: the {} packages' own tests pass from their .crate files",
        published.len()
    );
    Ok(())
}
