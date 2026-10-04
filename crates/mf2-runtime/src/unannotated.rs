//! Values no handler resolved — literals and arguments:
//! a string formats as itself (direction
//! unknown); an integer, float or decimal argument as its exact value in
//! plain neutral digits (`Ltr`) — no rounding, so the numeric handlers are
//! not linked unless the corpus uses them; an application value through
//! `CustomValue::as_str`; a fallback operand as its representation, `{$x}`.
//! A date/time is not formatted here — only by a date function, so an
//! unannotated one is a Bad Operand (`plan/08` §4.3) — and neither is a
//! measure, which only a handler produces. `:string` formats its operand the
//! same way.

use mf2_model::Dir;

use crate::error::FormatError;
use crate::host::Host;
use crate::number::{self, Number};
use crate::sink::{Sink, SubPartSink};
use crate::text::write_i64;
use crate::value::Value;

/// The part kind of an unannotated value.
pub(crate) fn kind(v: &Value<'_>) -> &'static str {
    match v {
        Value::Int(_) | Value::Float(_) | Value::Decimal(_) | Value::Number(_) => "number",
        _ => "string",
    }
}

/// Whether `v` is an unannotated numeric value: what
/// `Registry::with_numbers` localizes.
pub(crate) fn is_numeric(v: &Value<'_>) -> bool {
    matches!(v, Value::Int(_) | Value::Float(_) | Value::Decimal(_))
}

/// The direction of an unannotated value.
pub(crate) fn dir(v: &Value<'_>) -> Dir {
    match v {
        Value::Int(_) | Value::Float(_) | Value::Decimal(_) | Value::Number(_) => Dir::Ltr,
        _ => Dir::Auto,
    }
}

/// Whether an unannotated value formats; the error if not.
pub(crate) fn formattable(v: &Value<'_>, host: &dyn Host) -> Result<(), FormatError> {
    match v {
        Value::Str(_) | Value::Int(_) | Value::Number(_) | Value::Fallback(_) => Ok(()),
        Value::DateTime(_) | Value::Measure(_) => Err(FormatError::BadOperand),
        Value::Float(x) => match Number::from_f64(*x, host) {
            Some(_) => Ok(()),
            None => Err(FormatError::BadOperand),
        },
        Value::Decimal(s) => number::literal_error(s).map_or(Ok(()), Err),
        Value::Custom(c) => match c.as_str() {
            Some(_) => Ok(()),
            None => Err(FormatError::BadOperand),
        },
        Value::Boxed(_) => Err(FormatError::MessageFunctionError),
    }
}

/// Writes an unannotated value (see [`formattable`]; nothing if it does
/// not format).
pub(crate) fn format(v: &Value<'_>, host: &dyn Host, out: &mut dyn Sink) {
    match v {
        Value::Str(s) => out.push_str(s),
        Value::Int(n) => write_i64(*n, out),
        Value::Float(x) => {
            if let Some(n) = Number::from_f64(*x, host) {
                n.write_plain(out);
            }
        }
        Value::Decimal(s) => {
            if let Some(n) = Number::parse(s) {
                n.write_plain(out);
            }
        }
        Value::Number(n) => n.write_plain(out),
        Value::Custom(c) => out.push_str(c.as_str().unwrap_or("")),
        Value::Fallback(src) => {
            out.push_str("{");
            src.write(out);
            out.push_str("}");
        }
        Value::Boxed(_) | Value::DateTime(_) | Value::Measure(_) => {}
    }
}

/// The sub-parts of an unannotated number: `minusSign`, `integer`,
/// `decimal`, `fraction` (none for strings).
pub(crate) fn format_parts(v: &Value<'_>, host: &dyn Host, out: &mut dyn SubPartSink) {
    let n = match v {
        Value::Int(n) => Some(Number::from_i64(*n)),
        Value::Float(x) => Number::from_f64(*x, host),
        Value::Decimal(s) => Number::parse(s),
        Value::Number(n) => Some(n.bare()),
        _ => None,
    };
    if let Some(n) = n {
        n.plain_parts(out);
    }
}
