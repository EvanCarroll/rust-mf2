//! The encoder (feature `writer`, build side): [`catalog`] writes one
//! locale's `.mf2b` from a manifest and its messages; [`single`] compiles
//! one message into a one-message catalog with its own one-entry manifest —
//! what conformance L3/L4, `mf2::compile_str` and the CLI use.
//!
//! Output is deterministic (F8): it depends only on the inputs. The byte
//! format and the writer policies are `plans/02-catalog-format.md` §2.

pub mod currency;
mod encode;
pub mod nfc_map;
pub mod number;
mod pool;
pub mod unit;

use alloc::borrow::Cow;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use mf2_model::{Declaration, Dir, Message, Pattern, PatternPart};
use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};

use self::encode::{MsgEncoder, count, varint};
use self::pool::{Class, Pool};
use crate::error::WriteError;
use crate::format::{
    HEADER_LEN, IDS_RESTART, MAGIC, MAX_FALLBACK_LOCALES, MAX_MESSAGES, MAX_OFFSET,
    SECTION_ENTRY_LEN, VERSION, flags, kind, locale_key, section,
};
use crate::manifest::Manifest;
use crate::plural;
use crate::reader::CldrVersion;

/// What a catalog carries besides its messages.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Options {
    /// The BCP 47 tag (header, F7).
    pub locale: String,
    /// `Ltr` or `Rtl` (header, F7); `Auto` is refused.
    pub dir: Dir,
    /// Omit COLD (production client catalogs, §2.3).
    pub strip_cold: bool,
    /// Omit IDS (production client catalogs, §2.3).
    pub strip_ids: bool,
    /// The `MsgId` chunk (0 until chunking).
    pub chunk: u8,
    /// The CLDR version of `locale_entries`, if any.
    pub cldr_version: Option<CldrVersion>,
    /// LOCALE entries: `(key, payload)` (§2.7, §4), in any order.
    pub locale_entries: Vec<(u32, Vec<u8>)>,
    /// Messages whose text came from a fallback locale (F7): `(message
    /// index, locale tag)`, in any order.
    pub fallback: Vec<(u32, String)>,
}

impl Options {
    /// An unstripped catalog for `locale`, with no locale data and no
    /// fallbacks.
    pub fn new(locale: impl Into<String>, dir: Dir) -> Options {
        Options {
            locale: locale.into(),
            dir,
            strip_cold: false,
            strip_ids: false,
            chunk: 0,
            cldr_version: None,
            locale_entries: Vec::new(),
            fallback: Vec::new(),
        }
    }

    /// The same, stripped of COLD and IDS (production).
    #[must_use]
    pub fn stripped(mut self) -> Options {
        self.strip_cold = true;
        self.strip_ids = true;
        self
    }
}

/// `s` in Unicode Normalization Form C (borrowed when it already is).
pub(crate) fn nfc(s: &str) -> Cow<'_, str> {
    if is_nfc_quick(s.chars()) == IsNormalized::Yes {
        return Cow::Borrowed(s);
    }
    let n: String = s.nfc().collect();
    if n == s {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(n)
    }
}

