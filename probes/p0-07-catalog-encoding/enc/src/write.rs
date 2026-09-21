//! `.mf2b` writer with the layout variants P0.7 compares (plans/02 §6):
//! how string lengths are found, single vs split pools, INDEX layout,
//! stripped vs unstripped, dedup on/off.

use std::collections::HashMap;

use mf2b_format::{self as f, flags, kind, part, section};

use crate::error::Error;
use crate::manifest::Manifest;
use crate::model::{Attribute, Decl, Expr, Key, Markup, MarkupKind, Message, Operand, Part, Pattern};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrLayout {
    PrefixChar,
    RefLen,
    Offsets,
    Nul,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexLayout {
    Fixed,
    VarintDelta,
    Blocked,
    Planes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub strs: StrLayout,
    pub index: IndexLayout,
    pub split: bool,
    pub strip: bool,
    pub dedup: bool,
    /// Per-message relative StrRefs in MESSAGES.
    pub rel: bool,
    /// Pool strings sorted bytewise instead of first-use order (writer policy only).
    pub sorted: bool,
}

impl Layout {
    /// The layout the probe recommends (and the P0.3 reader implements).
    pub const RECOMMENDED: Layout =
        Layout { strs: StrLayout::Nul, index: IndexLayout::Planes, split: true, strip: true, dedup: true, rel: false, sorted: true };

    /// The closest valid reading of plans/02 §2 as written: length-prefixed
    /// strings (here a UTF-8 scalar prefix, since a LEB128 prefix breaks F4),
    /// fixed u32 INDEX, one pool in first-use order.
    pub const PLAN_BASELINE: Layout =
        Layout { strs: StrLayout::PrefixChar, index: IndexLayout::Fixed, split: false, strip: true, dedup: true, rel: false, sorted: false };

    pub fn name(&self) -> String {
        let s = match self.strs {
            StrLayout::PrefixChar => "prefix",
            StrLayout::RefLen => "reflen",
            StrLayout::Offsets => "offtab",
            StrLayout::Nul => "nul",
        };
        let i = match self.index {
            IndexLayout::Fixed => "fixed",
            IndexLayout::VarintDelta => "vdelta",
            IndexLayout::Blocked => "blocked",
            IndexLayout::Planes => "planes",
        };
        format!(
            "{s}-{i}-{}-{}{}{}",
            if self.split { "split" } else { "single" },
            if self.strip { "stripped" } else { "full" },
            if self.rel { "-rel" } else { "" },
            if self.dedup { "" } else { "-nodedup" }
        ) + if self.sorted { "-sorted" } else { "" }
    }

    pub fn flags(&self) -> u16 {
        let mut fl = match self.strs {
            StrLayout::PrefixChar => flags::STR_PREFIX_CHAR,
            StrLayout::RefLen => flags::STR_REF_LEN,
            StrLayout::Offsets => flags::STR_OFFSETS,
            StrLayout::Nul => flags::STR_NUL,
        };
        fl |= match self.index {
            IndexLayout::Fixed => flags::INDEX_FIXED,
            IndexLayout::VarintDelta => flags::INDEX_VARINT_DELTA,
            IndexLayout::Blocked => flags::INDEX_BLOCKED,
            IndexLayout::Planes => flags::INDEX_PLANES,
        };
        if self.rel {
            fl |= flags::REL_REFS;
        }
        if self.split {
            fl |= flags::SPLIT_POOLS;
        }
        if self.strip {
            fl |= flags::STRIPPED;
        }
        fl
    }
}

pub fn varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

/// Where one pool string ended up.
#[derive(Clone, Copy, Default, Debug)]
struct Placed {
    /// StrRef offset (prefix position for `PrefixChar`).
    off: u32,
    len: u32,
    /// Global string index (`Offsets`).
    idx: u32,
    /// First byte of the text itself.
    start: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PoolKind {
    Ident,
    Text,
}

/// Interned strings. Pass 1 records every occurrence; `finish` lays the pools
/// out; pass 2 replays the occurrences in the same order and gets final refs.
struct Strings {
    layout: Layout,
    /// Per pool: distinct strings in first-use order (or every occurrence without dedup).
    lists: [Vec<String>; 2],
    maps: [HashMap<String, usize>; 2],
    /// Occurrence log from pass 1: (pool, index into its list).
    log: Vec<(PoolKind, usize)>,
    cursor: usize,
    /// After `finish`: per pool, per string.
    placed: [Vec<Placed>; 2],
    region: Vec<u8>,
    idents_len: usize,
    ends: Vec<u32>,
    replay: bool,
    /// Current message base for relative refs (`None` = absolute).
    base: Option<i64>,
}

impl Strings {
    fn new(layout: Layout) -> Self {
        Strings {
            layout,
            lists: [Vec::new(), Vec::new()],
            maps: [HashMap::new(), HashMap::new()],
            log: Vec::new(),
            cursor: 0,
            placed: [Vec::new(), Vec::new()],
            region: Vec::new(),
            idents_len: 0,
            ends: Vec::new(),
            replay: false,
            base: None,
        }
    }

    fn pool(&self, k: PoolKind) -> usize {
        if !self.layout.split {
            return 0;
        }
        match k {
            PoolKind::Ident => 0,
            PoolKind::Text => 1,
        }
    }

    /// Records (pass 1) or resolves (pass 2) one occurrence.
    fn occ(&mut self, k: PoolKind, s: &str) -> Result<Placed, Error> {
        let p = self.pool(k);
        if !self.replay {
            let idx = if self.layout.dedup {
                if let Some(&i) = self.maps[p].get(s) {
                    i
                } else {
                    self.lists[p].push(s.to_owned());
                    let i = self.lists[p].len() - 1;
                    self.maps[p].insert(s.to_owned(), i);
                    i
                }
            } else {
                self.lists[p].push(s.to_owned());
                self.lists[p].len() - 1
            };
            self.log.push((k, idx));
            return Ok(Placed::default());
        }
        let &(lk, idx) = self.log.get(self.cursor).ok_or(Error::Other("replay overrun".into()))?;
        self.cursor += 1;
        if lk != k || self.lists[p][idx] != s {
            return Err(Error::Other("replay mismatch".into()));
        }
        Ok(self.placed[p][idx])
    }

    fn finish(&mut self) -> Result<(), Error> {
        let mut global = 0u32;
        let pools = if self.layout.split { 2 } else { 1 };
        for p in 0..pools {
            let list = std::mem::take(&mut self.lists[p]);
            let mut order: Vec<usize> = (0..list.len()).collect();
            if self.layout.sorted {
                order.sort_by(|&a, &b| list[a].cmp(&list[b]));
            }
            self.placed[p] = vec![Placed::default(); list.len()];
            for &li in &order {
                let s = &list[li];
                if s.contains('\0') && self.layout.strs == StrLayout::Nul {
                    return Err(Error::NulInString);
                }
                let len = u32::try_from(s.len()).map_err(|_| Error::StringTooLong(s.len()))?;
                let at;
                match self.layout.strs {
                    StrLayout::PrefixChar => {
                        at = self.region.len() as u32;
                        let c = f::prefix_char(len).ok_or(Error::StringTooLong(s.len()))?;
                        let mut buf = [0u8; 4];
                        self.region.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        self.region.extend_from_slice(s.as_bytes());
                    }
                    StrLayout::RefLen | StrLayout::Offsets => {
                        at = self.region.len() as u32;
                        self.region.extend_from_slice(s.as_bytes());
                        self.ends.push(self.region.len() as u32);
                    }
                    StrLayout::Nul => {
                        at = self.region.len() as u32;
                        self.region.extend_from_slice(s.as_bytes());
                        self.region.push(0);
                    }
                }
                let start = match self.layout.strs {
                    StrLayout::PrefixChar => self.region.len() as u32 - len,
                    _ => at,
                };
                self.placed[p][li] = Placed { off: at, len, idx: global, start };
                global += 1;
            }
            self.lists[p] = list;
            if p == 0 && self.layout.split {
                self.idents_len = self.region.len();
            }
        }
        self.replay = true;
        Ok(())
    }

    /// The value a StrRef carries (offset, or index for `Offsets`).
    fn ref_value(&self, p: Placed) -> i64 {
        if self.layout.strs == StrLayout::Offsets { i64::from(p.idx) } else { i64::from(p.off) }
    }

    /// Ref value of the next logged occurrence (pass 2), for a message base.
    fn peek_value(&self) -> i64 {
        if !self.replay {
            return 0;
        }
        match self.log.get(self.cursor) {
            Some(&(k, idx)) => self.ref_value(self.placed[self.pool(k)][idx]),
            None => 0,
        }
    }

    /// Writes a StrRef.
    fn strref(&mut self, k: PoolKind, s: &str, out: &mut Vec<u8>) -> Result<(), Error> {
        let pl = self.occ(k, s)?;
        let v = self.ref_value(pl);
        let enc = match self.base {
            Some(b) => zigzag(v - b),
            None => v as u64,
        };
        varint(enc, out);
        if self.layout.strs == StrLayout::RefLen {
            varint(u64::from(pl.len), out);
        }
        Ok(())
    }
}

/// Per-message encoding context.
struct Ctx<'m> {
    manifest: &'m Manifest,
    msg: usize,
    id: &'m str,
    locals: Vec<String>,
}

impl Ctx<'_> {
    fn varref(&self, name: &str) -> Result<u64, Error> {
        if let Some(i) = self.locals.iter().position(|l| l == name) {
            return Ok(((i as u64) << 1) | 1);
        }
        let slot = self
            .manifest
            .slot(self.msg, name)
            .ok_or_else(|| Error::UnknownVariable { id: self.id.to_owned(), var: name.to_owned() })?;
        Ok((slot as u64) << 1)
    }
}

