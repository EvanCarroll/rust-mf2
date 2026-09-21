//! Model-rebuilding decoder (build side, every layout variant). Used to prove
//! the encoding lossless on the workload (conformance L3 in miniature).

use mf2b_format::{self as f, flags, kind, part, section};

use crate::error::Error;
use crate::model::{Attribute, Decl, Expr, FunctionRef, Key, Markup, MarkupKind, Message, Operand, Part, Pattern, Variant};

pub struct Decoded {
    pub locale: String,
    pub rtl: bool,
    pub manifest_hash: u64,
    pub messages: Vec<Option<Message>>,
    pub ids: Option<Vec<String>>,
    pub locale_entries: Vec<(u32, Vec<u8>)>,
}

struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn u8(&mut self) -> Result<u8, Error> {
        let v = *self.b.get(self.i).ok_or(Error::Decode("eof"))?;
        self.i += 1;
        Ok(v)
    }
    fn var(&mut self) -> Result<u64, Error> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 63 {
                return Err(Error::Decode("varint"));
            }
        }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let s = self.b.get(self.i..self.i + n).ok_or(Error::Decode("eof"))?;
        self.i += n;
        Ok(s)
    }
}

fn u16le(b: &[u8], at: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(b.get(at..at + 2).ok_or(Error::Decode("header"))?.try_into().unwrap()))
}
fn u32le(b: &[u8], at: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(b.get(at..at + 4).ok_or(Error::Decode("header"))?.try_into().unwrap()))
}

struct Ctx<'a> {
    fl: u16,
    region: &'a str,
    stroff: Vec<u32>,
    funcs: Vec<String>,
    names: &'a [u8],
    cold: &'a [u8],
}

impl Ctx<'_> {
    fn str_at(&self, off: usize, len: usize) -> Result<String, Error> {
        Ok(self.region.get(off..off + len).ok_or(Error::Decode("strref"))?.to_owned())
    }

    /// Text of the string whose INDEX / prefix offset is `off` (PrefixChar / Nul).
    fn str_off(&self, off: usize) -> Result<String, Error> {
        match self.fl & flags::STR_MASK {
            flags::STR_PREFIX_CHAR => {
                let rest = self.region.get(off..).ok_or(Error::Decode("strref"))?;
                let c = rest.chars().next().ok_or(Error::Decode("prefix"))?;
                let len = f::prefix_len(c as u32) as usize;
                self.str_at(off + c.len_utf8(), len)
            }
            flags::STR_NUL => {
                let rest = self.region.get(off..).ok_or(Error::Decode("strref"))?;
                let end = rest.find('\0').ok_or(Error::Decode("nul"))?;
                Ok(rest[..end].to_owned())
            }
            _ => Err(Error::Decode("str_off")),
        }
    }

    fn str_idx(&self, idx: usize) -> Result<String, Error> {
        let end = *self.stroff.get(idx).ok_or(Error::Decode("stroff"))? as usize;
        let start = if idx == 0 { 0 } else { self.stroff[idx - 1] as usize };
        self.str_at(start, end - start)
    }

    fn strref(&self, c: &mut Cur) -> Result<String, Error> {
        self.strref_rel(c, None)
    }

    fn strref_rel(&self, c: &mut Cur, base: Option<i64>) -> Result<String, Error> {
        let raw = c.var()?;
        let v = match base {
            Some(b) => b + (((raw >> 1) as i64) ^ -((raw & 1) as i64)),
            None => raw as i64,
        } as usize;
        match self.fl & flags::STR_MASK {
            flags::STR_PREFIX_CHAR | flags::STR_NUL => self.str_off(v),
            flags::STR_REF_LEN => {
                let len = c.var()? as usize;
                self.str_at(v, len)
            }
            _ => self.str_idx(v),
        }
    }

    fn names(&self, names_ref: u64) -> Result<(Vec<String>, Vec<String>), Error> {
        if names_ref == 0 {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut c = Cur { b: self.names, i: names_ref as usize - 1 };
        let n = c.var()?;
        let ext = (0..n).map(|_| self.strref(&mut c)).collect::<Result<Vec<_>, _>>()?;
        let n = c.var()?;
        let loc = (0..n).map(|_| self.strref(&mut c)).collect::<Result<Vec<_>, _>>()?;
        Ok((ext, loc))
    }

    fn attrs(&self, at: u64) -> Result<Vec<Attribute>, Error> {
        let mut c = Cur { b: self.cold, i: at as usize };
        let n = c.var()?;
        (0..n)
            .map(|_| {
                let name = self.strref(&mut c)?;
                let value = if c.u8()? == 1 { Some(self.strref(&mut c)?) } else { None };
                Ok(Attribute { name, value })
            })
            .collect()
    }
}

