//! The one locale matcher:
//! which of an application's languages best serves a reader,
//! by CLDR's language-matching data and the algorithm of UTS #35 Part 1
//! §4.3–§4.4. Everything that matches a language goes through it: the
//! native store (`install`, `set_locale`, `with_locale`, `Catalogs`), the
//! web server's negotiation, the client-only boot, and the generated
//! `Locale` (`from_str`, `best_match`).
//!
//! **The data** is a [`LanguageMatching`]: all of CLDR's, which a server
//! carries, or the part one corpus needs, which `mf2-build` cuts and the
//! generated module holds as `LANGUAGE_MATCHING` — what a native
//! application's `CORPUS` and a client-only application's setup carry. A cut
//! gives the corpus's locales exactly the answers the whole table gives.
//!
//! **The rules**, as the specification's text states them and 19 §9 fixes
//! what it leaves to the implementation:
//! * each tag is read as a language, a script and a region (a POSIX name
//!   too: `fr_CA.UTF-8` is `fr-CA`), and filled in by likely subtags
//!   (`zh-TW` is `zh-Hant-TW`) — but a reader's `und`, which stands for no
//!   language, is left as it is; a tag the data cannot fill keeps its
//!   empty fields;
//! * the distance of two tags sums a distance per field that differs, the
//!   first rule of the data that matches that field's level, in the data's
//!   order (a `oneway` rule only from the reader's side);
//! * each later entry of the reader's list is demoted by 5, counted against
//!   the threshold: the best pair is served only when its weighted distance
//!   is below 50, so an exact match can come from at most the tenth entry;
//! * ties go to the earlier pair, but among one entry's candidates a
//!   paradigm locale wins; a macroregion is inside a match variable when
//!   all its contents are (the table holds each variable's regions so).
//!
//! No project rule sits on top of the data (question 15): Traditional and
//! Simplified Chinese are not served for each other, since CLDR has no rule
//! between them.
//!
//! Client-path code: no `core::fmt`, no panicking operation, no allocation.
//! It is written for size, since a client-only application's boot carries
//! it: one body for every caller, tags read byte by byte, and the table's
//! keys packed into integers.

use mf2_catalog::Dir;

// CLDR's whole table, for the builds that carry it: a server's.
#[cfg(feature = "host-std")]
#[rustfmt::skip]
mod cldr;

/// A match only below this weighted distance (19 §9): the top of the range
/// the specification's text allows, above the default region distance and
/// below the default script distance.
const THRESHOLD: u32 = 50;

/// The demotion of each later entry of a reader's list (19 §9): a little
/// more than the default region distance, as the text suggests, so that a
/// regional variant of a reader's first language beats an exact second.
const DEMOTION: u32 = 5;

/// A region pattern of a region-level rule: a region, `*` (0), or a match
/// variable (this bit, and the variable's index below it).
const VARIABLE: u16 = 0x8000;
/// With [`VARIABLE`]: every region *not* in the variable (`$!name`).
const NEGATED: u16 = 0x4000;

/// An ordered pair of the script level: (the reader's language and script,
/// the application's, the distance).
type ScriptPair = (u16, u32, u16, u32, u8);
/// A rule of the region level: the reader's pattern (language, script,
/// region; `0` is `*`), the application's, the distance, and whether it
/// goes one way only.
type RegionRule = (u16, u32, u16, u16, u32, u16, u8, bool);

/// CLDR's language-matching data — likely subtags, the rules of the three
/// levels and their match variables, the paradigm locales — in the form the
/// matcher reads: all of it (a server's), or the part one corpus needs,
/// which the build cuts and the generated module holds as
/// `LANGUAGE_MATCHING` (what a native application's corpus and a
/// client-only application's setup carry).
///
/// A build fact, like [`Corpus`](crate::Corpus): generated, never written
/// by hand.
#[derive(Clone, Copy)]
pub struct LanguageMatching {
    /// The script codes the likely subtags name by index, packed.
    scripts: &'static [u32],
    /// The region codes the likely subtags name by index, packed.
    regions: &'static [u16],
    /// The bare languages with likely subtags, packed, ascending…
    languages: &'static [u16],
    /// …and the script and region each fills (indices), in that order.
    language_values: &'static [(u8, u8)],
    /// A language with a script or a region, as [`tag_key`] packs it,
    /// ascending…
    tags: &'static [u64],
    /// …and the script and region each fills (indices), in that order.
    tag_values: &'static [(u8, u8)],
    /// The language level: each ordered pair a rule names, the reader's
    /// language above the application's, ascending — the first rule in the
    /// data's order that matches it decides it (a two-way rule gives both
    /// orders)…
    language_pairs: &'static [u32],
    /// …and its distance, in that order.
    language_distances: &'static [u8],
    /// The script level, likewise.
    script_pairs: &'static [ScriptPair],
    /// The region level, in the data's order.
    region_rules: &'static [RegionRule],
    /// Each match variable's regions, ascending: its regions and every
    /// macroregion all of whose contents are among them.
    variables: &'static [&'static [u16]],
    /// The paradigm locales, filled in: (language, script, region).
    paradigms: &'static [(u16, u32, u16)],
    /// The distance of each level's final rule, `*`: language, script,
    /// region.
    defaults: [u8; 3],
}

