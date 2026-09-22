//! `:currency` and `:unit` (`number.md`): their own options in Rust —
//! `currency` (a well-formed `3ALPHA` code, case-insensitive; not
//! overridable on a currency operand), `currencyDisplay`, `currencySign`,
//! `fractionDigits`; `unit` (a well-formed unit identifier), `unitDisplay`,
//! `usage` (*Unsupported Operation*: no unit conversion) — and the numeric
//! options of their rows in the option table, over `Intl.NumberFormat`'s
//! `style: "currency"` and `style: "unit"`. Neither selects.

#[cfg(feature = "key-codes")]
use mf2_runtime::Sink;
use mf2_runtime::{ErrorSink, FnContext, FormatError, OptionValue};

use crate::host;
use crate::number::{IntlNumber, IntlValue, Spec, Style};
use crate::options::{CUR, FracDefaults, UNIT, digit_size, find};
use crate::text::Text;

/// The longest unit identifier the probe keeps (the longest `Intl`
/// sanctions, a compound `X-per-Y`, is shorter).
const UNIT_CAP: usize = 40;

/// A currency or unit carried by a resolved value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Measure {
    /// Upper case.
    currency: Option<[u8; 3]>,
    /// 0 `symbol` (default), 1 `narrowSymbol`, 2 `name`, 3 `code`, 4 `never`.
    currency_display: u8,
    /// 0 `standard`, 1 `accounting`.
    currency_sign: u8,
    /// `None` = `auto` (the currency's digits).
    fraction_digits: Option<u8>,
    unit: [u8; UNIT_CAP],
    unit_len: u8,
    /// 0 `short` (default), 1 `narrow`, 2 `long`.
    unit_display: u8,
}

impl Default for Measure {
    fn default() -> Measure {
        Measure {
            currency: None,
            currency_display: 0,
            currency_sign: 0,
            fraction_digits: None,
            unit: [0; UNIT_CAP],
            unit_len: 0,
            unit_display: 0,
        }
    }
}

impl Measure {
    fn unit(&self) -> &str {
        core::str::from_utf8(self.unit.get(..usize::from(self.unit_len)).unwrap_or(&[]))
            .unwrap_or("")
    }

    /// Key fields 13–15 (`host.rs`, design A): `currencyDisplay`,
    /// `currencySign`, `unitDisplay`.
    #[cfg(feature = "key-codes")]
    pub(crate) fn fields(&self) -> (u8, u8, u8) {
        (self.currency_display, self.currency_sign, self.unit_display)
    }

    /// The key's tail (design A): the currency code, U+0001, the unit.
    #[cfg(feature = "key-codes")]
    pub(crate) fn write_codes(&self, k: &mut Text) {
        if let Some(c) = &self.currency {
            k.push_str(core::str::from_utf8(c).unwrap_or(""));
        }
        k.push_str("\u{1}");
        k.push_str(self.unit());
    }
}

impl Measure {
    /// The measure's `name=value,` pairs of a design B key (`never` is
    /// mapped by the JavaScript).
    #[cfg(not(feature = "key-codes"))]
    pub(crate) fn write_pairs(&self, k: &mut Text, style: Style) {
        use crate::number::pair;
        use crate::options::name_of;
        if style == Style::Currency {
            if let Some(c) = &self.currency {
                pair(k, "currency", core::str::from_utf8(c).unwrap_or(""));
            }
            if self.currency_display != 0 {
                pair(
                    k,
                    "currencyDisplay",
                    name_of(&DISPLAYS, self.currency_display),
                );
            }
            if self.currency_sign != 0 {
                pair(k, "currencySign", name_of(&SIGNS, self.currency_sign));
            }
        } else if style == Style::Unit {
            pair(k, "unit", self.unit());
            if self.unit_display != 0 {
                pair(k, "unitDisplay", name_of(&UNIT_DISPLAYS, self.unit_display));
            }
        }
    }
}

const DISPLAYS: [(&str, u8); 5] = [
    ("symbol", 0),
    ("narrowSymbol", 1),
    ("name", 2),
    ("code", 3),
    ("never", 4),
];
const SIGNS: [(&str, u8); 2] = [("standard", 0), ("accounting", 1)];
const UNIT_DISPLAYS: [(&str, u8); 3] = [("short", 0), ("narrow", 1), ("long", 2)];