/// Writes one locale's catalog. `messages[i]` is the message with `MsgId`
/// index `i` (the manifest's `ids[i]`), `None` when this locale lacks it.
pub fn catalog(
    manifest: &Manifest,
    messages: &[Option<&Message<'_>>],
    options: &Options,
) -> Result<Vec<u8>, WriteError> {
    manifest.validate().map_err(|e| match e {
        crate::ManifestError::Invalid(what) => WriteError::Manifest(what),
        _ => WriteError::Manifest("invalid"),
    })?;
    if messages.len() != manifest.ids.len() {
        return Err(WriteError::Manifest("one message slot per id"));
    }
    let n = count(messages.len(), "messages")?;
    if n > MAX_MESSAGES {
        return Err(WriteError::TooLarge("messages"));
    }
    let dir = match options.dir {
        Dir::Ltr => 0u8,
        Dir::Rtl => 1,
        Dir::Auto => return Err(WriteError::Dir),
    };
    let locale = locale_section(&options.locale_entries)?;
    let fallback = fallback_list(&options.fallback, n)?;
    let ids = if options.strip_ids {
        None
    } else {
        Some(ids_section(&manifest.ids)?)
    };

    let layout = names_layout(manifest, messages)?;
    let mut pool = Pool::new();
    // Every variant key and argument name, in NFC, as the one pass sees them
    // (`plan/01` §8 F3): the NFC section is built from them below.
    let mut keys = BTreeSet::new();
    encode_all(
        manifest, messages, options, &fallback, &layout, &mut pool, &mut keys,
    )?;
    pool.finish()?;
    let s = encode_all(
        manifest, messages, options, &fallback, &layout, &mut pool, &mut keys,
    )?;
    let nfc_section = nfc_map::build(keys.iter().map(String::as_str));

    let mut sections: Vec<(u16, Vec<u8>)> = Vec::with_capacity(10);
    sections.push((section::INDEX, planes(&s.index)));
    sections.push((section::MESSAGES, s.messages));
    if !options.strip_cold && !s.cold.is_empty() {
        sections.push((section::COLD, s.cold));
    }
    sections.push((section::NAMES, s.names));
    if let Some(f) = s.fallback {
        sections.push((section::FALLBACK, f));
    }
    sections.push((section::LOCALE, locale));
    sections.push((section::FUNCS, s.funcs));
    if let Some(ids) = ids {
        sections.push((section::IDS, ids));
    }
    if !nfc_section.is_empty() {
        sections.push((section::NFC, nfc_section));
    }
    sections.push((section::STRINGS, pool.into_bytes()));

    let mut fl = 0u16;
    if options.strip_cold {
        fl |= flags::COLD_STRIPPED;
    }
    if options.strip_ids {
        fl |= flags::IDS_STRIPPED;
    }
    let table_len = HEADER_LEN + sections.len() * SECTION_ENTRY_LEN;
    let total = table_len + sections.iter().map(|s| s.1.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&fl.to_le_bytes());
    out.extend_from_slice(&manifest.hash().to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&s.locale.to_le_bytes());
    out.extend_from_slice(
        &options
            .cldr_version
            .map_or(0, CldrVersion::to_u32)
            .to_le_bytes(),
    );
    out.push(options.chunk);
    out.push(dir);
    let nsec = u16::try_from(sections.len()).map_err(|_| WriteError::TooLarge("sections"))?;
    out.extend_from_slice(&nsec.to_le_bytes());
    let mut off = table_len;
    for (k, data) in &sections {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&count(off, "catalog")?.to_le_bytes());
        out.extend_from_slice(&count(data.len(), "section")?.to_le_bytes());
        off += data.len();
    }
    for (_, data) in sections {
        out.extend_from_slice(&data);
    }
    count(out.len(), "catalog")?;
    Ok(out)
}