struct W<'a> {
    strs: &'a mut Strings,
    cold: &'a mut Vec<u8>,
}

impl W<'_> {
    fn cold_attrs(&mut self, attrs: &[Attribute]) -> Result<u64, Error> {
        let saved = self.strs.base.take();
        let r = self.cold_attrs_abs(attrs);
        self.strs.base = saved;
        r
    }

    fn cold_attrs_abs(&mut self, attrs: &[Attribute]) -> Result<u64, Error> {
        let at = self.cold.len() as u64;
        let mut buf = Vec::new();
        varint(attrs.len() as u64, &mut buf);
        for a in attrs {
            self.strs.strref(PoolKind::Ident, &a.name, &mut buf)?;
            match &a.value {
                Some(v) => {
                    buf.push(1);
                    self.strs.strref(PoolKind::Text, v, &mut buf)?;
                }
                None => buf.push(0),
            }
        }
        self.cold.extend_from_slice(&buf);
        Ok(at)
    }

    fn value(&mut self, cx: &Ctx, v: &Operand, out: &mut Vec<u8>) -> Result<(), Error> {
        match v {
            Operand::Literal(s) => {
                out.push(0);
                self.strs.strref(PoolKind::Text, s, out)
            }
            Operand::Variable(n) => {
                varint((cx.varref(n)? << 1) | 1, out);
                Ok(())
            }
        }
    }

    fn options(&mut self, cx: &Ctx, opts: &[(String, Operand)], out: &mut Vec<u8>) -> Result<(), Error> {
        varint(opts.len() as u64, out);
        for (name, v) in opts {
            self.strs.strref(PoolKind::Ident, name, out)?;
            self.value(cx, v, out)?;
        }
        Ok(())
    }

    /// Expression tail after its header byte: operand, function, cold.
    fn expr_tail(&mut self, cx: &Ctx, e: &Expr, out: &mut Vec<u8>) -> Result<(), Error> {
        match &e.operand {
            None => {}
            Some(Operand::Literal(s)) => self.strs.strref(PoolKind::Text, s, out)?,
            Some(Operand::Variable(n)) => varint(cx.varref(n)?, out),
        }
        if let Some(func) = &e.function {
            let idx = cx.manifest.function(&func.name).ok_or(Error::Other("function not in manifest".into()))?;
            varint(idx as u64, out);
            self.options(cx, &func.options, out)?;
        }
        if !e.attributes.is_empty() {
            let at = self.cold_attrs(&e.attributes)?;
            varint(at, out);
        }
        Ok(())
    }

    fn expr_head(e: &Expr) -> u8 {
        let op = match &e.operand {
            None => part::OPERAND_NONE,
            Some(Operand::Literal(_)) => part::OPERAND_LITERAL,
            Some(Operand::Variable(_)) => part::OPERAND_VARIABLE,
        };
        let mut h = op << part::OPERAND_SHIFT;
        if e.function.is_some() {
            h |= part::HAS_FUNCTION;
        }
        if !e.attributes.is_empty() {
            h |= part::HAS_COLD;
        }
        h
    }

    fn markup(&mut self, cx: &Ctx, m: &Markup, out: &mut Vec<u8>) -> Result<(), Error> {
        let mut h = match m.kind {
            MarkupKind::Open => part::OPEN,
            MarkupKind::Standalone => part::STANDALONE,
            MarkupKind::Close => part::CLOSE,
        };
        if !m.options.is_empty() {
            h |= part::HAS_OPTIONS;
        }
        if !m.attributes.is_empty() {
            h |= part::HAS_COLD;
        }
        out.push(h);
        self.strs.strref(PoolKind::Ident, &m.name, out)?;
        if !m.options.is_empty() {
            self.options(cx, &m.options, out)?;
        }
        if !m.attributes.is_empty() {
            let at = self.cold_attrs(&m.attributes)?;
            varint(at, out);
        }
        Ok(())
    }

    fn pattern(&mut self, cx: &Ctx, p: &Pattern, out: &mut Vec<u8>) -> Result<(), Error> {
        varint(p.len() as u64, out);
        for part in p {
            match part {
                Part::Text(t) => {
                    out.push(part::TEXT);
                    self.strs.strref(PoolKind::Text, t, out)?;
                }
                Part::Expr(e) => {
                    out.push(part::EXPR | Self::expr_head(e));
                    self.expr_tail(cx, e, out)?;
                }
                Part::Markup(m) => self.markup(cx, m, out)?,
            }
        }
        Ok(())
    }

    fn message(&mut self, cx: &mut Ctx, msg: &Message, names_ref: u64, out: &mut Vec<u8>) -> Result<(), Error> {
        varint(names_ref, out);
        if self.strs.layout.rel {
            let b = self.strs.peek_value();
            varint(b as u64, out);
            self.strs.base = Some(b);
        }
        let r = self.message_body(cx, msg, out);
        self.strs.base = None;
        r
    }

    fn message_body(&mut self, cx: &mut Ctx, msg: &Message, out: &mut Vec<u8>) -> Result<(), Error> {
        let decls = msg.decls();
        varint(decls.len() as u64, out);
        for d in decls {
            match d {
                Decl::Input { name, expr } => {
                    out.push(Self::expr_head(expr));
                    let slot = cx
                        .manifest
                        .slot(cx.msg, name)
                        .ok_or_else(|| Error::UnknownVariable { id: cx.id.to_owned(), var: name.clone() })?;
                    varint(slot as u64, out);
                    self.expr_tail(cx, expr, out)?;
                }
                Decl::Local { name, expr } => {
                    out.push(f::decl::LOCAL | Self::expr_head(expr));
                    self.expr_tail(cx, expr, out)?;
                    cx.locals.push(name.clone());
                }
            }
        }
        match msg {
            Message::Pattern { pattern, .. } => self.pattern(cx, pattern, out),
            Message::Select { selectors, variants, .. } => {
                varint(selectors.len() as u64, out);
                for s in selectors {
                    varint(cx.varref(s)?, out);
                }
                varint(variants.len() as u64, out);
                for v in variants {
                    for k in &v.keys {
                        match k {
                            Key::CatchAll => out.push(0),
                            Key::Literal(s) => {
                                let n = crate::manifest::nfc(s);
                                if n == *s {
                                    out.push(1);
                                    self.strs.strref(PoolKind::Ident, s, out)?;
                                } else {
                                    // Key stored NFC; original spelling in COLD (plans/02 §2.2).
                                    out.push(2);
                                    self.strs.strref(PoolKind::Ident, &n, out)?;
                                    let at = self.cold.len() as u64;
                                    let mut buf = Vec::new();
                                    let saved = self.strs.base.take();
                                    self.strs.strref(PoolKind::Ident, s, &mut buf)?;
                                    self.strs.base = saved;
                                    self.cold.extend_from_slice(&buf);
                                    varint(at, out);
                                }
                            }
                        }
                    }
                    let mut pat = Vec::new();
                    self.pattern(cx, &v.pattern, &mut pat)?;
                    varint(pat.len() as u64, out);
                    out.extend_from_slice(&pat);
                }
                Ok(())
            }
        }
    }
}

