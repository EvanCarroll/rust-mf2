//! P0.9 stand-in for `mf2-build` (plans/05-tooling.md §3–4). Called from the
//! i18n crate's `build.rs`:
//!
//! ```ignore
//! fn main() { p09_build::Build::new().source_locale("en").run_or_exit(); }
//! ```
//!
//! It loads `locales/<tag>/*.mf2`, scans every message, derives the manifest
//! from the source locale, writes `manifest.mf2m` and one catalog per locale to
//! `OUT_DIR`, and generates `mf2_generated.rs`: `MANIFEST_HASH`, the locale
//! table, the server-only embedded catalogs, `pub use ::<facade> as __mf2;`
//! and the exported `tr!` wrapper with the manifest's absolute path (or, in
//! `inline` mode, its bytes) baked in. Throwaway probe code.

#![forbid(unsafe_code)]

pub mod error;
pub mod mf2;
pub mod resource;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use p09_catalog::wire::Fnv64;
use p09_catalog::{
    Body, Catalog, CatalogMessage, Manifest, ManifestEntry, Part, Selector, Variant,
};

pub use error::Error;
use mf2::{NBody, NKey, NPart, Operand, Scanned};

/// How the `tr!` wrapper tells the proc-macro where the manifest is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestMode {
    /// The design under test: the absolute path of `OUT_DIR/manifest.mf2m`.
    Path,
    /// The fallback: the manifest bytes as a byte-string literal.
    Inline,
}

/// Build configuration.
#[derive(Debug, Clone)]
pub struct Build {
    source_locale: String,
    facade: String,
    locales_dir: Option<PathBuf>,
    mode: Option<ManifestMode>,
}

impl Default for Build {
    fn default() -> Self {
        Self::new()
    }
}

struct Loaded {
    source: String,
    file: PathBuf,
}

/// What a run produced (also written to `OUT_DIR/mf2-build-report.txt`).
#[derive(Debug, Clone)]
pub struct Report {
    /// `manifest_hash`.
    pub manifest_hash: u64,
    /// Messages in the source locale.
    pub messages: usize,
    /// Catalog file names.
    pub catalogs: Vec<String>,
    /// Wall-clock of the run, µs.
    pub micros: u128,
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool, Error> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    std::fs::write(path, bytes)?;
    Ok(true)
}

fn rtl(tag: &str) -> bool {
    let lang = tag.split('-').next().unwrap_or(tag);
    matches!(lang, "ar" | "he" | "fa" | "ur") || tag.ends_with("-XB")
}

fn byte_literal(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2 + 3);
    s.push_str("b\"");
    for &b in bytes {
        match b {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(char::from(b)),
            _ => {
                let _ = write!(s, "\\x{b:02x}");
            }
        }
    }
    s.push('"');
    s
}

impl Build {
    /// Defaults: source locale `en`, facade `p09_mf2`, `locales/` beside
    /// `Cargo.toml`, mode from `P09_MANIFEST_MODE` (`path` | `inline`).
    pub fn new() -> Self {
        Self {
            source_locale: "en".into(),
            facade: "p09_mf2".into(),
            locales_dir: None,
            mode: None,
        }
    }

    /// Source locale tag.
    #[must_use]
    pub fn source_locale(mut self, tag: &str) -> Self {
        tag.clone_into(&mut self.source_locale);
        self
    }

    /// Crate path re-exported as `__mf2`.
    #[must_use]
    pub fn facade(mut self, path: &str) -> Self {
        path.clone_into(&mut self.facade);
        self
    }

    /// Runs; on error prints it and exits 1 (cargo shows build-script stderr).
    pub fn run_or_exit(self) {
        if let Err(e) = self.run() {
            eprintln!("error: mf2 i18n build failed: {e}");
            std::process::exit(1);
        }
    }

