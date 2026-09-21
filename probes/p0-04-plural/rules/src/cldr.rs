//! Loads `cldr-core/supplemental/{plurals,ordinals}.json` and parses every
//! locale's rules.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::Error;
use crate::rule::{Rule, category, parse_rule};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Cardinal,
    Ordinal,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Cardinal, Kind::Ordinal];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Cardinal => "cardinal",
            Kind::Ordinal => "ordinal",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Kind::Cardinal => "plurals.json",
            Kind::Ordinal => "ordinals.json",
        }
    }

    fn json_key(self) -> &'static str {
        match self {
            Kind::Cardinal => "plurals-type-cardinal",
            Kind::Ordinal => "plurals-type-ordinal",
        }
    }
}

/// One locale's rules of one kind, in JSON order, with the raw strings.
#[derive(Clone)]
pub struct LocaleRules {
    pub rules: Vec<Rule>,
    pub raw: Vec<(String, String)>,
}

pub struct Cldr {
    pub version: String,
    pub cardinal: BTreeMap<String, LocaleRules>,
    pub ordinal: BTreeMap<String, LocaleRules>,
}

impl Cldr {
    pub fn get(&self, kind: Kind) -> &BTreeMap<String, LocaleRules> {
        match kind {
            Kind::Cardinal => &self.cardinal,
            Kind::Ordinal => &self.ordinal,
        }
    }

    /// Rules for `locale`, falling back by truncating subtags (`pt-PT` → `pt`),
    /// then to "everything is `other`" (CLDR root). Returns the locale used.
    pub fn resolve(&self, kind: Kind, locale: &str) -> Option<(&str, &LocaleRules)> {
        let mut tag = locale;
        loop {
            if let Some((k, v)) = self.get(kind).get_key_value(tag) {
                return Some((k.as_str(), v));
            }
            tag = &tag[..tag.rfind('-')?];
        }
    }
}

/// Default input directory: the vendored CLDR subset (third_party/cldr-json).
pub fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../third_party/cldr-json/cldr-core/supplemental")
}

fn load_kind(dir: &Path, kind: Kind) -> Result<(String, BTreeMap<String, LocaleRules>), Error> {
    let text = std::fs::read_to_string(dir.join(kind.file()))?;
    let json: Value = serde_json::from_str(&text)?;
    let supplemental = json.get("supplemental").ok_or_else(|| Error::Shape("no `supplemental`".into()))?;
    let version = supplemental
        .pointer("/version/_cldrVersion")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_owned();
    let locales = supplemental
        .get(kind.json_key())
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Shape(kind.json_key().into()))?;
    let mut out = BTreeMap::new();
    for (locale, entries) in locales {
        let entries = entries.as_object().ok_or_else(|| Error::Shape(locale.clone()))?;
        let mut rules = Vec::new();
        let mut raw = Vec::new();
        for (key, text) in entries {
            let text = text.as_str().ok_or_else(|| Error::Shape(key.clone()))?;
            let name = key
                .strip_prefix("pluralRule-count-")
                .ok_or_else(|| Error::Shape(key.clone()))?;
            let wrap = |source| Error::Rule { locale: locale.clone(), kind: kind.name(), key: key.clone(), source };
            let rule = category(name).and_then(|c| parse_rule(c, text)).map_err(wrap)?;
            rules.push(rule);
            raw.push((name.to_owned(), text.to_owned()));
        }
        out.insert(locale.clone(), LocaleRules { rules, raw });
    }
    Ok((version, out))
}

/// Loads and parses both files from `dir`.
pub fn load(dir: &Path) -> Result<Cldr, Error> {
    let (version, cardinal) = load_kind(dir, Kind::Cardinal)?;
    let (_, ordinal) = load_kind(dir, Kind::Ordinal)?;
    Ok(Cldr { version, cardinal, ordinal })
}