/// Compiles one message into a one-message catalog (id `""`) and returns it
/// with its manifest. `slots` are the message's external variables in slot
/// order — `mf2_syntax::analyze(message).externals`' NFC names — so this
/// crate needs no parser. Markup names and functions are taken from the
/// message (NFC, ascending, unique).
pub fn single(
    message: &Message<'_>,
    slots: &[&str],
    options: &Options,
) -> Result<(Vec<u8>, Manifest), WriteError> {
    let mut markup = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for d in message.declarations() {
        let f = match d {
            Declaration::Input(x) => x.value.function.as_ref(),
            Declaration::Local(x) => x.value.function(),
            // The encoder refuses it; nothing to collect.
            _ => None,
        };
        if let Some(f) = f {
            functions.insert(nfc(&f.name).into_owned());
        }
    }
    let patterns: Vec<&Pattern<'_>> = match message {
        Message::Pattern(p) => alloc::vec![&p.pattern],
        Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
        _ => Vec::new(),
    };
    for p in patterns {
        for part in p.parts() {
            match part {
                PatternPart::Expression(e) => {
                    if let Some(f) = e.function() {
                        functions.insert(nfc(&f.name).into_owned());
                    }
                }
                PatternPart::Markup(m) => {
                    markup.insert(nfc(&m.name).into_owned());
                }
                _ => {}
            }
        }
    }
    let manifest = Manifest {
        ids: alloc::vec![String::new()],
        slots: alloc::vec![slots.iter().map(|s| String::from(*s)).collect()],
        markup: alloc::vec![markup.into_iter().collect()],
        functions: functions.into_iter().collect(),
    };
    let bytes = catalog(&manifest, &[Some(message)], options)?;
    Ok((bytes, manifest))
}

/// The pool-dependent sections, as one pass produces them.
struct Structure {
    locale: u32,
    index: Vec<u32>,
    messages: Vec<u8>,
    cold: Vec<u8>,
    names: Vec<u8>,
    funcs: Vec<u8>,
    fallback: Option<Vec<u8>>,
}

/// One pass over everything that refers to the pool.
fn encode_all(
    manifest: &Manifest,
    messages: &[Option<&Message<'_>>],
    options: &Options,
    fallback: &[(u32, &str)],
    layout: &NamesLayout<'_>,
    pool: &mut Pool,
    keys: &mut BTreeSet<String>,
) -> Result<Structure, WriteError> {
    let locale = pool.r(&options.locale, Class::Ident)?;
    let mut funcs = Vec::with_capacity(manifest.functions.len() * 4);
    for f in &manifest.functions {
        funcs.extend_from_slice(&pool.r(f, Class::Ident)?.to_le_bytes());
    }
    let fallback = if fallback.is_empty() {
        None
    } else {
        let mut tags: Vec<&str> = fallback.iter().map(|f| f.1).collect();
        tags.sort_unstable();
        tags.dedup();
        if tags.len() > MAX_FALLBACK_LOCALES {
            return Err(WriteError::Fallback("more than 256 fallback locales"));
        }
        let mut out = Vec::new();
        varint(count(tags.len(), "fallback locales")?, &mut out);
        for t in &tags {
            out.extend_from_slice(&pool.r(t, Class::Ident)?.to_le_bytes());
        }
        for (msg, tag) in fallback {
            let li = tags.binary_search(tag).unwrap_or(0);
            out.extend_from_slice(&(msg | (count(li, "fallback")? << 24)).to_le_bytes());
        }
        Some(out)
    };

    let mut names = Vec::new();
    for (slots, locals) in &layout.order {
        varint(count(slots.len(), "slots")?, &mut names);
        varint(count(locals.len(), "locals")?, &mut names);
        for s in slots.iter().map(String::as_str) {
            if !keys.contains(s) {
                keys.insert(String::from(s));
            }
        }
        for s in slots
            .iter()
            .map(String::as_str)
            .chain(locals.iter().copied())
        {
            names.extend_from_slice(&pool.r(s, Class::Ident)?.to_le_bytes());
        }
    }

    let mut index = Vec::with_capacity(messages.len());
    let mut out = Vec::new();
    let mut cold = Vec::new();
    let empty = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        let Some(m) = m else {
            index.push(kind::ABSENT << kind::SHIFT);
            continue;
        };
        if let Some(t) = simple_text(m) {
            index.push(pool.r(t, Class::Text)?);
            continue;
        }
        let slots = manifest.slots.get(i).unwrap_or(&empty);
        let mut body = Vec::new();
        let enc = MsgEncoder::new(
            pool,
            &manifest.functions,
            slots,
            i,
            options.strip_cold,
            keys,
        )
        .message(m, &mut body)?;
        let at = count(out.len(), "MESSAGES")?;
        if at > MAX_OFFSET {
            return Err(WriteError::TooLarge("MESSAGES"));
        }
        let k = if matches!(m, Message::Select(_)) {
            kind::SELECT
        } else {
            kind::PATTERN
        };
        index.push((k << kind::SHIFT) | at);
        let names_ref = layout
            .refs
            .get(&(slots.as_slice(), locals_of(m)))
            .copied()
            .unwrap_or(0);
        varint(names_ref, &mut out);
        let decls = count(m.declarations().len(), "declarations")?
            .checked_mul(2)
            .ok_or(WriteError::TooLarge("declarations"))?;
        match enc.cold {
            None => varint(decls, &mut out),
            Some(record) => {
                varint(decls | 1, &mut out);
                if options.strip_cold {
                    varint(0, &mut out);
                } else {
                    varint(count(cold.len(), "COLD")?, &mut out);
                    cold.extend_from_slice(&record);
                }
            }
        }
        out.extend_from_slice(&body);
    }
    if out.len() > MAX_OFFSET as usize {
        return Err(WriteError::TooLarge("MESSAGES"));
    }
    Ok(Structure {
        locale,
        index,
        messages: out,
        cold,
        names,
        funcs,
        fallback,
    })
}

