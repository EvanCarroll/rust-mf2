//! Unit data: the shipped all-locale table (`data/units.txt`),
//! resolved through
//! the parent chain of `data/numbers.txt` and CLDR's fallback (a missing
//! plural form is `other`'s), and the `unit.data` LOCALE entry.
//!
//! Table lines (TAB-separated; values escaped as `number::table::escape_tab`):
//! `unit-id <id> <category>` — MF2's unit identifier is CLDR's key without
//! its category (`length-kilometer` → `kilometer`), unique over CLDR 48.2.1's
//! 232 units; `per <locale> <width> pattern=…` — the `per` compound
//! pattern; `unit <locale> <width> <id> field=…` with the fields of
//! [`FIELDS`] a locale stores.
//!
//! **Composition.** A literal `X-per-Y` that CLDR has no unit for is
//! composed at format time from the units `X` and `Y` and the `per`
//! patterns (UTS #35 Part 2 §6.4; ECMA-402 supports the same `X-per-Y` of
//! two simple units): the build then carries `X` and `Y`. Other compounds —
//! `times`, `power2`/`power3`, SI and binary prefixes on arbitrary units —
//! are not composed: they need the unit names inflected for plural, gender
//! and case, which CLDR's data here does not give (`square-kilometer`,
//! `kilowatt-hour` and the other common compounds are CLDR units of their
//! own). `:unit` reports such an identifier as unsupported.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use mf2_catalog::number::TemplatePart;
use mf2_catalog::unit::Width;
use mf2_catalog::writer::unit::{UnitSpec, UnitWidthSpec, UnitsSpec, units};

use crate::blocks::{Block, Blocks, Cache, get};
use crate::error::Error;
use crate::number::{Selection, UnitNeeds};
use crate::template::parse as tpl;

/// Plural category keywords, in code order (0 zero … 5 other).
pub const CATEGORIES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

/// The widths, in entry order (`unitDisplay`).
pub const WIDTHS: [&str; 3] = ["long", "short", "narrow"];

/// The fields of a `unit` line, fallback targets first: the display name,
/// the per-unit pattern, the plural patterns.
pub const FIELDS: &[&str] = &["name", "per", "other", "zero", "one", "two", "few", "many"];

/// CLDR's fallback of a missing unit field: a plural form is `other`'s.
pub fn fallback(field: &str) -> Option<&'static str> {
    match field {
        "zero" | "one" | "two" | "few" | "many" => Some("other"),
        _ => None,
    }
}

/// The shipped table (`cargo xtask locale-data`).
const TABLE: &str = include_str!("../data/units.txt");

struct Loaded {
    blocks: Blocks<'static>,
    cache: Cache,
    /// Identifier → CLDR category.
    ids: BTreeMap<&'static str, &'static str>,
}

fn loaded() -> Result<&'static Loaded, Error> {
    static LOADED: OnceLock<Result<Loaded, String>> = OnceLock::new();
    let r = LOADED.get_or_init(|| {
        let blocks = Blocks::index(TABLE, &["per", "unit"]).map_err(|e| e.to_string())?;
        let mut ids = BTreeMap::new();
        for line in &blocks.globals {
            let mut w = line.split('\t');
            if w.next() != Some("unit-id") {
                continue;
            }
            let (Some(id), Some(category)) = (w.next(), w.next()) else {
                return Err(format!("bad unit-id line {line:?}"));
            };
            ids.insert(id, category);
        }
        Ok(Loaded {
            blocks,
            cache: Cache::default(),
            ids,
        })
    });
    r.as_ref().map_err(|message| Error::Table {
        line: 0,
        message: message.clone(),
    })
}

fn block(l: &Loaded, locale: &str) -> Result<Arc<Block<'static>>, Error> {
    l.cache.get(
        &l.blocks,
        locale,
        |kind| if kind == "unit" { 2 } else { 1 },
        |kind, f| {
            if kind == "per" {
                f == "pattern"
            } else {
                FIELDS.contains(&f)
            }
        },
    )
}

/// Every unit identifier CLDR has (232 at 48.2.1), ascending.
pub fn unit_ids() -> Result<Vec<&'static str>, Error> {
    Ok(loaded()?.ids.keys().copied().collect())
}

/// The CLDR category of unit `id` (`length` for `kilometer`), if CLDR has it.
pub fn unit_category(id: &str) -> Result<Option<&'static str>, Error> {
    Ok(loaded()?.ids.get(id).copied())
}

