//! Fuzz target `catalog`: the `.mf2b` reader, its views and the decoder of
//! `mf2-catalog` on arbitrary and on mutated catalogs.
//!
//! The input picks the mode:
//!
//! * **catalog** — an input that starts with the magic `MF2B` is a catalog.
//!   It is loaded with the manifest hash its own header carries, so a damaged
//!   catalog gets past the skew check (F6) to its structure.
//! * **source** — any other input is MF2 source up to its first NUL byte
//!   (which MF2 cannot contain), then an options byte and mutation
//!   instructions. A source that parses is compiled with `writer::single`
//!   (slots from `mf2_syntax::analyze`), which must succeed, deterministically;
//!   the catalog must load and decode to the parsed model (F1, layer L3).
//!   Then the instructions damage it — bytes set, flipped, inserted or deleted
//!   inside a chosen section, with the section table kept consistent — so
//!   mutated writer output is exercised even without catalog seeds.
//!
//! Every catalog that loads is checked in full:
//!
//! * no panic, no out-of-bounds access (the address sanitizer), and every view
//!   iterator yields at most its count and ends after a `Malformed`;
//! * the header accessors, `sections`, every FUNCS entry, the LOCALE keys,
//!   `names`, `fallback_locale`, and ids outside the catalog (another chunk,
//!   past the end) are `Absent`; a wrong manifest hash is `ManifestMismatch`;
//! * every message: `get`, a **full view walk** (every declaration, part,
//!   expression, option, markup, selector, variant and key; `text` on every
//!   string; every variable through NAMES; every function through FUNCS) and
//!   `decode_report`. A message that decodes walked cleanly, and its model has
//!   the same shape as the views (declarations, selectors, variants, keys,
//!   functions, markup, options, parts);
//! * the ids of IDS, rebuilt from the bytes, are found by `lookup` when they
//!   ascend; whatever `lookup` finds is in the catalog;
//! * **round trip**: the decoded models are written again with
//!   `writer::catalog` (a manifest from `analyze`), which must succeed,
//!   deterministically; that catalog loads, keeps the locale, direction,
//!   chunk, CLDR version, plural entries and fallbacks, finds every id, and
//!   decodes to the same models (F1); stripped, it decodes to them wherever no
//!   COLD data was dropped;
//! * **linear time**: the whole check takes at most 50 ms + 50 µs per input
//!   byte + 100 ns per byte of text it resolved. Resolved text is charged
//!   because `text` is linear in the string by design (F4) and one string may
//!   be referenced from many places, so the text a catalog resolves to can be
//!   quadratic in its size; for the same reason a message resolving more than
//!   16 MiB is not decoded, and the round trip is skipped past 64 MiB.

#![no_main]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::time::Duration;

#[path = "../common/budget.rs"]
mod budget;

use libfuzzer_sys::fuzz_target;
use mf2_catalog::writer::{self, Options};
use mf2_catalog::{
    Body, Catalog, CatalogError, CldrVersion, DeclView, DecodeError, Dir, Entry, ExprView, KeyView,
    Malformed, Manifest, MsgId, MsgView, Names, Operand, OptionsView, PartView, PatternView,
    StrRef, VarRef, decode, decode_report,
};
use mf2_model::{
    Attributes, Declaration, Expression, FunctionRef, Key, Message, OptionValue, Pattern,
    PatternMessage, PatternPart,
};

/// A message whose walk resolved more text than this is not decoded.
const MESSAGE_TEXT_CAP: usize = 16 << 20;
/// Past this much resolved text the round trip is skipped.
const ROUND_TRIP_TEXT_CAP: usize = 64 << 20;
/// Time charged per byte of resolved text (and of decoded model).
const NANOS_PER_TEXT_BYTE: u64 = 100;
/// At most this many mutation instructions are applied.
const MAX_MUTATIONS: usize = 64;
/// `en`'s `plural.cardinal` entry.
const EN_CARDINAL: [u8; 5] = [0x21, 0x01, 0x05, 0x82, 0x01];

