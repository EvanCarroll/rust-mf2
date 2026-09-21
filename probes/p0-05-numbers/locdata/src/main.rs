//! P0.5 probe (build side): slice `third_party/cldr-json` (48.2.1) into the
//! compact LOCALE entries `numloc` reads. One file per (locale, entry group),
//! so sizes can be reported per group; `<loc>.all-used.bin` concatenates what
//! the suite and the differential test use.

mod error;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use clap::Parser;
use serde_json::Value as J;

use crate::error::Error;

#[derive(Parser)]
#[command(about = "P0.5: CLDR JSON → fn-number LOCALE entries")]
struct Cli {
    /// `third_party/cldr-json`
    #[arg(long)]
    cldr: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, value_delimiter = ',', default_value = "en,es,de,fr,ar,he,ja,hi,ru,pl,cy")]
    locales: Vec<String>,
    /// The "used" currency set (a corpus's literal codes).
    #[arg(long, value_delimiter = ',', default_value = "USD,EUR,JPY,GBP")]
    currencies: Vec<String>,
    /// The "used" unit set.
    #[arg(long, value_delimiter = ',', default_value = "kilometer,kilogram,celsius,hour,megabyte")]
    units: Vec<String>,
}

// Tags: keep in sync with numloc::tag.
const SYMBOLS: u8 = 1;
const SYMBOLS_NATIVE: u8 = 2;
const PERCENT: u8 = 10;
const CUR_STANDARD: u8 = 11;
const CUR_ACCOUNTING: u8 = 12;
const CUR_STANDARD_ALPHA: u8 = 13;
const CUR_ACCOUNTING_ALPHA: u8 = 14;
const CUR_NO_CURRENCY: u8 = 15;
const CUR_UNIT_PATTERNS: u8 = 16;
const CURRENCIES: u8 = 20;
const UNITS: u8 = 30;

/// ISO 4217 minor units ≠ 2 — PLACEHOLDER: `cldr-core/supplemental/
/// currencyData.json` (CLDR's `fractions`) is not vendored (A4 gap, see
/// RESULT.md). Only affects `fractionDigits=auto`; one byte per currency.
const DIGITS: [(&str, u8); 20] = [
    ("BHD", 3), ("BIF", 0), ("CLP", 0), ("DJF", 0), ("GNF", 0), ("IQD", 0), ("ISK", 0), ("JOD", 3),
    ("JPY", 0), ("KMF", 0), ("KRW", 0), ("KWD", 3), ("LYD", 3), ("OMR", 3), ("PYG", 0), ("RWF", 0),
    ("TND", 3), ("UGX", 0), ("VND", 0), ("XAF", 0),
];

fn cat(c: &str) -> Option<u8> {
    Some(match c {
        "zero" => 0,
        "one" => 1,
        "two" => 2,
        "few" => 3,
        "many" => 4,
        "other" => 5,
        _ => return None,
    })
}

#[derive(Default)]
struct W(Vec<u8>);
impl W {
    fn u8(&mut self, x: u8) {
        self.0.push(x);
    }
    fn u16(&mut self, x: u16) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn str(&mut self, s: &str) -> Result<(), Error> {
        let n = u8::try_from(s.len()).map_err(|_| Error::TooLong(s.to_owned()))?;
        self.0.push(n);
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
    fn entry(&mut self, tag: u8, payload: &[u8]) -> Result<(), Error> {
        self.0.push(tag);
        self.u16(u16::try_from(payload.len()).map_err(|_| Error::TooLong(format!("entry {tag}")))?);
        self.0.extend_from_slice(payload);
        Ok(())
    }
}

fn read(p: &Path) -> Result<J, Error> {
    let text = std::fs::read_to_string(p).map_err(|e| Error::Io(p.display().to_string(), e))?;
    Ok(serde_json::from_str(&text)?)
}

fn s<'a>(j: &'a J, k: &str) -> Result<&'a str, Error> {
    j.get(k).and_then(J::as_str).ok_or_else(|| Error::Missing(k.to_owned()))
}

/// Split a CLDR number pattern into (prefix, number part, suffix).
fn split(p: &str) -> (String, String, String) {
    let p = p.replace('\'', "");
    let is_num = |c: char| matches!(c, '#' | '0'..='9' | ',' | '.' | '@');
    let start = p.find(is_num).unwrap_or(p.len());
    let end = p.rfind(is_num).map_or(start, |i| i + 1);
    (p[..start].to_owned(), p[start..end].to_owned(), p[end..].to_owned())
}

/// Grouping sizes of a number part: (primary, secondary); 0 = no grouping.
fn grouping(num: &str) -> (u8, u8) {
    let int = num.split('.').next().unwrap_or("");
    let groups: Vec<&str> = int.split(',').collect();
    if groups.len() < 2 {
        return (0, 0);
    }
    let len = |g: &str| u8::try_from(g.len()).unwrap_or(0);
    let primary = len(groups[groups.len() - 1]);
    let secondary = if groups.len() >= 3 { len(groups[groups.len() - 2]) } else { primary };
    (primary, secondary)
}