/// A NAMES entry's content: a message's slot names and its `.local` names
/// as declared.
type NamesKey<'a> = (&'a [String], Vec<&'a str>);

/// Where the NAMES entries go (a writer policy, §2.9): one per distinct
/// key, the most-referenced first and ties in first-use order, so that most
/// messages' `names` values stay one byte. Entry sizes do not depend on the
/// pool, so this is fixed before the two passes.
struct NamesLayout<'a> {
    order: Vec<NamesKey<'a>>,
    /// Each key's `names` value: its entry's offset + 1.
    refs: BTreeMap<NamesKey<'a>, u32>,
}

fn locals_of<'a>(m: &'a Message<'_>) -> Vec<&'a str> {
    m.declarations()
        .iter()
        .filter_map(|d| match d {
            Declaration::Local(x) => Some(&*x.name),
            _ => None,
        })
        .collect()
}

fn names_layout<'a>(
    manifest: &'a Manifest,
    messages: &[Option<&'a Message<'_>>],
) -> Result<NamesLayout<'a>, WriteError> {
    // Key → (references, first use).
    let mut uses: BTreeMap<NamesKey<'a>, (usize, usize)> = BTreeMap::new();
    for (i, m) in messages.iter().enumerate() {
        let Some(m) = m else { continue };
        if simple_text(m).is_some() {
            continue;
        }
        let slots = manifest.slots.get(i).map_or(&[][..], Vec::as_slice);
        let locals = locals_of(m);
        if slots.is_empty() && locals.is_empty() {
            continue;
        }
        uses.entry((slots, locals))
            .and_modify(|u| u.0 += 1)
            .or_insert((1, i));
    }
    let mut order: Vec<(NamesKey<'a>, (usize, usize))> = uses.into_iter().collect();
    order.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.1.1.cmp(&b.1.1)));
    let mut refs = BTreeMap::new();
    let mut off = 0usize;
    for (key, _) in &order {
        let n = key.0.len() + key.1.len();
        let r = count(off, "NAMES")?
            .checked_add(1)
            .ok_or(WriteError::TooLarge("NAMES"))?;
        refs.insert(key.clone(), r);
        off += varint_len(count(key.0.len(), "slots")?)
            + varint_len(count(key.1.len(), "locals")?)
            + 4 * n;
    }
    Ok(NamesLayout {
        order: order.into_iter().map(|(k, _)| k).collect(),
        refs,
    })
}

/// The length of `v` as a LEB128.
fn varint_len(v: u32) -> usize {
    let mut v = v >> 7;
    let mut n = 1;
    while v != 0 {
        v >>= 7;
        n += 1;
    }
    n
}

