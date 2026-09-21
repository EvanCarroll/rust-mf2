//! P0.5 probe — the `fn-number` localization layer over `numcore`: locale
//! symbols, grouping (primary/secondary sizes, minimum grouping digits),
//! numbering-system digits, `:percent`, and (features) `:currency`, `:unit`.
//! All locale data comes from catalog-borne LOCALE entries (`locdata` writes
//! them); nothing locale-specific is compiled in.
//!
//! Entry container: `tag u8, len u16 LE, payload`. Strings: `len u8, UTF-8`.
//! Plural categories: 0 zero, 1 one, 2 two, 3 few, 4 many, 5 other.
//!
//! `no_std`, `forbid(unsafe_code)`, fmt-free, panic-free.
#![no_std]
#![forbid(unsafe_code)]

use numcore::{ErrSink, Error, Formatted, Grouping, NumValue, OptValue, Operand, PluralOperands, SignOut};

/// LOCALE entry tags.
pub mod tag {
    pub const SYMBOLS: u8 = 1;
    pub const SYMBOLS_NATIVE: u8 = 2;
    pub const PERCENT: u8 = 10;
    pub const CUR_STANDARD: u8 = 11;
    pub const CUR_ACCOUNTING: u8 = 12;
    pub const CUR_STANDARD_ALPHA: u8 = 13;
    pub const CUR_ACCOUNTING_ALPHA: u8 = 14;
    pub const CUR_NO_CURRENCY: u8 = 15;
    pub const CUR_UNIT_PATTERNS: u8 = 16;
    pub const CURRENCIES: u8 = 20;
    pub const UNITS: u8 = 30;
}

/// Panic-free byte reader.
#[derive(Clone, Copy)]
struct Rd<'a> {
    b: &'a [u8],
}

impl<'a> Rd<'a> {
    fn u8(&mut self) -> Option<u8> {
        let (&x, rest) = self.b.split_first()?;
        self.b = rest;
        Some(x)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from(self.u8()?) | u16::from(self.u8()?) << 8)
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.b.split_at_checked(n)?;
        self.b = rest;
        Some(head)
    }
    fn str(&mut self) -> Option<&'a str> {
        let n = usize::from(self.u8()?);
        core::str::from_utf8(self.bytes(n)?).ok()
    }
}

/// Find a LOCALE entry by tag.
pub fn entry(data: &[u8], t: u8) -> Option<&[u8]> {
    entries(data, t).next()
}

/// All entries with tag `t` (large tables are split into chunks).
pub fn entries(data: &[u8], t: u8) -> impl Iterator<Item = &[u8]> {
    let mut r = Rd { b: data };
    core::iter::from_fn(move || {
        loop {
            let tt = r.u8()?;
            let len = usize::from(r.u16()?);
            let payload = r.bytes(len)?;
            if tt == t {
                return Some(payload);
            }
        }
    })
}

/// `number.symbols`: grouping sizes, minimum grouping digits, digits, symbols.
pub struct Symbols<'a> {
    pub primary: u8,
    pub secondary: u8,
    pub min_grouping: u8,
    /// Zero digit of a non-Latin numbering system; `None` = ASCII.
    pub zero: Option<char>,
    pub decimal: &'a str,
    pub group: &'a str,
    pub minus: &'a str,
    pub plus: &'a str,
    pub percent: &'a str,
    pub permille: &'a str,
}

pub fn symbols(data: &[u8], native: bool) -> Option<Symbols<'_>> {
    let p = native
        .then(|| entry(data, tag::SYMBOLS_NATIVE))
        .flatten()
        .or_else(|| entry(data, tag::SYMBOLS))?;
    let mut r = Rd { b: p };
    let primary = r.u8()?;
    let secondary = r.u8()?;
    let min_grouping = r.u8()?;
    let zero = r.str()?.chars().next();
    Some(Symbols {
        primary,
        secondary,
        min_grouping,
        zero,
        decimal: r.str()?,
        group: r.str()?,
        minus: r.str()?,
        plus: r.str()?,
        percent: r.str()?,
        permille: r.str()?,
    })
}

/// A number pattern split into affixes (`¤`, `%`, `‰`, `-` substituted at
/// write time) plus its grouping sizes.
pub struct Pattern<'a> {
    pub primary: u8,
    pub secondary: u8,
    pub pos: (&'a str, &'a str),
    pub neg: Option<(&'a str, &'a str)>,
}

