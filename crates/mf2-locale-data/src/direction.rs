//! The text direction of a locale, from CLDR (a catalog header's `dir`,
//! F7): a script subtag decides; otherwise the language's likely script
//! (`likelySubtags`, region-specific first), right-to-left when CLDR's
//! `scriptMetadata` says so. Shipped in `data/directions.txt`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use mf2_model::Dir;

use crate::error::Error;

/// The shipped table (`cargo xtask locale-data`).
const TABLE: &str = include_str!("../data/directions.txt");

struct Table {
    rtl_scripts: BTreeSet<String>,
    /// Languages whose likely script is right-to-left.
    rtl_languages: BTreeSet<String>,
    /// `lang-REGION` whose direction differs from the language's.
    exceptions: BTreeMap<String, Dir>,
}

fn parse(text: &str) -> Result<Table, Error> {
    let mut t = Table {
        rtl_scripts: BTreeSet::new(),
        rtl_languages: BTreeSet::new(),
        exceptions: BTreeMap::new(),
    };
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') || line.starts_with("cldr ") {
            continue;
        }
        let mut words = line.split(' ');
        let kind = words.next().unwrap_or("");
        let rest: Vec<&str> = words.collect();
        match kind {
            "rtl-scripts" => t.rtl_scripts.extend(rest.iter().map(|s| (*s).to_owned())),
            "rtl-languages" => t.rtl_languages.extend(rest.iter().map(|s| (*s).to_owned())),
            "ltr" | "rtl" => {
                let dir = if kind == "rtl" { Dir::Rtl } else { Dir::Ltr };
                for tag in rest {
                    t.exceptions.insert(tag.to_owned(), dir);
                }
            }
            _ => {
                return Err(Error::Table {
                    line: i + 1,
                    message: format!("unknown line kind {kind:?}"),
                });
            }
        }
    }
    Ok(t)
}

fn table() -> Result<&'static Table, Error> {
    static PARSED: OnceLock<Result<Table, String>> = OnceLock::new();
    match PARSED.get_or_init(|| parse(TABLE).map_err(|e| e.to_string())) {
        Ok(t) => Ok(t),
        Err(message) => Err(Error::Table {
            line: 0,
            message: message.clone(),
        }),
    }
}

/// The direction of `locale` (a BCP 47 tag; `_` is read as `-`, case is
/// ignored): `Rtl` or `Ltr`.
pub fn direction(locale: &str) -> Result<Dir, Error> {
    let t = table()?;
    let tag = locale.replace('_', "-");
    let mut subtags = tag.split('-');
    let lang = subtags.next().unwrap_or("").to_ascii_lowercase();
    let mut region = None;
    for s in subtags {
        if s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()) {
            let script: String = s
                .char_indices()
                .map(|(i, c)| {
                    if i == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect();
            return Ok(if t.rtl_scripts.contains(&script) {
                Dir::Rtl
            } else {
                Dir::Ltr
            });
        }
        let is_region = (s.len() == 2 && s.bytes().all(|b| b.is_ascii_alphabetic()))
            || (s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()));
        if region.is_none() && is_region {
            region = Some(s.to_ascii_uppercase());
        }
    }
    if let Some(r) = region
        && let Some(&d) = t.exceptions.get(&format!("{lang}-{r}"))
    {
        return Ok(d);
    }
    Ok(if t.rtl_languages.contains(&lang) {
        Dir::Rtl
    } else {
        Dir::Ltr
    })
}