/// The text of a simple message: a pattern message without declarations
/// whose pattern is empty or one text part.
fn simple_text<'m>(m: &'m Message<'_>) -> Option<&'m str> {
    match m {
        Message::Pattern(p) if p.declarations.is_empty() => p.pattern.as_simple_text(),
        _ => None,
    }
}

/// INDEX as four byte planes.
fn planes(entries: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 4);
    for plane in 0..4 {
        out.extend(
            entries
                .iter()
                .map(|e| e.to_le_bytes().get(plane).copied().unwrap_or(0)),
        );
    }
    out
}

/// LOCALE: entries sorted by key; the entries of known kinds checked
/// (plural §4.1, number §4.2–§4.3, currency §4.6, unit §4.7).
fn locale_section(entries: &[(u32, Vec<u8>)]) -> Result<Vec<u8>, WriteError> {
    let mut sorted: Vec<&(u32, Vec<u8>)> = entries.iter().collect();
    sorted.sort_by_key(|e| e.0);
    let mut out = Vec::new();
    varint(count(sorted.len(), "LOCALE")?, &mut out);
    let mut prev = None;
    for (key, payload) in sorted {
        if prev == Some(*key) {
            return Err(WriteError::LocaleEntry(*key));
        }
        prev = Some(*key);
        let valid = match *key {
            locale_key::PLURAL_CARDINAL | locale_key::PLURAL_ORDINAL => plural::valid(payload),
            locale_key::NUMBER_SYMBOLS => crate::number::Symbols::parse(payload).is_some(),
            locale_key::NUMBER_PATTERNS => crate::number::Patterns::new(payload).is_valid(),
            locale_key::CURRENCY_DATA => {
                crate::currency::Currencies::parse(payload).is_some_and(|c| c.is_valid())
            }
            locale_key::UNIT_DATA => {
                crate::unit::Units::parse(payload).is_some_and(|u| u.is_valid())
            }
            _ => true,
        };
        if !valid {
            return Err(WriteError::LocaleEntry(*key));
        }
        varint(*key, &mut out);
        varint(count(payload.len(), "LOCALE")?, &mut out);
        out.extend_from_slice(payload);
    }
    Ok(out)
}

/// FALLBACK entries sorted by message, checked.
fn fallback_list(list: &[(u32, String)], n: u32) -> Result<Vec<(u32, &str)>, WriteError> {
    let mut out: Vec<(u32, &str)> = list.iter().map(|(m, t)| (*m, t.as_str())).collect();
    out.sort_unstable();
    for w in out.windows(2) {
        if let [a, b] = w
            && a.0 == b.0
        {
            return Err(WriteError::Fallback("a message listed twice"));
        }
    }
    if out.iter().any(|(m, _)| *m >= n) {
        return Err(WriteError::Fallback("message index out of range"));
    }
    Ok(out)
}

/// IDS: restart table, then the front-coded ids.
fn ids_section(ids: &[String]) -> Result<Vec<u8>, WriteError> {
    let mut table = Vec::with_capacity(ids.len().div_ceil(IDS_RESTART) * 4);
    let mut entries = Vec::new();
    let mut prev: &[u8] = &[];
    for (i, id) in ids.iter().enumerate() {
        let id = id.as_bytes();
        let shared = if i % IDS_RESTART == 0 {
            table.extend_from_slice(&count(entries.len(), "IDS")?.to_le_bytes());
            0
        } else {
            prev.iter().zip(id).take_while(|(a, b)| a == b).count()
        };
        let suffix = id.get(shared..).unwrap_or(&[]);
        varint(count(shared, "IDS")?, &mut entries);
        varint(count(suffix.len(), "IDS")?, &mut entries);
        entries.extend_from_slice(suffix);
        prev = id;
    }
    table.extend_from_slice(&entries);
    Ok(table)
}
