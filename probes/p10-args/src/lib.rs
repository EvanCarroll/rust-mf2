//! A stand-in for `mf2`'s argument conversion (plans/19 §7, Phase 10 A8).
//!
//! `arg!(e)` is what `tr!` would emit for one argument `name = e`: a
//! three-step method dispatch on a wrapper, resolved at the call site where
//! the argument's type is concrete.
//!
//! 1. `Wrap<T>` by value: `T: IntoArg` — numbers, strings, dates, paths, … as
//!    typed values;
//! 2. `&Wrap<T>`: `T: Display` — anything else with a text, as that text;
//! 3. `&mut Wrap<T>`: always applies, and its method requires `T: IntoArg`,
//!    so a type with neither gets `IntoArg`'s own diagnostic.

use core::fmt::Display;

/// The call-site value (a stand-in for `mf2::ArgValue`).
#[derive(Debug, PartialEq)]
pub enum ArgValue {
    Int(i64),
    Float(f64),
    Str(String),
    Static(&'static str),
}

/// A value `tr!` accepts as a typed argument.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a message argument",
    label = "this argument has neither a number, string or date form nor `Display`",
    note = "an argument is a number, a string, a date, a path, or any type that implements `Display` (its text is used)",
    note = "for a type of your own, implement `Display`, or `mf2::IntoArg` to pass it as a number or a date"
)]
pub trait IntoArg {
    fn into_arg(self) -> ArgValue;
}

impl IntoArg for i64 {
    fn into_arg(self) -> ArgValue {
        ArgValue::Int(self)
    }
}
impl IntoArg for usize {
    fn into_arg(self) -> ArgValue {
        ArgValue::Int(i64::try_from(self).unwrap_or(i64::MAX))
    }
}
impl IntoArg for f64 {
    fn into_arg(self) -> ArgValue {
        ArgValue::Float(self)
    }
}
impl IntoArg for String {
    fn into_arg(self) -> ArgValue {
        ArgValue::Str(self)
    }
}
impl IntoArg for &str {
    fn into_arg(self) -> ArgValue {
        ArgValue::Str(self.to_owned())
    }
}
impl IntoArg for &std::path::Path {
    fn into_arg(self) -> ArgValue {
        ArgValue::Str(self.to_string_lossy().into_owned())
    }
}
impl<T: IntoArg + Copy> IntoArg for &T {
    fn into_arg(self) -> ArgValue {
        (*self).into_arg()
    }
}

#[doc(hidden)]
pub mod __arg {
    use super::{ArgValue, Display, IntoArg};

    pub struct Wrap<T>(pub T);

    pub trait ViaIntoArg {
        fn __mf2_arg(self) -> ArgValue;
    }
    impl<T: IntoArg> ViaIntoArg for Wrap<T> {
        #[inline]
        fn __mf2_arg(self) -> ArgValue {
            self.0.into_arg()
        }
    }

    pub trait ViaDisplay {
        fn __mf2_arg(self) -> ArgValue;
    }
    impl<T: Display> ViaDisplay for &Wrap<T> {
        #[inline]
        fn __mf2_arg(self) -> ArgValue {
            ArgValue::Str(self.0.to_string())
        }
    }

    pub trait ViaNeither<T> {
        fn __mf2_arg(self) -> ArgValue
        where
            T: IntoArg;
    }
    impl<T> ViaNeither<T> for &mut Wrap<T> {
        #[inline]
        fn __mf2_arg(self) -> ArgValue
        where
            T: IntoArg,
        {
            // Never reached with a type that is neither: the bound above is
            // what reports it. Reached only by a type that has `IntoArg`
            // but was taken here — impossible, since step 1 takes it first.
            ArgValue::Static("")
        }
    }
}

/// What `tr!` would emit for one argument (not a string literal: those are
/// `ArgValue::str_static`, as in 1.x).
#[macro_export]
macro_rules! arg {
    ($e:expr) => {{
        #[allow(unused_imports)]
        use $crate::__arg::{ViaDisplay as _, ViaIntoArg as _, ViaNeither as _};
        $crate::__arg::Wrap($e).__mf2_arg()
    }};
}
