//! Arguments and resolved values (`plans/03-runtime.md` §2.3).

use alloc::boxed::Box;
use alloc::string::String;
use core::any::Any;

use crate::host::Host;
use crate::number::Number;
use crate::parts::FallbackSource;

/// A positional argument: slot `i` of the call site is `args[i]` (the
/// manifest's slot order). Small on purpose: every variant is a code path in
/// the wasm. Non-exhaustive: Phase 4 adds `DateTime`.
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
    /// A number and its resolved options: `:number`, `:integer`, `:offset`.
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
    /// an integer, a finite float, a number (its value, without options), an
    /// application value's `as_number`.
    pub fn to_number(&self, host: &dyn Host) -> Option<Number> {
        match self {
            Value::Str(s) | Value::Decimal(s) => Number::parse(s),
            Value::Int(n) => Some(Number::from_i64(*n)),
            Value::Float(x) => Number::from_f64(*x, host),
            Value::Number(n) => Some(n.bare()),
            Value::Custom(c) => c.as_number(),
            Value::Boxed(_) | Value::Fallback(_) => None,
        }
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

    /// A copy, for every variant but [`Value::Boxed`].
    pub(crate) fn try_copy(&self) -> Option<Value<'a>> {
        Some(match self {
            Value::Str(s) => Value::Str(s),
            Value::Int(n) => Value::Int(*n),
            Value::Float(x) => Value::Float(*x),
            Value::Decimal(s) => Value::Decimal(s),
            Value::Number(n) => Value::Number(n.clone()),
            Value::Custom(c) => Value::Custom(*c),
            Value::Fallback(f) => Value::Fallback(*f),
            Value::Boxed(_) => return None,
        })
    }
}