/// The two CLDR units an `X-per-Y` identifier composes, when CLDR has no
/// unit `id` but has `X` and `Y` (`kilometer-per-second`).
pub fn composition(id: &str) -> Result<Option<(&'static str, &'static str)>, Error> {
    let l = loaded()?;
    if l.ids.contains_key(id) {
        return Ok(None);
    }
    let mut from = 0;
    while let Some(at) = id[from..].find("-per-") {
        let at = from + at;
        let (x, y) = (&id[..at], &id[at + 5..]);
        if let (Some((x, _)), Some((y, _))) = (l.ids.get_key_value(x), l.ids.get_key_value(y)) {
            return Ok(Some((x, y)));
        }
        from = at + 1;
    }
    Ok(None)
}

/// One width of one unit, CLDR's fallbacks applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitWidthData {
    /// `displayName`.
    pub name: Option<String>,
    /// `perUnitPattern`.
    pub per: Option<String>,
    /// `unitPattern-count-*` for each plural category (0 zero … 5 other);
    /// empty when CLDR has no pattern for the unit in this width.
    pub patterns: Vec<String>,
}

/// One unit in one locale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitData {
    /// The identifier.
    pub id: String,
    /// Per width (long, short, narrow).
    pub widths: [UnitWidthData; 3],
}

/// The identifiers `ids` selects, with the parts of any composed `X-per-Y`
/// in place of it; identifiers CLDR does not know are left out.
fn selected(l: &Loaded, ids: &Selection) -> Result<Vec<&'static str>, Error> {
    Ok(match ids {
        Selection::All => l.ids.keys().copied().collect(),
        Selection::Listed(set) => {
            let mut out = std::collections::BTreeSet::new();
            for id in set {
                if let Some((k, _)) = l.ids.get_key_value(id.as_str()) {
                    out.insert(*k);
                } else if let Some((x, y)) = composition(id)? {
                    out.insert(x);
                    out.insert(y);
                }
            }
            out.into_iter().collect()
        }
    })
}

/// The `per` compound patterns (long, short, narrow) and the data of the
/// units `ids` selects, for the CLDR locale chain `chain_names`.
pub(crate) fn unit_data(
    chain_names: &[&str],
    ids: &Selection,
) -> Result<([String; 3], Vec<UnitData>), Error> {
    let l = loaded()?;
    let blocks: Vec<Arc<Block<'static>>> = chain_names
        .iter()
        .map(|n| block(l, n))
        .collect::<Result<_, _>>()?;
    let chain: Vec<&Block<'_>> = blocks.iter().map(AsRef::as_ref).collect();
    let per = WIDTHS.map(|w| {
        get(&chain, &format!("per {w}"), "pattern", &fallback)
            .unwrap_or("{0}/{1}")
            .to_owned()
    });
    let mut out = Vec::new();
    for id in selected(l, ids)? {
        let widths = WIDTHS.map(|w| {
            let key = format!("unit {w} {id}");
            let f = |field: &str| get(&chain, &key, field, &fallback).map(str::to_owned);
            let patterns = if f("other").is_some() {
                CATEGORIES.iter().filter_map(|c| f(c)).collect()
            } else {
                Vec::new()
            };
            UnitWidthData {
                name: f("name"),
                per: f("per"),
                patterns,
            }
        });
        if widths
            .iter()
            .any(|w| !w.patterns.is_empty() || w.name.is_some())
        {
            out.push(UnitData {
                id: id.to_owned(),
                widths,
            });
        }
    }
    Ok((per, out))
}

/// The `unit.data` payload for `per` and `list` (§4.7), carrying what
/// `needs` asks for.
pub(crate) fn unit_entry(
    per: &[String; 3],
    list: &[UnitData],
    needs: &UnitNeeds,
) -> Result<Vec<u8>, Error> {
    let widths: Vec<Width> = Width::ALL
        .into_iter()
        .filter(|w| needs.widths[*w as usize])
        .collect();
    let per_specs: Vec<Vec<TemplatePart<'_>>> = widths
        .iter()
        .map(|w| tpl(&per[*w as usize], 2))
        .collect::<Result<_, _>>()?;
    let mut specs = Vec::with_capacity(list.len());
    for u in list {
        let mut blocks = Vec::with_capacity(widths.len());
        for w in &widths {
            let d = &u.widths[*w as usize];
            let patterns = d
                .patterns
                .iter()
                .enumerate()
                .map(|(c, p)| Ok((u8::try_from(c).unwrap_or(5), tpl(p, 1)?)))
                .collect::<Result<Vec<_>, Error>>()?;
            blocks.push(UnitWidthSpec {
                name: d.name.as_deref(),
                per_unit: d.per.as_deref().map(|p| tpl(p, 1)).transpose()?,
                patterns,
            });
        }
        specs.push(UnitSpec {
            id: &u.id,
            widths: blocks,
        });
    }
    Ok(units(&UnitsSpec {
        widths,
        names: needs.names,
        per: per_specs,
        units: specs,
    })?)
}