/// Is this message `simple` (a single text run, no declarations)?
pub fn simple_text(msg: &Message) -> Option<&str> {
    match msg {
        Message::Pattern { decls, pattern } if decls.is_empty() => match pattern.as_slice() {
            [] => Some(""),
            [Part::Text(t)] => Some(t),
            _ => None,
        },
        _ => None,
    }
}

pub struct Input<'a> {
    pub locale: &'a str,
    pub rtl: bool,
    /// Messages in MsgId order; `None` = absent.
    pub messages: &'a [Option<&'a Message>],
    /// LOCALE entries: (key, payload).
    pub locale_entries: &'a [(u32, Vec<u8>)],
}

/// Section sizes of one written catalog (for the tables).
#[derive(Default, Clone, Debug)]
pub struct Sizes {
    pub header: usize,
    pub sections: Vec<(u16, usize)>,
    pub total: usize,
}

pub fn write(manifest: &Manifest, input: &Input, layout: Layout) -> Result<(Vec<u8>, Sizes), Error> {
    let mut strs = Strings::new(layout);
    let mut out = None;
    for pass in 0..2 {
        if pass == 1 {
            strs.finish()?;
        }
        out = Some(encode_structure(manifest, input, layout, &mut strs)?);
    }
    let st = out.ok_or(Error::Other("no output".into()))?;
    assemble(manifest, input, layout, &strs, st)
}

