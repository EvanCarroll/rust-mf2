//! `NUMBER` (`plans/05-tooling.md` §6.1, "`NUMBER` options").
//!
//! [`Opts`] mirrors `fluent-bundle` 0.16's `FluentNumberOptions` field for
//! field, and [`Opts::merge`] its `merge`, because a number key matches a
//! selector only when the two are equal *including their options*: which
//! variants a message can reach depends on it.

use std::borrow::Cow;

use mf2_model::{FunctionRef, Literal, OptionValue, Options};

/// `FluentNumberStyle`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Style {
    #[default]
    Decimal,
    Currency,
    Percent,
}

/// `FluentNumberCurrencyDisplayStyle`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CurrencyDisplay {
    #[default]
    Symbol,
    Code,
    Name,
}

/// `FluentNumberOptions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Opts {
    pub(super) ordinal: bool,
    pub(super) style: Style,
    pub(super) currency: Option<String>,
    pub(super) currency_display: CurrencyDisplay,
    pub(super) use_grouping: bool,
    pub(super) minimum_integer_digits: Option<usize>,
    pub(super) minimum_fraction_digits: Option<usize>,
    pub(super) maximum_fraction_digits: Option<usize>,
    pub(super) minimum_significant_digits: Option<usize>,
    pub(super) maximum_significant_digits: Option<usize>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            ordinal: false,
            style: Style::Decimal,
            currency: None,
            currency_display: CurrencyDisplay::Symbol,
            use_grouping: true,
            minimum_integer_digits: None,
            minimum_fraction_digits: None,
            maximum_fraction_digits: None,
            minimum_significant_digits: None,
            maximum_significant_digits: None,
        }
    }
}

/// A resolved argument of a Fluent call: what `get_arguments` gives for a
/// literal.
#[derive(Clone, Debug)]
pub(super) enum Arg {
    Str(String),
    Num(Num),
}

/// `FluentNumber`: an `f64` and its options.
#[derive(Clone, Debug)]
pub(super) struct Num {
    pub(super) value: f64,
    pub(super) opts: Opts,
    /// The literal as written, when the number came from one.
    pub(super) text: Option<String>,
}

impl PartialEq for Num {
    /// `FluentNumber`'s derived equality: the value and every option.
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.opts == other.opts
    }
}

impl Num {
    /// `FluentNumber::from_str`: `None` where `f64` does not parse.
    pub(super) fn parse(text: &str) -> Option<Num> {
        let value: f64 = text.parse().ok()?;
        let opts = Opts {
            minimum_fraction_digits: text.find('.').map(|pos| text.len() - pos - 1),
            ..Opts::default()
        };
        Some(Num {
            value,
            opts,
            text: Some(text.to_owned()),
        })
    }

    /// `FluentNumber::as_string`: `f64`'s `Display`, padded to the minimum
    /// fraction digits.
    pub(super) fn as_string(&self) -> String {
        let mut val = self.value.to_string();
        if let Some(minfd) = self.opts.minimum_fraction_digits {
            if let Some(pos) = val.find('.') {
                let frac = val.len() - pos - 1;
                val.push_str(&"0".repeat(minfd.saturating_sub(frac)));
            } else {
                val.push('.');
                val.push_str(&"0".repeat(minfd));
            }
        }
        val
    }

    /// The text of an MF2 number literal of the same value: the literal as
    /// written when MF2 accepts it (`1.50` keeps its digits), else the value
    /// `fluent-bundle` reads from it (`007` → `7`).
    pub(super) fn mf2_literal(&self) -> String {
        match &self.text {
            Some(t) if is_mf2_number(t) => t.clone(),
            _ => self.value.to_string(),
        }
    }
}

/// `number-literal` of `spec/message.abnf`.
pub(super) fn is_mf2_number(s: &str) -> bool {
    let b = s.strip_prefix('-').unwrap_or(s).as_bytes();
    let int_len = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if int_len == 0 || (int_len > 1 && b[0] == b'0') {
        return false;
    }
    let mut rest = &b[int_len..];
    if let Some(frac) = rest.strip_prefix(b".") {
        let n = frac.iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return false;
        }
        rest = &frac[n..];
    }
    if let Some(exp) = rest.strip_prefix(b"e").or_else(|| rest.strip_prefix(b"E")) {
        let exp = exp
            .strip_prefix(b"+")
            .or_else(|| exp.strip_prefix(b"-"))
            .unwrap_or(exp);
        return !exp.is_empty() && exp.iter().all(u8::is_ascii_digit);
    }
    rest.is_empty()
}

/// An option `merge` ignored: its value had the wrong type, or its name is
/// not one `fluent-bundle` knows.
pub(super) struct Ignored<'a> {
    pub(super) name: &'a str,
}

