//! Currency data (`plans/05-tooling.md` §7, `plans/02-catalog-format.md`
//! §4.6): the shipped all-locale table (`data/currencies.txt`), resolved
//! through the parent chain of `data/numbers.txt` and CLDR's fallbacks, and
//! the `currency.data` LOCALE entry built from it.
//!
//! Table lines (TAB-separated; values escaped as `number::table::escape_tab`):
//! `fraction <CODE|DEFAULT> digits=… [rounding=…]` (CLDR `currencyData`),
//! and `currency <locale> <CODE> field=…` with the fields of [`FIELDS`] a
//! locale stores — those that differ from what its ancestors and
//! [`fallback`] give.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use mf2_catalog::number::{OTHER, TemplatePart};
use mf2_catalog::writer::currency::{CurrenciesSpec, CurrencySpec, currencies};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::blocks::{Block, Blocks, Cache, get, keys};
use crate::error::Error;
use crate::number::{CurrencyNeeds, NumberData, Selection};
use crate::unit::CATEGORIES;

/// The fields of a `currency` line, fallback targets first.
pub const FIELDS: &[&str] = &[
    "symbol",
    "narrow",
    "pattern",
    "decimal",
    "group",
    "name",
    "name-other",
    "name-zero",
    "name-one",
    "name-two",
    "name-few",
    "name-many",
];

/// CLDR's fallback of a missing currency field: a narrow symbol is the
/// symbol; a plural form of the name is `name-other`, which is the name.
/// (A missing symbol or name is the ISO code; the lookup applies it.)
pub fn fallback(field: &str) -> Option<&'static str> {
    match field {
        "narrow" => Some("symbol"),
        "name-other" => Some("name"),
        "name-zero" | "name-one" | "name-two" | "name-few" | "name-many" => Some("name-other"),
        _ => None,
    }
}

/// The shipped table (`cargo xtask locale-data`).
const TABLE: &str = include_str!("../data/currencies.txt");

struct Loaded {
    blocks: Blocks<'static>,
    cache: Cache,
    /// Code → (digits, rounding); `DEFAULT` included.
    fractions: BTreeMap<&'static str, (u8, u32)>,
}

fn loaded() -> Result<&'static Loaded, Error> {
    static LOADED: OnceLock<Result<Loaded, String>> = OnceLock::new();
    let r = LOADED.get_or_init(|| {
        let blocks = Blocks::index(TABLE, &["currency"]).map_err(|e| e.to_string())?;
        let mut fractions = BTreeMap::new();
        for line in &blocks.globals {
            let mut w = line.split('\t');
            if w.next() != Some("fraction") {
                continue;
            }
            let code = w.next().ok_or("fraction without a code")?;
            let (mut digits, mut rounding) = (None, 0u32);
            for f in w {
                match f.split_once('=') {
                    Some(("digits", v)) => digits = v.parse::<u8>().ok(),
                    Some(("rounding", v)) => rounding = v.parse().map_err(|_| "bad rounding")?,
                    _ => return Err(format!("bad fraction field {f:?}")),
                }
            }
            fractions.insert(code, (digits.ok_or("fraction without digits")?, rounding));
        }
        if !fractions.contains_key("DEFAULT") {
            return Err("no DEFAULT fraction".to_owned());
        }
        Ok(Loaded {
            blocks,
            cache: Cache::default(),
            fractions,
        })
    });
    r.as_ref().map_err(|message| Error::Table {
        line: 0,
        message: message.clone(),
    })
}

fn block(l: &Loaded, locale: &str) -> Result<Arc<Block<'static>>, Error> {
    l.cache
        .get(&l.blocks, locale, |_| 1, |_, f| FIELDS.contains(&f))
}

/// One currency's data in one locale, CLDR's fallbacks applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrencyData {
    /// The ISO 4217 code.
    pub code: String,
    /// Fraction digits (`currencyData`; `DEFAULT` for a code it lacks).
    pub digits: u8,
    /// Rounding increment (0 = none).
    pub rounding: u32,
    /// The symbol (the code when CLDR has none).
    pub symbol: String,
    /// The narrow symbol (the symbol when CLDR has none).
    pub narrow: String,
    /// The display name (the code when CLDR has none).
    pub name: String,
    /// The display name for each plural category (0 zero … 5 other), CLDR's
    /// fallbacks applied.
    pub names: [String; 6],
    /// The currency's own standard pattern (`en-DE` EUR `¤#,##0.00`).
    pub pattern: Option<String>,
    /// The currency's own decimal separator (`pt-CV` CVE `$`).
    pub decimal: Option<String>,
    /// The currency's own group separator.
    pub group: Option<String>,
}

