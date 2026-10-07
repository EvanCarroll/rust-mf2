//! B12 harness for the catalog reader (`mf2-catalog`, no features — the
//! client path): the `b12-base` scaffolding plus `Catalog::new` and every
//! public reader API, with a full view walk over every message — every
//! accessor, every iterator, every variant.
//!
//! The catalog bytes, the manifest hash, the lookup key and the odd ids and
//! keys come from imports, so nothing is constant-folded. Every result is
//! folded into one accumulator (numbers, flags, string lengths — a string's
//! `Some`/`None` already depends on its UTF-8 check) that goes to the host's
//! sink, so nothing is dead, while the harness's own code stays small: the
//! size delta against `b12-base` is mostly the reader. What survives LTO +
//! `wasm-opt -Oz` is the reader as a client links it, and `check.sh` proves it
//! holds no panic path (the panic import is absent) and no `core::fmt`.
#![no_std]
#![allow(clippy::cast_possible_truncation)]

extern crate alloc;

use alloc::vec::Vec;

use b12_harness::{INPUT_CATALOG, INPUT_KEY, input, param, sink, sink_bytes};
use mf2_catalog::format::locale_key;
use mf2_catalog::{
    Body, Catalog, CatalogError, CldrVersion, DeclView, Entry, ExprView, KeyView, Malformed,
    MarkupView, MsgId, MsgView, Names, Operand, OptionsView, PartView, PatternView, SelectView,
    StrRef, VarRef,
};

/// `Catalog::new`, kept out of line so that `twiggy` attributes its size.
#[inline(never)]
fn load(bytes: Vec<u8>, manifest: u64) -> Result<Catalog, CatalogError> {
    Catalog::new(bytes, manifest)
}

/// A number per `CatalogError` variant, without `as` (the enum is
/// `#[non_exhaustive]`) and without formatting.
fn error_code(e: CatalogError) -> u64 {
    match e {
        CatalogError::Magic => 1,
        CatalogError::Version => 2,
        CatalogError::ManifestMismatch => 3,
        CatalogError::Truncated => 4,
        CatalogError::Header => 5,
        CatalogError::SectionTable => 6,
        CatalogError::MissingSection => 7,
        CatalogError::Index => 8,
        CatalogError::Locale => 9,
        CatalogError::Funcs => 10,
        CatalogError::Fallback => 11,
        CatalogError::Names => 12,
        CatalogError::Ids => 13,
        CatalogError::Strings => 14,
        _ => 15,
    }
}

/// Loads the host's catalog and exercises the whole reader.
#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    let Some(bytes) = input(INPUT_CATALOG) else {
        return 1;
    };
    let Some(key) = input(INPUT_KEY) else {
        return 1;
    };
    let cat = match load(bytes, param(0)) {
        Ok(c) => c,
        Err(e) => {
            sink(error_code(e));
            return 2;
        }
    };
    let mut w = Walk { cat: &cat, acc: 0 };
    w.header();
    w.tables();
    w.messages();
    if let Ok(key) = core::str::from_utf8(&key) {
        w.id(cat.lookup(key));
    }
    w.id(cat.lookup(cat.locale()));
    sink(w.acc);
    let bytes = cat.into_bytes();
    sink_bytes(&key);
    sink_bytes(&bytes);
    0
}

/// The walk: the catalog and the accumulator every result is folded into.
struct Walk<'a> {
    cat: &'a Catalog,
    acc: u64,
}

