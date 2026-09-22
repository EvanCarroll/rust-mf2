//! Digits → localized text or sub-parts (`plans/03-runtime.md` §5.1): the
//! catalog's `number.symbols` (decimal and group separators, signs, the
//! percent sign, the numbering system's digits, grouping sizes and minimum
//! grouping digits) and `number.patterns` (the percent pattern's affixes and
//! grouping) applied to the rounded digits the runtime's numeric core
//! resolved. Client path: no allocation, no panic, no `core::fmt`.

use mf2_catalog::Catalog;
use mf2_catalog::number::{AffixPart, Grouping as Sizes, Patterns, SignShown, Style, Symbols};
use mf2_runtime::{Digits, Grouping, Sign, Sink, SubPartSink};

/// Where localized output goes: text, or `Intl`-style sub-parts.
pub(crate) enum Out<'o> {
    Text(&'o mut dyn Sink),
    Parts(&'o mut dyn SubPartSink),
}

impl Out<'_> {
    fn put(&mut self, kind: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        match self {
            Out::Text(s) => s.push_str(text),
            Out::Parts(p) => p.sub_part(kind, text),
        }
    }
}

/// The pattern a number is written with.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout {
    /// The locale's decimal pattern (no affixes): `:number`, `:integer`,
    /// `:offset`, unannotated numbers.
    Decimal,
    /// The percent pattern (`:percent`).
    Percent,
}

/// Whether the integer part of `n` digits is grouped with `sizes`.
fn groups(n: u16, sizes: Sizes, grouping: Grouping, locale_min: u8) -> bool {
    let min = match grouping {
        Grouping::Never => return false,
        Grouping::Auto => locale_min.max(1),
        Grouping::Always => 1,
        Grouping::Min2 => 2,
    };
    sizes.primary != 0 && u32::from(n) >= u32::from(sizes.primary) + u32::from(min)
}

/// A run of digits, written as one part (in chunks of at most 64 bytes).
struct Run {
    buf: [u8; 64],
    len: usize,
}

impl Run {
    const fn new() -> Run {
        Run {
            buf: [0; 64],
            len: 0,
        }
    }

    fn push(&mut self, s: &str, kind: &str, out: &mut Out<'_>) {
        if self.len + s.len() > self.buf.len() {
            self.flush(kind, out);
        }
        if s.len() > self.buf.len() {
            out.put(kind, s);
            return;
        }
        let at = self.len;
        for (b, &c) in self.buf.iter_mut().skip(at).zip(s.as_bytes()) {
            *b = c;
        }
        self.len += s.len();
    }

    fn flush(&mut self, kind: &str, out: &mut Out<'_>) {
        // Only whole `&str`s are pushed, so the bytes are valid UTF-8.
        let text = self
            .buf
            .get(..self.len)
            .and_then(|b| core::str::from_utf8(b).ok())
            .unwrap_or("");
        out.put(kind, text);
        self.len = 0;
    }
}

/// Writes `d` localized for `catalog`'s locale, with `useGrouping`
/// `grouping` (`None`: `auto`), in `layout`.
pub(crate) fn write(
    catalog: &Catalog,
    d: &Digits<'_>,
    grouping: Option<Grouping>,
    layout: Layout,
    out: &mut Out<'_>,
) {
    let Some(sym) = Symbols::of(catalog) else {
        // No `number.symbols` (a catalog built without number data, or a
        // malformed entry): the core's neutral output — the wasm holds no
        // symbols of its own (B6).
        match out {
            Out::Text(s) => d.write_neutral(*s),
            Out::Parts(p) => d.neutral_parts(*p),
        }
        if layout == Layout::Percent {
            out.put("percentSign", "%");
        }
        return;
    };
    let shown = match d.sign() {
        Sign::None => SignShown::None,
        Sign::Minus => SignShown::Minus,
        Sign::Plus => SignShown::Plus,
    };
    let pattern = match layout {
        Layout::Decimal => None,
        Layout::Percent => Patterns::of(catalog).and_then(|p| p.resolve(Style::Percent)),
    };
    let sizes = pattern.map_or(sym.grouping(), |p| p.grouping());
    let sign = |out: &mut Out<'_>| match shown {
        SignShown::None => {}
        SignShown::Minus => out.put("minusSign", sym.minus()),
        SignShown::Plus => out.put("plusSign", sym.plus()),
    };
    let affix = |a: mf2_catalog::number::Affix<'_>, out: &mut Out<'_>| {
        for part in a.parts() {
            match part {
                AffixPart::Text(t) => out.put("literal", t),
                AffixPart::Sign => sign(out),
                AffixPart::Percent => out.put("percentSign", sym.percent()),
                // No currency in these layouts.
                AffixPart::Currency => {}
            }
        }
    };
    if let Some(p) = pattern {
        let s = p.signed(shown);
        if s.sign_first {
            sign(out);
        }
        affix(s.prefix, out);
        digits(&sym, sizes, d, grouping, out);
        affix(s.suffix, out);
    } else {
        sign(out);
        digits(&sym, sizes, d, grouping, out);
        if layout == Layout::Percent {
            // No percent pattern in the catalog: the symbol after the
            // digits (root's `#,##0%`).
            out.put("percentSign", sym.percent());
        }
    }
}

/// Writes the digits: integer digits grouped, the decimal separator, the
/// fraction digits, in the catalog's numbering system.
fn digits(
    sym: &Symbols<'_>,
    sizes: Sizes,
    d: &Digits<'_>,
    grouping: Option<Grouping>,
    out: &mut Out<'_>,
) {
    let native = sym.digits();
    let n = d.integer_count();
    let grouped = groups(
        n,
        sizes,
        grouping.unwrap_or(Grouping::Auto),
        sym.minimum_grouping_digits(),
    );
    let mut run = Run::new();
    let mut m = n;
    while m > 0 {
        m -= 1;
        let mag = i16::try_from(m).unwrap_or(i16::MAX);
        run.push(native.digit(d.digit(mag)), "integer", out);
        if grouped && m > 0 && sizes.separator_after(u32::from(m)) {
            run.flush("integer", out);
            out.put("group", sym.group());
        }
    }
    run.flush("integer", out);
    let f = d.fraction_count();
    if f > 0 {
        out.put("decimal", sym.decimal());
        for k in 1..=f {
            let mag = i16::try_from(k).map_or(i16::MIN, |k| -k);
            run.push(native.digit(d.digit(mag)), "fraction", out);
        }
        run.flush("fraction", out);
    }
}

#[cfg(test)]
mod tests {
    use super::{Grouping, Sizes, groups};

    #[test]
    fn grouping_thresholds() {
        let three = Sizes {
            primary: 3,
            secondary: 3,
        };
        // en: minimum grouping 1.
        assert!(!groups(3, three, Grouping::Auto, 1));
        assert!(groups(4, three, Grouping::Auto, 1));
        // es / pl: minimum grouping 2.
        assert!(!groups(4, three, Grouping::Auto, 2));
        assert!(groups(5, three, Grouping::Auto, 2));
        assert!(groups(4, three, Grouping::Always, 2));
        assert!(!groups(4, three, Grouping::Min2, 1));
        assert!(groups(5, three, Grouping::Min2, 1));
        assert!(!groups(9, three, Grouping::Never, 1));
        assert!(!groups(9, Sizes::NONE, Grouping::Always, 1));
    }
}
