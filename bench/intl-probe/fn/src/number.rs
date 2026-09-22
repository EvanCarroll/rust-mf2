//! The numeric handlers over `Intl`: `:number`, `:integer`, `:offset`,
//! `:percent`, and the shared resolution of `:currency` / `:unit`
//! (`measure.rs`).
//!
//! Resolution is the runtime's (`crates/mf2-runtime/src/number.rs`,
//! `resolve`, at 3fc4735) step for step — operand rules, option
//! inheritance and the discard lists, `select`'s literal-only rule, the
//! option table, `:offset`'s exact addition, the digit plan with its *Bad
//! Option* reports — with three substitutions: `:integer`'s rounding of the
//! value, the display, and the plural category come from `Intl` (`host.rs`)
//! instead of the runtime's digit buffer, display and evaluator.
//!
//! A resolved value is a [`Value::Boxed`] [`IntlValue`] (the runtime's
//! `Number` is opaque, so the probe cannot carry its own state in
//! `Value::Number`): one allocation per numeric resolution, which the
//! adopted option would not need.

use alloc::boxed::Box;
use core::cell::Cell;

use mf2_runtime::{
    Category, Dir, ErrorSink, FnContext, FormatError, Function, Host, Number, OptionValue, Options,
    Sink, SubPartSink, Value,
};

use crate::host;
#[cfg(feature = "key-codes")]
use crate::host::KEY_FIELDS;
use crate::measure::Measure;
use crate::options::{
    DigitPlan, FracDefaults, INT, Kind, NUM, NumOpts, OPTIONS, PCT, RoundingMode, RoundingType,
    Select, apply, digit_plan, digit_size, select_named,
};
#[cfg(not(feature = "key-codes"))]
use crate::options::{
    RoundingPriority, SignDisplay, grouping_name, mode_name, option_name, priority_name, sign_name,
    trailing_name,
};
use crate::text::{Cmp, Text, add_small, is_number_literal, plain, scaled};

/// The `Intl.NumberFormat` style.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Decimal,
    Percent,
    Currency,
    Unit,
}

/// A function's extra options (`:currency`, `:unit`): `true` when `name`
/// was one of them.
pub(crate) type ExtraOption =
    fn(name: &str, v: OptionValue<'_, '_>, m: &mut Measure, errs: &mut dyn ErrorSink) -> bool;

/// A function's last resolution step (`:currency`, `:unit`): the fraction
/// defaults and whether they are the currency's own (`fractionDigits=auto`),
/// or `None` = a fallback value (reported).
pub(crate) type Finish = fn(m: &Measure, errs: &mut dyn ErrorSink) -> Option<(FracDefaults, bool)>;