pub fn pattern(data: &[u8], t: u8) -> Option<Pattern<'_>> {
    let mut r = Rd { b: entry(data, t)? };
    let primary = r.u8()?;
    let secondary = r.u8()?;
    let pos = (r.str()?, r.str()?);
    let neg = match r.u8()? {
        0 => None,
        _ => Some((r.str()?, r.str()?)),
    };
    Some(Pattern {
        primary,
        secondary,
        pos,
        neg,
    })
}

/// Output sink (not `core::fmt::Write`).
pub trait StrSink {
    fn push_str(&mut self, s: &str);
    fn push_char(&mut self, c: char);
}

/// Substitutions for pattern placeholders.
pub struct Subst<'a> {
    pub currency: &'a str,
    pub sign: &'a str,
}

fn write_affix(a: &str, sym: &Symbols<'_>, sub: &Subst<'_>, out: &mut impl StrSink) {
    for c in a.chars() {
        match c {
            '¤' => out.push_str(sub.currency),
            '%' => out.push_str(sym.percent),
            '‰' => out.push_str(sym.permille),
            '-' => out.push_str(sub.sign),
            c => out.push_char(c),
        }
    }
}

fn digit(sym: &Symbols<'_>, d: u8) -> char {
    match sym.zero {
        Some(z) => char::from_u32(u32::from(z) + u32::from(d)).unwrap_or('?'),
        None => char::from(b'0' + d),
    }
}

/// Localized rendering of `f` with `sym`, optionally inside `pat`.
pub fn write(
    f: &Formatted,
    grouping: Grouping,
    sym: &Symbols<'_>,
    pat: Option<&Pattern<'_>>,
    currency: &str,
    out: &mut impl StrSink,
) {
    let sign = match f.sign {
        SignOut::Minus => sym.minus,
        SignOut::Plus => sym.plus,
        SignOut::None => "",
    };
    let sub = Subst { currency, sign };
    let (prefix, suffix, signed_affix) = match pat {
        Some(p) => match (f.sign, p.neg) {
            (SignOut::None, _) | (_, None) => (p.pos.0, p.pos.1, false),
            (_, Some(n)) => (n.0, n.1, true),
        },
        None => ("", "", false),
    };
    if !signed_affix {
        out.push_str(sign);
    }
    write_affix(prefix, sym, &sub, out);
    let (primary, secondary) = match pat {
        Some(p) => (p.primary, p.secondary),
        None => (sym.primary, sym.secondary),
    };
    let min = match grouping {
        Grouping::Never => 0,
        Grouping::Always => 1,
        Grouping::Min2 => 2,
        Grouping::Auto => sym.min_grouping.max(1),
    };
    let r = f.abs.magnitude_range();
    let (lo, hi) = (*r.start(), (*r.end()).max(0));
    let p = i16::from(primary);
    let s = i16::from(secondary.max(1));
    let group = min > 0 && p > 0 && hi + 1 >= p + i16::from(min);
    let mut m = hi;
    while m >= lo.min(0) {
        if m == -1 {
            out.push_str(sym.decimal);
        }
        out.push_char(digit(sym, f.abs.digit_at(m)));
        if group && m > 0 && (m == p || (m > p && (m - p) % s == 0)) {
            out.push_str(sym.group);
        }
        m -= 1;
    }
    write_affix(suffix, sym, &sub, out);
}

// -------------------------------------------------------------- :percent

/// `:percent`: `:number`'s machinery with the percent option set, fraction
/// digits 0/0, the value × 100 when formatting and selecting.
pub const PERCENT: numcore::Spec = numcore::Spec {
    bit: numcore::PCT,
    frac: Some(numcore::FracDefaults { min: 0, max: 0 }),
    integer: false,
    selectable: true,
    scale: 2,
};

/// A localized numeric value: the core value plus what kind of function made it.
#[derive(Clone)]
pub struct LocValue {
    pub num: NumValue,
    pub kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Number,
    Percent,
    #[cfg(feature = "currency")]
    Currency(currency::Cur),
    #[cfg(feature = "unit")]
    Unit(unit::UnitOpts),
}