    /// Runs the build (see the crate docs).
    pub fn run(self) -> Result<Report, Error> {
        let t0 = Instant::now();
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
        let out = PathBuf::from(std::env::var("OUT_DIR")?);
        let dir = self
            .locales_dir
            .clone()
            .unwrap_or_else(|| manifest_dir.join("locales"));
        println!("cargo::rerun-if-changed={}", dir.display());
        println!("cargo::rerun-if-env-changed=P09_MANIFEST_MODE");
        let mode =
            self.mode
                .unwrap_or_else(|| match std::env::var("P09_MANIFEST_MODE").as_deref() {
                    Ok("inline") => ManifestMode::Inline,
                    _ => ManifestMode::Path,
                });

        // 1. Load every locale.
        let mut locales: BTreeMap<String, BTreeMap<String, Loaded>> = BTreeMap::new();
        let mut tags: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        tags.sort();
        for tdir in tags {
            let tag = tdir
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    Error::Locale(tdir.display().to_string(), "bad directory name".into())
                })?
                .to_owned();
            let mut files: Vec<PathBuf> = std::fs::read_dir(&tdir)?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "mf2"))
                .collect();
            files.sort();
            let mut msgs = BTreeMap::new();
            for file in files {
                let text = std::fs::read_to_string(&file)?;
                let res = resource::parse(&file, &text)?;
                if res.locale != tag {
                    return Err(Error::Locale(
                        tag,
                        format!("{} declares @locale {}", file.display(), res.locale),
                    ));
                }
                for e in res.entries {
                    if msgs.contains_key(&e.id) {
                        return Err(Error::Resource {
                            file: file.clone(),
                            line: e.line,
                            message: format!("duplicate message id `{}`", e.id),
                        });
                    }
                    msgs.insert(
                        e.id,
                        Loaded {
                            source: e.source,
                            file: file.clone(),
                        },
                    );
                }
            }
            locales.insert(tag, msgs);
        }
        let source = locales.get(&self.source_locale).ok_or_else(|| {
            Error::Locale(
                self.source_locale.clone(),
                "source locale directory missing".into(),
            )
        })?;

        // 2. Scan every message of every locale.
        let mut scanned: BTreeMap<&str, BTreeMap<&str, Scanned>> = BTreeMap::new();
        let mut functions = BTreeSet::new();
        for (tag, msgs) in &locales {
            let mut m = BTreeMap::new();
            for (id, l) in msgs {
                let s = mf2::scan(&l.source).map_err(|message| Error::Message {
                    file: l.file.clone(),
                    id: id.clone(),
                    message,
                })?;
                functions.extend(s.functions.iter().cloned());
                m.insert(id.as_str(), s);
            }
            scanned.insert(tag.as_str(), m);
        }

        // 3. Manifest from the source locale.
        let src_scan = &scanned[self.source_locale.as_str()];
        let entries: Vec<ManifestEntry> = source
            .keys()
            .map(|id| {
                let s = &src_scan[id.as_str()];
                let mut vars: Vec<String> = s.externals.iter().cloned().collect();
                vars.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
                ManifestEntry {
                    id: id.clone(),
                    vars,
                    markup: s.markup.iter().cloned().collect(),
                }
            })
            .collect();
        let manifest = Manifest::new(
            &self.source_locale,
            entries,
            functions.into_iter().collect(),
        );
        let manifest_bytes = manifest.encode();
        let manifest_path = out.join("manifest.mf2m");
        write_if_changed(&manifest_path, &manifest_bytes)?;

        // 4. One catalog per locale, flattened onto the source (D5).
        let mut catalogs = Vec::new();
        for (tag, msgs) in &locales {
            let sc = &scanned[tag.as_str()];
            for id in msgs.keys() {
                if manifest.lookup(id).is_none() {
                    return Err(Error::Translation {
                        locale: tag.clone(),
                        id: id.clone(),
                        message: "id not present in the source locale".into(),
                    });
                }
            }
            let mut messages = Vec::with_capacity(manifest.entries.len());
            for e in &manifest.entries {
                let (s, fallback) = match sc.get(e.id.as_str()) {
                    Some(s) => (s, None),
                    None => (&src_scan[e.id.as_str()], Some(self.source_locale.clone())),
                };
                for v in &s.externals {
                    if !e.vars.contains(v) {
                        return Err(Error::Translation {
                            locale: tag.clone(),
                            id: e.id.clone(),
                            message: format!(
                                "uses variable `${v}`, which the source message does not declare"
                            ),
                        });
                    }
                }
                for m in &s.markup {
                    if !e.markup.contains(m) {
                        return Err(Error::Translation {
                            locale: tag.clone(),
                            id: e.id.clone(),
                            message: format!(
                                "uses markup `{m}`, which the source message does not have"
                            ),
                        });
                    }
                }
                let body = lower(&s.body, &e.vars).map_err(|message| Error::Translation {
                    locale: tag.clone(),
                    id: e.id.clone(),
                    message,
                })?;
                messages.push(Some(CatalogMessage { body, fallback }));
            }
            let cat = Catalog {
                manifest_hash: manifest.hash,
                locale: tag.clone(),
                rtl: rtl(tag),
                messages,
            };
            let bytes = cat.encode();
            let mut h = Fnv64::default();
            h.write(&bytes);
            let content_hash = h.finish();
            let file = format!("{tag}.{content_hash:016x}.mf2b");
            write_if_changed(&out.join(&file), &bytes)?;
            catalogs.push((tag.clone(), file, content_hash, rtl(tag)));
        }
        // Remove catalogs of earlier runs.
        for e in std::fs::read_dir(&out)?.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".mf2b") && !catalogs.iter().any(|c| c.1 == name) {
                std::fs::remove_file(e.path())?;
            }
        }

        // 5. Generated module.
        let code = self.codegen(&manifest, &manifest_path, &manifest_bytes, &catalogs, mode);
        write_if_changed(&out.join("mf2_generated.rs"), code.as_bytes())?;

        let report = Report {
            manifest_hash: manifest.hash,
            messages: manifest.entries.len(),
            catalogs: catalogs.iter().map(|c| c.1.clone()).collect(),
            micros: t0.elapsed().as_micros(),
        };
        let text = format!(
            "manifest_hash {:016x}\nmessages {}\nlocales {}\nmode {:?}\nbuild_rs_micros {}\ncatalogs {}\n",
            report.manifest_hash,
            report.messages,
            catalogs.len(),
            mode,
            report.micros,
            report.catalogs.join(" ")
        );
        std::fs::write(out.join("mf2-build-report.txt"), text)?;
        Ok(report)
    }

    fn codegen(
        &self,
        manifest: &Manifest,
        manifest_path: &Path,
        manifest_bytes: &[u8],
        catalogs: &[(String, String, u64, bool)],
        mode: ManifestMode,
    ) -> String {
        let f = &self.facade;
        let mut s = String::new();
        let _ = writeln!(s, "// @generated by p09-build from locales/ — do not edit.");
        let _ = writeln!(
            s,
            "// {} messages, {} locales, {} functions.\n",
            manifest.entries.len(),
            catalogs.len(),
            manifest.functions.len()
        );
        let _ = writeln!(
            s,
            "/// `manifest_hash` (plans/02-catalog-format.md §3): ids, slots, markup, functions."
        );
        let _ = writeln!(
            s,
            "pub const MANIFEST_HASH: u64 = 0x{:016x};",
            manifest.hash
        );
        let _ = writeln!(s, "/// Source locale.");
        let _ = writeln!(
            s,
            "pub const SOURCE_LOCALE: &str = {:?};",
            self.source_locale
        );
        let _ = writeln!(s, "/// The facade every `tr!` expansion goes through.");
        let _ = writeln!(s, "#[doc(hidden)]\npub use ::{f} as __mf2;\n");
        let _ = writeln!(
            s,
            "/// Locale table: tag and direction (no hashes on the client)."
        );
        let _ = writeln!(s, "pub static LOCALES: &[__mf2::LocaleInfo] = &[");
        for (tag, _, _, rtl) in catalogs {
            let _ = writeln!(s, "    __mf2::LocaleInfo {{ tag: {tag:?}, rtl: {rtl} }},");
        }
        let _ = writeln!(s, "];\n");
        let _ = writeln!(
            s,
            "/// Catalogs embedded for the server only (the client wasm embeds nothing)."
        );
        let _ = writeln!(
            s,
            "#[cfg(feature = \"ssr\")]\npub static CATALOGS: &[__mf2::CatalogFile] = &["
        );
        for (tag, file, hash, _) in catalogs {
            let _ = writeln!(
                s,
                "    __mf2::CatalogFile {{ tag: {tag:?}, file: {file:?}, hash: 0x{hash:016x}, bytes: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{file}\")) }},"
            );
        }
        let _ = writeln!(s, "];\n");
        let _ = writeln!(
            s,
            "/// Installs the embedded catalogs in the facade (server start-up).\n#[cfg(feature = \"ssr\")]\npub fn install() {{\n    __mf2::install(CATALOGS, MANIFEST_HASH, SOURCE_LOCALE);\n}}\n"
        );
        let source = match mode {
            ManifestMode::Path => format!("{:?}", manifest_path.display().to_string()),
            ManifestMode::Inline => format!("bytes {}", byte_literal(manifest_bytes)),
        };
        let _ = writeln!(
            s,
            "/// `tr!(\"id\", name = value, …)` — checked against the manifest at compile time.\n#[macro_export]\nmacro_rules! tr {{\n    ($($t:tt)*) => {{\n        $crate::__tr_impl!({source} 0x{:016x}u64 ; $crate ; $($t)*)\n    }};\n}}\n",
            manifest.hash
        );
        let _ = writeln!(
            s,
            "/// Timing control: same input syntax, no manifest (P0.9 only).\n#[macro_export]\nmacro_rules! tr0 {{\n    ($($t:tt)*) => {{\n        $crate::__tr0_impl!($crate ; $($t)*)\n    }};\n}}"
        );
        s
    }
}

