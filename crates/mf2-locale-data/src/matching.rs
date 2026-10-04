//! CLDR's language-matching data, build side:
//! likely subtags (UTS #35 Part 1 §4.3), and the
//! rules, match variables and paradigm locales of language matching (§4.4),
//! shipped as `data/matching.txt` (`cargo xtask locale-data`, from the
//! vendored `likelySubtags.json`, `languageMatching.json` and
//! `territoryContainment.json`).
//!
//! * [`Matching::shipped`] reads the table;
//! * [`Matching::cut`] keeps what a corpus's languages need, which gives them
//!   the answers the whole table gives;
//! * [`Matching::encode`] packs a table as `mf2`'s matcher reads it, and
//!   [`Encoded::rust`] writes that as Rust: the generated module's
//!   `LANGUAGE_MATCHING` (`mf2-build`), and `mf2`'s own whole table.
//!
//! The packing is `mf2`'s (`crates/mf2/src/matching.rs`): a language, script
//! or two-letter region as its letters, 5 bits each (`a` is 1); a
//! three-digit region as 1024 plus its number; a region-level pattern's
//! match variable as `0x8000` and its index, `0x4000` more for `$!`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

use crate::error::Error;

/// The shipped table (`cargo xtask locale-data`).
const TABLE: &str = include_str!("../data/matching.txt");

/// A region pattern's match variable, as `mf2` reads it.
const VARIABLE: u16 = 0x8000;
/// A negated one (`$!name`).
const NEGATED: u16 = 0x4000;

/// CLDR's language-matching data as the data states it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Matching {
    /// The CLDR release it comes from.
    pub cldr: String,
    /// The paradigm locales, as the data lists them.
    pub paradigms: Vec<String>,
    /// The match variables, in the data's order.
    pub variables: Vec<Variable>,
    /// The rules, in the data's order: each level's specific rules, then
    /// its default (`*`, `*-*`, `*-*-*`).
    pub rules: Vec<Rule>,
    /// Likely subtags: a tag with a language (never `und`, which the
    /// matcher never fills) → language-script-region.
    pub likely: BTreeMap<String, String>,
}

/// A match variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    /// Its name, with the `$`.
    pub name: String,
    /// Its value as the data writes it (`019`, `HK+MO`).
    pub value: String,
    /// The regions inside it, ascending: the value's, each macroregion
    /// replaced by its contents, and every macroregion all of whose contents
    /// are among them (the reading that reproduces the specification's
    /// `es-419` examples: 18, C3's text half).
    pub regions: Vec<String>,
}

/// One `languageMatch` rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// The reader's side: one, two or three fields, `*` for any.
    pub desired: String,
    /// The application's side.
    pub supported: String,
    /// The distance it adds.
    pub distance: u8,
    /// Whether it matches only from the reader's side to the application's.
    pub oneway: bool,
}

impl Rule {
    /// 1, 2 or 3: language, script or region.
    fn level(&self) -> usize {
        self.desired.split('-').count()
    }

    fn is_default(&self) -> bool {
        self.desired.split('-').all(|f| f == "*") && self.supported.split('-').all(|f| f == "*")
    }
}

