//! The handlers: `:datetime`, `:date`, `:time` (datetime.md). An
//! unannotated date/time has none: it is a Bad Operand (`plan/08` §4.3).

use mf2_runtime::{
    DateFields, DateLength, DateStyle, DateTime, DateTimeOptions, Dir, ErrorSink, FnContext,
    FormatError, Function, Options, Sink, SubPartSink, TimePrecision, Value,
};

use crate::literal::parse_literal;
use crate::options::{self, Kind, Own};
use crate::plan::{Backend, Plan};
use crate::zone;

/// A date/time handler over the backend `B`: `:datetime`, `:date` or
/// `:time` — the statics [`crate::DATETIME`], [`crate::DATE`],
/// [`crate::TIME`] over the default backend, or one built with the
/// constructors over another.
#[derive(Clone, Copy, Debug)]
pub struct DateTimeFunction<B = crate::DefaultBackend> {
    kind: Kind,
    backend: B,
}

impl<B> DateTimeFunction<B> {
    /// `:datetime`.
    pub const fn datetime(backend: B) -> Self {
        DateTimeFunction {
            kind: Kind::DateTime,
            backend,
        }
    }

    /// `:date`.
    pub const fn date(backend: B) -> Self {
        DateTimeFunction {
            kind: Kind::Date,
            backend,
        }
    }

    /// `:time`.
    pub const fn time(backend: B) -> Self {
        DateTimeFunction {
            kind: Kind::Time,
            backend,
        }
    }

    /// The backend.
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    const fn kind(&self) -> Kind {
        self.kind
    }
}

/// The value of a date/time operand (datetime.md, "Date and Time
/// Operands"): a date/time — an argument, or a date/time function's value
/// with its options —, an application value's `as_date_time`, or a string
/// that is a *date/time literal value* (a literal, a string argument, an
/// application value's `as_str`). Anything else — a number, a fallback
/// value, an application value with neither conversion — is none.
pub fn operand<'a>(v: &Value<'a>) -> Option<DateTime<'a>> {
    match v {
        Value::DateTime(d) => Some(*d),
        Value::Str(s) => parse_literal(s),
        Value::Custom(c) => c
            .as_date_time()
            .or_else(|| c.as_str().and_then(parse_literal)),
        _ => None,
    }
}

/// The parts `kind` shows under its own options `own`, with their defaults:
/// the date (`year-month-day`, `medium`) for `:datetime` and `:date`, the
/// time (`minute`) for `:datetime` and `:time`, and their zone style.
fn parts<'a>(kind: Kind, own: &Own<'a>) -> DateTimeOptions<'a> {
    let date = DateStyle {
        fields: own.fields.unwrap_or(DateFields::YearMonthDay),
        length: own.length.unwrap_or(DateLength::Medium),
    };
    let time = own.precision.unwrap_or(TimePrecision::Minute);
    let mut o = DateTimeOptions::default();
    match kind {
        Kind::DateTime => {
            o.date = Some(date);
            o.time = Some(time);
            o.time_zone_style = own.zone_style;
        }
        Kind::Date => o.date = Some(date),
        Kind::Time => {
            o.time = Some(time);
            o.time_zone_style = own.zone_style;
        }
    }
    o
}

/// Build side (`mf2-locale-data`'s `icu.blob` slicing, `plans/02-catalog-format.md`
/// §4.4): what an expression of `function` — `datetime`, `date` or `time`
/// — shows, from its literal options: `literal(name)` is
/// the literal value of the option `name`, `None` when the expression has
/// none or sets it by a variable. The result has the date and time parts
/// with their defaults, the zone style, and `hour12` and `calendar` when a
/// literal sets them — as the handler resolves them, since a non-override
/// option set by a variable, or an invalid value, is ignored (*Bad Option*)
/// and takes its default. Not the zone, nor what a date/time operand passes
/// on. Another function name: `None`.
pub fn literal_options<'s>(
    function: &str,
    literal: &dyn Fn(&str) -> Option<&'s str>,
) -> Option<DateTimeOptions<'s>> {
    let kind = options::kind(function)?;
    let own = options::read_literals(kind, literal);
    let mut o = parts(kind, &own);
    o.hour12 = own.hour12;
    o.calendar = own.calendar;
    Some(o)
}

/// Function resolution for `kind` with the options `own`: the operand's
/// value (else *Bad Operand*, fallback), the options with their defaults,
/// the override options inherited from the operand where the expression
/// sets none, and the value moved into its time zone ([`zone::place`]).
fn resolve<'a>(
    kind: Kind,
    cx: &FnContext<'_>,
    operand: Option<&Value<'a>>,
    own: &Own<'a>,
    errs: &mut dyn ErrorSink,
) -> Option<DateTime<'a>> {
    let Some(mut d) = operand.and_then(self::operand) else {
        errs.error(FormatError::BadOperand);
        return None;
    };
    // Only the override options travel from the operand; datetime.md drops
    // every other option an operand carries.
    let inherited = d.options;
    let mut o = parts(kind, own);
    o.hour12 = own.hour12.or(inherited.hour12);
    o.calendar = own.calendar.or(inherited.calendar);
    match zone::place(cx, &mut d, own.time_zone.or(inherited.time_zone), errs) {
        Ok(z) => o.time_zone = z,
        Err(e) => {
            errs.error(e);
            return None;
        }
    }
    d.options = o;
    Some(d)
}

impl<B> DateTimeFunction<B> {
    /// The resolved value to format: a date/time function's value.
    fn value<'v>(
        &self,
        _cx: &FnContext<'_>,
        value: &Value<'v>,
    ) -> Result<DateTime<'v>, FormatError> {
        match value {
            Value::DateTime(d) => Ok(*d),
            _ => Err(FormatError::MessageFunctionError),
        }
    }
}

impl<B: Backend> Function for DateTimeFunction<B> {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        let kind = self.kind();
        let own = options::read(kind, *options, errs);
        resolve(kind, cx, operand, &own, errs).map(Value::DateTime)
    }

    fn formattable(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        let d = self.value(cx, value)?;
        self.backend.supports(cx, &Plan::new(cx, &d))
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Ok(d) = self.value(cx, value) {
            self.backend.format(cx, &Plan::new(cx, &d), out);
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Ok(d) = self.value(cx, value) {
            self.backend.format_parts(cx, &Plan::new(cx, &d), out);
        }
    }

    fn part_kind(&self) -> &'static str {
        "datetime"
    }

    /// Every one of them: a message that calls one is a date message.
    fn formats_dates(&self) -> bool {
        true
    }

    fn dir(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Dir {
        match self.value(cx, value) {
            Ok(d) => self.backend.dir(cx, &Plan::new(cx, &d)),
            Err(_) => Dir::Auto,
        }
    }
}