/// Every currency code of the table's locales and of `currencyData`,
/// ascending — what `Selection::All` selects.
fn all_codes(l: &Loaded, chain: &[&Block<'_>]) -> Vec<String> {
    let mut out: std::collections::BTreeSet<String> = keys(chain, "currency")
        .into_iter()
        .filter_map(|k| k.strip_prefix("currency ").map(str::to_owned))
        .collect();
    out.extend(
        l.fractions
            .keys()
            .filter(|c| **c != "DEFAULT")
            .map(|c| (*c).to_owned()),
    );
    out.into_iter().collect()
}

/// The data of the currencies `codes` selects, for the CLDR locale chain
/// `chain` (a locale, its ancestors, root). Codes are upper-cased; a
/// malformed one (not three ASCII letters) is refused.
pub(crate) fn currency_data(
    chain_names: &[&str],
    codes: &Selection,
) -> Result<Vec<CurrencyData>, Error> {
    let l = loaded()?;
    let blocks: Vec<Arc<Block<'static>>> = chain_names
        .iter()
        .map(|n| block(l, n))
        .collect::<Result<_, _>>()?;
    let chain: Vec<&Block<'_>> = blocks.iter().map(AsRef::as_ref).collect();
    let list: Vec<String> = match codes {
        Selection::All => all_codes(l, &chain),
        Selection::Listed(set) => {
            let mut v: Vec<String> = set.iter().map(|c| c.to_ascii_uppercase()).collect();
            v.sort();
            v.dedup();
            v
        }
    };
    let default = l.fractions.get("DEFAULT").copied().unwrap_or((2, 0));
    let mut out = Vec::with_capacity(list.len());
    for code in list {
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(Error::Missing {
                locale: chain_names.first().copied().unwrap_or("und").to_owned(),
                system: code,
                field: "a well-formed currency code",
            });
        }
        let key = format!("currency {code}");
        let f = |field: &str| get(&chain, &key, field, &fallback).map(str::to_owned);
        let symbol = f("symbol").unwrap_or_else(|| code.clone());
        let narrow = f("narrow").unwrap_or_else(|| symbol.clone());
        let name = f("name").unwrap_or_else(|| code.clone());
        let names = CATEGORIES.map(|c| f(&format!("name-{c}")).unwrap_or_else(|| name.clone()));
        let (digits, rounding) = l.fractions.get(code.as_str()).copied().unwrap_or(default);
        out.push(CurrencyData {
            digits,
            rounding,
            narrow,
            name,
            names,
            pattern: f("pattern"),
            decimal: f("decimal"),
            group: f("group"),
            symbol,
            code,
        });
    }
    Ok(out)
}

/// CLDR's `fractions/DEFAULT` digits.
pub(crate) fn default_digits() -> Result<u8, Error> {
    Ok(loaded()?.fractions.get("DEFAULT").map_or(2, |d| d.0))
}

/// Whether `c` is in CLDR's `currencyMatch` set `[[:^S:]&[:^Z:]]`.
fn letter_like(c: char) -> bool {
    !matches!(
        c.general_category_group(),
        GeneralCategoryGroup::Symbol | GeneralCategoryGroup::Separator
    )
}

/// The edge bits of `s` (`mf2_catalog::currency::Edges`).
pub fn edges(s: &str) -> u8 {
    let first = s.chars().next().is_some_and(letter_like);
    let last = s.chars().next_back().is_some_and(letter_like);
    u8::from(first) | u8::from(last) << 1
}

/// The `currency.data` payload for `list` (§4.6), carrying what `needs`
/// asks for; the name patterns are `data`'s.
pub(crate) fn currency_entry(
    data: &NumberData,
    list: &[CurrencyData],
    needs: &CurrencyNeeds,
) -> Result<Vec<u8>, Error> {
    let name_patterns: Vec<(u8, Vec<TemplatePart<'_>>)> = if needs.names {
        data.currency_name_patterns
            .iter()
            .map(|(c, p)| Ok((*c, crate::template::parse(p, 2)?)))
            .collect::<Result<_, Error>>()?
    } else {
        Vec::new()
    };
    let parsed: Vec<Option<crate::number::pattern::Parsed>> = list
        .iter()
        .map(|c| {
            c.pattern
                .as_deref()
                .map(crate::number::pattern::parse)
                .transpose()
        })
        .collect::<Result<_, _>>()?;
    let spec = CurrenciesSpec {
        default_digits: default_digits()?,
        narrow: needs.narrow,
        names: needs.names,
        name_patterns,
        currencies: list
            .iter()
            .zip(&parsed)
            .map(|(c, p)| CurrencySpec {
                code: &c.code,
                digits: c.digits,
                rounding: c.rounding,
                symbol: &c.symbol,
                narrow: &c.narrow,
                name: &c.name,
                names: (0u8..=OTHER)
                    .zip(c.names.iter())
                    .map(|(k, s)| (k, s.as_str()))
                    .collect(),
                edges: edges(&c.symbol) | edges(&c.narrow) << 2,
                pattern: p.as_ref().map(crate::number::pattern::Parsed::spec),
                decimal: c.decimal.as_deref(),
                group: c.group.as_deref(),
            })
            .collect(),
    };
    Ok(currencies(&spec)?)
}