fuzz_target!(|data: &[u8]| {
    let cpu = budget::Cpu::start();
    let mut work = 0usize;
    if data.starts_with(b"MF2B") {
        check(data.to_vec(), &mut work);
    } else {
        source(data, &mut work);
    }
    let budget = budget::budget(
        data.len(),
        Duration::from_nanos(
            u64::try_from(work)
                .unwrap_or(u64::MAX)
                .saturating_mul(NANOS_PER_TEXT_BYTE),
        ),
    );
    let used = cpu.elapsed();
    assert!(
        used <= budget,
        "{used:?} of CPU for {} bytes and {work} bytes of text (budget {budget:?})",
        data.len()
    );
});

// ── source mode ──────────────────────────────────────────────────────────────

fn source(data: &[u8], work: &mut usize) {
    let (src, tail) = match data.iter().position(|&b| b == 0) {
        Some(i) => (&data[..i], &data[i + 1..]),
        None => (data, &[][..]),
    };
    let src = String::from_utf8_lossy(src);
    let Some(model) = mf2_syntax::parse_model(&src).message else {
        return;
    };
    let (opt, ops) = tail
        .split_first()
        .map_or((0, &[][..]), |(o, rest)| (*o, rest));
    let options = options_from(opt);
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let (bytes, manifest) = writer::single(&model, &slots, &options)
        .unwrap_or_else(|e| panic!("a parsed model is not writable: {e:?}"));
    let again = writer::single(&model, &slots, &options).map(|w| w.0);
    assert_eq!(
        again.as_deref().ok(),
        Some(&bytes[..]),
        "the writer is not deterministic"
    );

    let cat = Catalog::new(bytes.clone(), manifest.hash())
        .unwrap_or_else(|e| panic!("writer output does not load: {e:?}"));
    let id = MsgId::new(options.chunk, 0).expect("chunk ids are valid");
    assert_eq!(cat.message_count(), 1);
    assert_eq!(cat.locale(), options.locale);
    assert_eq!(cat.dir(), options.dir);
    assert_eq!(cat.chunk(), options.chunk);
    assert_eq!(cat.cldr_version(), options.cldr_version);
    assert_eq!(cat.cold_stripped(), options.strip_cold);
    assert_eq!(cat.ids_stripped(), options.strip_ids);
    assert_eq!(
        cat.lookup(""),
        if options.strip_ids { None } else { Some(id) }
    );
    let fallback = options.fallback.first().map(|f| f.1.as_str());
    assert_eq!(cat.fallback_locale(id), fallback);
    for (key, payload) in &options.locale_entries {
        assert_eq!(cat.locale_entry(*key), Some(&payload[..]));
    }
    let d = decode_report(&cat, id).unwrap_or_else(|e| panic!("writer output: {e:?}"));
    if options.strip_cold {
        if !d.cold_dropped {
            assert_eq!(d.message, model, "stripped, nothing dropped");
        }
    } else {
        assert!(!d.cold_dropped);
        assert_eq!(d.message, model, "L3: decode(write(m)) differs from m");
    }
    drop(d);
    drop(cat);

    check(bytes.clone(), work);
    if !ops.is_empty() {
        let mut damaged = bytes;
        mutate(&mut damaged, ops);
        check(damaged, work);
    }
}

/// Writer options from one input byte.
fn options_from(b: u8) -> Options {
    let mut o = Options::new("en", if b & 4 != 0 { Dir::Rtl } else { Dir::Ltr });
    o.strip_cold = b & 1 != 0;
    o.strip_ids = b & 2 != 0;
    if b & 8 != 0 {
        o.fallback = vec![(0, String::from("de"))];
    }
    if b & 16 != 0 {
        o.locale_entries = vec![(1, EN_CARDINAL.to_vec()), (2, Vec::new())];
        o.cldr_version = Some(CldrVersion {
            major: 48,
            minor: 2,
            patch: 1,
        });
    }
    o.chunk = b >> 5;
    o
}