/// A check of the resolved value against `Intl` (`:unit`: its sanctioned
/// units); `false` = a fallback value (reported).
pub(crate) type Check = fn(v: &IntlValue, cx: &FnContext<'_>, errs: &mut dyn ErrorSink) -> bool;

/// How one numeric function resolves (closed world: a handler is a `Spec`).
#[derive(Clone, Copy)]
pub(crate) struct Spec {
    /// Option-set bit (`NUM`, `INT`, `PCT`, `CUR`, `UNIT`); 0 = `:offset`.
    pub(crate) bit: u8,
    /// Fraction-digit defaults; `None` keeps the operand's (`:offset`, and
    /// `:currency`, whose `Finish` decides).
    pub(crate) frac: Option<FracDefaults>,
    /// Round the value to an integer (`:integer`).
    pub(crate) integer: bool,
    /// Supports selection (`:currency` and `:unit` do not).
    pub(crate) selectable: bool,
    /// Power of ten applied when selecting (`:percent` = 2; `Intl`'s
    /// `style: "percent"` applies it when formatting).
    pub(crate) scale: i8,
    pub(crate) style: Style,
    /// Locale symbols (the catalog's locale) instead of neutral output.
    pub(crate) localized: bool,
    pub(crate) extra: Option<ExtraOption>,
    pub(crate) finish: Option<Finish>,
    pub(crate) check: Option<Check>,
}

/// What a numeric handler resolved.
pub(crate) struct IntlValue {
    /// The numeric value (after `:integer`'s rounding and `:offset`'s
    /// adjustment), unscaled.
    pub(crate) value: Number,
    opts: NumOpts,
    frac: FracDefaults,
    /// `:currency` with `fractionDigits=auto`: the currency's digits, which
    /// only `Intl` knows — no fraction digits are passed.
    frac_auto: bool,
    scale: i8,
    style: Style,
    localized: bool,
    /// `false` when `select` came from a variable or from the operand.
    selectable: bool,
    plan: DigitPlan,
    pub(crate) measure: Measure,
    /// The plural category once computed (one `Intl.PluralRules` call per
    /// selector, not per key): `Category as u8 + 1`, 0 = not yet.
    category: Cell<u8>,
}

/// A numeric handler over `Intl`.
#[derive(Clone, Copy)]
pub struct IntlNumber {
    pub(crate) spec: Spec,
}

const fn spec(
    bit: u8,
    frac: Option<FracDefaults>,
    integer: bool,
    scale: i8,
    style: Style,
    localized: bool,
) -> Spec {
    Spec {
        bit,
        frac,
        integer,
        selectable: true,
        scale,
        style,
        localized,
        extra: None,
        finish: None,
        check: None,
    }
}

const NUMBER_FRAC: Option<FracDefaults> = Some(FracDefaults { min: 0, max: 3 });
const INTEGER_FRAC: Option<FracDefaults> = Some(FracDefaults { min: 0, max: 0 });

impl IntlNumber {
    pub(crate) const fn number(localized: bool) -> IntlNumber {
        IntlNumber {
            spec: spec(NUM, NUMBER_FRAC, false, 0, Style::Decimal, localized),
        }
    }

    pub(crate) const fn integer(localized: bool) -> IntlNumber {
        IntlNumber {
            spec: spec(INT, INTEGER_FRAC, true, 0, Style::Decimal, localized),
        }
    }

    pub(crate) const fn offset(localized: bool) -> IntlNumber {
        IntlNumber {
            spec: spec(0, None, false, 0, Style::Decimal, localized),
        }
    }

    pub(crate) const fn percent() -> IntlNumber {
        IntlNumber {
            spec: spec(PCT, INTEGER_FRAC, false, 2, Style::Percent, true),
        }
    }
}

/// The numeric value of an operand that is not a resolved number: the
/// runtime's `numeric_operand`.
fn numeric_operand(v: &Value<'_>, host: &dyn Host) -> Result<Number, FormatError> {
    match v {
        Value::Str(s) | Value::Decimal(s) => {
            if !is_number_literal(s.as_bytes()) {
                return Err(FormatError::BadOperand);
            }
            Number::parse(s).ok_or(FormatError::UnsupportedOperation)
        }
        Value::Int(n) => Ok(Number::from_i64(*n)),
        _ => v.to_number(host).ok_or(FormatError::BadOperand),
    }
}

/// Resolves a numeric expression; `None` = a fallback value (reported).
pub(crate) fn resolve(
    spec: &Spec,
    cx: &FnContext<'_>,
    operand: Option<&Value<'_>>,
    options: Options<'_, '_>,
    errs: &mut dyn ErrorSink,
) -> Option<IntlValue> {
    let is_offset = spec.bit == 0;
    let Some(operand) = operand else {
        errs.error(FormatError::BadOperand);
        return None;
    };
    let mut measure = Measure::default();
    let mut style = spec.style;
    let mut frac_auto = false;
    let (mut value, mut o, inherited, mut selectable) =
        if let Some(r) = operand.downcast_ref::<IntlValue>() {
            let mut o = r.opts;
            let mut selectable = r.selectable;
            if o.select.is_some() {
                // `select` set by an operand: Bad Option and no selection for
                // the selecting functions (number.md, "Number Selection");
                // `:offset` passes it on.
                if spec.bit & (NUM | INT) != 0 {
                    errs.error(FormatError::BadOption);
                    selectable = false;
                }
                if !is_offset {
                    o.select = None;
                }
            }
            if spec.bit == INT {
                o.min_frac = None;
                o.max_frac = None;
                o.min_sig = None;
            }
            if spec.bit == PCT {
                o.min_int = None;
                o.rounding_increment = None;
            }
            // A measure carries its currency or unit into its own function,
            // and through `:offset`.
            if is_offset {
                style = r.style;
                measure = r.measure;
                frac_auto = r.frac_auto;
            } else if r.style == spec.style {
                measure = r.measure;
            }
            (r.value.clone(), o, Some((r.frac, r.scale)), selectable)
        } else {
            match numeric_operand(operand, cx.host()) {
                Ok(d) => (d, NumOpts::default(), None, true),
                Err(e) => {
                    errs.error(e);
                    return None;
                }
            }
        };
    let mut offset: Option<(bool, u8)> = None;
    let mut offset_bad = false;
    for (name, v) in options.iter() {
        if is_offset {
            let sub = match name {
                "add" => false,
                "subtract" => true,
                _ => continue, // other options are ignored
            };
            match (digit_size(v.value), offset) {
                (Some(d), None) => offset = Some((sub, d)),
                _ => offset_bad = true,
            }
            continue;
        }
        if let Some(extra) = spec.extra
            && extra(name, v, &mut measure, errs)
        {
            continue;
        }
        let Some(kind) = OPTIONS
            .iter()
            .find(|(n, mask, _)| *n == name && mask & spec.bit != 0)
            .map(|&(_, _, k)| k)
        else {
            continue; // not an option of this function: ignored
        };
        if kind == Kind::Select {
            // MUST be a literal; otherwise Bad Option and no selection.
            if v.literal {
                match v.value.as_str().and_then(select_named) {
                    Some(x) => o.select = Some(x),
                    None => errs.error(FormatError::BadOption),
                }
            } else {
                errs.error(FormatError::BadOption);
                selectable = false;
            }
            continue;
        }
        if !apply(&mut o, kind, v.value) {
            errs.error(FormatError::BadOption);
        }
    }
    if spec.integer && !value.is_integer() {
        // The resolved value of `:integer` is the integer value: rounded by
        // `Intl` (maximumFractionDigits 0, the rounding mode), neutral.
        let mode = o.rounding_mode.unwrap_or(RoundingMode::HalfExpand);
        let key = integer_key(mode);
        let mut out = Text::new();
        let text = plain(&value);
        let Some(v) = host::format(key.as_str(), text.as_str(), &mut out)
            .then(|| Number::parse(out.as_str()))
            .flatten()
        else {
            errs.error(FormatError::UnsupportedOperation);
            return None;
        };
        value = v;
    }
    let (mut frac, scale) = match (spec.frac, inherited) {
        (Some(f), _) => (f, spec.scale),
        (None, Some(inherited)) if is_offset => inherited,
        (None, _) => (FracDefaults { min: 0, max: 3 }, spec.scale),
    };
    if is_offset {
        let Some((sub, d)) = offset.filter(|_| !offset_bad) else {
            errs.error(FormatError::BadOption);
            return None;
        };
        let Some(sum) = add_small(&value, d, sub) else {
            // An implementation limit (number.md allows Unsupported Operation).
            errs.error(FormatError::UnsupportedOperation);
            return None;
        };
        value = sum;
    }
    if let Some(finish) = spec.finish {
        let (f, auto) = finish(&measure, errs)?;
        frac = f;
        frac_auto = auto;
    }
    let plan = digit_plan(&o, frac, errs);
    Some(IntlValue {
        value,
        opts: o,
        frac,
        frac_auto,
        scale,
        style,
        localized: spec.localized,
        selectable: selectable && spec.selectable,
        plan,
        measure,
        category: Cell::new(0),
    })
}

/// One `name=value,` of a design B key.
#[cfg(not(feature = "key-codes"))]
pub(crate) fn pair(k: &mut Text, name: &str, value: &str) {
    k.push_str(name);
    k.push_str("=");
    k.push_str(value);
    k.push_str(",");
}

#[cfg(not(feature = "key-codes"))]
fn pair_n(k: &mut Text, name: &str, value: u32) {
    k.push_str(name);
    k.push_str("=");
    k.push_u32(value);
    k.push_str(",");
}

/// The key of `:integer`'s rounding of its value: neutral,
/// maximumFractionDigits 0, `mode` (design B).
#[cfg(not(feature = "key-codes"))]
fn integer_key(mode: RoundingMode) -> Text {
    let mut k = Text::new();
    k.push_str("en\u{1}en\u{1}numberingSystem=latn,useGrouping=false,");
    pair_n(&mut k, option_name(Kind::MaxFrac), 0);
    if mode != RoundingMode::HalfExpand {
        pair(&mut k, option_name(Kind::Mode), mode_name(mode));
    }
    k
}

/// Digits 1..=21: `Intl`'s range for integer and significant digits. MF2
/// lets an implementation replace a digit size past its limit with its own
/// value (number.md, "Digit Size Options"); the runtime's limit is 99.
fn clamp21(n: u8) -> u8 {
    n.clamp(1, 21)
}

/// A key field that is not set (design A).
#[cfg(feature = "key-codes")]
const UNSET: u8 = 127;

/// `roundingIncrement`'s index in `Intl`'s list (1, 2, 5, 10, …, 5000;
/// design A).
#[cfg(feature = "key-codes")]
fn increment_index(n: u16) -> u8 {
    const INCREMENTS: [u16; 15] = [
        1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
    ];
    INCREMENTS
        .iter()
        .position(|&i| i == n)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

/// The key of `:integer`'s rounding of its value: neutral,
/// maximumFractionDigits 0, `mode` (design A).
#[cfg(feature = "key-codes")]
fn integer_key(mode: RoundingMode) -> Text {
    let mut fields = [0u8; KEY_FIELDS];
    let set = [(1, 1), (2, 1), (5, UNSET), (6, UNSET), (9, mode as u8)];
    for (i, v) in set {
        if let Some(f) = fields.get_mut(i) {
            *f = v;
        }
    }
    let mut k = Text::new();
    k.push_str("en\u{1}");
    k.push_str(core::str::from_utf8(&fields).unwrap_or(""));
    k.push_str("\u{1}");
    k
}

impl IntlValue {
    /// The `Intl` key (`host.rs`, design A) of this value's display
    /// (`neutral` = false) or of its neutral decimal form, which exact keys
    /// compare with and which plural selection shares.
    #[cfg(feature = "key-codes")]
    pub(crate) fn key(&self, locale: &str, neutral: bool) -> Text {
        let p = &self.plan;
        let o = &self.opts;
        let frac = p.ty != RoundingType::Significant && (!self.frac_auto || neutral);
        let sig = p.ty != RoundingType::Fraction;
        let m = if neutral {
            Measure::default()
        } else {
            self.measure
        };
        let (cd, cs, ud) = m.fields();
        let fields: [u8; KEY_FIELDS] = [
            if neutral { 0 } else { self.style as u8 },
            u8::from(neutral || !self.localized),
            clamp21(p.min_int),
            if frac { p.min_frac } else { UNSET },
            if frac { p.max_frac } else { UNSET },
            if sig { clamp21(p.min_sig) } else { UNSET },
            if sig { clamp21(p.max_sig) } else { UNSET },
            match p.ty {
                RoundingType::More => 1,
                RoundingType::Less => 2,
                _ => 0,
            },
            increment_index(p.increment),
            p.mode as u8,
            u8::from(p.strip_if_integer),
            o.sign_display.map_or(0, |s| s as u8),
            if neutral {
                0
            } else {
                o.use_grouping.map_or(0, |g| g as u8)
            },
            cd,
            cs,
            ud,
            u8::from(o.select == Some(Select::Ordinal)),
        ];
        let mut k = Text::new();
        k.push_str(locale);
        k.push_str("\u{1}");
        k.push_str(core::str::from_utf8(&fields).unwrap_or(""));
        m.write_codes(&mut k);
        k
    }

    /// The `Intl` key (`host.rs`, design B): `locale U+0001 [en] U+0001
    /// name=value,…` of this value's display (`neutral` = false) or of its
    /// neutral decimal form, which exact keys compare with and which plural
    /// selection shares. Digit options equal to Intl's defaults for the
    /// style are left out (Intl resolves them back to the same plan).
    #[cfg(not(feature = "key-codes"))]
    pub(crate) fn key(&self, locale: &str, neutral: bool) -> Text {
        let p = &self.plan;
        let o = &self.opts;
        let style = if neutral { Style::Decimal } else { self.style };
        let mut k = Text::new();
        k.push_str(locale);
        if neutral || !self.localized {
            k.push_str("\u{1}en\u{1}numberingSystem=latn,useGrouping=false,");
        } else {
            k.push_str("\u{1}\u{1}");
            if let Some(g) = o.use_grouping {
                pair(&mut k, option_name(Kind::Grouping), grouping_name(g));
            }
        }
        let (dmin, dmax) = match style {
            Style::Decimal | Style::Unit => (0, 3),
            Style::Percent => (0, 0),
            Style::Currency => (u8::MAX, u8::MAX),
        };
        if style != Style::Decimal {
            pair(
                &mut k,
                "style",
                match style {
                    Style::Percent => "percent",
                    Style::Currency => "currency",
                    _ => "unit",
                },
            );
            self.measure.write_pairs(&mut k, style);
        }
        if p.min_int != 1 {
            pair_n(&mut k, option_name(Kind::MinInt), clamp21(p.min_int).into());
        }
        if p.ty != RoundingType::Significant && (!self.frac_auto || neutral) {
            if p.min_frac != dmin {
                pair_n(&mut k, option_name(Kind::MinFrac), p.min_frac.into());
            }
            if p.max_frac != dmax {
                pair_n(&mut k, option_name(Kind::MaxFrac), p.max_frac.into());
            }
        }
        // Significant rounding needs a significant option to be set; More/Less
        // take Intl's defaults (1, 21) otherwise.
        let all_sig = p.ty == RoundingType::Significant;
        if all_sig || (p.ty != RoundingType::Fraction && p.min_sig != 1) {
            pair_n(&mut k, option_name(Kind::MinSig), clamp21(p.min_sig).into());
        }
        if all_sig || (p.ty != RoundingType::Fraction && p.max_sig != 21) {
            pair_n(&mut k, option_name(Kind::MaxSig), clamp21(p.max_sig).into());
        }
        match p.ty {
            RoundingType::More => pair(
                &mut k,
                option_name(Kind::Priority),
                priority_name(RoundingPriority::MorePrecision),
            ),
            RoundingType::Less => pair(
                &mut k,
                option_name(Kind::Priority),
                priority_name(RoundingPriority::LessPrecision),
            ),
            _ => {}
        }
        if p.increment != 1 {
            pair_n(&mut k, option_name(Kind::Increment), p.increment.into());
        }
        if p.mode != RoundingMode::HalfExpand {
            pair(&mut k, option_name(Kind::Mode), mode_name(p.mode));
        }
        if p.strip_if_integer {
            pair(&mut k, option_name(Kind::TrailingZero), trailing_name(true));
        }
        if let Some(s) = o.sign_display
            && s != SignDisplay::Auto
        {
            pair(&mut k, option_name(Kind::Sign), sign_name(s));
        }
        if o.select == Some(Select::Ordinal) {
            pair(&mut k, "type", "ordinal");
        }
        k
    }

    /// The value × 10^scale, what selection sees.
    fn selected(&self) -> Option<Number> {
        scaled(&self.value, self.scale)
    }

    /// Writes the display; `false` when `Intl` rejected the option set.
    fn write_display(&self, cx: &FnContext<'_>, out: &mut dyn Sink) -> bool {
        let text = plain(&self.value);
        let key = self.key(cx.locale(), false);
        host::format(key.as_str(), text.as_str(), out)
    }

    /// The exact-match serialization (number.md): the integer form when the
    /// value is an integer and none of the min-fraction, min-integer,
    /// min/max-significant options is set; otherwise (implementation-
    /// defined, as in the runtime) the neutral display form.
    fn exact_matches(&self, cx: &FnContext<'_>, key: &str) -> bool {
        let o = &self.opts;
        let plain_form = o.min_frac.is_none()
            && o.min_int.is_none()
            && o.min_sig.is_none()
            && o.max_sig.is_none();
        let Some(value) = self.selected() else {
            return false;
        };
        let mut c = Cmp::new(key);
        if plain_form && value.is_integer() {
            if value.to_i64() == Some(0) {
                c.push_str("0");
            } else {
                value.write_plain(&mut c);
            }
        } else {
            let text = plain(&value);
            let k = self.key(cx.locale(), true);
            host::format(k.as_str(), text.as_str(), &mut c);
        }
        c.done()
    }

    /// The plural category of the value × 10^scale under the digit options
    /// and the plural type, from `Intl.PluralRules`, computed once.
    fn category(&self, cx: &FnContext<'_>) -> Category {
        let cached = self.category.get();
        if cached != 0 {
            return Category::from_code(cached - 1);
        }
        let c = self
            .selected()
            .and_then(|v| {
                let text = plain(&v);
                let k = self.key(cx.locale(), true);
                host::plural(k.as_str(), text.as_str())
            })
            .unwrap_or(Category::Other);
        self.category.set(c as u8 + 1);
        c
    }
}

impl Function for IntlNumber {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        let v = resolve(&self.spec, cx, operand, *options, errs)?;
        if let Some(check) = self.spec.check
            && !check(&v, cx, errs)
        {
            return None;
        }
        Some(Value::Boxed(Box::new(v)))
    }

    fn formattable(&self, _cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value.downcast_ref::<IntlValue>() {
            Some(_) => Ok(()),
            None => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Some(v) = value.downcast_ref::<IntlValue>()
            && !v.write_display(cx, out)
        {
            // Only an engine without the option set's features gets here.
            v.value.write_plain(out);
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Some(v) = value.downcast_ref::<IntlValue>() {
            let text = plain(&v.value);
            let key = v.key(cx.locale(), false);
            host::parts(key.as_str(), text.as_str(), out);
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        Dir::Ltr
    }

    fn selectable(&self, value: &Value<'_>) -> bool {
        value
            .downcast_ref::<IntlValue>()
            .is_some_and(|v| v.selectable)
    }

    fn matches(
        &self,
        cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        errs: &mut dyn ErrorSink,
    ) -> bool {
        let Some(v) = value.downcast_ref::<IntlValue>() else {
            return false;
        };
        if is_number_literal(key.as_bytes()) {
            return v.exact_matches(cx, key);
        }
        let Some(keyword) = Category::from_keyword(key) else {
            errs.error(FormatError::BadVariantKey);
            return false;
        };
        match v.opts.select.unwrap_or(Select::Plural) {
            Select::Exact => false,
            Select::Plural | Select::Ordinal => v.category(cx) == keyword,
        }
    }

    fn better_than(&self, _cx: &FnContext<'_>, _value: &Value<'_>, key1: &str, key2: &str) -> bool {
        is_number_literal(key1.as_bytes()) && !is_number_literal(key2.as_bytes())
    }
}