/// Its size, not its hundreds of entries.
impl core::fmt::Debug for LanguageMatching {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LanguageMatching")
            .field("languages", &self.languages.len())
            .field("tags", &self.tags.len())
            .field("language_pairs", &self.language_pairs.len())
            .field("script_pairs", &self.script_pairs.len())
            .field("region_rules", &self.region_rules.len())
            .field("variables", &self.variables.len())
            .field("paradigms", &self.paradigms.len())
            .finish_non_exhaustive()
    }
}

/// A tag as the matcher reads it: its language, script and region, each
/// packed ([`pack`], [`region`]); 0 where it has none.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Lsr {
    language: u64,
    script: u32,
    region: u16,
}

/// `und`, packed.
const UND: u64 = packed(b"und", 0);
/// `Zzzz` and `ZZ`, packed: no script and no region (§4.3).
const ZZZZ: u64 = packed(b"zzzz", 0);
const ZZ: u64 = packed(b"zz", 0);
/// `POSIX`, packed: a POSIX locale name, no language (neither is `C`, which
/// is one letter).
const POSIX: u64 = packed(b"posix", 0);

/// [`pack`] for the constants: lowercase letters, after `out`'s.
#[allow(clippy::cast_lossless, reason = "`u64::from` is not `const`")]
const fn packed(letters: &[u8], out: u64) -> u64 {
    match letters {
        [b, rest @ ..] => packed(rest, (out << 5) | (*b - b'a' + 1) as u64),
        [] => out,
    }
}

/// A language with a script or a region, as [`LanguageMatching`]'s `tags`
/// key it: the language above the script above the region.
const fn tag_key(language: u16, script: u32, region: u16) -> u64 {
    ((language as u64) << 36) | ((script as u64) << 16) | region as u64
}

impl LanguageMatching {
    /// No data: likely subtags fill nothing and every field that differs
    /// adds its level's default (CLDR 48's 80, 50 and 4). What a client
    /// matches with when its setup carries no table.
    #[doc(hidden)]
    pub const EMPTY: LanguageMatching = LanguageMatching {
        scripts: &[],
        regions: &[],
        languages: &[],
        language_values: &[],
        tags: &[],
        tag_values: &[],
        language_pairs: &[],
        language_distances: &[],
        script_pairs: &[],
        region_rules: &[],
        variables: &[],
        paradigms: &[],
        defaults: [80, 50, 4],
    };

