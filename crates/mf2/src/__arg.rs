//! What `tr!` expands an argument to — never written by hand.
//!
//! The argument `e` becomes
//!
//! ```text
//! convert(e, |probe| (&&&probe).__mf2_kind())
//! ```
//!
//! with the `Kind*` traits in scope. The kind is chosen from the type alone
//! (autoref specialization over a zero-sized [`Probe`], a step for each
//! reference taken off it), where the argument's type is known — rustc
//! checks a closure argument after the others, so the closure sees `e`'s:
//!
//! 1. `&&&Probe<T>`: `T: IntoArg` — the typed conversion;
//! 2. `&&Probe<T>`: `T: Into<ArgValue>` — 1.x's conversion, for an
//!    application's own `From<T> for ArgValue` and generic code bounded on it;
//! 3. `&Probe<T>`: `T: Display` — the value's text, made now;
//! 4. `Probe<T>`: always applicable, and its method requires `T: IntoArg`, so
//!    a type with none of these gets that trait's message at the argument.
//!
//! [`convert`] then takes the value as 1.x's `ArgValue::from(e)` took it,
//! straight in, never bound or borrowed at the call site, and converts it by
//! the kind: step 1 compiles to what `IntoArg::into_arg` does, and a call
//! site to the code 1.x's did. (Two forms that bound the value first — in an
//! `Option`, or in a `match` that borrowed it for the probe — changed the
//! code around every argument: C1's record.)

use core::fmt::Display;
use core::marker::PhantomData;

use crate::arg::ArgValue;
use crate::into_arg::{IntoArg, display};

/// The argument's type, and nothing else: what the kind is chosen from.
pub struct Probe<T>(PhantomData<T>);

impl<T> Clone for Probe<T> {
    fn clone(&self) -> Probe<T> {
        *self
    }
}

impl<T> Copy for Probe<T> {}

/// `Probe`: the argument's type need not have a `Debug`.
impl<T> core::fmt::Debug for Probe<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Probe")
    }
}

/// Step 1's kind: `IntoArg`.
#[derive(Clone, Copy, Debug)]
pub struct ViaIntoArg;

/// Step 2's kind: `From<T> for ArgValue`.
#[derive(Clone, Copy, Debug)]
pub struct ViaFrom;

/// Step 3's kind: `Display`.
#[derive(Clone, Copy, Debug)]
pub struct ViaDisplay;

/// Step 4's kind, which only a type with `IntoArg` could have been given.
#[derive(Clone, Copy, Debug)]
pub struct ViaNeither;

/// Step 1: the typed conversion.
pub trait KindIntoArg {
    /// The kind.
    fn __mf2_kind(self) -> ViaIntoArg;
}

impl<T: IntoArg> KindIntoArg for &&&Probe<T> {
    #[inline(always)]
    #[allow(clippy::inline_always, reason = "a zero-sized value")]
    fn __mf2_kind(self) -> ViaIntoArg {
        ViaIntoArg
    }
}

/// Step 2: an application's own `From<T> for ArgValue`, as 1.x's `tr!`
/// converted every argument.
pub trait KindFrom {
    /// The kind.
    fn __mf2_kind(self) -> ViaFrom;
}

impl<T: Into<ArgValue>> KindFrom for &&Probe<T> {
    #[inline(always)]
    #[allow(clippy::inline_always, reason = "a zero-sized value")]
    fn __mf2_kind(self) -> ViaFrom {
        ViaFrom
    }
}

/// Step 3: the value's text.
pub trait KindDisplay {
    /// The kind.
    fn __mf2_kind(self) -> ViaDisplay;
}

impl<T: Display> KindDisplay for &Probe<T> {
    #[inline(always)]
    #[allow(clippy::inline_always, reason = "a zero-sized value")]
    fn __mf2_kind(self) -> ViaDisplay {
        ViaDisplay
    }
}

/// Step 4: none of these, which `IntoArg`'s bound refuses at the argument.
pub trait KindNeither<T> {
    /// The kind.
    fn __mf2_kind(self) -> ViaNeither
    where
        T: IntoArg;
}

impl<T> KindNeither<T> for Probe<T> {
    #[inline(always)]
    #[allow(clippy::inline_always, reason = "a zero-sized value")]
    fn __mf2_kind(self) -> ViaNeither
    where
        T: IntoArg,
    {
        ViaNeither
    }
}

/// How a kind converts a value of type `T`.
pub trait Convert<T> {
    /// The argument's value.
    fn __mf2_convert(self, value: T) -> ArgValue;
}

impl<T: IntoArg> Convert<T> for ViaIntoArg {
    #[inline(always)]
    #[allow(
        clippy::inline_always,
        reason = "the whole of a call site's conversion"
    )]
    fn __mf2_convert(self, value: T) -> ArgValue {
        value.into_arg()
    }
}

impl<T: Into<ArgValue>> Convert<T> for ViaFrom {
    #[inline(always)]
    #[allow(
        clippy::inline_always,
        reason = "the whole of a call site's conversion"
    )]
    fn __mf2_convert(self, value: T) -> ArgValue {
        value.into()
    }
}

impl<T: Display> Convert<T> for ViaDisplay {
    #[inline]
    fn __mf2_convert(self, value: T) -> ArgValue {
        display(&value)
    }
}

/// Never reached: step 4's kind exists only for a type with `IntoArg`, which
/// step 1 takes first. Unbounded, so that a type with none of the forms is
/// reported once, at the kind.
impl<T> Convert<T> for ViaNeither {
    #[inline]
    fn __mf2_convert(self, _value: T) -> ArgValue {
        ArgValue::Unset
    }
}

/// `value`, converted by the kind `kind` picks from its type's probe: the
/// value is passed straight in, as 1.x's `ArgValue::from(e)` took it, and
/// never bound or borrowed at the call site.
#[inline(always)]
#[allow(
    clippy::inline_always,
    reason = "the whole of a call site's conversion"
)]
pub fn convert<T, K: Convert<T>>(value: T, kind: impl FnOnce(Probe<T>) -> K) -> ArgValue {
    kind(Probe(PhantomData)).__mf2_convert(value)
}
