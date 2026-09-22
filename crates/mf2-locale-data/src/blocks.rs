//! Per-locale line blocks of the large tables (`data/currencies.txt`,
//! `data/units.txt`): an index from locale to its contiguous lines, built
//! with one scan, and a parser for one block — so a lookup parses only the
//! locales on its parent chain, not the whole table.
//!
//! A table line is TAB-separated words: `kind <locale> <key…> field=value…`
//! (values escaped as in `number::table::escape_tab`) for the kinds that
//! belong to a locale, or `kind …` for global lines (`cldr`, `fraction`,
//! `unit-id`), which the owner parses itself. A locale's lines are
//! contiguous.
//!
//! A field absent from a locale's line resolves through the parent chain,
//! then through the kind's **fallback** — CLDR's own rule for a missing form
//! (a plural form → `other`; a narrow symbol → the symbol): see [`get`].

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use crate::error::Error;
use crate::number::table::{Fields, unescape};

/// The index of a table.
#[derive(Debug, Default)]
pub struct Blocks<'t> {
    /// The global lines, in order (comments and blank lines dropped).
    pub globals: Vec<&'t str>,
    /// Locale → the text of its lines.
    pub by_locale: BTreeMap<&'t str, &'t str>,
}

/// One locale's lines, parsed: key (the kind and the key words, joined by
/// spaces: `unit short kilometer`) → fields.
pub type Block<'t> = BTreeMap<String, Fields<'t>>;

impl<'t> Blocks<'t> {
    /// Indexes `text`; `locale_kinds` are the kinds whose second word is a
    /// locale. Fails when a locale's lines are not contiguous.
    pub fn index(text: &'t str, locale_kinds: &[&str]) -> Result<Blocks<'t>, Error> {
        let mut b = Blocks::default();
        let mut current: Option<(&'t str, usize, usize)> = None;
        let mut at = 0usize;
        for (n, line) in text.split_inclusive('\n').enumerate() {
            let start = at;
            at += line.len();
            let body = line.trim_end_matches('\n');
            if body.is_empty() || body.starts_with('#') {
                continue;
            }
            let mut words = body.splitn(3, '\t');
            let kind = words.next().unwrap_or("");
            if !locale_kinds.contains(&kind) {
                b.globals.push(body);
                continue;
            }
            let locale = words.next().ok_or_else(|| Error::Table {
                line: n + 1,
                message: "no locale".to_owned(),
            })?;
            match current {
                Some((l, s, _)) if l == locale => current = Some((l, s, at)),
                _ => {
                    if let Some((l, s, e)) = current.take()
                        && b.by_locale.insert(l, &text[s..e]).is_some()
                    {
                        return Err(Error::Table {
                            line: n + 1,
                            message: format!("the lines of {l} are not contiguous"),
                        });
                    }
                    if b.by_locale.contains_key(locale) {
                        return Err(Error::Table {
                            line: n + 1,
                            message: format!("the lines of {locale} are not contiguous"),
                        });
                    }
                    current = Some((locale, start, at));
                }
            }
        }
        if let Some((l, s, e)) = current {
            b.by_locale.insert(l, &text[s..e]);
        }
        Ok(b)
    }

    /// The parsed lines of `locale` (empty when it has none). `key_words`
    /// gives, per kind, how many words after the locale form the key;
    /// `known` the fields a kind may have.
    pub fn block(
        &self,
        locale: &str,
        key_words: impl Fn(&str) -> usize,
        known: impl Fn(&str, &str) -> bool,
    ) -> Result<Block<'t>, Error> {
        let mut out = Block::new();
        let Some(text) = self.by_locale.get(locale) else {
            return Ok(out);
        };
        for line in text.lines() {
            let bad = |message: String| Error::Table { line: 0, message };
            let mut words = line.split('\t');
            let kind = words.next().unwrap_or("");
            let _locale = words.next();
            let mut key = kind.to_owned();
            for _ in 0..key_words(kind) {
                let w = words
                    .next()
                    .ok_or_else(|| bad(format!("{locale}: short line {line:?}")))?;
                key.push(' ');
                key.push_str(w);
            }
            let mut fields = Fields::new();
            for w in words {
                let (k, v) = w
                    .split_once('=')
                    .ok_or_else(|| bad(format!("{locale}: expected key=value, found {w:?}")))?;
                if !known(kind, k) {
                    return Err(bad(format!("{locale}: unknown field {k:?} of {kind}")));
                }
                let v: Cow<'t, str> =
                    unescape(v).ok_or_else(|| bad(format!("{locale}: bad escape in {w:?}")))?;
                fields.insert(k, v);
            }
            if out.insert(key, fields).is_some() {
                return Err(bad(format!("{locale}: a key twice in {line:?}")));
            }
        }
        Ok(out)
    }
}

/// Parsed blocks, cached per locale for the life of the process (a build
/// asks for the same locales many times: every message of a catalog).
#[derive(Debug, Default)]
pub struct Cache {
    blocks: Mutex<HashMap<String, Arc<Block<'static>>>>,
}

impl Cache {
    /// The parsed block of `locale`, from `blocks` on first use.
    pub fn get(
        &self,
        blocks: &Blocks<'static>,
        locale: &str,
        key_words: impl Fn(&str) -> usize,
        known: impl Fn(&str, &str) -> bool,
    ) -> Result<Arc<Block<'static>>, Error> {
        if let Some(b) = self
            .blocks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(locale)
        {
            return Ok(Arc::clone(b));
        }
        let b = Arc::new(blocks.block(locale, key_words, known)?);
        self.blocks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(locale.to_owned(), Arc::clone(&b));
        Ok(b)
    }
}

/// Field `field` of `key` through `chain` (a locale's blocks, the locale
/// first): from the first block that has it; else, when `fallback` names
/// another field, that field's value (recursively); else `None`.
pub fn get<'b>(
    chain: &[&'b Block<'_>],
    key: &str,
    field: &str,
    fallback: &impl Fn(&str) -> Option<&'static str>,
) -> Option<&'b str> {
    for b in chain {
        if let Some(v) = b.get(key).and_then(|f| f.get(field)) {
            return Some(v.as_ref());
        }
    }
    get(chain, key, fallback(field)?, fallback)
}

/// Every field of `key` in `fields` through `chain`, as [`get`] resolves it.
pub fn resolve<'b>(
    chain: &[&'b Block<'_>],
    key: &str,
    fields: &[&'static str],
    fallback: &impl Fn(&str) -> Option<&'static str>,
) -> BTreeMap<&'static str, &'b str> {
    fields
        .iter()
        .filter_map(|f| Some((*f, get(chain, key, f, fallback)?)))
        .collect()
}

/// The keys of `chain` with the prefix `kind ` (a locale's resolved keys).
pub fn keys<'b>(chain: &[&'b Block<'_>], kind: &str) -> std::collections::BTreeSet<&'b str> {
    let prefix = format!("{kind} ");
    chain
        .iter()
        .flat_map(|b| b.keys())
        .filter(|k| k.starts_with(&prefix))
        .map(String::as_str)
        .collect()
}