impl Opts {
    /// `FluentNumberOptions::merge`: applies each named argument it knows,
    /// in order, and returns the ones it ignores.
    pub(super) fn merge<'a>(&mut self, named: &[(&'a str, Arg)]) -> Vec<Ignored<'a>> {
        let mut ignored = Vec::new();
        for (name, value) in named {
            match (*name, value) {
                ("type", Arg::Str(s)) => self.ordinal = s == "ordinal",
                ("style", Arg::Str(s)) => {
                    self.style = match s.as_str() {
                        "currency" => Style::Currency,
                        "percent" => Style::Percent,
                        _ => Style::Decimal,
                    };
                }
                ("currency", Arg::Str(s)) => self.currency = Some(s.clone()),
                ("currencyDisplay", Arg::Str(s)) => {
                    self.currency_display = match s.as_str() {
                        "code" => CurrencyDisplay::Code,
                        "name" => CurrencyDisplay::Name,
                        _ => CurrencyDisplay::Symbol,
                    };
                }
                ("useGrouping", Arg::Str(s)) => self.use_grouping = s != "false",
                ("minimumIntegerDigits", Arg::Num(n)) => {
                    self.minimum_integer_digits = Some(as_usize(n.value));
                }
                ("minimumFractionDigits", Arg::Num(n)) => {
                    self.minimum_fraction_digits = Some(as_usize(n.value));
                }
                ("maximumFractionDigits", Arg::Num(n)) => {
                    self.maximum_fraction_digits = Some(as_usize(n.value));
                }
                ("minimumSignificantDigits", Arg::Num(n)) => {
                    self.minimum_significant_digits = Some(as_usize(n.value));
                }
                ("maximumSignificantDigits", Arg::Num(n)) => {
                    self.maximum_significant_digits = Some(as_usize(n.value));
                }
                _ => ignored.push(Ignored { name }),
            }
        }
        ignored
    }
}

/// `n as usize`, as `From<&FluentNumber> for usize` casts.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the cast fluent-bundle makes, saturating and truncating alike"
)]
fn as_usize(n: f64) -> usize {
    n as usize
}

/// Why a `NUMBER` has no MF2 form, or loses an option on the way.
pub(super) enum Loss {
    /// `style: "currency"` without `currency`.
    CurrencyMissing,
    /// An option MF2's function cannot take.
    Option(&'static str),
}

/// The MF2 function a `NUMBER` placeholder becomes, with its options, and
/// what did not fit.
pub(super) fn placeholder(opts: &Opts) -> (Option<FunctionRef<'static>>, Vec<Loss>) {
    let mut losses = Vec::new();
    let mut options = Options::new();
    let mut put = |name: &'static str, value: String| {
        options.push(Cow::Borrowed(name), literal(value));
    };
    let name = match opts.style {
        Style::Decimal => "number",
        Style::Percent => "percent",
        Style::Currency => "currency",
    };
    if opts.style == Style::Currency {
        if let Some(c) = &opts.currency {
            put("currency", c.clone());
        } else {
            losses.push(Loss::CurrencyMissing);
            return (None, losses);
        }
        match opts.currency_display {
            CurrencyDisplay::Symbol => {}
            CurrencyDisplay::Code => put("currencyDisplay", "code".to_owned()),
            CurrencyDisplay::Name => put("currencyDisplay", "name".to_owned()),
        }
    }
    if !opts.use_grouping {
        put("useGrouping", "never".to_owned());
    }
    if let Some(n) = opts.minimum_integer_digits {
        put("minimumIntegerDigits", n.to_string());
    }
    if opts.style == Style::Currency {
        match (opts.minimum_fraction_digits, opts.maximum_fraction_digits) {
            (None, None) => {}
            (Some(a), Some(b)) if a == b => put("fractionDigits", a.to_string()),
            (Some(_), _) => losses.push(Loss::Option("minimumFractionDigits")),
            (None, Some(_)) => losses.push(Loss::Option("maximumFractionDigits")),
        }
    } else {
        if let Some(n) = opts.minimum_fraction_digits {
            put("minimumFractionDigits", n.to_string());
        }
        if let Some(n) = opts.maximum_fraction_digits {
            put("maximumFractionDigits", n.to_string());
        }
    }
    if let Some(n) = opts.minimum_significant_digits {
        put("minimumSignificantDigits", n.to_string());
    }
    if let Some(n) = opts.maximum_significant_digits {
        put("maximumSignificantDigits", n.to_string());
    }
    if opts.ordinal && opts.style == Style::Decimal {
        put("select", "ordinal".to_owned());
    }
    (
        Some(FunctionRef {
            name: Cow::Borrowed(name),
            options,
        }),
        losses,
    )
}

/// The MF2 annotation of a `NUMBER` selector: `:number`, keeping only the
/// options that change `fluent-bundle`'s choice — the rule set (`type`) and
/// the visible fraction digits (`minimumFractionDigits`, which pads the
/// plural operands). Rounding options do not reach Fluent's plural operands,
/// so carrying them would make MF2 select on a rounded value Fluent never
/// sees.
pub(super) fn selector(opts: &Opts) -> FunctionRef<'static> {
    let mut options = Options::new();
    if opts.ordinal {
        options.push(Cow::Borrowed("select"), literal("ordinal".to_owned()));
    }
    if let Some(n) = opts.minimum_fraction_digits {
        options.push(
            Cow::Borrowed("minimumFractionDigits"),
            literal(n.to_string()),
        );
    }
    FunctionRef {
        name: Cow::Borrowed("number"),
        options,
    }
}

fn literal(value: String) -> OptionValue<'static> {
    OptionValue::Literal(Literal {
        value: Cow::Owned(value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_string_is_fluent_bundles() {
        assert_eq!(
            Num::parse("007").map(|n| n.as_string()).as_deref(),
            Some("7")
        );
        assert_eq!(
            Num::parse("1.50").map(|n| n.as_string()).as_deref(),
            Some("1.50")
        );
        assert_eq!(
            Num::parse("-0").map(|n| n.as_string()).as_deref(),
            Some("-0")
        );
        assert_eq!(
            Num::parse("2.0").map(|n| n.as_string()).as_deref(),
            Some("2.0")
        );
    }

    #[test]
    fn mf2_number_literals() {
        for ok in ["0", "-1", "1.50", "10", "1e3", "2.5E-1"] {
            assert!(is_mf2_number(ok), "{ok}");
        }
        for bad in ["007", "01.5", "1.", ".5", "-", "1e"] {
            assert!(!is_mf2_number(bad), "{bad}");
        }
    }
}