// ── mutation ─────────────────────────────────────────────────────────────────

/// `(entry position, kind, offset, length)` of every section-table entry
/// that lies inside the buffer.
fn section_table(b: &[u8]) -> Vec<(usize, u16, usize, usize)> {
    let Some(&[c0, c1]) = b.get(30..32) else {
        return Vec::new();
    };
    let n = usize::from(u16::from_le_bytes([c0, c1]));
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let at = 32 + 10 * i;
        let Some(e) = b.get(at..at + 10) else { break };
        let kind = u16::from_le_bytes([e[0], e[1]]);
        let off = u32::from_le_bytes([e[2], e[3], e[4], e[5]]) as usize;
        let len = u32::from_le_bytes([e[6], e[7], e[8], e[9]]) as usize;
        out.push((at, kind, off, len));
    }
    out
}

/// Applies 4-byte instructions `[op, pos_lo, pos_hi, val]`: `op & 0x0f`
/// picks the region (0: the whole file; `k`: section-table entry
/// `(k − 1) mod count`), `pos` a byte inside it, `op >> 4` the edit.
/// Insertions and deletions inside a section update its length and the
/// offsets of the sections after it, so the catalog keeps loading and the
/// damage reaches the records.
fn mutate(b: &mut Vec<u8>, ops: &[u8]) {
    for op in ops.chunks_exact(4).take(MAX_MUTATIONS) {
        let &[code, lo, hi, val] = op else { continue };
        let table = section_table(b);
        let region = usize::from(code & 0x0f);
        let (start, len, entry) = match region.checked_sub(1) {
            Some(r) if !table.is_empty() => {
                let i = r % table.len();
                let (_, _, off, len) = table[i];
                if off.checked_add(len).is_some_and(|end| end <= b.len()) {
                    (off, len, Some(i))
                } else {
                    (0, b.len(), None)
                }
            }
            _ => (0, b.len(), None),
        };
        let pos = start
            + if len == 0 {
                0
            } else {
                usize::from(u16::from_le_bytes([lo, hi])) % len
            };
        let inside = len > 0 && pos < b.len();
        match (code >> 4) % 8 {
            0 if inside => b[pos] = val,
            1 if inside => b[pos] ^= val,
            2 if inside => b[pos] = b[pos].wrapping_add(val),
            3 => {
                b.insert(pos.min(b.len()), val);
                resize(b, &table, entry, 1);
            }
            4 if inside => {
                b.remove(pos);
                resize(b, &table, entry, -1);
            }
            5 => {
                // A run of zero bytes: TEXT parts, empty counts, key `*`.
                let k = usize::from(val & 0x0f) + 1;
                let at = pos.min(b.len());
                b.splice(at..at, std::iter::repeat_n(0, k));
                resize(b, &table, entry, isize::try_from(k).unwrap_or(0));
            }
            6 if inside => {
                // A long varint (up to u32::MAX) over the bytes at `pos`.
                let end = (pos + 5).min(start + len).min(b.len());
                let v = [0x80 | val, 0x80 | val, 0x80 | val, 0x80 | val, val & 0x0f];
                for (dst, src) in b[pos..end].iter_mut().zip(v) {
                    *dst = src;
                }
            }
            7 if inside => {
                let from = start + (pos - start + usize::from(val)) % len;
                b[pos] = b[from];
            }
            _ => {}
        }
    }
}

/// After inserting (`delta` > 0) or deleting bytes inside table entry
/// `entry`: its length and the offsets of every later entry move by `delta`.
fn resize(b: &mut [u8], table: &[(usize, u16, usize, usize)], entry: Option<usize>, delta: isize) {
    let Some(e) = entry else { return };
    let set = |b: &mut [u8], at: usize, v: usize| {
        if let (Ok(v), Some(dst)) = (u32::try_from(v), b.get_mut(at..at + 4)) {
            dst.copy_from_slice(&v.to_le_bytes());
        }
    };
    for (i, &(at, _, off, len)) in table.iter().enumerate() {
        if i == e {
            set(b, at + 6, len.saturating_add_signed(delta));
        } else if i > e {
            set(b, at + 2, off.saturating_add_signed(delta));
        }
    }
}

