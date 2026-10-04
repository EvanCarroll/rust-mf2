//! The `intl` path (owner decision 4):
//! with `mf2-runtime`'s feature `intl` (`mf2`'s `number-intl`) on `wasm32-unknown-unknown`
//! ([`mf2_runtime::INTL_NUMBERS`]) the localized numbers, currencies and
//! units are written by the host's number formatter — the browser's
//! `Intl.NumberFormat`, with the catalog's locale — instead of from the
//! catalog's number, currency and unit data, which such a client does not
//! need; the runtime still resolves (options, operand rules, inheritance)
//! and selects (exact keys in Rust, the plural category from
//! `Intl.PluralRules`), and `measure.rs` still resolves the currency or unit
//! and its options. A host without a number formatter gets the value's exact
//! digits in neutral symbols (the runtime reported *Unsupported Operation*
//! at resolution), with the currency code or unit identifier as the Rust
//! path writes them without data.

use mf2_catalog::unit::Width;
use mf2_catalog::{Catalog, StrRef};
use mf2_runtime::{
    CurrencyDisplay, FnContext, FormatError, Measure, MeasureUnit, Number, NumberOut, NumberStyle,
    Sink, UnitDisplay, Value,
};

use crate::localize::Layout;
use crate::measure::{Display, accounting, display_of, own_digits, width_of};

/// `n` through the host in `style`, or its exact digits, neutral, and then
/// `suffix` (`(kind, text)` parts after a literal `gap`).
fn host_or_exact(
    n: &Number,
    cx: &FnContext<'_>,
    style: NumberStyle<'_>,
    suffix: Option<(&str, &str, &str)>,
    out: NumberOut<'_>,
) {
    match out {
        NumberOut::Text(o) => {
            if !n.format_by_host(cx, style, false, NumberOut::Text(&mut *o)) {
                n.exact_digits().write_neutral(o);
                if let Some((gap, _, text)) = suffix {
                    o.push_str(gap);
                    o.push_str(text);
                }
            }
        }
        NumberOut::Parts(o) => {
            if !n.format_by_host(cx, style, false, NumberOut::Parts(&mut *o)) {
                n.exact_digits().neutral_parts(o);
                if let Some((gap, kind, text)) = suffix {
                    o.sub_part("literal", gap);
                    o.sub_part(kind, text);
                }
            }
        }
    }
}

/// A localized handler's display (`:number`, `:integer`, `:offset`,
/// `:percent`).
pub(crate) fn format(cx: &FnContext<'_>, value: &Value<'_>, layout: Layout, out: NumberOut<'_>) {
    if let Value::Number(n) = value {
        let style = match layout {
            Layout::Decimal => NumberStyle::Decimal,
            Layout::Percent => NumberStyle::Percent,
        };
        host_or_exact(n, cx, style, None, out);
    }
}

/// An unannotated number: its exact value with the locale's symbols.
pub(crate) fn format_unannotated(cx: &FnContext<'_>, value: &Value<'_>, out: NumberOut<'_>) {
    if let Some(n) = value.to_number(cx.host()) {
        host_or_exact(&n, cx, NumberStyle::Decimal, None, out);
    }
}

/// How a `:currency` or `:unit` value shows: the style its options say.
pub(crate) fn style<'m>(m: &'m Measure<'_>) -> NumberStyle<'m> {
    match m.unit {
        MeasureUnit::Currency(_) => NumberStyle::Currency {
            code: m.unit.as_str(),
            display: match display_of(m.flags) {
                Display::NarrowSymbol => CurrencyDisplay::NarrowSymbol,
                Display::Symbol => CurrencyDisplay::Symbol,
                Display::Name => CurrencyDisplay::Name,
                Display::Code => CurrencyDisplay::Code,
                Display::Never => CurrencyDisplay::Never,
            },
            accounting: accounting(m.flags),
            own_digits: own_digits(m.flags),
        },
        MeasureUnit::Unit(unit) => NumberStyle::Unit {
            unit,
            display: match width_of(m.flags) {
                Width::Short => UnitDisplay::Short,
                Width::Narrow => UnitDisplay::Narrow,
                Width::Long => UnitDisplay::Long,
            },
        },
    }
}

/// A `:currency` or `:unit` value through the host.
pub(crate) fn measure(cx: &FnContext<'_>, m: &Measure<'_>, out: NumberOut<'_>) {
    let suffix = match m.unit {
        MeasureUnit::Currency(_) => ("\u{a0}", "currency", m.unit.as_str()),
        MeasureUnit::Unit(u) => (" ", "unit", u),
    };
    host_or_exact(&m.number, cx, style(m), Some(suffix), out);
}

/// Writes nothing (and reads no catalog text: the trait's default
/// `push_catalog_text` would link the reader's text access).
struct Discard;

impl Sink for Discard {
    fn push_str(&mut self, _s: &str) {}

    fn push_catalog_text(&mut self, _catalog: &Catalog, _r: StrRef) -> bool {
        true
    }
}

/// Whether a `:unit` value formats: its unit is one the host's formatter
/// has (`Intl.NumberFormat` sanctions 45 units and their `-per-` compounds);
/// *Unsupported Operation* otherwise, as the Rust path says for a unit the
/// catalog has no data for. Without a formatter it formats (neutrally).
pub(crate) fn unit_formattable(cx: &FnContext<'_>, m: &Measure<'_>) -> Result<(), FormatError> {
    if cx.host().numbers().is_none()
        || m.number
            .format_by_host(cx, style(m), false, NumberOut::Text(&mut Discard))
    {
        Ok(())
    } else {
        Err(FormatError::UnsupportedOperation)
    }
}