fn lower_parts(parts: &[NPart], vars: &[String]) -> Result<Vec<Part>, String> {
    let slot = |n: &str| -> Result<u32, String> {
        vars.iter()
            .position(|v| v == n)
            .and_then(|i| u32::try_from(i).ok())
            .ok_or_else(|| format!("variable `${n}` has no slot"))
    };
    parts
        .iter()
        .map(|p| {
            Ok(match p {
                NPart::Text(t) => Part::Text(t.clone()),
                NPart::Expr(Operand::Var(n)) => Part::Var(slot(n)?),
                NPart::Expr(Operand::Lit(l)) => Part::Lit(l.clone()),
                NPart::Expr(Operand::None) => Part::Lit(String::new()),
                NPart::Open(n) => Part::MarkupOpen(n.clone()),
                NPart::Close(n) => Part::MarkupClose(n.clone()),
                NPart::Standalone(n) => Part::MarkupStandalone(n.clone()),
            })
        })
        .collect()
}

fn lower(body: &NBody, vars: &[String]) -> Result<Body, String> {
    Ok(match body {
        NBody::Pattern(p) => Body::Pattern(lower_parts(p, vars)?),
        NBody::Select {
            selectors,
            variants,
        } => {
            let selectors = selectors
                .iter()
                .map(|(op, func)| match op {
                    Operand::Var(n) => vars
                        .iter()
                        .position(|v| v == n)
                        .and_then(|i| u32::try_from(i).ok())
                        .map(|slot| Selector { slot, func: *func })
                        .ok_or_else(|| format!("selector `${n}` has no slot")),
                    _ => Err("probe: only variable selectors are supported".into()),
                })
                .collect::<Result<Vec<_>, String>>()?;
            let variants = variants
                .iter()
                .map(|(keys, p)| {
                    Ok(Variant {
                        keys: keys
                            .iter()
                            .map(|k| match k {
                                NKey::Star => p09_catalog::Key::Star,
                                NKey::Lit(l) => p09_catalog::Key::Lit(l.clone()),
                            })
                            .collect(),
                        pattern: lower_parts(p, vars)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Body::Select {
                selectors,
                variants,
            }
        }
    })
}
