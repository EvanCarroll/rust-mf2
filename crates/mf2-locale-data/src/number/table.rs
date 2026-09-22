//! The number table's text form (`data/numbers.txt`): parsing, CLDR's
//! parent chain, and resolution of a locale's fields. The same code reads
//! the shipped table and, in `extract`, checks a freshly generated one.
//!
//! Line kinds (fields are `key=value`, values escaped with `\u{…}` for
//! spaces, controls, format characters and `\`; see [`escape`]):
//!
//! * `cldr <version>`
//! * `digits <nu> <ten digits>` — every numeric numbering system but `latn`
//! * `likely <lang> <Script>` — a language's likely script; `likely
//!   <lang-REGION> <Script>` — a region whose likely script differs from its
//!   language's and has a `lang-Script` locale (`zh-TW` → `Hant`)
//! * `parent <locale> <parent>` — CLDR's explicit parents (`und` is root)
//! * `locale <tag> [default=<nu>] [native=<nu>] [min-grouping=<n>]` — every
//!   CLDR locale, with the fields that differ from its parent's
//! * `system <tag> <nu> key=value…` — the fields of one of the locale's
//!   numbering systems that differ from its parent's for the same system
//!   (all of them when the parent does not have the system); lines with
//!   nothing to say are omitted

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::error::Error;

/// Locale-level fields, in line order.
pub const LOCALE_FIELDS: &[&str] = &["default", "native", "min-grouping"];

/// Per-numbering-system fields, in line order.
pub const SYSTEM_FIELDS: &[&str] = &[
    "decimal",
    "group",
    "minus",
    "plus",
    "percent",
    "decimal-pattern",
    "percent-pattern",
    "currency",
    "currency-alpha",
    "currency-none",
    "accounting",
    "accounting-alpha",
    "accounting-none",
];

/// The root locale.
pub const ROOT: &str = "und";

/// Whether `c` is written as `\u{…}` in the table: `\`, `=`, whitespace,
/// controls and the format characters CLDR uses (bidi marks, joiners), so
/// that every invisible difference (U+00A0 vs U+202F, U+200E vs U+200F) is
/// visible in review.
fn escaped(c: char) -> bool {
    c == '\\'
        || c == '='
        || c.is_whitespace()
        || c.is_control()
        || matches!(c,
            '\u{ad}' | '\u{600}'..='\u{605}' | '\u{61c}' | '\u{6dd}' | '\u{70f}' | '\u{180e}'
            | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}' | '\u{feff}' | '\u{fff9}'..='\u{fffb}')
}

/// The table form of a value.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if escaped(c) {
            let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// Reverses [`escape`].
pub fn unescape(s: &str) -> Option<Cow<'_, str>> {
    if !s.contains('\\') {
        return Some(Cow::Borrowed(s));
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let tail = rest[at..].strip_prefix("\\u{")?;
        let end = tail.find('}')?;
        let c = char::from_u32(u32::from_str_radix(&tail[..end], 16).ok()?)?;
        out.push(c);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Some(Cow::Owned(out))
}

/// Fields of one line: name → value.
pub type Fields<'t> = BTreeMap<&'t str, Cow<'t, str>>;

/// A parsed table.
#[derive(Default, Debug)]
pub struct Table<'t> {
    pub cldr: &'t str,
    pub digits: BTreeMap<&'t str, Cow<'t, str>>,
    pub likely: BTreeMap<&'t str, &'t str>,
    pub parents: BTreeMap<&'t str, &'t str>,
    pub locales: BTreeMap<&'t str, Fields<'t>>,
    /// Locale → numbering system → fields.
    pub systems: BTreeMap<&'t str, BTreeMap<&'t str, Fields<'t>>>,
}

fn fields<'t>(
    words: impl Iterator<Item = &'t str>,
    known: &[&str],
    line: usize,
) -> Result<Fields<'t>, Error> {
    let mut out = Fields::new();
    for w in words {
        let (k, v) = w.split_once('=').ok_or_else(|| Error::Table {
            line,
            message: format!("expected key=value, found {w:?}"),
        })?;
        if !known.contains(&k) {
            return Err(Error::Table {
                line,
                message: format!("unknown field {k:?}"),
            });
        }
        let v = unescape(v).ok_or_else(|| Error::Table {
            line,
            message: format!("bad escape in {w:?}"),
        })?;
        if out.insert(k, v).is_some() {
            return Err(Error::Table {
                line,
                message: format!("field {k:?} twice"),
            });
        }
    }
    Ok(out)
}