// ── the checks ───────────────────────────────────────────────────────────────

fn header_hash(b: &[u8]) -> u64 {
    b.get(8..16)
        .and_then(|h| h.try_into().ok())
        .map_or(0, u64::from_le_bytes)
}

/// Loads `bytes` with the hash its header carries and, if it loads, checks
/// it in full.
fn check(bytes: Vec<u8>, work: &mut usize) {
    let hash = header_hash(&bytes);
    let copy = bytes.clone();
    let Ok(cat) = Catalog::new(bytes, hash) else {
        return;
    };
    assert_eq!(
        Catalog::new(copy, hash ^ 1).err(),
        Some(CatalogError::ManifestMismatch),
        "F6"
    );
    let decoded = walk_catalog(&cat, work);
    if *work <= ROUND_TRIP_TEXT_CAP {
        round_trip(&cat, &decoded, work);
    }
}

fn walk_catalog<'c>(cat: &'c Catalog, work: &mut usize) -> Vec<Option<Message<'c>>> {
    assert_eq!(cat.format_version() >> 8, 2);
    assert_eq!(cat.manifest_hash(), header_hash(cat.as_bytes()));
    assert!(matches!(cat.dir(), Dir::Ltr | Dir::Rtl));
    let _ = (cat.cldr_version(), cat.cold_stripped(), cat.ids_stripped());
    *work += cat.locale().len();
    let table = section_table(cat.as_bytes());
    let declared = u16::from_le_bytes([cat.as_bytes()[30], cat.as_bytes()[31]]);
    assert_eq!(table.len(), usize::from(declared));
    assert!(
        cat.sections()
            .map(|(k, off, len)| (k, off as usize, len as usize))
            .eq(table.iter().map(|&(_, k, off, len)| (k, off, len))),
        "sections() is the section table"
    );
    for i in 0..cat.function_count() {
        *work += cat.function(i).map_or(0, str::len);
    }
    assert!(cat.function(cat.function_count()).is_none());
    for key in [0, 1, 2, 3, 4, u32::MAX] {
        *work += cat.locale_entry(key).map_or(0, <[u8]>::len);
    }

    let (chunk, count) = (cat.chunk(), cat.message_count());
    assert!(count <= 1 << 24);
    let mut decoded = Vec::with_capacity(count as usize);
    let mut probes: Vec<&str> = vec!["", "0", "m00", "zz", "\u{10ffff}"];
    for i in 0..count {
        let id = MsgId::new(chunk, i).expect("count ≤ 2²⁴");
        let elsewhere = MsgId::new(chunk.wrapping_add(1), i).expect("valid");
        assert!(matches!(cat.get(elsewhere), Entry::Absent));
        assert!(cat.fallback_locale(elsewhere).is_none());
        *work += cat.fallback_locale(id).map_or(0, str::len);
        names(cat, cat.names(id), work);
        if let (Entry::Simple(r), true) = (cat.get(id), probes.len() < 9) {
            probes.extend(cat.text(r));
        }
        decoded.push(message(cat, id, work));
    }
    if let Some(past) = MsgId::new(chunk, count) {
        assert!(matches!(cat.get(past), Entry::Absent));
        assert!(matches!(decode(cat, past), Err(DecodeError::Absent)));
    }
    for p in probes {
        *work += p.len();
        if let Some(found) = cat.lookup(p) {
            assert!(
                found.chunk() == chunk && found.index() < count,
                "lookup({p:?})"
            );
        }
    }
    lookup_ids(cat, work);
    decoded
}