struct Structure {
    locale_ref: (u32, u32),
    index_payload: Vec<(u32, u32)>, // (kind, payload)
    slen: Vec<u16>,
    messages: Vec<u8>,
    cold: Vec<u8>,
    names: Vec<u8>,
    funcs: Vec<u8>,
}

fn encode_structure(manifest: &Manifest, input: &Input, layout: Layout, strs: &mut Strings) -> Result<Structure, Error> {
    let loc = strs.occ(PoolKind::Ident, input.locale)?;
    let mut funcs = Vec::new();
    varint(manifest.functions.len() as u64, &mut funcs);
    for fname in &manifest.functions {
        strs.strref(PoolKind::Ident, fname, &mut funcs)?;
    }
    let mut messages = Vec::new();
    let mut cold = Vec::new();
    let mut names = Vec::new();
    let mut names_dedup: HashMap<(Vec<String>, Vec<String>), u64> = HashMap::new();
    let mut index = Vec::with_capacity(input.messages.len());
    let mut slen = Vec::with_capacity(input.messages.len());
    for (i, m) in input.messages.iter().enumerate() {
        let id = manifest.ids[i].as_str();
        let Some(msg) = m else {
            index.push((kind::ABSENT, 0));
            slen.push(0);
            continue;
        };
        if let Some(t) = simple_text(msg) {
            let Placed { off, len, idx, .. } = strs.occ(PoolKind::Text, t)?;
            let payload = if layout.strs == StrLayout::Offsets { idx } else { off };
            index.push((kind::SIMPLE, payload));
            slen.push(u16::try_from(len).map_err(|_| Error::StringTooLong(len as usize))?);
            continue;
        }
        slen.push(0);
        // NAMES entry: external names by slot, then local names.
        let ext = manifest.slots[i].clone();
        let locals: Vec<String> = msg
            .decls()
            .iter()
            .filter_map(|d| match d {
                Decl::Local { name, .. } => Some(name.clone()),
                Decl::Input { .. } => None,
            })
            .collect();
        let names_ref = if ext.is_empty() && locals.is_empty() {
            0
        } else {
            let key = (ext.clone(), locals.clone());
            if let Some(&r) = names_dedup.get(&key).filter(|_| layout.dedup) {
                r
            } else {
                let at = names.len() as u64 + 1;
                varint(ext.len() as u64, &mut names);
                for n in &ext {
                    strs.strref(PoolKind::Ident, n, &mut names)?;
                }
                varint(locals.len() as u64, &mut names);
                for n in &locals {
                    strs.strref(PoolKind::Ident, n, &mut names)?;
                }
                names_dedup.insert(key, at);
                at
            }
        };
        let k = if matches!(msg, Message::Select { .. }) { kind::SELECT } else { kind::PATTERN };
        let at = u32::try_from(messages.len()).map_err(|_| Error::OffsetTooLarge(messages.len()))?;
        let mut cx = Ctx { manifest, msg: i, id, locals: Vec::new() };
        W { strs, cold: &mut cold }.message(&mut cx, msg, names_ref, &mut messages)?;
        index.push((k, at));
    }
    Ok(Structure { locale_ref: (loc.start, loc.len), index_payload: index, slen, messages, cold, names, funcs })
}