    /// What the generated module and `mf2`'s own table call; the
    /// arguments are the fields, in their order.
    #[doc(hidden)]
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        scripts: &'static [u32],
        regions: &'static [u16],
        languages: &'static [u16],
        language_values: &'static [(u8, u8)],
        tags: &'static [u64],
        tag_values: &'static [(u8, u8)],
        language_pairs: &'static [u32],
        language_distances: &'static [u8],
        script_pairs: &'static [ScriptPair],
        region_rules: &'static [RegionRule],
        variables: &'static [&'static [u16]],
        paradigms: &'static [(u16, u32, u16)],
        defaults: [u8; 3],
    ) -> LanguageMatching {
        LanguageMatching {
            scripts,
            regions,
            languages,
            language_values,
            tags,
            tag_values,
            language_pairs,
            language_distances,
            script_pairs,
            region_rules,
            variables,
            paradigms,
            defaults,
        }
    }

    /// The index in `supported` of the language that best serves a reader
    /// of `desired`, their languages in order of preference; `None` when
    /// none is close enough (the application's source language is then
    /// the answer).
    #[doc(hidden)]
    pub fn best_match<'d>(
        &self,
        desired: impl IntoIterator<Item = &'d str>,
        supported: &[(&str, Dir)],
    ) -> Option<usize> {
        self.best(&mut desired.into_iter(), supported)
    }

    /// [`best_match`](LanguageMatching::best_match), compiled once whatever
    /// the caller's list.
    fn best(
        &self,
        desired: &mut dyn Iterator<Item = &str>,
        supported: &[(&str, Dir)],
    ) -> Option<usize> {
        let mut best = None;
        let mut best_distance = THRESHOLD;
        let mut best_entry = 0;
        let mut best_paradigm = false;
        let mut entry = 0;
        for tag in desired {
            let demotion = entry * DEMOTION;
            if demotion >= THRESHOLD {
                break;
            }
            // A value that is no tag (`*`, `C`, an empty one) takes no
            // place in the list.
            let Some(reader) = self.reader(tag) else {
                continue;
            };
            entry += 1;
            for (i, (tag, _)) in supported.iter().enumerate() {
                let Some(own) = parse(tag) else {
                    continue;
                };
                let own = self.maximize(own);
                let weighted = demotion + self.distance(reader, own);
                let paradigm = weighted <= best_distance && self.is_paradigm(own);
                if weighted < best_distance
                    || (weighted == best_distance
                        && best_entry == entry
                        && paradigm
                        && !best_paradigm)
                {
                    best = Some(i);
                    best_distance = weighted;
                    best_entry = entry;
                    best_paradigm = paradigm;
                }
            }
        }
        best
    }

    /// A reader's tag, filled in unless its language is `und`.
    fn reader(&self, tag: &str) -> Option<Lsr> {
        let reader = parse(tag)?;
        Some(if reader.language == UND {
            reader
        } else {
            self.maximize(reader)
        })
    }

    /// The matching distance of a reader's `desired` and an application's
    /// `supported`, each filled in by likely subtags (a reader's `und`
    /// excepted); `None` when either is no tag. For tests and tools.
    #[doc(hidden)]
    #[must_use]
    pub fn distance_of(&self, desired: &str, supported: &str) -> Option<u32> {
        let reader = self.reader(desired)?;
        Some(self.distance(reader, self.maximize(parse(supported)?)))
    }

    /// §4.3's Add Likely Subtags: a tag with a script and a region is taken
    /// as it is; else the first of language-script, language-region and
    /// language the data has fills the fields the tag lacks.
    fn maximize(&self, tag: Lsr) -> Lsr {
        let Ok(language) = u16::try_from(tag.language) else {
            return tag;
        };
        let found = if tag.script != 0 && tag.region != 0 {
            None
        } else {
            [(tag.script, 0), (0, tag.region)]
                .into_iter()
                .filter(|&(s, r)| s != 0 || r != 0)
                .find_map(|(s, r)| lookup(self.tags, self.tag_values, tag_key(language, s, r)))
                .or_else(|| lookup(self.languages, self.language_values, language))
        };
        match found {
            Some((script, region)) => Lsr {
                language: tag.language,
                script: if tag.script == 0 {
                    self.scripts.get(usize::from(script)).copied().unwrap_or(0)
                } else {
                    tag.script
                },
                region: if tag.region == 0 {
                    self.regions.get(usize::from(region)).copied().unwrap_or(0)
                } else {
                    tag.region
                },
            },
            None => tag,
        }
    }

    /// §4.4's matching distance of two filled-in tags: for each of
    /// language, script and region that differs, the distance of the first
    /// rule of that level that matches.
    fn distance(&self, reader: Lsr, own: Lsr) -> u32 {
        let [language, script, region] = self.defaults.map(u32::from);
        let pair = u16::try_from(reader.language)
            .ok()
            .zip(u16::try_from(own.language).ok());
        let mut total = 0;
        if reader.language != own.language {
            total += pair
                .and_then(|(d, s)| {
                    let key = (u32::from(d) << 16) | u32::from(s);
                    lookup(self.language_pairs, self.language_distances, key)
                })
                .map_or(language, u32::from);
        }
        if reader.script != own.script {
            total += pair
                .and_then(|(d, s)| {
                    self.script_pairs
                        .iter()
                        .find(|p| (p.0, p.1, p.2, p.3) == (d, reader.script, s, own.script))
                })
                .map_or(script, |p| u32::from(p.4));
        }
        if reader.region != own.region {
            total += self
                .region_rules
                .iter()
                .find(|&&(dl, ds, dr, sl, ss, sr, _, oneway)| {
                    let (d, s) = ((dl, ds, dr), (sl, ss, sr));
                    (self.fits(d, reader) && self.fits(s, own))
                        || (!oneway && self.fits(d, own) && self.fits(s, reader))
                })
                .map_or(region, |rule| u32::from(rule.6));
        }
        total
    }

    /// Whether a rule's pattern (language, script, region; `0` is `*`)
    /// matches a tag.
    fn fits(&self, (language, script, region): (u16, u32, u16), tag: Lsr) -> bool {
        (language == 0 || u64::from(language) == tag.language)
            && (script == 0 || script == tag.script)
            && (region == 0
                || if region & VARIABLE == 0 {
                    region == tag.region
                } else {
                    let inside = self
                        .variables
                        .get(usize::from(region & !(VARIABLE | NEGATED)))
                        .is_some_and(|regions| regions.binary_search(&tag.region).is_ok());
                    inside != (region & NEGATED != 0)
                })
    }

    /// Whether a filled-in tag is one of the paradigm locales.
    fn is_paradigm(&self, tag: Lsr) -> bool {
        self.paradigms
            .iter()
            .any(|&(l, s, r)| u64::from(l) == tag.language && s == tag.script && r == tag.region)
    }
}

