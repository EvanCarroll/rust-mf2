//! `cargo xtask xliff-sync [--list] [--check]`: vendor the XLIFF 2 core — the
//! OASIS Standard's specification and its XML schemas — into
//! `third_party/xliff/` (D13).
//!
//! OASIS publishes over HTTPS, not git: `{upstream}{product}/v{version}/{stage}/`
//! is an Apache index. The release's package (its ZIP) is fetched with `curl`
//! into `target/xtask-cache/xliff/`, the files named in the PIN are taken
//! from it with `unzip`, and each is checked against the PIN's SHA-256.
//! They are copied into `third_party/` only when the PIN records, after the
//! licence was read, `redistribute = yes`; with `no` they stay in the cache,
//! and with the field missing the command stops before copying anything.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;
use crate::pin::{Pin, sha256_hex};

/// The cache directory under the repository root.
pub(crate) fn cache_dir(root: &Path) -> PathBuf {
    root.join("target").join("xtask-cache").join("xliff")
}

pub(crate) fn run(root: &Path, list: bool, check: bool) -> Result<()> {
    let dir = root.join("third_party").join("xliff");
    let mut pin = Pin::load(&dir.join("PIN"))?;
    let upstream = pin.get("upstream")?.to_owned();
    let product = pin.get("product")?.to_owned();
    if list {
        return list_versions(&upstream, &product);
    }
    let version = pin.get("version")?.to_owned();
    let stage = pin.get("stage")?.to_owned();
    let package = pin.get("package")?.to_owned();
    let url = format!("{upstream}{product}/v{version}/{stage}/{package}");
    let wanted: Vec<String> = pin
        .get("files")?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let digests = pin.digests()?.unwrap_or_default();

    // The release's ZIP, not the files beside it: the served `.html` is
    // rewritten on every request (the CDN obfuscates each e-mail address
    // afresh), so only the package has stable bytes.
    let cache = cache_dir(root).join(format!("v{version}-{stage}"));
    eprintln!("xliff-sync: {url}");
    let zip = fetch(&url)?;
    let zip_path = cache.join(&package);
    fsx::write(&zip_path, &zip)?;
    let mut fetched: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    fetched.insert(package.clone(), zip);
    let entries = unzip(&zip_path, "-Z1", None)?;
    let entries = String::from_utf8_lossy(&entries).into_owned();
    for rel in &wanted {
        let entry = entries
            .lines()
            .find(|e| *e == rel || e.ends_with(&format!("/{rel}")))
            .ok_or_else(|| Error::Xliff(format!("{package} has no {rel}")))?;
        let bytes = unzip(&zip_path, "-p", Some(entry))?;
        fetched.insert(rel.clone(), bytes);
    }
    for (rel, bytes) in &fetched {
        let got = sha256_hex(bytes);
        if let Some(want) = digests.get(rel)
            && *want != got
        {
            return Err(Error::Xliff(format!(
                "{rel}: sha256 {got}, the PIN says {want} — upstream changed the \
                 published release; read it before re-pinning"
            )));
        }
    }
    // The package itself is the cache's, never vendored.
    let package_bytes = fetched.remove(&package).unwrap_or_default();

    if check {
        let local = vendored(&dir)?;
        return compare(&fetched, &local, &pin);
    }

    let redistribute = pin.get("redistribute").ok().map(str::to_owned);
    let vendor = match redistribute.as_deref() {
        Some("yes") => true,
        Some("no") => false,
        Some(other) => {
            return Err(Error::Xliff(format!(
                "PIN `redistribute = {other}`: expected `yes` or `no`"
            )));
        }
        None => {
            return Err(Error::Xliff(format!(
                "fetched {} files into {} and copied nothing: read the licence \
                 (the specification's Notices) and record `redistribute = yes|no` \
                 and `license` in third_party/xliff/PIN, then run again",
                fetched.len(),
                cache.display()
            )));
        }
    };

    let mut listing = format!("{}  {package}\n", sha256_hex(&package_bytes));
    for (rel, bytes) in &fetched {
        let _ = writeln!(listing, "{}  {rel}", sha256_hex(bytes));
    }
    pin.set("digests", listing.trim_end(), Some("files"));
    if vendor {
        for entry in std::fs::read_dir(&dir).map_err(|source| Error::IoAt {
            path: dir.clone(),
            source,
        })? {
            let entry = entry?;
            if entry.file_name() != "PIN" {
                fsx::remove(&entry.path())?;
            }
        }
        for (rel, bytes) in &fetched {
            fsx::write(&dir.join(rel), bytes)?;
        }
    }
    pin.save()?;
    let total: usize = fetched.values().map(Vec::len).sum();
    println!(
        "xliff-sync: {product} {version} ({stage}), {} files, {total} bytes {}",
        fetched.len(),
        if vendor {
            "vendored into third_party/xliff/".to_owned()
        } else {
            format!("cached in {} (not redistributable)", cache.display())
        }
    );
    Ok(())
}