fn encode_index(layout: Layout, entries: &[(u32, u32)]) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let mut out = Vec::new();
    let mut blocks = Vec::new();
    match layout.index {
        IndexLayout::Fixed => {
            for &(k, p) in entries {
                if p > kind::OFFSET_MASK {
                    return Err(Error::OffsetTooLarge(p as usize));
                }
                out.extend_from_slice(&((k << kind::SHIFT) | p).to_le_bytes());
            }
        }
        IndexLayout::Planes => {
            let words: Vec<u32> = entries
                .iter()
                .map(|&(k, p)| if p > kind::OFFSET_MASK { Err(Error::OffsetTooLarge(p as usize)) } else { Ok((k << kind::SHIFT) | p) })
                .collect::<Result<_, _>>()?;
            for plane in 0..4 {
                out.extend(words.iter().map(|w| w.to_le_bytes()[plane]));
            }
        }
        IndexLayout::VarintDelta | IndexLayout::Blocked => {
            let mut prev = [0i64; 2];
            for (i, &(k, p)) in entries.iter().enumerate() {
                if layout.index == IndexLayout::Blocked && i % 16 == 0 {
                    blocks.extend_from_slice(&(out.len() as u32).to_le_bytes());
                    prev = [0, 0];
                }
                let class = usize::from(k != kind::SIMPLE);
                let d = if k == kind::ABSENT { 0 } else { i64::from(p) - prev[class] };
                if k != kind::ABSENT {
                    prev[class] = i64::from(p);
                }
                varint((zigzag(d) << 2) | u64::from(k), &mut out);
            }
        }
    }
    Ok((out, blocks))
}