/// `:currency`'s own options.
#[allow(clippy::many_single_char_names)]
fn currency_option(
    name: &str,
    v: OptionValue<'_, '_>,
    m: &mut Measure,
    errs: &mut dyn ErrorSink,
) -> bool {
    let t = v.value.as_str();
    let ok = match name {
        "currency" => {
            if m.currency.is_some() {
                // The operand's currency MUST NOT be overridden.
                false
            } else {
                match t.map(str::as_bytes) {
                    Some(&[a, b, c])
                        if a.is_ascii_alphabetic()
                            && b.is_ascii_alphabetic()
                            && c.is_ascii_alphabetic() =>
                    {
                        m.currency = Some([
                            a.to_ascii_uppercase(),
                            b.to_ascii_uppercase(),
                            c.to_ascii_uppercase(),
                        ]);
                        true
                    }
                    _ => false,
                }
            }
        }
        "currencyDisplay" => t
            .and_then(|t| find(&DISPLAYS, t))
            .map(|x| m.currency_display = x)
            .is_some(),
        "currencySign" => t
            .and_then(|t| find(&SIGNS, t))
            .map(|x| m.currency_sign = x)
            .is_some(),
        "fractionDigits" => {
            if t == Some("auto") {
                m.fraction_digits = None;
                true
            } else {
                digit_size(v.value)
                    .map(|d| m.fraction_digits = Some(d))
                    .is_some()
            }
        }
        _ => return false,
    };
    if !ok {
        errs.error(FormatError::BadOption);
    }
    true
}

/// A numeric operand without a currency is a Bad Operand; the fraction
/// digits are `fractionDigits`, or the currency's (`auto`: only `Intl`
/// knows them; the plan's checks see `2`/`2`, which no check depends on).
fn currency_finish(m: &Measure, errs: &mut dyn ErrorSink) -> Option<(FracDefaults, bool)> {
    if m.currency.is_none() {
        errs.error(FormatError::BadOperand);
        return None;
    }
    Some(match m.fraction_digits {
        Some(d) => (FracDefaults { min: d, max: d }, false),
        None => (FracDefaults { min: 2, max: 2 }, true),
    })
}

/// A well-formed unit identifier, as far as the probe checks it: lower-case
/// ASCII letters and digits in `-`-separated parts, the first part a letter
/// (UTS #35 `unit_identifier` without its prefixes' fine print).
fn well_formed_unit(u: &[u8]) -> bool {
    !u.is_empty()
        && u.len() <= UNIT_CAP
        && u.first().is_some_and(u8::is_ascii_lowercase)
        && u.split(|&c| c == b'-').all(|p| {
            !p.is_empty()
                && p.iter()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// `:unit`'s own options.
fn unit_option(
    name: &str,
    v: OptionValue<'_, '_>,
    m: &mut Measure,
    errs: &mut dyn ErrorSink,
) -> bool {
    let t = v.value.as_str();
    let ok = match name {
        "unit" => match t.map(str::as_bytes) {
            Some(u) if well_formed_unit(u) => {
                let mut buf = [0u8; UNIT_CAP];
                for (d, &c) in buf.iter_mut().zip(u) {
                    *d = c;
                }
                m.unit = buf;
                m.unit_len = u8::try_from(u.len()).unwrap_or(0);
                true
            }
            _ => false,
        },
        "unitDisplay" => t
            .and_then(|t| find(&UNIT_DISPLAYS, t))
            .map(|x| m.unit_display = x)
            .is_some(),
        "usage" => {
            // No unit conversion: Unsupported Operation (number.md, "Unit
            // Conversion"); the unit formats unconverted.
            errs.error(FormatError::UnsupportedOperation);
            return true;
        }
        _ => return false,
    };
    if !ok {
        errs.error(FormatError::BadOption);
    }
    true
}

fn unit_finish(m: &Measure, errs: &mut dyn ErrorSink) -> Option<(FracDefaults, bool)> {
    if m.unit_len == 0 {
        errs.error(FormatError::BadOperand);
        return None;
    }
    Some((FracDefaults { min: 0, max: 3 }, false))
}

/// A well-formed unit that `Intl` does not sanction (`furlong`,
/// `square-meter`): Unsupported Operation.
fn unit_check(v: &IntlValue, cx: &FnContext<'_>, errs: &mut dyn ErrorSink) -> bool {
    let ok = host::supported(v.key(cx.locale(), false).as_str());
    if !ok {
        errs.error(FormatError::UnsupportedOperation);
    }
    ok
}

impl IntlNumber {
    pub(crate) const fn currency() -> IntlNumber {
        IntlNumber {
            spec: Spec {
                bit: CUR,
                frac: None,
                integer: false,
                selectable: false,
                scale: 0,
                style: Style::Currency,
                localized: true,
                extra: Some(currency_option),
                finish: Some(currency_finish),
                check: None,
            },
        }
    }

    pub(crate) const fn unit() -> IntlNumber {
        IntlNumber {
            spec: Spec {
                bit: UNIT,
                frac: None,
                integer: false,
                selectable: false,
                scale: 0,
                style: Style::Unit,
                localized: true,
                extra: Some(unit_option),
                finish: Some(unit_finish),
                check: Some(unit_check),
            },
        }
    }
}