fn pattern_entry(p: &str) -> Result<Vec<u8>, Error> {
    let mut parts = p.splitn(2, ';');
    let pos = parts.next().unwrap_or("");
    let (pp, num, ps) = split(pos);
    let (g1, g2) = grouping(&num);
    let mut w = W::default();
    w.u8(g1);
    w.u8(g2);
    w.str(&pp)?;
    w.str(&ps)?;
    match parts.next() {
        Some(neg) => {
            let (np, _, ns) = split(neg);
            w.u8(1);
            w.str(&np)?;
            w.str(&ns)?;
        }
        None => w.u8(0),
    }
    Ok(w.0)
}

/// Zero digit of numbering system `ns` — from ICU4X's compiled data, since
/// `numberingSystems.json` is not vendored.
fn zero_digit(ns: &str) -> Option<char> {
    if ns == "latn" {
        return None;
    }
    let tag = format!("und-u-nu-{ns}");
    let loc = icu_locale_core::Locale::try_from_str(&tag).ok()?;
    let f = icu_decimal::DecimalFormatter::try_new((&loc).into(), Default::default()).ok()?;
    let d = fixed_decimal::Decimal::from(0);
    let text = writeable::Writeable::write_to_string(&f.format(&d)).into_owned();
    text.chars().next().filter(|c| *c != '0')
}

fn symbols_entry(n: &J, ns: &str, min_grouping: u8) -> Result<Vec<u8>, Error> {
    let sym = n.get(format!("symbols-numberSystem-{ns}")).ok_or_else(|| Error::Missing(format!("symbols {ns}")))?;
    let dec = n
        .get(format!("decimalFormats-numberSystem-{ns}"))
        .and_then(|d| d.get("standard"))
        .and_then(J::as_str)
        .ok_or_else(|| Error::Missing(format!("decimalFormats {ns}")))?;
    let (_, num, _) = split(dec);
    let (g1, g2) = grouping(&num);
    let mut w = W::default();
    w.u8(g1);
    w.u8(g2);
    w.u8(min_grouping);
    w.str(&zero_digit(ns).map(String::from).unwrap_or_default())?;
    for k in ["decimal", "group", "minusSign", "plusSign", "percentSign", "perMille", "exponential", "infinity", "nan"] {
        w.str(s(sym, k)?)?;
    }
    Ok(w.0)
}

fn currency_table(cur: &J, codes: &[String]) -> Result<Vec<u8>, Error> {
    let mut w = W::default();
    w.u16(u16::try_from(codes.len()).unwrap_or(0));
    for code in codes {
        let c = cur.get(code).ok_or_else(|| Error::Missing(code.clone()))?;
        w.0.extend_from_slice(code.as_bytes());
        w.u8(DIGITS.iter().find(|(k, _)| k == code).map_or(2, |(_, d)| *d));
        let symbol = c.get("symbol").and_then(J::as_str).unwrap_or(code);
        let narrow = c.get("symbol-alt-narrow").and_then(J::as_str).unwrap_or(symbol);
        let alpha = |t: &str| {
            u8::from(t.chars().next().is_some_and(char::is_alphabetic))
                | u8::from(t.chars().last().is_some_and(char::is_alphabetic)) << 1
        };
        w.u8(alpha(symbol) | alpha(narrow) << 2);
        w.str(symbol)?;
        w.str(narrow)?;
        w.str(c.get("displayName").and_then(J::as_str).unwrap_or(code))?;
        let counts: Vec<(u8, &str)> = c
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((cat(k.strip_prefix("displayName-count-")?)?, v.as_str()?)))
            .collect();
        w.u8(u8::try_from(counts.len()).unwrap_or(0));
        for (k, v) in counts {
            w.u8(k);
            w.str(v)?;
        }
    }
    Ok(w.0)
}

fn unit_table(units: &J, ids: &[String]) -> Result<Vec<u8>, Error> {
    let mut w = W::default();
    w.u16(u16::try_from(ids.len()).unwrap_or(0));
    for id in ids {
        w.str(id)?;
        for width in ["long", "short", "narrow"] {
            let table = units.get(width).and_then(J::as_object).ok_or_else(|| Error::Missing(width.into()))?;
            let u = table
                .iter()
                .find(|(k, _)| k.split_once('-').is_some_and(|(_, rest)| rest == id))
                .map(|(_, v)| v)
                .ok_or_else(|| Error::Missing(format!("unit {id} ({width})")))?;
            let pats: Vec<(u8, &str)> = u
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((cat(k.strip_prefix("unitPattern-count-")?)?, v.as_str()?)))
                .collect();
            w.u8(u8::try_from(pats.len()).unwrap_or(0));
            for (k, v) in pats {
                w.u8(k);
                w.str(v)?;
            }
        }
    }
    Ok(w.0)
}

