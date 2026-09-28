//! `mf2-fn-number` — the localized numeric functions of rust-mf2: the
//! runtime's numeric core
//! (`mf2_runtime::NumberSpec`, `Number::resolve`) resolves — operand rules,
//! every digit and rounding option, `signDisplay`, inheritance, selection —
//! and this crate writes the rounded digits with the catalog's locale data
//! (its `number.symbols` and `number.patterns` entries):
//! decimal and group separators, signs, the numbering system's digits,
//! grouping (`useGrouping` `auto` / `always` / `min2` / `never`, with the
//! locale's minimum grouping digits), and the percent pattern.
//!
//! | Static | Function |
//! |---|---|
//! | [`NUMBER`], [`INTEGER`], [`OFFSET`] | `:number`, `:integer`, `:offset`, localized (the core's are neutral) |
//! | [`PERCENT`] | `:percent` |
//! | [`CURRENCY`], [`UNIT`] | `:currency`, `:unit` (Draft): a `Measure` with the catalog's `currency.data` / `unit.data` |
//! | [`NUMBERS`] | unannotated numbers, localized: `Registry::with_numbers(&NUMBERS)` |
//!
//! With feature `intl` on `wasm32-unknown-unknown`
//! ([`mf2_runtime::INTL_NUMBERS`]) the text comes from the host's number
//! formatter (the browser's `Intl.NumberFormat`) instead.
//!
//! Closed world: an application's registry names only the handlers
//! its corpus uses. Selection, exact-match keys and plural operands are the
//! core's (neutral digits, the formatted digits' plural category): a
//! localized handler only changes the text. Client-path code: `no_std`,
//! `forbid(unsafe_code)`, no `core::fmt`, no panicking operation, no
//! allocation.
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! See the [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) for the
//! ecosystem and application guides. An application reaches this crate through
//! [`mf2`](https://docs.rs/mf2), as `mf2::fn_number` (feature `fn-number`).

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

mod intl;
mod localize;
mod measure;

pub use measure::{CURRENCY, CurrencyFunction, UNIT, UnitFunction};

use mf2_runtime::{
    Dir, ErrorSink, FnContext, FormatError, Function, INTL_NUMBERS, Number, NumberOut, NumberSpec,
    Options, Sink, SubPartSink, Value,
};

use localize::{Layout, Out};

/// A localized numeric handler: `:number`, `:integer`, `:offset` or
/// `:percent`.
#[derive(Clone, Copy)]
pub struct NumberFunction {
    spec: NumberSpec,
    layout: Layout,
}

impl NumberFunction {
    const fn new(spec: NumberSpec, layout: Layout) -> Self {
        NumberFunction { spec, layout }
    }
}

/// `:number` (`functions/number.md`), localized.
pub static NUMBER: NumberFunction = NumberFunction::new(NumberSpec::NUMBER, Layout::Decimal);

/// `:integer` (`functions/number.md`), localized.
pub static INTEGER: NumberFunction = NumberFunction::new(NumberSpec::INTEGER, Layout::Decimal);

/// `:offset` (`functions/number.md`), localized.
pub static OFFSET: NumberFunction = NumberFunction::new(NumberSpec::OFFSET, Layout::Decimal);

/// `:percent` (`functions/number.md`): the value × 100 with the locale's
/// percent pattern; selects on the scaled value (plural).
pub static PERCENT: NumberFunction = NumberFunction::new(NumberSpec::PERCENT, Layout::Percent);

impl Function for NumberFunction {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        Number::resolve(self.spec, cx, operand, options, errs).map(Value::Number)
    }

    fn formattable(&self, _cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value {
            Value::Number(_) => Ok(()),
            _ => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if INTL_NUMBERS {
            return intl::format(cx, value, self.layout, NumberOut::Text(out));
        }
        if let Value::Number(n) = value
            && let Some(d) = n.digits()
        {
            localize::write(
                cx.catalog(),
                &d,
                n.grouping(),
                self.layout,
                &mut Out::Text(out),
            );
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if INTL_NUMBERS {
            return intl::format(cx, value, self.layout, NumberOut::Parts(out));
        }
        if let Value::Number(n) = value
            && let Some(d) = n.digits()
        {
            localize::write(
                cx.catalog(),
                &d,
                n.grouping(),
                self.layout,
                &mut Out::Parts(out),
            );
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        // Digits read left to right in every numbering system CLDR has.
        Dir::Ltr
    }

    fn selectable(&self, value: &Value<'_>) -> bool {
        matches!(value, Value::Number(n) if n.is_selectable())
    }

    fn matches(
        &self,
        cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        errs: &mut dyn ErrorSink,
    ) -> bool {
        match value {
            Value::Number(n) => n.matches(cx, key, errs),
            _ => false,
        }
    }

    fn better_than(&self, _cx: &FnContext<'_>, _value: &Value<'_>, key1: &str, key2: &str) -> bool {
        Number::better_than(key1, key2)
    }
}

impl core::fmt::Debug for NumberFunction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("NumberFunction")
    }
}

/// Unannotated numbers — integer, float and decimal arguments — as their
/// exact value (no rounding) with the locale's symbols, grouping (`auto`)
/// and digits: the handler of `Registry::with_numbers` (`plans/03-runtime.md`
/// §2.7). The evaluator checks the value first (a non-finite float is a
/// Bad Operand, as without this handler).
#[derive(Clone, Copy, Default, Debug)]
pub struct Unannotated;

/// The handler for unannotated numbers: `Registry::new(…).with_numbers(&NUMBERS)`.
pub static NUMBERS: Unannotated = Unannotated;

impl Function for Unannotated {
    fn resolve<'a>(
        &self,
        _cx: &FnContext<'_>,
        _operand: Option<&Value<'a>>,
        _options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        // Not a function a message can name: only `with_numbers` uses it.
        errs.error(FormatError::MessageFunctionError);
        None
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if INTL_NUMBERS {
            return intl::format_unannotated(cx, value, NumberOut::Text(out));
        }
        if let Some(n) = value.to_number(cx.host()) {
            localize::write(
                cx.catalog(),
                &n.exact_digits(),
                None,
                Layout::Decimal,
                &mut Out::Text(out),
            );
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if INTL_NUMBERS {
            return intl::format_unannotated(cx, value, NumberOut::Parts(out));
        }
        if let Some(n) = value.to_number(cx.host()) {
            localize::write(
                cx.catalog(),
                &n.exact_digits(),
                None,
                Layout::Decimal,
                &mut Out::Parts(out),
            );
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        Dir::Ltr
    }
}