/// Operand of a localized function.
pub enum LocOperand<'a> {
    Core(Operand<'a>),
    Loc(&'a LocValue),
}

fn core_operand<'a>(o: &'a LocOperand<'a>) -> Operand<'a> {
    match o {
        LocOperand::Core(op) => *op,
        LocOperand::Loc(v) => Operand::Num(&v.num),
    }
}

pub fn resolve_percent(
    operand: &LocOperand<'_>,
    opts: &[(&str, OptValue<'_>)],
    errs: &mut impl ErrSink,
) -> Option<LocValue> {
    numcore::resolve_spec(&PERCENT, &core_operand(operand), opts, errs).map(|num| LocValue {
        num,
        kind: Kind::Percent,
    })
}

/// Format any localized value. `native` selects the locale's native digits.
pub fn format(
    v: &LocValue,
    data: &[u8],
    native: bool,
    category: &dyn Fn(&PluralOperands, bool) -> &'static str,
    out: &mut impl StrSink,
    errs: &mut impl ErrSink,
) {
    let Some(sym) = symbols(data, native) else {
        errs.push(Error::UnsupportedOperation);
        return;
    };
    let grouping = v.num.opts.use_grouping.unwrap_or(Grouping::Auto);
    let f = numcore::format_digits(&v.num, errs);
    let _ = category;
    match v.kind {
        Kind::Number => write(&f, grouping, &sym, None, "", out),
        Kind::Percent => {
            let pat = pattern(data, tag::PERCENT);
            write(&f, grouping, &sym, pat.as_ref(), "", out);
        }
        #[cfg(feature = "currency")]
        Kind::Currency(c) => currency::format(&f, &c, grouping, &sym, data, category, out),
        #[cfg(feature = "unit")]
        Kind::Unit(u) => unit::format(&f, &u, grouping, &sym, data, category, out),
    }
}

#[cfg(any(feature = "currency", feature = "unit"))]
fn cat_code(c: &str) -> u8 {
    match c.as_bytes() {
        b"zero" => 0,
        b"one" => 1,
        b"two" => 2,
        b"few" => 3,
        b"many" => 4,
        _ => 5,
    }
}

/// Pick `(cat, str)` for category `want`, falling back to `other` (5).
#[cfg(any(feature = "currency", feature = "unit"))]
fn by_category<'a>(r: &mut Rd<'a>, want: u8) -> Option<&'a str> {
    let n = r.u8()?;
    let mut other = None;
    let mut hit = None;
    for _ in 0..n {
        let c = r.u8()?;
        let s = r.str()?;
        if c == want {
            hit = Some(s);
        }
        if c == 5 {
            other = Some(s);
        }
    }
    hit.or(other)
}

/// Write `pattern` (`{0}` = number, `{1}` = name), rendering the number with
/// `num`. Byte-wise scan (no `str` pattern machinery).
#[cfg(any(feature = "currency", feature = "unit"))]
fn write_unit_pattern<S: StrSink>(pattern: &str, name: &str, out: &mut S, num: impl Fn(&mut S)) {
    let b = pattern.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        let hole = match (b.get(i), b.get(i + 1), b.get(i + 2)) {
            (Some(b'{'), Some(b'0'), Some(b'}')) => Some(true),
            (Some(b'{'), Some(b'1'), Some(b'}')) => Some(false),
            _ => None,
        };
        if let Some(is_num) = hole {
            out.push_str(pattern.get(start..i).unwrap_or(""));
            if is_num {
                num(out);
            } else {
                out.push_str(name);
            }
            i += 3;
            start = i;
        } else {
            i += 1;
        }
    }
    out.push_str(pattern.get(start..).unwrap_or(""));
}

// -------------------------------------------------------------- :currency

#[cfg(feature = "currency")]
pub mod currency {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Display {
        Symbol,
        Narrow,
        Name,
        Code,
        Never,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Cur {
        pub code: [u8; 3],
        pub accounting: bool,
        pub display: Display,
    }

    pub const CURRENCY: numcore::Spec = numcore::Spec {
        bit: numcore::CUR,
        frac: Some(numcore::FracDefaults { min: 2, max: 2 }),
        integer: false,
        selectable: false,
        scale: 0,
    };

    /// One row of the currency table.
    pub struct Row<'a> {
        pub digits: u8,
        /// bit0: symbol starts alphabetic, bit1: ends; bit2/3: same for narrow.
        pub alpha: u8,
        pub symbol: &'a str,
        pub narrow: &'a str,
        pub name: &'a str,
        names: Rd<'a>,
    }