/// Every simple unit id with a counted pattern in all three widths.
fn all_units(units: &J) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(long) = units.get("long").and_then(J::as_object) {
        for (k, v) in long {
            let Some((_, id)) = k.split_once('-') else { continue };
            let has = |w: &str| {
                units.get(w).and_then(|t| t.get(k)).and_then(|u| u.get("unitPattern-count-other")).is_some()
            };
            if v.get("unitPattern-count-other").is_some() && has("short") && has("narrow") && !ids.iter().any(|x| x == id) {
                ids.push(id.to_owned());
            }
        }
    }
    ids
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out).map_err(|e| Error::Io(cli.out.display().to_string(), e))?;
    for loc in &cli.locales {
        let numbers = read(&cli.cldr.join(format!("cldr-numbers-full/main/{loc}/numbers.json")))?;
        let n = &numbers["main"][loc]["numbers"];
        let ns = s(n, "defaultNumberingSystem")?;
        let min_grouping = s(n, "minimumGroupingDigits")?.parse::<u8>().unwrap_or(1);
        let mut files: BTreeMap<&str, W> = BTreeMap::new();

        let core = files.entry("core").or_default();
        core.entry(SYMBOLS, &symbols_entry(n, ns, min_grouping)?)?;
        if let Some(native) = n.get("otherNumberingSystems").and_then(|o| o.get("native")).and_then(J::as_str) {
            if native != ns && zero_digit(native).is_some() {
                core.entry(SYMBOLS_NATIVE, &symbols_entry(n, native, min_grouping)?)?;
            }
        }
        let pct = s(&n[format!("percentFormats-numberSystem-{ns}")], "standard")?;
        files.entry("percent").or_default().entry(PERCENT, &pattern_entry(pct)?)?;

        let cf = &n[format!("currencyFormats-numberSystem-{ns}")];
        let pats = files.entry("cur-patterns").or_default();
        for (tag, key) in [
            (CUR_STANDARD, "standard"),
            (CUR_ACCOUNTING, "accounting"),
            (CUR_STANDARD_ALPHA, "standard-alphaNextToNumber"),
            (CUR_ACCOUNTING_ALPHA, "accounting-alphaNextToNumber"),
            (CUR_NO_CURRENCY, "standard-noCurrency"),
        ] {
            if let Some(p) = cf.get(key).and_then(J::as_str) {
                pats.entry(tag, &pattern_entry(p)?)?;
            }
        }
        let mut up = W::default();
        let counts: Vec<(u8, &str)> = cf
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((cat(k.strip_prefix("unitPattern-count-")?)?, v.as_str()?)))
            .collect();
        up.u8(u8::try_from(counts.len()).unwrap_or(0));
        for (k, v) in counts {
            up.u8(k);
            up.str(v)?;
        }
        pats.entry(CUR_UNIT_PATTERNS, &up.0)?;

        let currencies = read(&cli.cldr.join(format!("cldr-numbers-full/main/{loc}/currencies.json")))?;
        let cur = &currencies["main"][loc]["numbers"]["currencies"];
        let all_codes: Vec<String> = cur.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        files.entry("cur-used").or_default().entry(CURRENCIES, &currency_table(cur, &cli.currencies)?)?;
        // The full table is larger than one u16-length entry allows only if
        // it exceeds 64 KB; write it as consecutive chunks of 64 codes.
        let all = files.entry("cur-all").or_default();
        for chunk in all_codes.chunks(64) {
            all.entry(CURRENCIES, &currency_table(cur, chunk)?)?;
        }

        let units_json = read(&cli.cldr.join(format!("cldr-units-full/main/{loc}/units.json")))?;
        let units = &units_json["main"][loc]["units"];
        files.entry("unit-used").or_default().entry(UNITS, &unit_table(units, &cli.units)?)?;
        let ids = all_units(units);
        let all = files.entry("unit-all").or_default();
        for chunk in ids.chunks(24) {
            all.entry(UNITS, &unit_table(units, chunk)?)?;
        }

        let mut used = Vec::new();
        for k in ["core", "percent", "cur-patterns", "cur-used", "unit-used"] {
            used.extend_from_slice(&files[k].0);
        }
        for (k, w) in &files {
            let p = cli.out.join(format!("{loc}.{k}.bin"));
            std::fs::write(&p, &w.0).map_err(|e| Error::Io(p.display().to_string(), e))?;
        }
        let p = cli.out.join(format!("{loc}.all-used.bin"));
        std::fs::write(&p, &used).map_err(|e| Error::Io(p.display().to_string(), e))?;
        println!("{loc}: numbering system {ns}, {} currencies, {} units in the full tables", all_codes.len(), ids.len());
    }
    Ok(())
}