/// The ends of a NAMES entry, and one past them.
fn names(cat: &Catalog, names: Names<'_>, work: &mut usize) {
    let (e, l) = (names.external_count(), names.local_count());
    for s in [0, e / 2, e.wrapping_sub(1)] {
        if s < e {
            let r = names.external(s).expect("a slot inside the entry");
            *work += cat.text(r).map_or(0, str::len);
        }
    }
    for s in [0, l / 2, l.wrapping_sub(1)] {
        if s < l {
            let r = names.local(s).expect("a local inside the entry");
            *work += cat.text(r).map_or(0, str::len);
        }
    }
    assert!(names.external(e).is_none() && names.local(l).is_none());
    assert!(names.var(VarRef::External(e)).is_none() && names.var(VarRef::Local(l)).is_none());
}

/// Rebuilds the ids of IDS from the bytes; when they ascend (as the writer
/// writes them), `lookup` finds each one.
fn lookup_ids(cat: &Catalog, work: &mut usize) {
    let b = cat.as_bytes();
    let Some(&(_, _, off, len)) = section_table(b).iter().find(|s| s.1 == 8) else {
        return;
    };
    let Some(ids) = b.get(off..off + len) else {
        return;
    };
    let count = cat.message_count() as usize;
    let mut c = count.div_ceil(16) * 4;
    let mut all: Vec<Vec<u8>> = Vec::with_capacity(count);
    for _ in 0..count {
        let (Some(shared), Some(n)) = (leb(ids, &mut c), leb(ids, &mut c)) else {
            return;
        };
        let Some(suffix) = ids.get(c..c + n) else {
            return;
        };
        c += n;
        let prev = all.last().map_or(&[][..], Vec::as_slice);
        let Some(head) = prev.get(..shared) else {
            return;
        };
        let mut id = head.to_vec();
        id.extend_from_slice(suffix);
        all.push(id);
    }
    let ascending = all.windows(2).all(|w| w[0] < w[1]);
    for (i, id) in all.iter().enumerate() {
        *work += 32 * id.len();
        let Ok(s) = std::str::from_utf8(id) else {
            continue;
        };
        let found = cat.lookup(s);
        if ascending {
            let want = MsgId::new(cat.chunk(), u32::try_from(i).expect("≤ 2²⁴"));
            assert_eq!(found, want, "lookup({s:?})");
        }
    }
}

/// A minimal LEB128 `u32`, as the reader reads it (a lenient read suffices
/// here: `Catalog::new` has validated IDS already).
fn leb(b: &[u8], at: &mut usize) -> Option<usize> {
    let mut v = 0usize;
    for shift in [0, 7, 14, 21, 28] {
        let x = *b.get(*at)?;
        *at += 1;
        v |= usize::from(x & 0x7f) << shift;
        if x & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// One message: `get`, the full view walk, `decode_report`, and their
/// agreement. Returns the decoded model.
fn message<'c>(cat: &'c Catalog, id: MsgId, work: &mut usize) -> Option<Message<'c>> {
    let entry = cat.get(id);
    match entry {
        Entry::Absent => {
            assert!(matches!(decode_report(cat, id), Err(DecodeError::Absent)));
            None
        }
        Entry::Simple(r) => {
            let text = cat.text(r);
            *work += text.map_or(0, str::len);
            match (text, decode_report(cat, id)) {
                (Some(t), Ok(d)) => {
                    assert!(!d.cold_dropped);
                    let want = Message::Pattern(PatternMessage {
                        declarations: Vec::new(),
                        pattern: Pattern::from_text(Cow::Borrowed(t)),
                    });
                    assert_eq!(d.message, want);
                    Some(d.message)
                }
                (None, Err(DecodeError::String)) => None,
                (t, d) => panic!("simple message: text {t:?}, decoded {d:?}"),
            }
        }
        Entry::Pattern(v) | Entry::Select(v) => {
            let select = matches!(entry, Entry::Select(_));
            let mut w = Walk {
                cat,
                names: Names::EMPTY,
                shape: Shape::default(),
                clean: true,
                text: 0,
                locals: 0,
            };
            w.message(v, select);
            *work += w.text;
            if w.text > MESSAGE_TEXT_CAP {
                return None;
            }
            let d = decode_report(cat, id).ok()?;
            assert!(w.clean, "decoded although the walk found damage");
            assert_eq!(select, matches!(d.message, Message::Select(_)));
            let mut m = ModelWalk::default();
            m.message(&d.message);
            *work += m.bytes;
            assert_eq!(m.shape, w.shape, "decoded model and views disagree");
            Some(d.message)
        }
    }
}