impl Matching {
    /// The shipped table.
    pub fn shipped() -> Result<&'static Matching, Error> {
        static PARSED: OnceLock<Result<Matching, String>> = OnceLock::new();
        match PARSED.get_or_init(|| Matching::parse(TABLE).map_err(|e| e.to_string())) {
            Ok(m) => Ok(m),
            Err(message) => Err(Error::Table {
                line: 0,
                message: message.clone(),
            }),
        }
    }

    /// Reads `data/matching.txt`'s format.
    pub fn parse(text: &str) -> Result<Matching, Error> {
        let mut m = Matching {
            cldr: String::new(),
            paradigms: Vec::new(),
            variables: Vec::new(),
            rules: Vec::new(),
            likely: BTreeMap::new(),
        };
        for (i, line) in text.lines().enumerate() {
            let bad = |message: &str| Error::Table {
                line: i + 1,
                message: message.to_owned(),
            };
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let words: Vec<&str> = line.split(' ').collect();
            match words.as_slice() {
                ["cldr", version] => (*version).clone_into(&mut m.cldr),
                ["paradigms", rest @ ..] => {
                    m.paradigms = rest.iter().map(|s| (*s).to_owned()).collect();
                }
                ["variable", name, value, ":", regions @ ..] => m.variables.push(Variable {
                    name: (*name).to_owned(),
                    value: (*value).to_owned(),
                    regions: regions.iter().map(|s| (*s).to_owned()).collect(),
                }),
                ["rule", desired, supported, distance, rest @ ..] => {
                    let distance = distance.parse().map_err(|_| bad("a distance"))?;
                    let oneway = match rest {
                        [] => false,
                        ["oneway"] => true,
                        _ => return Err(bad("`oneway` or nothing after the distance")),
                    };
                    m.rules.push(Rule {
                        desired: (*desired).to_owned(),
                        supported: (*supported).to_owned(),
                        distance,
                        oneway,
                    });
                }
                ["likely", from, to] => {
                    m.likely.insert((*from).to_owned(), (*to).to_owned());
                }
                _ => return Err(bad("unknown line")),
            }
        }
        Ok(m)
    }

    /// The table's text: what [`Matching::parse`] reads and `cargo xtask
    /// locale-data` writes.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "# mf2-locale-data: CLDR's language-matching data: likely subtags (UTS #35 Part 1 §4.3),\n\
             # and the rules, match variables and paradigm locales of language matching (§4.4).\n",
        );
        out.push_str(
            "# Generated by `cargo xtask locale-data` from third_party/cldr-json. Do not edit.\n",
        );
        out.push_str(
            "# `variable NAME VALUE : REGIONS`: the regions inside the variable, macroregions\n\
             # replaced by their contents, and each macroregion all of whose contents are inside.\n\
             # `rule DESIRED SUPPORTED DISTANCE [oneway]`, in the data's order. `likely FROM TO`,\n\
             # without `und`, which the matcher never fills.\n",
        );
        let _ = writeln!(out, "cldr {}", self.cldr);
        let _ = writeln!(out, "paradigms {}", self.paradigms.join(" "));
        for v in &self.variables {
            let _ = writeln!(
                out,
                "variable {} {} : {}",
                v.name,
                v.value,
                v.regions.join(" ")
            );
        }
        for r in &self.rules {
            let _ = writeln!(
                out,
                "rule {} {} {}{}",
                r.desired,
                r.supported,
                r.distance,
                if r.oneway { " oneway" } else { "" }
            );
        }
        for (from, to) in &self.likely {
            let _ = writeln!(out, "likely {from} {to}");
        }
        out
    }

    /// What a corpus of `locales` needs, and nothing more: for any reader's
    /// tags, matching against those locales (or any of them) gives what the
    /// whole table gives.
    ///
    /// * the rules whose application's side can be one of the corpus's
    ///   languages (a two-way rule's either side), and the defaults;
    /// * the likely subtags of the corpus's languages, and of every language
    ///   a kept language-level rule accepts for one of them: of any other
    ///   language, a reader is farther than the default language distance
    ///   from every locale, which no demotion or tie can bring back;
    /// * the variables the kept rules name, and the paradigm locales of the
    ///   corpus's languages.
    #[must_use]
    pub fn cut(&self, locales: &[&str]) -> Matching {
        let own: BTreeSet<String> = locales
            .iter()
            .map(|t| language_of(t).to_ascii_lowercase())
            .collect();
        let applies = |field: &str| field == "*" || own.contains(field);
        let mut readers = own.clone();
        let mut rules = Vec::new();
        for rule in &self.rules {
            let d = language_of(&rule.desired);
            let s = language_of(&rule.supported);
            let forward = applies(s);
            let backward = !rule.oneway && applies(d);
            if !(forward || backward) {
                continue;
            }
            if rule.level() == 1 {
                if forward && d != "*" {
                    readers.insert(d.to_owned());
                }
                if backward && s != "*" {
                    readers.insert(s.to_owned());
                }
            }
            rules.push(rule.clone());
        }
        let named: BTreeSet<&str> = rules
            .iter()
            .flat_map(|r| [r.desired.as_str(), r.supported.as_str()])
            .filter_map(|p| p.split('-').nth(2))
            .filter_map(|region| region.strip_prefix('$'))
            .map(|name| name.strip_prefix('!').unwrap_or(name))
            .collect();
        Matching {
            cldr: self.cldr.clone(),
            paradigms: self
                .paradigms
                .iter()
                .filter(|p| own.contains(language_of(p)))
                .cloned()
                .collect(),
            variables: self
                .variables
                .iter()
                .filter(|v| named.contains(v.name.trim_start_matches('$')))
                .cloned()
                .collect(),
            rules,
            likely: self
                .likely
                .iter()
                .filter(|(from, _)| readers.contains(language_of(from)))
                .map(|(from, to)| (from.clone(), to.clone()))
                .collect(),
        }
    }

    /// The table packed as `mf2`'s matcher reads it.
    pub fn encode(&self) -> Result<Encoded, Error> {
        let bad = |message: String| Error::Table { line: 0, message };
        let mut scripts = BTreeSet::new();
        let mut regions = BTreeSet::new();
        let mut filled = Vec::with_capacity(self.likely.len());
        for (from, to) in &self.likely {
            let key = split(from).ok_or_else(|| bad(format!("likely key {from:?}")))?;
            let [_, script, region] = split(to)
                .filter(|[_, s, r]| !s.is_empty() && !r.is_empty())
                .ok_or_else(|| bad(format!("likely value {to:?}")))?;
            let script = pack_script(script).ok_or_else(|| bad(format!("script of {to:?}")))?;
            let region = pack_region(region).ok_or_else(|| bad(format!("region of {to:?}")))?;
            scripts.insert(script);
            regions.insert(region);
            filled.push((key, script, region));
        }
        let scripts: Vec<u32> = scripts.into_iter().collect();
        let regions: Vec<u16> = regions.into_iter().collect();
        let index = |list: &[u32], v: u32| {
            list.binary_search(&v)
                .ok()
                .and_then(|i| u8::try_from(i).ok())
        };
        let region_index = |v: u16| {
            regions
                .binary_search(&v)
                .ok()
                .and_then(|i| u8::try_from(i).ok())
        };
        let mut languages = Vec::new();
        let mut tags = Vec::new();
        for ([language, script, region], s, r) in filled {
            let si = index(&scripts, s).ok_or_else(|| bad("more than 256 scripts".to_owned()))?;
            let ri = region_index(r).ok_or_else(|| bad("more than 256 regions".to_owned()))?;
            let l = pack_language(language)
                .ok_or_else(|| bad(format!("likely language {language:?}")))?;
            if script.is_empty() && region.is_empty() {
                languages.push((l, si, ri));
            } else {
                let ks = if script.is_empty() {
                    Some(0)
                } else {
                    pack_script(script)
                };
                let kr = if region.is_empty() {
                    Some(0)
                } else {
                    pack_region(region)
                };
                let (Some(ks), Some(kr)) = (ks, kr) else {
                    return Err(bad(format!("likely key {language}-{script}-{region}")));
                };
                tags.push((l, ks, kr, si, ri));
            }
        }
        languages.sort_unstable();
        tags.sort_unstable_by_key(|&(l, ks, kr, _, _)| tag_key(l, ks, kr));

        let mut defaults = [None::<u8>; 3];
        let mut language_pairs = BTreeMap::new();
        let mut script_pairs = BTreeMap::new();
        let mut region_rules = Vec::new();
        // The variables the region rules name, as indices into
        // `self.variables`, in the order first named.
        let mut variables: Vec<usize> = Vec::new();
        for rule in &self.rules {
            let level = rule.level();
            let default = defaults
                .get_mut(level - 1)
                .ok_or_else(|| bad(format!("rule {} has {level} fields", rule.desired)))?;
            if default.is_some() {
                return Err(bad(format!(
                    "rule {} {} after its level's default",
                    rule.desired, rule.supported
                )));
            }
            if rule.is_default() {
                *default = Some(rule.distance);
                continue;
            }
            let pattern = |p: &str, variables: &mut Vec<usize>| -> Result<(u16, u32, u16), Error> {
                let fields: Vec<&str> = p.split('-').collect();
                let field = |i: usize| fields.get(i).copied().unwrap_or("*");
                let language = match field(0) {
                    "*" => 0,
                    l => pack_language(l).ok_or_else(|| bad(format!("rule language {l:?}")))?,
                };
                let script = match field(1) {
                    "*" => 0,
                    s => pack_script(s).ok_or_else(|| bad(format!("rule script {s:?}")))?,
                };
                let region = match field(2) {
                    "*" => 0,
                    r if r.starts_with('$') => {
                        let (negated, name) = match r.strip_prefix("$!") {
                            Some(name) => (true, name),
                            None => (false, r.trim_start_matches('$')),
                        };
                        let index = self
                            .variables
                            .iter()
                            .position(|v| v.name.trim_start_matches('$') == name)
                            .ok_or_else(|| bad(format!("no variable {name}")))?;
                        let at = if let Some(at) = variables.iter().position(|&i| i == index) {
                            at
                        } else {
                            variables.push(index);
                            variables.len() - 1
                        };
                        let at = u16::try_from(at)
                            .ok()
                            .filter(|at| *at < NEGATED)
                            .ok_or_else(|| bad("too many variables".to_owned()))?;
                        VARIABLE | if negated { NEGATED } else { 0 } | at
                    }
                    r => pack_region(r).ok_or_else(|| bad(format!("rule region {r:?}")))?,
                };
                Ok((language, script, region))
            };
            let d = pattern(&rule.desired, &mut variables)?;
            let s = pattern(&rule.supported, &mut variables)?;
            match level {
                // The first rule in the data's order that matches an ordered
                // pair decides it; a two-way rule gives both orders. Only the
                // default has a `*` at these levels (checked below).
                1 | 2 if d.0 == 0 || s.0 == 0 || (level == 2 && (d.1 == 0 || s.1 == 0)) => {
                    return Err(bad(format!(
                        "rule {} {}: a `*` outside the default",
                        rule.desired, rule.supported
                    )));
                }
                1 => {
                    language_pairs.entry((d.0, s.0)).or_insert(rule.distance);
                    if !rule.oneway {
                        language_pairs.entry((s.0, d.0)).or_insert(rule.distance);
                    }
                }
                2 => {
                    script_pairs
                        .entry((d.0, d.1, s.0, s.1))
                        .or_insert(rule.distance);
                    if !rule.oneway {
                        script_pairs
                            .entry((s.0, s.1, d.0, d.1))
                            .or_insert(rule.distance);
                    }
                }
                _ => region_rules.push((d.0, d.1, d.2, s.0, s.1, s.2, rule.distance, rule.oneway)),
            }
        }
        let [Some(language), Some(script), Some(region)] = defaults else {
            return Err(bad("a level without its default rule".to_owned()));
        };
        let mut paradigms = Vec::new();
        for p in &self.paradigms {
            let [l, s, r] = self
                .maximize(p)
                .ok_or_else(|| bad(format!("paradigm {p:?}")))?;
            let packed = (pack_language(&l), pack_script(&s), pack_region(&r));
            let (Some(l), Some(s), Some(r)) = packed else {
                return Err(bad(format!("paradigm {p:?}")));
            };
            paradigms.push((l, s, r));
        }
        let variables = variables
            .into_iter()
            .filter_map(|i| self.variables.get(i))
            .map(|v| {
                let mut packed = v
                    .regions
                    .iter()
                    .map(|r| {
                        pack_region(r).ok_or_else(|| bad(format!("region {r:?} of {}", v.name)))
                    })
                    .collect::<Result<Vec<u16>, Error>>()?;
                packed.sort_unstable();
                Ok(packed)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Encoded {
            scripts,
            regions,
            languages: languages.iter().map(|e| e.0).collect(),
            language_values: languages.iter().map(|e| (e.1, e.2)).collect(),
            tags: tags
                .iter()
                .map(|&(l, ks, kr, _, _)| tag_key(l, ks, kr))
                .collect(),
            tag_values: tags.iter().map(|e| (e.3, e.4)).collect(),
            language_pairs: language_pairs
                .keys()
                .map(|&(d, s)| (u32::from(d) << 16) | u32::from(s))
                .collect(),
            language_distances: language_pairs.values().copied().collect(),
            script_pairs: script_pairs
                .into_iter()
                .map(|((dl, ds, sl, ss), n)| (dl, ds, sl, ss, n))
                .collect(),
            region_rules,
            variables,
            paradigms,
            defaults: [language, script, region],
        })
    }

    /// `crates/mf2/src/matching/cldr.rs`: the whole table as `mf2` carries
    /// it on a server and in a native application (`cargo xtask
    /// locale-data` writes it; `tests/table.rs` checks the committed copy).
    pub fn mf2_module(&self) -> Result<String, Error> {
        let table = self.encode()?.rust("super::LanguageMatching");
        Ok(format!(
            "// @generated by `cargo xtask locale-data` from third_party/cldr-json (CLDR {cldr}): the\n\
             // whole of CLDR's language-matching data, packed as `super::LanguageMatching` reads it\n\
             // (mf2-locale-data's `data/matching.txt`, `Matching::encode`). Do not edit.\n\
             \n\
             /// CLDR's whole table: what a server and a native application match against.\n\
             pub(super) static CLDR: super::LanguageMatching = {table};\n",
            cldr = self.cldr
        ))
    }

    /// §4.3's Add Likely Subtags on the table's own strings: language,
    /// script and region, the empty ones filled by the first of
    /// language-script, language-region and language the table has.
    fn maximize(&self, tag: &str) -> Option<[String; 3]> {
        let [language, script, region] = split(tag)?;
        if !script.is_empty() && !region.is_empty() {
            return Some([language.to_owned(), script.to_owned(), region.to_owned()]);
        }
        let keys = [
            (!script.is_empty()).then(|| format!("{language}-{script}")),
            (!region.is_empty()).then(|| format!("{language}-{region}")),
            Some(language.to_owned()),
        ];
        let found = keys
            .into_iter()
            .flatten()
            .find_map(|k| self.likely.get(&k))?;
        let [_, s, r] = split(found)?;
        Some([
            language.to_owned(),
            (if script.is_empty() { s } else { script }).to_owned(),
            (if region.is_empty() { r } else { region }).to_owned(),
        ])
    }
}