impl<'t> Table<'t> {
    /// Parses the text form.
    pub fn parse(text: &'t str) -> Result<Table<'t>, Error> {
        let mut t = Table::default();
        for (i, line) in text.lines().enumerate() {
            let n = i + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |message: &str| Error::Table {
                line: n,
                message: message.to_owned(),
            };
            let mut words = line.split(' ');
            let kind = words.next().unwrap_or("");
            match kind {
                "cldr" => t.cldr = words.next().ok_or_else(|| bad("cldr version"))?,
                "digits" => {
                    let (Some(nu), Some(d)) = (words.next(), words.next()) else {
                        return Err(bad("expected `digits nu digits`"));
                    };
                    let d = unescape(d).ok_or_else(|| bad("bad escape"))?;
                    if d.chars().count() != 10 {
                        return Err(bad("not ten digits"));
                    }
                    t.digits.insert(nu, d);
                }
                "likely" | "parent" => {
                    let (Some(a), Some(b)) = (words.next(), words.next()) else {
                        return Err(bad("expected two words"));
                    };
                    let map = if kind == "likely" {
                        &mut t.likely
                    } else {
                        &mut t.parents
                    };
                    map.insert(a, b);
                }
                "locale" => {
                    let tag = words.next().ok_or_else(|| bad("locale tag"))?;
                    let f = fields(words, LOCALE_FIELDS, n)?;
                    if t.locales.insert(tag, f).is_some() {
                        return Err(bad("locale twice"));
                    }
                }
                "system" => {
                    let (Some(tag), Some(nu)) = (words.next(), words.next()) else {
                        return Err(bad("expected `system tag nu …`"));
                    };
                    if !t.locales.contains_key(tag) {
                        return Err(bad("system line before its locale line"));
                    }
                    let f = fields(words, SYSTEM_FIELDS, n)?;
                    if t.systems.entry(tag).or_default().insert(nu, f).is_some() {
                        return Err(bad("system twice"));
                    }
                }
                _ => return Err(bad("unknown line kind")),
            }
        }
        if !t.locales.contains_key(ROOT) {
            return Err(Error::Table {
                line: 0,
                message: "no root (`und`) locale".to_owned(),
            });
        }
        Ok(t)
    }

    /// CLDR's parent of `locale` (`None` for root): an explicit parent;
    /// else root for a bare language; else, for `lang-Script`, the language
    /// when `Script` is its likely script and root when not
    /// (`parentLocales.json` `_localeRules`: `nonlikelyScript` → root); else
    /// the tag without its last subtag.
    pub fn parent(&self, locale: &str) -> Option<String> {
        if locale == ROOT {
            return None;
        }
        if let Some(p) = self.parents.get(locale) {
            return Some((*p).to_owned());
        }
        let parts: Vec<&str> = locale.split('-').collect();
        match parts.as_slice() {
            [_] => Some(ROOT.to_owned()),
            [lang, script] if script.len() == 4 => {
                if self.likely.get(lang) == Some(script) {
                    Some((*lang).to_owned())
                } else {
                    Some(ROOT.to_owned())
                }
            }
            _ => locale.rsplit_once('-').map(|(p, _)| p.to_owned()),
        }
    }

    /// `locale` and its ancestors present in the table, root last.
    pub fn chain(&self, locale: &str) -> Vec<&'t str> {
        let mut out = Vec::new();
        let mut cur = Some(locale.to_owned());
        while let Some(c) = cur {
            if let Some((k, _)) = self.locales.get_key_value(c.as_str()) {
                out.push(*k);
            }
            cur = self.parent(&c);
        }
        out
    }