/// What a message contains, counted the same way from the views and from
/// the decoded model.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct Shape {
    decls: usize,
    selectors: usize,
    variants: usize,
    keys: usize,
    functions: usize,
    markup: usize,
    options: usize,
    /// The decoder rejects empty and adjacent text, so this is exact too.
    parts: usize,
}

/// Drains a view iterator: `Ok` items go to `f`. It must yield at most
/// `len` items, exactly `len` when all are `Ok`, and nothing after a
/// `Malformed`. Returns whether every item was `Ok`.
fn each<T>(
    len: u32,
    mut it: impl Iterator<Item = Result<T, Malformed>>,
    mut f: impl FnMut(T),
) -> bool {
    let mut seen = 0u32;
    while let Some(item) = it.next() {
        seen += 1;
        assert!(seen <= len, "a view iterator yields more than its count");
        let Ok(x) = item else {
            assert!(
                it.next().is_none(),
                "a view iterator goes on after Malformed"
            );
            return false;
        };
        f(x);
    }
    assert_eq!(seen, len, "a view iterator ends early without Malformed");
    true
}

/// The full view walk of one message.
struct Walk<'c> {
    cat: &'c Catalog,
    names: Names<'c>,
    shape: Shape,
    /// Every record, string, name and function the decoder needs resolved.
    clean: bool,
    /// Bytes of resolved text.
    text: usize,
    /// `.local`s seen so far.
    locals: u32,
}

impl<'c> Walk<'c> {
    fn text(&mut self, r: StrRef) {
        match self.cat.text(r) {
            Some(t) => self.text += t.len(),
            None => self.clean = false,
        }
    }

    fn var(&mut self, v: VarRef) {
        match self.names.var(v) {
            Some(r) => self.text(r),
            None => self.clean = false,
        }
    }

    fn operand(&mut self, o: Operand) {
        match o {
            Operand::Literal(r) => self.text(r),
            Operand::Variable(v) => self.var(v),
        }
    }

    fn options(&mut self, o: OptionsView<'c>) {
        let ok = each(o.len(), o, |(name, value)| {
            self.shape.options += 1;
            self.text(name);
            self.operand(value);
        });
        self.clean &= ok;
    }

    fn expr(&mut self, e: ExprView<'c>) {
        match e.operand() {
            Some(o) => self.operand(o),
            None => assert!(
                e.function().is_some(),
                "an expression without operand or function"
            ),
        }
        if let Some(f) = e.function() {
            self.shape.functions += 1;
            match self.cat.function(f.index()) {
                Some(name) => self.text += name.len(),
                None => self.clean = false,
            }
            self.options(f.options());
        }
    }

    fn pattern(&mut self, p: PatternView<'c>) {
        let ok = each(p.len(), p.parts(), |part| {
            self.shape.parts += 1;
            match part {
                PartView::Text(r) => self.text(r),
                PartView::Expression(e) => self.expr(e),
                PartView::Markup(m) => {
                    self.shape.markup += 1;
                    self.text(m.name());
                    self.options(m.options());
                }
            }
        });
        self.clean &= ok;
    }