/// Every version directory of the product, and the stages each holds.
fn list_versions(upstream: &str, product: &str) -> Result<()> {
    let root = format!("{upstream}{product}/");
    let index = String::from_utf8_lossy(&fetch(&root)?).into_owned();
    let mut newest = None;
    for version in links(&index)
        .into_iter()
        .filter(|l| l.starts_with('v') && l.ends_with('/'))
    {
        let inner = String::from_utf8_lossy(&fetch(&format!("{root}{version}"))?).into_owned();
        let stages: Vec<String> = links(&inner)
            .into_iter()
            .filter(|l| l.ends_with('/') && !l.starts_with('/') && !l.starts_with('?'))
            .collect();
        println!("{version:<8} {}", stages.join(" "));
        if stages.iter().any(|s| s == "os/") {
            newest = Some(format!("{root}{version}os/"));
        }
    }
    // The files of the newest OASIS Standard, for the PIN's `files`.
    if let Some(base) = newest {
        println!("files of the newest OASIS Standard, {base}:");
        let mut stack = vec![String::new()];
        while let Some(sub) = stack.pop() {
            let page = String::from_utf8_lossy(&fetch(&format!("{base}{sub}"))?).into_owned();
            for link in links(&page) {
                if link.starts_with('/') || link.starts_with('?') {
                    continue;
                }
                if link.ends_with('/') {
                    stack.push(format!("{sub}{link}"));
                } else {
                    println!("  {sub}{link}");
                }
            }
        }
    }
    Ok(())
}

/// The relative `href`s of an Apache index page.
fn links(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("href=\"") {
        rest = &rest[at + 6..];
        let Some(end) = rest.find('"') else { break };
        let link = &rest[..end];
        if !link.contains("://") && !link.starts_with("..") && !out.iter().any(|l| l == link) {
            out.push(link.to_owned());
        }
        rest = &rest[end..];
    }
    out
}

fn fetch(url: &str) -> Result<Vec<u8>> {
    let args = ["-fsSL", "--proto", "=https", "--max-time", "120", url];
    let refs: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    cmd::run_capture(OsStr::new("curl"), &refs, Path::new("."), &[])
}

/// `unzip -Z1 ZIP` (the entries) or `unzip -p ZIP ENTRY` (one entry's bytes).
fn unzip(zip: &Path, flag: &str, entry: Option<&str>) -> Result<Vec<u8>> {
    let mut args = vec![OsStr::new(flag), zip.as_os_str()];
    args.extend(entry.map(OsStr::new));
    cmd::run_capture(OsStr::new("unzip"), &args, Path::new("."), &[])
}

fn vendored(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut local = BTreeMap::new();
    for entry in std::fs::read_dir(dir).map_err(|source| Error::IoAt {
        path: dir.to_path_buf(),
        source,
    })? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name != "PIN" {
            local.extend(fsx::read_tree(dir, &name)?);
        }
    }
    Ok(local)
}

fn compare(
    upstream: &BTreeMap<String, Vec<u8>>,
    local: &BTreeMap<String, Vec<u8>>,
    pin: &Pin,
) -> Result<()> {
    if pin.get("redistribute").ok() == Some("no") {
        println!(
            "xliff-sync --check: not redistributable; {} files fetched and match the PIN's digests",
            upstream.len()
        );
        return Ok(());
    }
    let mut differences = Vec::new();
    for path in upstream.keys().chain(local.keys()) {
        match (upstream.get(path), local.get(path)) {
            (Some(u), Some(l)) if u == l => {}
            (Some(_), Some(_)) => differences.push(format!("changed: {path}")),
            (Some(_), None) => differences.push(format!("missing: {path}")),
            (None, Some(_)) => differences.push(format!("extra: {path}")),
            (None, None) => {}
        }
    }
    differences.dedup();
    if differences.is_empty() {
        println!(
            "xliff-sync --check: third_party/xliff matches upstream byte for byte ({} files)",
            upstream.len()
        );
        Ok(())
    } else {
        Err(Error::Xliff(format!(
            "third_party/xliff differs from upstream: {}",
            differences.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::links;

    #[test]
    fn reads_an_apache_index() {
        let html = r#"<a href="?C=N;O=D">Name</a> <a href="/xliff/">Parent</a>
            <a href="../">up</a> <a href="v2.0/">v2.0/</a> <a href="v2.1/">v2.1/</a>
            <a href="https://www.oasis-open.org/">x</a> <a href="v2.1/">again</a>"#;
        assert_eq!(links(html), ["?C=N;O=D", "/xliff/", "v2.0/", "v2.1/"]);
    }
}
