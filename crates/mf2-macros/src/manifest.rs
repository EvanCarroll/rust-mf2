//! Getting the manifest, once per compiler process
//! (probe P0.9).
//!
//! The generated `tr!` wrapper bakes in the manifest's absolute path and its
//! hash, so 2,000 expansions in one rustc (or one long-lived rust-analyzer
//! proc-macro server) read the file once: the cache is keyed by the path and
//! every hit is verified against the baked hash, and a manifest that hashes
//! to something else is *reported*, never used.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use mf2_catalog::Manifest;

use crate::error::ManifestError;

/// Where an expansion's manifest comes from: the baked path, or — opt-in,
/// for builds whose target directory moves under them — the bytes themselves.
pub(crate) enum Source {
    Path(String),
    /// The literal is kept as the compiler gave it and only unescaped on a
    /// cache miss, so a hit never re-stringifies a large literal.
    Bytes(proc_macro::Literal),
}

/// The manifests this process has read, by the path they were baked with —
/// **with the hash they were verified against**, so that a cache hit is a
/// `u64` comparison. Asking the manifest for its hash again would
/// re-serialize the whole corpus on every expansion, which is most of what a
/// call site would cost (2,000 of them: 0.84 s against 0.15 s).
type Cache = Mutex<HashMap<String, (u64, Arc<Manifest>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The manifest `source` names, verified against `hash`.
pub(crate) fn load(source: &Source, hash: u64) -> Result<Arc<Manifest>, ManifestError> {
    let key = match source {
        Source::Path(p) => p.clone(),
        Source::Bytes(_) => format!("inline:{hash:016x}"),
    };
    let mut map = cache().lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((verified, m)) = map.get(&key)
        && *verified == hash
    {
        return Ok(Arc::clone(m));
    }
    crate::stats::read();
    let (bytes, path) = read(source)?;
    let m = Manifest::read(&bytes).map_err(|source| ManifestError::Invalid {
        path: path.clone(),
        source,
    })?;
    if m.hash() != hash {
        return Err(ManifestError::Stale {
            path: key,
            found: m.hash(),
            expected: hash,
        });
    }
    let m = Arc::new(m);
    map.insert(key, (hash, Arc::clone(&m)));
    Ok(m)
}

/// The manifest's bytes, and the path they came from (for diagnostics).
fn read(source: &Source) -> Result<(Vec<u8>, PathBuf), ManifestError> {
    match source {
        Source::Path(p) => {
            let path = PathBuf::from(p);
            match std::fs::read(&path) {
                Ok(bytes) => Ok((bytes, path)),
                Err(source) => match relocated(p) {
                    Some(alt) => {
                        let bytes = std::fs::read(&alt).map_err(|source| ManifestError::Read {
                            path: alt.clone(),
                            source,
                        })?;
                        Ok((bytes, alt))
                    }
                    None => Err(ManifestError::Read { path, source }),
                },
            }
        }
        Source::Bytes(literal) => {
            let tokens =
                proc_macro::TokenStream::from(proc_macro::TokenTree::Literal(literal.clone()));
            let lit = syn::parse::<syn::LitByteStr>(tokens)
                .map_err(|e| ManifestError::Literal(e.to_string()))?;
            Ok((lit.value(), PathBuf::from("<inline>")))
        }
    }
}

/// Fallback for a target directory that moved (a CI cache restored
/// elsewhere, a container mounting another path): the i18n crate is fresh,
/// so its build script did not rerun and the baked `OUT_DIR` path is gone.
///
/// Look for the same `build/<pkg>-<hash>/out/manifest.mf2m` under the profile
/// directories of *this* compilation, which rustc — our host process —
/// received as `-L dependency=<target>[/<triple>]/<profile>/deps`. The caller
/// still verifies the hash, so a file found this way can never be the wrong
/// corpus.
fn relocated(baked: &str) -> Option<PathBuf> {
    relocated_in(baked, std::env::args_os())
}

/// [`relocated`], over the arguments given — so that it can be tested
/// without being a rustc.
fn relocated_in(baked: &str, args: impl IntoIterator<Item = OsString>) -> Option<PathBuf> {
    let comps: Vec<Component<'_>> = Path::new(baked).components().collect();
    let build = comps.iter().rposition(|c| c.as_os_str() == "build")?;
    let suffix: PathBuf = comps.get(build..)?.iter().collect();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        let v = if a == "-L" { args.next()? } else { a };
        let v = v.to_string_lossy();
        let v = v.strip_prefix("-L").unwrap_or(&v);
        if let Some(deps) = v.strip_prefix("dependency=")
            && let Some(profile) = Path::new(deps).parent()
        {
            let candidate = profile.join(&suffix);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::relocated_in;
    use std::ffi::OsString;

    /// A target directory restored somewhere else: the baked path is dead,
    /// and the same `build/<pkg>-<hash>/out/manifest.mf2m` is under one of
    /// the `-L dependency=` directories rustc was given.
    #[test]
    fn a_relocated_manifest_is_found_under_the_dependency_paths() {
        let root = std::env::temp_dir().join(format!("mf2-reloc-{}", std::process::id()));
        let out = root
            .join("wasm32-unknown-unknown")
            .join("debug")
            .join("build")
            .join("my-i18n-0123456789abcdef")
            .join("out");
        std::fs::create_dir_all(&out).expect("mkdir");
        let manifest = out.join("manifest.mf2m");
        std::fs::write(&manifest, b"not read by this test").expect("write");

        let baked = concat!(
            "/ci/cache/elsewhere/target/wasm32-unknown-unknown/debug/",
            "build/my-i18n-0123456789abcdef/out/manifest.mf2m"
        );
        let deps = root
            .join("wasm32-unknown-unknown")
            .join("debug")
            .join("deps");
        let args =
            |list: &[&str]| -> Vec<OsString> { list.iter().map(|a| OsString::from(*a)).collect() };

        // As rustc receives them: `-L` and its value as two arguments…
        let found = relocated_in(
            baked,
            args(&[
                "rustc",
                "-L",
                &format!("dependency={}", deps.display()),
                "--edition=2024",
            ]),
        );
        assert_eq!(found.as_deref(), Some(manifest.as_path()));

        // …and as one.
        let found = relocated_in(
            baked,
            args(&["rustc", &format!("-Ldependency={}", deps.display())]),
        );
        assert_eq!(found.as_deref(), Some(manifest.as_path()));

        // Nothing to find: no guess is made.
        assert_eq!(relocated_in(baked, args(&["rustc"])), None);
        assert_eq!(
            relocated_in("/nowhere/manifest.mf2m", args(&["rustc"])),
            None
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