    fn message(&mut self, v: MsgView<'c>, select: bool) {
        assert_eq!(v.is_select(), select);
        assert!(std::ptr::eq(v.catalog(), self.cat));
        self.names = v.names();
        let mut decls = v.declarations();
        let ok = each(decls.len(), &mut decls, |d| {
            self.shape.decls += 1;
            match d {
                DeclView::Input(e) => {
                    assert!(matches!(e.operand(), Some(Operand::Variable(_))));
                    self.expr(e);
                }
                DeclView::Local { index, expr } => {
                    assert_eq!(index, self.locals, "locals are numbered in order");
                    self.locals += 1;
                    self.expr(expr);
                    match self.names.local(index) {
                        Some(r) => self.text(r),
                        None => self.clean = false,
                    }
                }
            }
        });
        self.clean &= ok;
        match decls.body() {
            Err(Malformed) => self.clean = false,
            Ok(Body::Pattern(p)) => {
                assert!(!select);
                self.pattern(p);
            }
            Ok(Body::Select(s)) => {
                assert!(select);
                let sels = s.selectors();
                let ok = each(sels.len(), sels, |v| {
                    self.shape.selectors += 1;
                    self.var(v);
                });
                self.clean &= ok;
                let variants = s.variants();
                let ok = each(variants.len(), variants, |variant| {
                    self.shape.variants += 1;
                    let keys = variant.keys();
                    let ok = each(keys.len(), keys, |k| {
                        self.shape.keys += 1;
                        if let KeyView::Literal(r) = k {
                            self.text(r);
                        }
                    });
                    self.clean &= ok;
                    self.pattern(variant.pattern());
                });
                self.clean &= ok;
            }
        }
    }
}

/// The shape of a decoded model and the bytes of its strings.
#[derive(Default)]
struct ModelWalk {
    shape: Shape,
    bytes: usize,
}

impl ModelWalk {
    fn attributes(&mut self, a: &Attributes<'_>) {
        for (name, value) in a.iter() {
            self.bytes += name.len() + value.map_or(0, |v| v.value.len());
        }
    }

    fn function(&mut self, f: &FunctionRef<'_>) {
        self.shape.functions += 1;
        self.bytes += f.name.len();
        self.options(&f.options);
    }

    fn options(&mut self, o: &mf2_model::Options<'_>) {
        for (name, value) in o.iter() {
            self.shape.options += 1;
            self.bytes += name.len()
                + match value {
                    OptionValue::Literal(l) => l.value.len(),
                    OptionValue::Variable(v) => v.name.len(),
                    _ => 0,
                };
        }
    }

    fn expression(&mut self, e: &Expression<'_>) {
        self.bytes += match e {
            Expression::Literal(x) => x.arg.value.len(),
            Expression::Variable(x) => x.arg.name.len(),
            _ => 0,
        };
        if let Some(f) = e.function() {
            self.function(f);
        }
        self.attributes(e.attributes());
    }

    fn pattern(&mut self, p: &Pattern<'_>) {
        for part in p.parts() {
            self.shape.parts += 1;
            match part {
                PatternPart::Text(t) => self.bytes += t.len(),
                PatternPart::Expression(e) => self.expression(e),
                PatternPart::Markup(m) => {
                    self.shape.markup += 1;
                    self.bytes += m.name.len();
                    self.options(&m.options);
                    self.attributes(&m.attributes);
                }
                _ => {}
            }
        }
    }

    fn message(&mut self, m: &Message<'_>) {
        for d in m.declarations() {
            self.shape.decls += 1;
            match d {
                Declaration::Input(x) => {
                    self.bytes += x.name.len() + x.value.arg.name.len();
                    if let Some(f) = &x.value.function {
                        self.function(f);
                    }
                    self.attributes(&x.value.attributes);
                }
                Declaration::Local(x) => {
                    self.bytes += x.name.len();
                    self.expression(&x.value);
                }
                _ => {}
            }
        }
        match m {
            Message::Pattern(p) => self.pattern(&p.pattern),
            Message::Select(s) => {
                for v in &s.selectors {
                    self.shape.selectors += 1;
                    self.bytes += v.name.len();
                }
                for v in &s.variants {
                    self.shape.variants += 1;
                    for k in &v.keys {
                        self.shape.keys += 1;
                        self.bytes += match k {
                            Key::Literal(l) => l.value.len(),
                            Key::CatchAll(c) => c.value.as_ref().map_or(0, |v| v.len()),
                            _ => 0,
                        };
                    }
                    self.pattern(&v.value);
                }
            }
            _ => {}
        }
    }
}