impl Walk<'_> {
    #[inline(never)]
    fn n(&mut self, v: u64) {
        self.acc = self.acc.rotate_left(7) ^ v;
    }

    fn flag(&mut self, v: bool) {
        self.n(u64::from(v));
    }

    #[inline(never)]
    fn str(&mut self, s: Option<&str>) {
        self.n(s.map_or(u64::MAX, |s| s.len() as u64));
    }

    fn text(&mut self, r: StrRef) {
        let s = self.cat.text(r);
        self.str(s);
    }

    fn id(&mut self, id: Option<MsgId>) {
        self.n(id.map_or(u64::MAX, |id| u64::from(id.raw())));
    }

    fn malformed(&mut self, _: Malformed) {
        self.n(u64::MAX - 1);
    }

    /// The header accessors and the section table.
    fn header(&mut self) {
        let cat = self.cat;
        self.n(u64::from(cat.format_version()));
        self.n(cat.manifest_hash());
        self.str(Some(cat.locale()));
        self.n(cat.dir() as u64);
        self.n(u64::from(cat.chunk()));
        match cat.cldr_version() {
            Some(v) => {
                self.n(u64::from(v.to_u32()));
                self.n(u64::from(v.major) ^ u64::from(v.minor) ^ u64::from(v.patch));
            }
            None => self.n(0),
        }
        self.n(CldrVersion::from_u32(param(3) as u32).map_or(0, |v| u64::from(v.to_u32())));
        self.flag(cat.cold_stripped());
        self.flag(cat.ids_stripped());
        self.n(u64::from(cat.message_count()));
        self.n(cat.as_bytes().len() as u64);
        for (kind, off, len) in cat.sections() {
            self.n(u64::from(kind) ^ u64::from(off) ^ u64::from(len));
        }
    }

    /// FUNCS and LOCALE (one index past the end, and a key from the host).
    fn tables(&mut self) {
        let cat = self.cat;
        let n = cat.function_count();
        self.n(u64::from(n));
        for i in 0..=n {
            self.str(cat.function(i));
        }
        for key in [
            locale_key::PLURAL_CARDINAL,
            locale_key::PLURAL_ORDINAL,
            param(1) as u32,
        ] {
            self.n(cat.locale_entry(key).map_or(u64::MAX, |p| p.len() as u64));
        }
    }

    /// Every id of the catalog's chunk, one past the end, and one from the
    /// host (any chunk).
    fn messages(&mut self) {
        let chunk = self.cat.chunk();
        for i in 0..=self.cat.message_count() {
            if let Some(id) = MsgId::new(chunk, i) {
                self.message(id);
            }
        }
        self.message(MsgId::from_raw(param(2) as u32));
    }

    fn message(&mut self, id: MsgId) {
        let cat = self.cat;
        self.str(cat.fallback_locale(id));
        self.names(cat.names(id));
        match cat.get(id) {
            Entry::Simple(r) => self.text(r),
            Entry::Pattern(m) | Entry::Select(m) => self.msg_view(m),
            Entry::Absent => self.n(3),
        }
    }

    /// A name table: every entry, plus one past each end.
    fn names(&mut self, n: Names<'_>) {
        self.n(u64::from(n.external_count()) << 32 | u64::from(n.local_count()));
        for s in 0..=n.external_count() {
            let s = n.external(s).and_then(|r| self.cat.text(r));
            self.str(s);
        }
        for i in 0..=n.local_count() {
            let s = n.local(i).and_then(|r| self.cat.text(r));
            self.str(s);
        }
    }

    fn msg_view(&mut self, m: MsgView<'_>) {
        let names = m.names();
        self.flag(m.is_select());
        self.flag(core::ptr::eq(m.catalog(), self.cat));
        self.names(names);
        let mut decls = m.declarations();
        self.n(u64::from(decls.len()));
        self.flag(decls.is_empty());
        for d in &mut decls {
            match d {
                Ok(DeclView::Input(e)) => {
                    self.n(1);
                    self.expr(names, e);
                }
                Ok(DeclView::Local { index, expr }) => {
                    self.n(u64::from(index));
                    self.expr(names, expr);
                }
                Err(e) => self.malformed(e),
            }
        }
        // The body after an exhausted walk, and through `MsgView::body`.
        match decls.body() {
            Ok(b) => self.body(names, b),
            Err(e) => self.malformed(e),
        }
        match m.body() {
            Ok(b) => self.body(names, b),
            Err(e) => self.malformed(e),
        }
    }

    fn var(&mut self, names: Names<'_>, v: VarRef) {
        match v {
            VarRef::External(s) => self.n(u64::from(s) << 1),
            VarRef::Local(i) => self.n((u64::from(i) << 1) | 1),
        }
        let s = names.var(v).and_then(|r| self.cat.text(r));
        self.str(s);
    }

    fn operand(&mut self, names: Names<'_>, o: Operand) {
        match o {
            Operand::Literal(r) => self.text(r),
            Operand::Variable(v) => self.var(names, v),
        }
    }

    fn expr(&mut self, names: Names<'_>, e: ExprView<'_>) {
        match e.operand() {
            Some(o) => self.operand(names, o),
            None => self.n(0),
        }
        if let Some(f) = e.function() {
            self.n(u64::from(f.index()));
            let s = self.cat.function(f.index());
            self.str(s);
            self.options(names, f.options());
        }
    }

    fn options(&mut self, names: Names<'_>, o: OptionsView<'_>) {
        self.n(u64::from(o.len()));
        self.flag(o.is_empty());
        for r in o {
            match r {
                Ok((name, value)) => {
                    self.text(name);
                    self.operand(names, value);
                }
                Err(e) => self.malformed(e),
            }
        }
    }

    fn markup(&mut self, names: Names<'_>, m: MarkupView<'_>) {
        self.n(m.kind() as u64);
        self.text(m.name());
        self.options(names, m.options());
    }

    fn pattern(&mut self, names: Names<'_>, p: PatternView<'_>) {
        self.n(u64::from(p.len()));
        self.flag(p.is_empty());
        for part in p.parts() {
            match part {
                Ok(PartView::Text(r)) => self.text(r),
                Ok(PartView::Expression(e)) => self.expr(names, e),
                Ok(PartView::Markup(m)) => self.markup(names, m),
                Err(e) => self.malformed(e),
            }
        }
    }

    fn select(&mut self, names: Names<'_>, s: SelectView<'_>) {
        let selectors = s.selectors();
        self.n(u64::from(selectors.len()));
        self.flag(selectors.is_empty());
        for r in selectors {
            match r {
                Ok(v) => self.var(names, v),
                Err(e) => self.malformed(e),
            }
        }
        let variants = s.variants();
        self.n(u64::from(variants.len()));
        self.flag(variants.is_empty());
        for r in variants {
            match r {
                Ok(v) => {
                    let keys = v.keys();
                    self.n(u64::from(keys.len()));
                    self.flag(keys.is_empty());
                    for k in keys {
                        match k {
                            Ok(KeyView::CatchAll) => self.n(0),
                            Ok(KeyView::Literal(r)) => self.text(r),
                            Err(e) => self.malformed(e),
                        }
                    }
                    self.pattern(names, v.pattern());
                }
                Err(e) => self.malformed(e),
            }
        }
    }

    fn body(&mut self, names: Names<'_>, b: Body<'_>) {
        match b {
            Body::Pattern(p) => self.pattern(names, p),
            Body::Select(s) => self.select(names, s),
        }
    }
}