    pub fn row<'a>(data: &'a [u8], code: &[u8; 3]) -> Option<Row<'a>> {
        entries(data, tag::CURRENCIES).find_map(|chunk| row_in(chunk, code))
    }

    fn row_in<'a>(chunk: &'a [u8], code: &[u8; 3]) -> Option<Row<'a>> {
        let mut r = Rd { b: chunk };
        let n = r.u16()?;
        for _ in 0..n {
            let c = r.bytes(3)?;
            let digits = r.u8()?;
            let alpha = r.u8()?;
            let symbol = r.str()?;
            let narrow = r.str()?;
            let name = r.str()?;
            let names = r;
            let k = r.u8()?;
            for _ in 0..k {
                r.u8()?;
                r.str()?;
            }
            if c == code {
                return Some(Row { digits, alpha, symbol, narrow, name, names });
            }
        }
        None
    }

    const DISPLAYS: [(&[u8], Display); 5] = [
        (b"symbol", Display::Symbol),
        (b"narrowSymbol", Display::Narrow),
        (b"name", Display::Name),
        (b"code", Display::Code),
        (b"never", Display::Never),
    ];

    fn text<'a>(v: OptValue<'a>) -> Option<&'a str> {
        match v {
            OptValue::Literal(s) | OptValue::VarStr(s) => Some(s),
            OptValue::VarInt(_) => None,
        }
    }

    pub fn resolve(
        operand: &LocOperand<'_>,
        opts: &[(&str, OptValue<'_>)],
        data: &[u8],
        errs: &mut impl ErrSink,
    ) -> Option<LocValue> {
        let mut cur = match operand {
            LocOperand::Loc(LocValue { kind: Kind::Currency(c), .. }) => Some(*c),
            _ => None,
        };
        let inherited = cur.is_some();
        let mut acc = cur.map(|c| c.accounting);
        let mut disp = cur.map(|c| c.display);
        let mut frac: Option<Option<u8>> = None; // Some(None) = auto
        for &(name, v) in opts {
            match name.as_bytes() {
                b"currency" => match text(v).map(str::as_bytes) {
                    Some(&[a, b, c]) if [a, b, c].iter().all(u8::is_ascii_alphabetic) => {
                        if inherited {
                            errs.push(Error::BadOption);
                        } else {
                            let code = [a.to_ascii_uppercase(), b.to_ascii_uppercase(), c.to_ascii_uppercase()];
                            cur = Some(Cur { code, accounting: false, display: Display::Symbol });
                        }
                    }
                    _ => errs.push(Error::BadOption),
                },
                b"currencySign" => match text(v).map(str::as_bytes) {
                    Some(b"accounting") => acc = Some(true),
                    Some(b"standard") => acc = Some(false),
                    _ => errs.push(Error::BadOption),
                },
                b"currencyDisplay" => match text(v).and_then(|t| super::find(&DISPLAYS, t.as_bytes())) {
                    Some(d) => disp = Some(d),
                    None => errs.push(Error::BadOption),
                },
                b"fractionDigits" => match v {
                    OptValue::Literal("auto") | OptValue::VarStr("auto") => frac = Some(None),
                    _ => match numcore::digit_size(v) {
                        Some(d) => frac = Some(Some(d)),
                        None => errs.push(Error::BadOption),
                    },
                },
                _ => {}
            }
        }
        let mut num = numcore::resolve_spec(&CURRENCY, &core_operand(operand), opts, errs)?;
        let Some(mut c) = cur else {
            // A numeric operand without `currency`: Bad Operand.
            errs.push(Error::BadOperand);
            return None;
        };
        c.accounting = acc.unwrap_or(false);
        c.display = disp.unwrap_or(Display::Symbol);
        let digits = match frac.unwrap_or(None) {
            Some(d) => d,
            None => row(data, &c.code).map_or(2, |r| r.digits),
        };
        num.frac = numcore::FracDefaults { min: digits, max: digits };
        Some(LocValue { num, kind: Kind::Currency(c) })
    }

    pub fn format(
        f: &Formatted,
        c: &Cur,
        grouping: Grouping,
        sym: &Symbols<'_>,
        data: &[u8],
        category: &dyn Fn(&PluralOperands, bool) -> &'static str,
        out: &mut impl StrSink,
    ) {
        let r = row(data, &c.code);
        let code = core::str::from_utf8(&c.code).unwrap_or("XXX");
        if c.display == Display::Name {
            // `{0} {1}` with the count form of the name (plural on the digits).
            let cat = cat_code(category(&numcore::plural_operands(f), false));
            let name = r
                .as_ref()
                .and_then(|r| by_category(&mut r.names.clone(), cat).or(Some(r.name)))
                .unwrap_or(code);
            let pat = entry(data, tag::CUR_UNIT_PATTERNS)
                .and_then(|p| by_category(&mut Rd { b: p }, cat))
                .unwrap_or("{0} {1}");
            write_unit_pattern(pat, name, out, |o| write(f, grouping, sym, None, "", o));
            return;
        }
        let (s, alpha_bits) = match (c.display, &r) {
            (Display::Symbol, Some(r)) => (r.symbol, r.alpha & 3),
            (Display::Narrow, Some(r)) => (r.narrow, r.alpha >> 2),
            (Display::Never, _) => ("", 0),
            _ => (code, 3),
        };
        let t = match (c.display, c.accounting) {
            (Display::Never, _) => tag::CUR_NO_CURRENCY,
            (_, false) => tag::CUR_STANDARD,
            (_, true) => tag::CUR_ACCOUNTING,
        };
        let mut pat = pattern(data, t);
        // alphaNextToNumber variants when the symbol's letter touches the digits.
        if let Some(p) = &pat {
            let before = p.pos.0.ends_with('¤');
            let touches = if before { alpha_bits & 2 != 0 } else { alpha_bits & 1 != 0 };
            if touches && c.display != Display::Never {
                let alt = if c.accounting { tag::CUR_ACCOUNTING_ALPHA } else { tag::CUR_STANDARD_ALPHA };
                if let Some(a) = pattern(data, alt) {
                    pat = Some(a);
                }
            }
        }
        write(f, grouping, sym, pat.as_ref(), s, out);
    }
}

// ------------------------------------------------------------------ :unit

#[cfg(feature = "unit")]
pub mod unit {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct UnitOpts {
        /// Unit identifier (inline; ids are short).
        pub id: [u8; 40],
        pub len: u8,
        /// 0 long, 1 short, 2 narrow.
        pub width: u8,
    }

    pub const UNIT: numcore::Spec = numcore::Spec {
        bit: numcore::UNIT,
        frac: Some(numcore::FracDefaults { min: 0, max: 3 }),
        integer: false,
        selectable: false,
        scale: 0,
    };

    fn text<'a>(v: OptValue<'a>) -> Option<&'a str> {
        match v {
            OptValue::Literal(s) | OptValue::VarStr(s) => Some(s),
            OptValue::VarInt(_) => None,
        }
    }

    pub fn resolve(
        operand: &LocOperand<'_>,
        opts: &[(&str, OptValue<'_>)],
        errs: &mut impl ErrSink,
    ) -> Option<LocValue> {
        let mut u = match operand {
            LocOperand::Loc(LocValue { kind: Kind::Unit(u), .. }) => Some(*u),
            _ => None,
        };
        let mut width = u.map_or(1, |u| u.width);
        for &(name, v) in opts {
            match name.as_bytes() {
                b"unit" => match text(v) {
                    Some(id) if !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') => {
                        let mut buf = [0u8; 40];
                        if let Some(dst) = buf.get_mut(..id.len()) {
                            dst.copy_from_slice(id.as_bytes());
                        }
                        #[allow(clippy::cast_possible_truncation)]
                        let len = id.len() as u8;
                        u = Some(UnitOpts { id: buf, len, width });
                    }
                    _ => errs.push(Error::BadOption),
                },
                b"unitDisplay" => match text(v).map(str::as_bytes) {
                    Some(b"long") => width = 0,
                    Some(b"short") => width = 1,
                    Some(b"narrow") => width = 2,
                    _ => errs.push(Error::BadOption),
                },
                // Unit conversion is optional; not implemented.
                b"usage" => errs.push(Error::UnsupportedOperation),
                _ => {}
            }
        }
        let num = numcore::resolve_spec(&UNIT, &core_operand(operand), opts, errs)?;
        let Some(mut u) = u else {
            errs.push(Error::BadOperand);
            return None;
        };
        u.width = width;
        Some(LocValue { num, kind: Kind::Unit(u) })
    }

    pub fn format(
        f: &Formatted,
        u: &UnitOpts,
        grouping: Grouping,
        sym: &Symbols<'_>,
        data: &[u8],
        category: &dyn Fn(&PluralOperands, bool) -> &'static str,
        out: &mut impl StrSink,
    ) {
        let id = u.id.get(..usize::from(u.len)).unwrap_or(&[]);
        let cat = cat_code(category(&numcore::plural_operands(f), false));
        let pat = entries(data, tag::UNITS).find_map(|p| {
            let mut r = Rd { b: p };
            let n = r.u16()?;
            for _ in 0..n {
                let uid = r.str()?;
                let mut hit = None;
                for w in 0..3u8 {
                    let s = by_category(&mut r, cat);
                    if w == u.width {
                        hit = s;
                    }
                }
                if uid.as_bytes() == id {
                    return hit;
                }
            }
            None
        });
        match pat {
            Some(p) => write_unit_pattern(p, "", out, |o| write(f, grouping, sym, None, "", o)),
            // Unknown unit: number then the identifier.
            None => {
                write(f, grouping, sym, None, "", out);
                out.push_char(' ');
                out.push_str(core::str::from_utf8(id).unwrap_or(""));
            }
        }
    }
}

#[cfg(feature = "currency")]
fn find<T: Copy>(table: &[(&[u8], T)], v: &[u8]) -> Option<T> {
    table.iter().find(|(k, _)| *k == v).map(|&(_, t)| t)
}