/// The value of `key` in two parallel arrays, the keys ascending.
fn lookup<K: Ord + Copy, V: Copy>(keys: &[K], values: &[V], key: K) -> Option<V> {
    let at = keys.binary_search(&key).ok()?;
    values.get(at).copied()
}

#[cfg(feature = "host-std")]
impl LanguageMatching {
    /// CLDR's whole table: what a server matches with, whatever tags it is
    /// given.
    #[doc(hidden)]
    #[must_use]
    pub fn cldr() -> &'static LanguageMatching {
        &cldr::CLDR
    }
}

/// A tag as language, script and region: `-` or `_` between subtags, any
/// case, a POSIX `.codeset` or `@modifier` left out, an extended language
/// subtag skipped, and what follows the region (variants, extensions,
/// private use) ignored. `None` for what is no tag: an empty value, `*`,
/// `C`, `POSIX`, or a language that is not two, three or five to eight
/// letters.
fn parse(tag: &str) -> Option<Lsr> {
    let bytes = tag.as_bytes();
    let end = bytes
        .iter()
        .position(|&b| b == b'.' || b == b'@')
        .unwrap_or(bytes.len());
    let mut subtags = bytes.get(..end)?.split(|&b| b == b'-' || b == b'_');
    let first = subtags.next()?;
    let language = pack(first).filter(|&l| l != POSIX && matches!(first.len(), 2 | 3 | 5..=8))?;
    let mut next = subtags.next();
    // Extended language subtags (`zh-yue`): up to three, skipped.
    let mut extlangs = 0;
    while let Some(s) = next
        && extlangs < 3
        && s.len() == 3
        && pack(s).is_some()
    {
        extlangs += 1;
        next = subtags.next();
    }
    let mut script = 0;
    if let Some(s) = next
        && s.len() == 4
        && let Some(packed) = pack(s)
    {
        script = packed;
        next = subtags.next();
    }
    let region = next.and_then(region).map_or(0, u64::from);
    // §4.3: the unknown script and region are no script and no region.
    Some(Lsr {
        language,
        script: if script == ZZZZ {
            0
        } else {
            u32::try_from(script).ok()?
        },
        region: if region == ZZ {
            0
        } else {
            u16::try_from(region).ok()?
        },
    })
}

/// ASCII letters, 5 bits each (`a` is 1), in any case; `None` for anything
/// else. Up to twelve letters fit.
fn pack(letters: &[u8]) -> Option<u64> {
    letters.iter().try_fold(0, |packed, &b| {
        b.is_ascii_alphabetic()
            .then(|| (packed << 5) | u64::from(b.to_ascii_lowercase() - b'a' + 1))
    })
}

/// A region: two letters, packed as [`pack`] does, or three digits, as
/// 1024 plus their number.
fn region(subtag: &[u8]) -> Option<u16> {
    match subtag.len() {
        2 => pack(subtag).and_then(|p| u16::try_from(p).ok()),
        3 => subtag.iter().try_fold(1024, |n: u16, &b| {
            b.is_ascii_digit()
                .then(|| 1024 + (n - 1024) * 10 + u16::from(b - b'0'))
        }),
        _ => None,
    }
}