/// A table packed as `mf2`'s `LanguageMatching` holds it: each field is its
/// namesake there, in the same order.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::type_complexity)]
pub struct Encoded {
    /// Script codes, by index.
    pub scripts: Vec<u32>,
    /// Region codes, by index.
    pub regions: Vec<u16>,
    /// The bare languages with likely subtags, ascending…
    pub languages: Vec<u16>,
    /// …and the script and region each fills (indices).
    pub language_values: Vec<(u8, u8)>,
    /// A language with a script or a region ([`tag_key`]), ascending…
    pub tags: Vec<u64>,
    /// …and the script and region each fills (indices).
    pub tag_values: Vec<(u8, u8)>,
    /// The language level's ordered pairs, the reader's language above the
    /// application's, ascending…
    pub language_pairs: Vec<u32>,
    /// …and their distances.
    pub language_distances: Vec<u8>,
    /// The script level's ordered pairs.
    pub script_pairs: Vec<(u16, u32, u16, u32, u8)>,
    /// The region level's rules, in the data's order, without the default.
    pub region_rules: Vec<(u16, u32, u16, u16, u32, u16, u8, bool)>,
    /// Each variable the region rules name, its regions ascending.
    pub variables: Vec<Vec<u16>>,
    /// The paradigm locales, filled in.
    pub paradigms: Vec<(u16, u32, u16)>,
    /// The three levels' defaults.
    pub defaults: [u8; 3],
}

