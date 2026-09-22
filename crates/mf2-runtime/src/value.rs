//! Arguments and resolved values (`plans/03-runtime.md` §2.3).

use alloc::boxed::Box;
use alloc::string::String;
use core::any::Any;

use crate::datetime::DateTime;
use crate::host::Host;
use crate::number::{Measure, Number};
use crate::parts::FallbackSource;

/// A positional argument: slot `i` of the call site is `args[i]` (the
/// manifest's slot order). Small on purpose: every variant is a code path in
/// the wasm. Non-exhaustive, so variants can be added.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum Arg<'a> {
    /// A string.
    Str(&'a str),
    /// An integer.
    Int(i64),
    /// A float (finite values format; others are a Bad Operand).
    Float(f64),
    /// An exact decimal as `number-literal` text (`-12.50`, `1e3`).
    Decimal(&'a str),
    /// An application value.
    Custom(&'a dyn CustomValue),
    /// No value: Unresolved Variable.
    Unset,
    /// A date/time (by reference, so `Arg` stays small).
    DateTime(&'a DateTime<'a>),
}

impl<'a> From<&'a DateTime<'a>> for Arg<'a> {
    #[inline]
    fn from(d: &'a DateTime<'a>) -> Self {
        Arg::DateTime(d)
    }
}

impl<'a> From<&'a str> for Arg<'a> {
    #[inline]
    fn from(s: &'a str) -> Self {
        Arg::Str(s)
    }
}

impl<'a> From<&'a String> for Arg<'a> {
    #[inline]
    fn from(s: &'a String) -> Self {
        Arg::Str(s)
    }
}

macro_rules! int_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg<'_> {
            #[inline]
            fn from(n: $t) -> Self {
                Arg::Int(i64::from(n))
            }
        }
    )*};
}
int_arg!(i8, i16, i32, i64, u8, u16, u32);

impl From<f64> for Arg<'_> {
    #[inline]
    fn from(x: f64) -> Self {
        Arg::Float(x)
    }
}

impl From<f32> for Arg<'_> {
    #[inline]
    fn from(x: f32) -> Self {
        Arg::Float(f64::from(x))
    }
}

/// An application's own argument type: what it converts to.
pub trait CustomValue {
    /// Its string form (unannotated placeholders, `:string`).
    fn as_str(&self) -> Option<&str> {
        None
    }

    /// Its numeric value (numeric operands).
    fn as_number(&self) -> Option<Number> {
        None
    }

    /// Itself, for handlers that know the concrete type.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    /// Its date/time value (date/time operands, `datetime.md`).
    fn as_date_time(&self) -> Option<DateTime<'_>> {
        None
    }

    /// Its number with a currency or a unit (`:currency` and `:unit`
    /// operands, `number.md`; a numeric operand for the other numeric
    /// functions).
    fn as_measure(&self) -> Option<Measure<'_>> {
        None
    }
}

/// A resolved value's data. The handler that resolved it decides how it
/// formats and selects; a value no handler resolved (a literal, an
/// argument) is *unannotated* (`plans/03-runtime.md` §2.6).
#[non_exhaustive]
pub enum Value<'a> {
    /// A string: a literal, a string argument, or `:string`'s operand.
    Str(&'a str),
    /// An integer argument.
    Int(i64),
    /// A float argument.
    Float(f64),
    /// A decimal argument (`number-literal` text).
    Decimal(&'a str),
    /// A number and its resolved options: `:number`, `:integer`, `:offset`,
    /// `:percent`.
    Number(Number),
    /// An application value.
    Custom(&'a dyn CustomValue),
    /// A custom handler's own data (the only allocation a handler makes).
    Boxed(Box<dyn Any>),
    /// A fallback value, as a function's operand: the operand failed to
    /// resolve (an unresolved variable, a declaration that failed). The
    /// handler decides — the numeric ones report Bad Operand, `:string`
    /// takes the text of its representation (`{$x}`), as the suite expects
    /// (`plans/03-runtime.md` §2.6).
    Fallback(FallbackSource<'a>),
    /// A date/time: an argument, or what `:datetime`, `:date`, `:time`
    /// resolved (with its options).
    DateTime(DateTime<'a>),
    /// A number with a currency or a unit: what `:currency` and `:unit`
    /// resolved.
    Measure(Measure<'a>),
}

impl<'a> Value<'a> {
    /// The value of an argument; `None` for [`Arg::Unset`].
    pub fn from_arg(arg: Arg<'a>) -> Option<Value<'a>> {
        Some(match arg {
            Arg::Str(s) => Value::Str(s),
            Arg::Int(n) => Value::Int(n),
            Arg::Float(x) => Value::Float(x),
            Arg::Decimal(s) => Value::Decimal(s),
            Arg::Custom(c) => Value::Custom(c),
            Arg::DateTime(d) => Value::DateTime(*d),
            Arg::Unset => return None,
        })
    }

    /// Its string form when it has one without formatting: a string, a
    /// decimal's text, or an application value's `as_str`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Decimal(s) => Some(s),
            Value::Custom(c) => c.as_str(),
            _ => None,
        }
    }

    /// Its numeric value under the numeric-operand rules (`number.md`,
    /// "Numeric Operands"): a string or decimal matching `number-literal`,
    /// an integer, a finite float, a number or a measure (its value, without
    /// options), an application value's `as_number` (else its measure's).
    pub fn to_number(&self, host: &dyn Host) -> Option<Number> {
        match self {
            Value::Str(s) | Value::Decimal(s) => Number::parse(s),
            Value::Int(n) => Some(Number::from_i64(*n)),
            Value::Float(x) => Number::from_f64(*x, host),
            Value::Number(n) => Some(n.bare()),
            Value::Measure(m) => Some(m.number.bare()),
            Value::Custom(c) => c
                .as_number()
                .or_else(|| c.as_measure().map(|m| m.number.bare())),
            Value::Boxed(_) | Value::Fallback(_) | Value::DateTime(_) => None,
        }
    }

    /// Its value as a *digit size option* (`number.md`): an integer 0–99 —
    /// an integer, an integral float, a number, or a string of one or two
    /// digits without a leading zero. For the function crates' own options
    /// of that type (`fractionDigits`).
    pub fn digit_size(&self) -> Option<u8> {
        crate::number::digit_size(self)
    }

    /// The concrete value, when it is a `T`: a [`Value::Boxed`], or an
    /// application value whose `as_any` gives one.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        match self {
            Value::Boxed(b) => b.downcast_ref(),
            Value::Custom(c) => c.as_any()?.downcast_ref(),
            _ => None,
        }
    }

    /// A copy, for the values `:string` takes: every variant but
    /// [`Value::Boxed`], [`Value::DateTime`] and [`Value::Measure`].
    pub(crate) fn try_copy(&self) -> Option<Value<'a>> {
        Some(match self {
            Value::Str(s) => Value::Str(s),
            Value::Int(n) => Value::Int(*n),
            Value::Float(x) => Value::Float(*x),
            Value::Decimal(s) => Value::Decimal(s),
            Value::Number(n) => Value::Number(n.clone()),
            Value::Custom(c) => Value::Custom(*c),
            Value::Fallback(f) => Value::Fallback(*f),
            // No string form (`:string`): Bad Operand.
            Value::Boxed(_) | Value::DateTime(_) | Value::Measure(_) => return None,
        })
    }
}