    /// The first locale present in the table on `tag`'s chain: the tag
    /// itself, its truncations and parents, root.
    pub fn lookup(&self, base: &str) -> &'t str {
        self.chain(base).first().copied().unwrap_or(ROOT)
    }

    /// A locale-level field of `locale` (present in the table), resolved
    /// through its parents.
    pub fn locale_field(&self, locale: &str, field: &str) -> Option<&str> {
        self.chain(locale)
            .into_iter()
            .find_map(|l| self.locales.get(l)?.get(field).map(AsRef::as_ref))
    }

    /// The numbering systems `locale` has data for: `latn`, its default and
    /// its native one.
    pub fn systems_of(&self, locale: &str) -> Vec<&str> {
        let mut out = vec!["latn"];
        for f in ["default", "native"] {
            if let Some(nu) = self.locale_field(locale, f)
                && !out.contains(&nu)
            {
                out.push(nu);
            }
        }
        out.sort_unstable();
        out
    }

    /// Whether `locale` has data for the numbering system `nu`.
    pub fn has_system(&self, locale: &str, nu: &str) -> bool {
        nu == "latn"
            || ["default", "native"]
                .into_iter()
                .any(|f| self.locale_field(locale, f) == Some(nu))
    }

    /// A per-system field of `(locale, nu)`, resolved through the parents
    /// that have the system.
    pub fn system_field(&self, locale: &str, nu: &str, field: &str) -> Option<&str> {
        for l in self.chain(locale) {
            if !self.has_system(l, nu) {
                return None;
            }
            if let Some(v) = self
                .systems
                .get(l)
                .and_then(|m| m.get(nu))
                .and_then(|f| f.get(field))
            {
                return Some(v);
            }
        }
        None
    }

    /// Every field of `locale`, resolved: what CLDR's resolved JSON says.
    pub fn resolved(&self, locale: &str) -> Record {
        let mut r = Record::default();
        for f in LOCALE_FIELDS {
            if let Some(v) = self.locale_field(locale, f) {
                r.fields.insert((*f).to_owned(), v.to_owned());
            }
        }
        for nu in self.systems_of(locale) {
            let mut s = BTreeMap::new();
            for f in SYSTEM_FIELDS {
                if let Some(v) = self.system_field(locale, nu, f) {
                    s.insert((*f).to_owned(), v.to_owned());
                }
            }
            r.systems.insert(nu.to_owned(), s);
        }
        r
    }
}

/// One locale's number data, resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    /// [`LOCALE_FIELDS`].
    pub fields: BTreeMap<String, String>,
    /// Numbering system → [`SYSTEM_FIELDS`].
    pub systems: BTreeMap<String, BTreeMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::{Table, escape, unescape};

    #[test]
    fn escapes_round_trip() {
        for s in [
            "¤\u{a0}#,##0.00",
            "\u{200f}-",
            "a\\b",
            "x=y",
            "٫",
            "\u{202f}%",
        ] {
            let e = escape(s);
            assert!(!e.contains(' '));
            assert_eq!(unescape(&e).as_deref(), Some(s));
        }
        assert_eq!(escape("\u{a0}"), "\\u{a0}");
        assert!(unescape("\\u{zz}").is_none());
    }

    #[test]
    fn parents_and_lookup() {
        let text = "cldr 48.2.1\nlikely en Latn\nlikely sr Cyrl\nlikely sr-ME Latn\n\
                    parent en-AU en-001\nparent sr-Latn und\n\
                    locale und default=latn native=latn min-grouping=1\n\
                    system und latn decimal=.\nlocale en\nlocale en-001\nlocale en-AU\n\
                    locale sr\nlocale sr-Latn\n";
        let t = Table::parse(text).expect("parses");
        assert_eq!(t.chain("en-AU"), ["en-AU", "en-001", "en", "und"]);
        assert_eq!(t.lookup("en-Latn-US"), "en");
        assert_eq!(t.lookup("en-Cyrl"), "und");
        assert_eq!(t.lookup("sr-Latn-RS"), "sr-Latn");
        assert_eq!(t.chain("sr-Latn"), ["sr-Latn", "und"]);
        assert_eq!(t.system_field("en-AU", "latn", "decimal"), Some("."));
        assert_eq!(t.system_field("en-AU", "arab", "decimal"), None);
    }
}