/// An integer as a Rust literal, its digits grouped by three from five
/// digits on (`394_894`), so that clippy's pedantic lints stay quiet in the
/// crate that includes the table.
struct Int(u64);

impl core::fmt::Display for Int {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let digits = self.0.to_string();
        if digits.len() < 5 {
            return f.write_str(&digits);
        }
        for (i, d) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                f.write_str("_")?;
            }
            write!(f, "{d}")?;
        }
        Ok(())
    }
}

impl Encoded {
    /// A Rust expression of the table: `{ty}::new(…)`, where `ty` is the
    /// path of `mf2`'s `LanguageMatching` (`__mf2::LanguageMatching` in the
    /// generated module).
    #[must_use]
    pub fn rust(&self, ty: &str) -> String {
        // Ten numbers or pairs to a line, a tuple of more to a line of its
        // own.
        fn list<T>(out: &mut String, items: &[T], item: impl Fn(&mut String, &T)) {
            if items.is_empty() {
                out.push_str("    &[],\n");
                return;
            }
            let per_line = if core::mem::size_of::<T>() <= 8 {
                10
            } else {
                1
            };
            out.push_str("    &[\n");
            for line in items.chunks(per_line) {
                out.push_str("       ");
                for i in line {
                    out.push(' ');
                    item(out, i);
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("    ],\n");
        }
        let mut out = String::with_capacity(64 + 24 * self.languages.len());
        let _ = writeln!(out, "{ty}::new(");
        let n = |v: u32| Int(u64::from(v));
        list(&mut out, &self.scripts, |o, s| {
            let _ = write!(o, "{}", n(*s));
        });
        list(&mut out, &self.regions, |o, r| {
            let _ = write!(o, "{}", n((*r).into()));
        });
        let pair = |o: &mut String, (s, r): &(u8, u8)| {
            let _ = write!(o, "({s}, {r})");
        };
        list(&mut out, &self.languages, |o, l| {
            let _ = write!(o, "{}", n((*l).into()));
        });
        list(&mut out, &self.language_values, pair);
        list(&mut out, &self.tags, |o, key| {
            let _ = write!(o, "{}", Int(*key));
        });
        list(&mut out, &self.tag_values, pair);
        list(&mut out, &self.language_pairs, |o, key| {
            let _ = write!(o, "{}", n(*key));
        });
        list(&mut out, &self.language_distances, |o, d| {
            let _ = write!(o, "{d}");
        });
        list(&mut out, &self.script_pairs, |o, (dl, ds, sl, ss, dist)| {
            let _ = write!(
                o,
                "({}, {}, {}, {}, {dist})",
                n((*dl).into()),
                n(*ds),
                n((*sl).into()),
                n(*ss)
            );
        });
        list(
            &mut out,
            &self.region_rules,
            |o, (dl, ds, dr, sl, ss, sr, dist, one)| {
                let _ = write!(
                    o,
                    "({}, {}, {}, {}, {}, {}, {dist}, {one})",
                    n((*dl).into()),
                    n(*ds),
                    n((*dr).into()),
                    n((*sl).into()),
                    n(*ss),
                    n((*sr).into())
                );
            },
        );
        list(&mut out, &self.variables, |o, v| {
            o.push_str("&[");
            for (i, r) in v.iter().enumerate() {
                if i > 0 {
                    o.push_str(", ");
                }
                let _ = write!(o, "{}", n((*r).into()));
            }
            o.push(']');
        });
        list(&mut out, &self.paradigms, |o, (l, s, r)| {
            let _ = write!(o, "({}, {}, {})", n((*l).into()), n(*s), n((*r).into()));
        });
        let [l, s, r] = self.defaults;
        let _ = write!(out, "    [{l}, {s}, {r}],\n)");
        out
    }
}

/// A language with a script or a region, as `mf2`'s `tags` key it: the
/// language above the script above the region.
pub fn tag_key(language: u16, script: u32, region: u16) -> u64 {
    (u64::from(language) << 36) | (u64::from(script) << 16) | u64::from(region)
}

/// The language field of a tag or a rule's side.
fn language_of(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or("")
}

/// A tag of the table as language, script and region, each `""` where it
/// has none.
fn split(tag: &str) -> Option<[&str; 3]> {
    let mut fields = tag.split('-');
    let language = fields.next().filter(|l| !l.is_empty())?;
    let mut next = fields.next();
    let mut script = "";
    if let Some(s) = next.filter(|s| s.len() == 4) {
        script = s;
        next = fields.next();
    }
    let mut region = "";
    if let Some(r) = next {
        region = r;
        next = fields.next();
    }
    next.is_none().then_some([language, script, region])
}

/// Letters packed 5 bits each (`a` is 1), in any case.
fn pack(letters: &str) -> Option<u64> {
    letters.bytes().try_fold(0, |packed, b| {
        b.is_ascii_alphabetic()
            .then(|| (packed << 5) | u64::from(b.to_ascii_lowercase() - b'a' + 1))
    })
}

/// A language of two or three letters, as `mf2`'s tables hold one.
pub fn pack_language(language: &str) -> Option<u16> {
    matches!(language.len(), 2 | 3)
        .then(|| pack(language))
        .flatten()
        .and_then(|p| u16::try_from(p).ok())
}

/// A script of four letters.
pub fn pack_script(script: &str) -> Option<u32> {
    (script.len() == 4)
        .then(|| pack(script))
        .flatten()
        .and_then(|p| u32::try_from(p).ok())
}

/// A region: two letters, or three digits as 1024 plus their number.
pub fn pack_region(region: &str) -> Option<u16> {
    match region.len() {
        2 => pack(region).and_then(|p| u16::try_from(p).ok()),
        3 => region
            .parse::<u16>()
            .ok()
            .filter(|_| region.bytes().all(|b| b.is_ascii_digit()))
            .map(|n| 1024 + n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Int, Matching, TABLE};

    #[test]
    fn the_table_reads_back_as_written() {
        let m = Matching::parse(TABLE).expect("the shipped table parses");
        assert_eq!(m.text(), TABLE);
        assert_eq!(m.cldr, "48.2.1");
        assert_eq!(
            m.likely.get("zh-TW").map(String::as_str),
            Some("zh-Hant-TW")
        );
        assert!(!m.likely.keys().any(|k| k == "und" || k.starts_with("und-")));
    }

    #[test]
    fn a_cut_keeps_what_serves_the_corpus() {
        let m = Matching::shipped().expect("the shipped table");
        let cut = m.cut(&["ar", "en", "fr"]);
        let has = |d: &str, s: &str| cut.rules.iter().any(|r| r.desired == d && r.supported == s);
        // Readers of Acholi accept English, of Breton French: kept, with
        // those languages' likely subtags.
        assert!(has("ach", "en") && has("br", "fr"));
        assert!(cut.likely.contains_key("ach") && cut.likely.contains_key("br"));
        // Readers of Abkhazian accept Russian, which the corpus lacks.
        assert!(!has("ab", "ru") && !cut.likely.contains_key("ab"));
        // Each level's default, English's and Arabic's region rules and
        // their variables; Spanish's `$americas` not.
        assert!(has("*", "*") && has("*-*", "*-*") && has("*-*-*", "*-*-*"));
        let names: Vec<&str> = cut.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["$enUS", "$maghreb"]);
        assert_eq!(cut.paradigms, ["en", "en-GB"]);
        assert!(cut.encode().is_ok());
    }

    #[test]
    fn a_variable_holds_its_macroregions_when_all_their_contents_are_inside() {
        let m = Matching::shipped().expect("the shipped table");
        let americas = m
            .variables
            .iter()
            .find(|v| v.name == "$americas")
            .expect("$americas");
        for inside in ["019", "419", "005", "013", "021", "029", "MX", "US"] {
            assert!(americas.regions.iter().any(|r| r == inside), "{inside}");
        }
        for outside in ["001", "150", "ES", "EU"] {
            assert!(!americas.regions.iter().any(|r| r == outside), "{outside}");
        }
    }

    #[test]
    fn integers_are_grouped_from_five_digits() {
        assert_eq!(Int(0).to_string(), "0");
        assert_eq!(Int(1234).to_string(), "1234");
        assert_eq!(Int(12_345).to_string(), "12_345");
        assert_eq!(Int(394_894).to_string(), "394_894");
        assert_eq!(Int(1_048_575).to_string(), "1_048_575");
    }
}
