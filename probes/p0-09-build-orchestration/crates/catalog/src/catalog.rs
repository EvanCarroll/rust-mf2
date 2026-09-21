//! The per-locale catalog file (`<tag>.<content-hash>.mf2b`) — a probe-sized
//! encoding, not plans/02's format: header, then one optional body per `MsgId`.

use crate::error::DecodeError;
use crate::model::{Body, Func, Key, Part, Selector, Variant};
use crate::wire::{Reader, Writer};

const MAGIC: &str = "P09C";
const VERSION: u8 = 1;

/// One message in a catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogMessage {
    /// The body.
    pub body: Body,
    /// `Some(locale)` if the text was borrowed from a fallback locale (D5).
    pub fallback: Option<String>,
}

/// A decoded catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// The manifest this catalog was built against.
    pub manifest_hash: u64,
    /// Locale tag.
    pub locale: String,
    /// Right-to-left?
    pub rtl: bool,
    /// Indexed by `MsgId`; `None` = absent.
    pub messages: Vec<Option<CatalogMessage>>,
}

fn put_parts(w: &mut Writer, parts: &[Part]) {
    w.len(parts.len());
    for p in parts {
        match p {
            Part::Text(s) => {
                w.u8(0);
                w.str(s);
            }
            Part::Var(slot) => {
                w.u8(1);
                w.u32(*slot);
            }
            Part::Lit(s) => {
                w.u8(2);
                w.str(s);
            }
            Part::MarkupOpen(s) => {
                w.u8(3);
                w.str(s);
            }
            Part::MarkupClose(s) => {
                w.u8(4);
                w.str(s);
            }
            Part::MarkupStandalone(s) => {
                w.u8(5);
                w.str(s);
            }
        }
    }
}

fn get_parts(r: &mut Reader<'_>) -> Result<Vec<Part>, DecodeError> {
    let n = r.len()?;
    let mut parts = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        parts.push(match r.u8()? {
            0 => Part::Text(r.str()?.to_owned()),
            1 => Part::Var(r.u32()?),
            2 => Part::Lit(r.str()?.to_owned()),
            3 => Part::MarkupOpen(r.str()?.to_owned()),
            4 => Part::MarkupClose(r.str()?.to_owned()),
            5 => Part::MarkupStandalone(r.str()?.to_owned()),
            t => return Err(DecodeError::Tag(t)),
        });
    }
    Ok(parts)
}

impl Catalog {
    /// Serializes.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.buf.extend_from_slice(MAGIC.as_bytes());
        w.u8(VERSION);
        w.u64(self.manifest_hash);
        w.str(&self.locale);
        w.u8(u8::from(self.rtl));
        w.len(self.messages.len());
        for m in &self.messages {
            let Some(m) = m else {
                w.u8(0);
                continue;
            };
            match &m.fallback {
                Some(f) => {
                    w.u8(2);
                    w.str(f);
                }
                None => w.u8(1),
            }
            match &m.body {
                Body::Pattern(parts) => {
                    w.u8(0);
                    put_parts(&mut w, parts);
                }
                Body::Select {
                    selectors,
                    variants,
                } => {
                    w.u8(1);
                    w.len(selectors.len());
                    for s in selectors {
                        w.u32(s.slot);
                        w.u8(match s.func {
                            Func::Plain => 0,
                            Func::Integer => 1,
                            Func::Number => 2,
                        });
                    }
                    w.len(variants.len());
                    for v in variants {
                        for k in &v.keys {
                            match k {
                                Key::Star => w.u8(0),
                                Key::Lit(s) => {
                                    w.u8(1);
                                    w.str(s);
                                }
                            }
                        }
                        put_parts(&mut w, &v.pattern);
                    }
                }
            }
        }
        w.buf
    }

    /// Deserializes.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        r.magic(MAGIC)?;
        let v = r.u8()?;
        if v != VERSION {
            return Err(DecodeError::Version(v));
        }
        let manifest_hash = r.u64()?;
        let locale = r.str()?.to_owned();
        let rtl = r.u8()? != 0;
        let n = r.len()?;
        let mut messages = Vec::with_capacity(n.min(1 << 20));
        for _ in 0..n {
            let fallback = match r.u8()? {
                0 => {
                    messages.push(None);
                    continue;
                }
                1 => None,
                2 => Some(r.str()?.to_owned()),
                t => return Err(DecodeError::Tag(t)),
            };
            let body = match r.u8()? {
                0 => Body::Pattern(get_parts(&mut r)?),
                1 => {
                    let ns = r.len()?;
                    let mut selectors = Vec::with_capacity(ns.min(16));
                    for _ in 0..ns {
                        let slot = r.u32()?;
                        let func = match r.u8()? {
                            0 => Func::Plain,
                            1 => Func::Integer,
                            2 => Func::Number,
                            t => return Err(DecodeError::Tag(t)),
                        };
                        selectors.push(Selector { slot, func });
                    }
                    let nv = r.len()?;
                    let mut variants = Vec::with_capacity(nv.min(64));
                    for _ in 0..nv {
                        let mut keys = Vec::with_capacity(ns);
                        for _ in 0..ns {
                            keys.push(match r.u8()? {
                                0 => Key::Star,
                                1 => Key::Lit(r.str()?.to_owned()),
                                t => return Err(DecodeError::Tag(t)),
                            });
                        }
                        variants.push(Variant {
                            keys,
                            pattern: get_parts(&mut r)?,
                        });
                    }
                    Body::Select {
                        selectors,
                        variants,
                    }
                }
                t => return Err(DecodeError::Tag(t)),
            };
            messages.push(Some(CatalogMessage { body, fallback }));
        }
        Ok(Self {
            manifest_hash,
            locale,
            rtl,
            messages,
        })
    }
}