struct Msg<'a> {
    ext: Vec<String>,
    loc: Vec<String>,
    cx: &'a Ctx<'a>,
    base: Option<i64>,
}

impl Msg<'_> {
    fn sref(&self, c: &mut Cur) -> Result<String, Error> {
        self.cx.strref_rel(c, self.base)
    }
    fn var(&self, r: u64) -> Result<String, Error> {
        let list = if r & 1 == 1 { &self.loc } else { &self.ext };
        list.get((r >> 1) as usize).cloned().ok_or(Error::Decode("varref"))
    }
    fn value(&self, c: &mut Cur) -> Result<Operand, Error> {
        let t = c.var()?;
        if t == 0 { Ok(Operand::Literal(self.sref(c)?)) } else { Ok(Operand::Variable(self.var(t >> 1)?)) }
    }
    fn options(&self, c: &mut Cur) -> Result<Vec<(String, Operand)>, Error> {
        let n = c.var()?;
        (0..n).map(|_| Ok((self.sref(c)?, self.value(c)?))).collect()
    }
    fn expr(&self, h: u8, c: &mut Cur) -> Result<Expr, Error> {
        let operand = match (h & part::OPERAND_MASK) >> part::OPERAND_SHIFT {
            part::OPERAND_NONE => None,
            part::OPERAND_LITERAL => Some(Operand::Literal(self.sref(c)?)),
            _ => Some(Operand::Variable(self.var(c.var()?)?)),
        };
        let function = if h & part::HAS_FUNCTION != 0 {
            let idx = c.var()? as usize;
            let name = self.cx.funcs.get(idx).cloned().ok_or(Error::Decode("fn idx"))?;
            Some(FunctionRef { name, options: self.options(c)? })
        } else {
            None
        };
        let attributes = if h & part::HAS_COLD != 0 { self.cx.attrs(c.var()?)? } else { Vec::new() };
        Ok(Expr { operand, function, attributes })
    }
    fn pattern(&self, c: &mut Cur) -> Result<Pattern, Error> {
        let n = c.var()?;
        let mut out = Vec::new();
        for _ in 0..n {
            let h = c.u8()?;
            out.push(match h & part::KIND_MASK {
                part::TEXT => Part::Text(self.sref(c)?),
                part::EXPR => Part::Expr(self.expr(h, c)?),
                k => {
                    let kind = match k {
                        part::OPEN => MarkupKind::Open,
                        part::STANDALONE => MarkupKind::Standalone,
                        _ => MarkupKind::Close,
                    };
                    let name = self.sref(c)?;
                    let options = if h & part::HAS_OPTIONS != 0 { self.options(c)? } else { Vec::new() };
                    let attributes = if h & part::HAS_COLD != 0 { self.cx.attrs(c.var()?)? } else { Vec::new() };
                    Part::Markup(Markup { kind, name, options, attributes })
                }
            });
        }
        Ok(out)
    }
}