/// Writes the decoded models again and checks the result (F1, F7, F8).
fn round_trip(cat: &Catalog, decoded: &[Option<Message<'_>>], work: &mut usize) {
    let mut manifest = Manifest::default();
    let mut functions = BTreeSet::new();
    for (i, m) in decoded.iter().enumerate() {
        manifest.ids.push(format!("{i:08}"));
        let (slots, markup) = match m {
            Some(m) => {
                let a = mf2_syntax::analyze(m);
                functions.extend(a.functions.iter().map(|n| n.nfc.to_string()));
                (
                    a.externals.iter().map(|n| n.nfc.to_string()).collect(),
                    a.markup.iter().map(|n| n.nfc.to_string()).collect(),
                )
            }
            None => (Vec::new(), Vec::new()),
        };
        manifest.slots.push(slots);
        manifest.markup.push(markup);
    }
    manifest.functions = functions.into_iter().collect();
    let chunk = cat.chunk();
    let id = |i: usize| MsgId::new(chunk, u32::try_from(i).expect("≤ 2²⁴")).expect("valid");
    let mut options = Options::new(cat.locale(), cat.dir());
    options.chunk = chunk;
    options.cldr_version = cat.cldr_version();
    for key in [1, 2] {
        if let Some(p) = cat.locale_entry(key) {
            options.locale_entries.push((key, p.to_vec()));
        }
    }
    options.fallback = (0..decoded.len())
        .filter_map(|i| {
            let tag = cat.fallback_locale(id(i))?;
            Some((u32::try_from(i).expect("≤ 2²⁴"), String::from(tag)))
        })
        .collect();
    let refs: Vec<Option<&Message<'_>>> = decoded.iter().map(Option::as_ref).collect();
    let bytes = writer::catalog(&manifest, &refs, &options)
        .unwrap_or_else(|e| panic!("a decoded model is not writable: {e:?}"));
    let twice = writer::catalog(&manifest, &refs, &options);
    assert_eq!(
        twice.as_deref().ok(),
        Some(&bytes[..]),
        "the writer is not deterministic"
    );
    *work += bytes.len();

    let again = Catalog::new(bytes, manifest.hash())
        .unwrap_or_else(|e| panic!("writer output does not load: {e:?}"));
    assert_eq!(again.locale(), cat.locale());
    assert_eq!(again.dir(), cat.dir());
    assert_eq!(again.chunk(), chunk);
    assert_eq!(again.cldr_version(), cat.cldr_version());
    assert_eq!(again.message_count(), cat.message_count());
    for key in [1, 2] {
        assert_eq!(again.locale_entry(key), cat.locale_entry(key));
    }
    for (i, m) in decoded.iter().enumerate() {
        let id = id(i);
        assert_eq!(again.fallback_locale(id), cat.fallback_locale(id));
        assert_eq!(again.lookup(&manifest.ids[i]), Some(id));
        match m {
            Some(m) => {
                let d = decode(&again, id).unwrap_or_else(|e| panic!("re-encoded: {e:?}"));
                assert_eq!(&d, m, "decode(write(m)) differs from m");
            }
            None => assert!(matches!(again.get(id), Entry::Absent)),
        }
    }
    assert_eq!(again.lookup("~"), None);

    let stripped = writer::catalog(&manifest, &refs, &options.clone().stripped())
        .unwrap_or_else(|e| panic!("stripped: {e:?}"));
    let stripped = Catalog::new(stripped, manifest.hash())
        .unwrap_or_else(|e| panic!("stripped writer output does not load: {e:?}"));
    for (i, m) in decoded.iter().enumerate() {
        if let Some(m) = m {
            let d = decode_report(&stripped, id(i))
                .unwrap_or_else(|e| panic!("stripped, re-encoded: {e:?}"));
            if !d.cold_dropped {
                assert_eq!(&d.message, m, "stripped, nothing dropped");
            }
        }
    }
    if let Some(first) = manifest.ids.first() {
        assert_eq!(stripped.lookup(first), None);
    }
}