fn encode_ids(ids: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut prev: &str = "";
    for id in ids {
        let shared = prev.bytes().zip(id.bytes()).take_while(|(a, b)| a == b).count();
        varint(shared as u64, &mut out);
        let suffix = &id.as_bytes()[shared..];
        varint(suffix.len() as u64, &mut out);
        out.extend_from_slice(suffix);
        prev = id;
    }
    out
}

fn assemble(manifest: &Manifest, input: &Input, layout: Layout, strs: &Strings, st: Structure) -> Result<(Vec<u8>, Sizes), Error> {
    let (index, blocks) = encode_index(layout, &st.index_payload)?;
    let mut locale = Vec::new();
    varint(input.locale_entries.len() as u64, &mut locale);
    for (key, payload) in input.locale_entries {
        varint(u64::from(*key), &mut locale);
        varint(payload.len() as u64, &mut locale);
        locale.extend_from_slice(payload);
    }
    let mut secs: Vec<(u16, Vec<u8>)> = vec![(section::INDEX, index)];
    if layout.index == IndexLayout::Blocked {
        secs.push((section::INDEX_BLOCKS, blocks));
    }
    if layout.strs == StrLayout::RefLen {
        secs.push((section::SLEN, st.slen.iter().flat_map(|l| l.to_le_bytes()).collect()));
    }
    secs.push((section::MESSAGES, st.messages));
    if !layout.strip && !st.cold.is_empty() {
        secs.push((section::COLD, st.cold));
    }
    secs.push((section::NAMES, st.names));
    secs.push((section::FUNCS, st.funcs));
    secs.push((section::LOCALE, locale));
    if !layout.strip {
        secs.push((section::IDS, encode_ids(&manifest.ids)));
    }
    if layout.strs == StrLayout::Offsets {
        secs.push((section::STROFF, strs.ends.iter().flat_map(|e| e.to_le_bytes()).collect()));
    }
    // One pool section; with `split` the identifiers are grouped first in it.
    secs.push((section::STRINGS, strs.region.clone()));

    let header_len = f::header::SECTIONS + secs.len() * f::header::SECTION_ENTRY;
    let mut out = Vec::with_capacity(header_len + secs.iter().map(|s| s.1.len()).sum::<usize>());
    out.extend_from_slice(&f::MAGIC);
    out.extend_from_slice(&f::VERSION.to_le_bytes());
    out.extend_from_slice(&layout.flags().to_le_bytes());
    out.extend_from_slice(&manifest.hash.to_le_bytes());
    out.push(0); // chunk
    out.push(u8::from(input.rtl));
    out.extend_from_slice(&(input.messages.len() as u32).to_le_bytes());
    out.extend_from_slice(&st.locale_ref.0.to_le_bytes());
    out.extend_from_slice(&(st.locale_ref.1 as u16).to_le_bytes());
    out.extend_from_slice(&(secs.len() as u16).to_le_bytes());
    let mut off = header_len;
    for (k, data) in &secs {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&(off as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        off += data.len();
    }
    debug_assert_eq!(out.len(), header_len);
    let mut sizes = Sizes { header: header_len, ..Default::default() };
    for (k, data) in secs {
        sizes.sections.push((k, data.len()));
        out.extend_from_slice(&data);
    }
    sizes.total = out.len();
    Ok((out, sizes))
}

pub fn section_name(k: u16) -> &'static str {
    match k {
        section::INDEX => "INDEX",
        section::MESSAGES => "MESSAGES",
        section::COLD => "COLD",
        section::NAMES => "NAMES",
        section::FALLBACK => "FALLBACK",
        section::LOCALE => "LOCALE",
        section::FUNCS => "FUNCS",
        section::IDS => "IDS",
        section::SLEN => "SLEN",
        section::STROFF => "STROFF",
        section::INDEX_BLOCKS => "INDEX_BLOCKS",
        section::IDENTS => "IDENTS",
        section::STRINGS => "STRINGS",
        _ => "?",
    }
}