fn message(cx: &Ctx, messages: &[u8], at: usize, select: bool) -> Result<Message, Error> {
    let mut c = Cur { b: messages, i: at };
    let names_ref = c.var()?;
    let (ext, loc) = cx.names(names_ref)?;
    let base = if cx.fl & flags::REL_REFS != 0 { Some(c.var()? as i64) } else { None };
    let mut m = Msg { ext, loc: Vec::new(), cx, base };
    let n = c.var()?;
    let mut decls = Vec::new();
    for _ in 0..n {
        let h = c.u8()?;
        if h & f::decl::LOCAL != 0 {
            let expr = m.expr(h & !f::decl::LOCAL, &mut c)?;
            let name = loc.get(m.loc.len()).cloned().ok_or(Error::Decode("local name"))?;
            m.loc.push(name.clone());
            decls.push(Decl::Local { name, expr });
        } else {
            let slot = c.var()? as usize;
            let name = m.ext.get(slot).cloned().ok_or(Error::Decode("slot"))?;
            let expr = m.expr(h, &mut c)?;
            decls.push(Decl::Input { name, expr });
        }
    }
    if !select {
        let pattern = m.pattern(&mut c)?;
        return Ok(Message::Pattern { decls, pattern });
    }
    let ns = c.var()?;
    let selectors = (0..ns).map(|_| m.var(c.var()?)).collect::<Result<Vec<_>, _>>()?;
    let nv = c.var()?;
    let mut variants = Vec::new();
    for _ in 0..nv {
        let mut keys = Vec::new();
        for _ in 0..ns {
            keys.push(match c.u8()? {
                0 => Key::CatchAll,
                1 => Key::Literal(m.sref(&mut c)?),
                _ => {
                    let _nfc = m.sref(&mut c)?;
                    let at = c.var()? as usize;
                    let mut cc = Cur { b: cx.cold, i: at };
                    Key::Literal(cx.strref(&mut cc)?)
                }
            });
        }
        let len = c.var()? as usize;
        let body = c.take(len)?;
        let pattern = m.pattern(&mut Cur { b: body, i: 0 })?;
        variants.push(Variant { keys, pattern });
    }
    Ok(Message::Select { decls, selectors, variants })
}

