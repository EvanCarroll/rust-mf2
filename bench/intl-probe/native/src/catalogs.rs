//! Catalogs for the page: many messages of one locale in one catalog
//! (`mf2_catalog::writer::catalog`, the way `mf2::compile_str` writes one),
//! and a blob of catalogs with a JSON index the page reads.

use std::collections::BTreeSet;
use std::path::Path;

use mf2_catalog::Manifest;
use mf2_catalog::writer::{self, Options};
use mf2_locale_data::{PluralKind, direction, plural_locale_entries};
use mf2_model::{Declaration, Message, Pattern, PatternPart};
use serde_json::{Value, json};

use crate::error::{Error, Result, io};

/// A written catalog: bytes and the manifest hash `Catalog::new` expects.
pub(crate) struct Written {
    pub(crate) bytes: Vec<u8>,
    pub(crate) hash: u64,
    pub(crate) locale: String,
    pub(crate) messages: usize,
}

fn functions_and_markup(m: &Message<'_>, functions: &mut BTreeSet<String>) -> BTreeSet<String> {
    let mut markup = BTreeSet::new();
    for d in m.declarations() {
        let f = match d {
            Declaration::Input(x) => x.value.function.as_ref(),
            Declaration::Local(x) => x.value.function(),
        };
        if let Some(f) = f {
            functions.insert(f.name.to_string());
        }
    }
    let patterns: Vec<&Pattern<'_>> = match m {
        Message::Pattern(p) => vec![&p.pattern],
        Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
    };
    for p in patterns {
        for part in p.parts() {
            match part {
                PatternPart::Expression(e) => {
                    if let Some(f) = e.function() {
                        functions.insert(f.name.to_string());
                    }
                }
                PatternPart::Markup(mk) => {
                    markup.insert(mk.name.to_string());
                }
                _ => {}
            }
        }
    }
    markup
}

/// `sources` as one catalog of `locale`, message `i` = `sources[i]`, with
/// the locale's direction and plural rules (both kinds, CLDR 48.2.1) — what
/// `mf2::compile_str` writes for one message. Function and markup names are
/// taken as written (the probe's corpora are ASCII).
pub(crate) fn multi(sources: &[String], locale: &str) -> Result<Written> {
    let entries = plural_locale_entries(locale, &[PluralKind::Cardinal, PluralKind::Ordinal])?;
    multi_with(sources, locale, entries)
}

/// [`multi`] with the given LOCALE entries.
pub(crate) fn multi_with(
    sources: &[String],
    locale: &str,
    entries: Vec<(u32, Vec<u8>)>,
) -> Result<Written> {
    let mut models = Vec::with_capacity(sources.len());
    for src in sources {
        let parsed = mf2_syntax::parse_model(src);
        match parsed.message {
            Some(m) if parsed.diagnostics.is_empty() => models.push(m),
            _ => {
                return Err(Error::Invalid {
                    src: src.clone(),
                    kinds: parsed
                        .diagnostics
                        .iter()
                        .map(|d| d.kind.suite_name().to_owned())
                        .collect(),
                });
            }
        }
    }
    let mut functions = BTreeSet::new();
    let mut slots = Vec::with_capacity(models.len());
    let mut markup = Vec::with_capacity(models.len());
    for m in &models {
        let analysis = mf2_syntax::analyze(m);
        slots.push(
            analysis
                .externals
                .iter()
                .map(|n| n.nfc.to_string())
                .collect::<Vec<_>>(),
        );
        markup.push(
            functions_and_markup(m, &mut functions)
                .into_iter()
                .collect(),
        );
    }
    let manifest = Manifest {
        ids: (0..models.len()).map(|i| format!("m{i:07}")).collect(),
        slots,
        markup,
        functions: functions.into_iter().collect(),
    };
    let mut options = Options::new(locale, direction(locale)?);
    options.cldr_version = Some(mf2_locale_data::CLDR_VERSION);
    options.locale_entries = entries;
    let refs: Vec<Option<&Message<'_>>> = models.iter().map(Some).collect();
    let bytes = writer::catalog(&manifest, &refs, &options)?;
    // Loads (the page's `Catalog::new` must accept it).
    mf2_catalog::Catalog::new(bytes.clone(), manifest.hash())?;
    Ok(Written {
        bytes,
        hash: manifest.hash(),
        locale: locale.to_owned(),
        messages: models.len(),
    })
}

/// Catalogs in one file, `<name>.bin`, and their index entries.
#[derive(Default)]
pub(crate) struct Blob {
    bytes: Vec<u8>,
    index: Vec<Value>,
}

impl Blob {
    /// Adds a catalog; its index in the blob.
    pub(crate) fn push(&mut self, w: &Written) -> usize {
        self.index.push(json!({
            "off": self.bytes.len(),
            "len": w.bytes.len(),
            "hash": format!("{:016x}", w.hash),
            "locale": w.locale,
            "messages": w.messages,
        }));
        self.bytes.extend_from_slice(&w.bytes);
        self.index.len() - 1
    }

    /// Adds a one-message catalog from `mf2::compile_str` (the conformance
    /// harness's compile).
    pub(crate) fn push_compiled(&mut self, c: &mf2::Compiled) -> usize {
        let w = Written {
            bytes: c.catalog.as_bytes().to_vec(),
            hash: c.manifest.hash(),
            locale: c.catalog.locale().to_owned(),
            messages: 1,
        };
        self.push(&w)
    }

    /// Writes `<dir>/<name>.bin` and `<dir>/<name>.json` = `meta` plus
    /// `"catalogs"` and `"bin"`.
    pub(crate) fn write(self, dir: &Path, name: &str, mut meta: Value) -> Result<()> {
        std::fs::create_dir_all(dir).map_err(io(dir))?;
        let bin = dir.join(format!("{name}.bin"));
        std::fs::write(&bin, &self.bytes).map_err(io(&bin))?;
        if let Value::Object(m) = &mut meta {
            m.insert("bin".into(), json!(format!("{name}.bin")));
            m.insert("catalogs".into(), Value::Array(self.index));
        }
        let path = dir.join(format!("{name}.json"));
        std::fs::write(&path, meta.to_string()).map_err(io(&path))?;
        eprintln!(
            "{}: {} catalogs, {} bytes",
            path.display(),
            meta.get("catalogs")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            self.bytes.len()
        );
        Ok(())
    }
}