pub fn decode(b: &[u8]) -> Result<Decoded, Error> {
    if b.get(..4) != Some(&f::MAGIC[..]) || u16le(b, 4)? != f::VERSION {
        return Err(Error::Decode("magic/version"));
    }
    let fl = u16le(b, f::header::FLAGS)?;
    let manifest_hash = u64::from_le_bytes(b[8..16].try_into().unwrap());
    let rtl = b[f::header::DIR] == 1;
    let count = u32le(b, f::header::MESSAGE_COUNT)? as usize;
    let loc_off = u32le(b, f::header::LOCALE_OFF)? as usize;
    let loc_len = u16le(b, f::header::LOCALE_LEN)? as usize;
    let nsec = u16le(b, f::header::SECTION_COUNT)? as usize;
    let mut secs = std::collections::HashMap::new();
    for s in 0..nsec {
        let at = f::header::SECTIONS + s * f::header::SECTION_ENTRY;
        let k = u16le(b, at)?;
        let off = u32le(b, at + 2)? as usize;
        let len = u32le(b, at + 6)? as usize;
        secs.insert(k, b.get(off..off + len).ok_or(Error::Decode("section bounds"))?);
    }
    let get = |k: u16| secs.get(&k).copied().unwrap_or(&[]);
    let region_start = {
        let s = get(section::STRINGS);
        s.as_ptr() as usize - b.as_ptr() as usize
    };
    let region = std::str::from_utf8(&b[region_start..]).map_err(|_| Error::Decode("pool utf-8"))?;
    let stroff = get(section::STROFF).chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    let mut cx = Ctx { fl, region, stroff, funcs: Vec::new(), names: get(section::NAMES), cold: get(section::COLD) };
    let mut fc = Cur { b: get(section::FUNCS), i: 0 };
    let nf = fc.var()?;
    cx.funcs = (0..nf).map(|_| cx.strref(&mut fc)).collect::<Result<Vec<_>, _>>()?;
    let locale = cx.str_at(loc_off, loc_len)?;

    // INDEX → (kind, payload) per message.
    let index = get(section::INDEX);
    let mut entries = Vec::with_capacity(count);
    match fl & flags::INDEX_MASK {
        flags::INDEX_PLANES => {
            let n = index.len() / 4;
            for i in 0..n {
                let v = u32::from_le_bytes([index[i], index[n + i], index[2 * n + i], index[3 * n + i]]);
                entries.push((v >> kind::SHIFT, v & kind::OFFSET_MASK));
            }
        }
        flags::INDEX_FIXED => {
            for c in index.chunks_exact(4) {
                let v = u32::from_le_bytes(c.try_into().unwrap());
                entries.push((v >> kind::SHIFT, v & kind::OFFSET_MASK));
            }
        }
        _ => {
            let blocked = fl & flags::INDEX_MASK == flags::INDEX_BLOCKED;
            let mut c = Cur { b: index, i: 0 };
            let mut prev = [0i64; 2];
            for i in 0..count {
                if blocked && i % 16 == 0 {
                    prev = [0, 0];
                }
                let v = c.var()?;
                let k = (v & 3) as u32;
                let z = v >> 2;
                let d = ((z >> 1) as i64) ^ -((z & 1) as i64);
                let class = usize::from(k != kind::SIMPLE);
                let p = if k == kind::ABSENT { 0 } else { prev[class] + d };
                if k != kind::ABSENT {
                    prev[class] = p;
                }
                entries.push((k, p as u32));
            }
        }
    }
    if entries.len() != count {
        return Err(Error::Decode("index count"));
    }
    let slen: Vec<u16> = get(section::SLEN).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let messages_sec = get(section::MESSAGES);
    let mut messages = Vec::with_capacity(count);
    for (i, &(k, p)) in entries.iter().enumerate() {
        messages.push(match k {
            kind::SIMPLE => {
                let text = match fl & flags::STR_MASK {
                    flags::STR_REF_LEN => cx.str_at(p as usize, *slen.get(i).ok_or(Error::Decode("slen"))? as usize)?,
                    flags::STR_OFFSETS => cx.str_idx(p as usize)?,
                    _ => cx.str_off(p as usize)?,
                };
                let pattern = if text.is_empty() { Vec::new() } else { vec![Part::Text(text)] };
                Some(Message::Pattern { decls: Vec::new(), pattern })
            }
            kind::PATTERN => Some(message(&cx, messages_sec, p as usize, false)?),
            kind::SELECT => Some(message(&cx, messages_sec, p as usize, true)?),
            _ => None,
        });
    }
    let ids = secs.get(&section::IDS).map(|ids| {
        let mut c = Cur { b: ids, i: 0 };
        let mut out: Vec<String> = Vec::with_capacity(count);
        let mut prev = String::new();
        for _ in 0..count {
            let shared = c.var().unwrap_or(0) as usize;
            let len = c.var().unwrap_or(0) as usize;
            let suffix = c.take(len).unwrap_or(&[]);
            let mut s = prev[..shared.min(prev.len())].to_owned();
            s.push_str(std::str::from_utf8(suffix).unwrap_or(""));
            prev = s.clone();
            out.push(s);
        }
        out
    });
    let mut lc = Cur { b: get(section::LOCALE), i: 0 };
    let n = lc.var().unwrap_or(0);
    let mut locale_entries = Vec::new();
    for _ in 0..n {
        let k = lc.var()? as u32;
        let len = lc.var()? as usize;
        locale_entries.push((k, lc.take(len)?.to_vec()));
    }
    Ok(Decoded { locale, rtl, manifest_hash, messages, ids, locale_entries })
}
