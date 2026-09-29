//! A reactive argument.
//!
//! `tr!("users-online", count = count)` with `count: Signal<i32>` converts it
//! through [`IntoArg`] like any other argument, for a signal of any argument
//! type. The signal is **not** read at the call site: it is wrapped (in a
//! [`SignalArg`] through `ArgValue::from` and [`signal_arg`], in its private
//! twin through `IntoArg`), and the core reads it through
//! [`ArgSource::arg_value`] once per format — inside whatever reactive
//! context is formatting, which is exactly where it has to be read for the
//! node to subscribe.
//!
//! The wrapper is generic over the signal, not over the call site: one
//! instance per signal *type*, so 200 call sites reading `Signal<i32>`
//! share one. That is the difference P0.1 measured against the
//! `move || tr!(…, count = count.get())` form, which costs a closure type
//! per site.
//!
//! A disposed signal reads as [`ArgValue::Unset`] rather than panicking
//! ([`Get::get`] would): the placeholder then reports an Unresolved
//! Variable, which is a defined outcome, and the client path takes no panic.

use crate::line::reactive_graph;
use reactive_graph::computed::{ArcMemo, Memo};
use reactive_graph::graph::Observer;
use reactive_graph::signal::{ArcReadSignal, ArcRwSignal, ReadSignal, RwSignal};
use reactive_graph::traits::{Get, GetUntracked};
use reactive_graph::wrappers::read::{ArcSignal, Signal};

use crate::arg::{ArgSource, ArgValue};
use crate::into_arg::IntoArg;

/// A signal as an argument: read at format time, never before.
pub struct SignalArg<S>(S);

/// `SignalArg(..)`: reading the signal to show it would subscribe whatever
/// is formatting.
impl<S> core::fmt::Debug for SignalArg<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SignalArg(..)")
    }
}

impl<S> SignalArg<S> {
    /// Wraps `signal`.
    pub const fn new(signal: S) -> SignalArg<S> {
        SignalArg(signal)
    }
}

impl<S> ArgSource for SignalArg<S>
where
    S: Get + GetUntracked<Value = <S as Get>::Value> + Send + Sync + 'static,
    <S as Get>::Value: Into<ArgValue>,
{
    fn arg_value(&self) -> ArgValue {
        // Subscribe when something is watching, and do not when nothing is
        // (04 §4: reading outside an observer — a build, an event handler, a
        // `to_string()` — must not warn, and `try_get` warns).
        let value = if Observer::get().is_some() {
            self.0.try_get()
        } else {
            self.0.try_get_untracked()
        };
        match value {
            Some(value) => value.into(),
            None => ArgValue::Unset,
        }
    }
}

/// A signal as a `tr!` argument through [`IntoArg`]: [`SignalArg`] with its
/// value converted as `tr!` converts one, so that a signal of any argument
/// type (`u64`, `bool`, a date) is one. `SignalArg` keeps 1.x's bound,
/// `Into<ArgValue>`, which [`signal_arg`] and the `From` impls below take.
struct SignalIntoArg<S>(S);

impl<S> ArgSource for SignalIntoArg<S>
where
    S: Get + GetUntracked<Value = <S as Get>::Value> + Send + Sync + 'static,
    <S as Get>::Value: IntoArg,
{
    fn arg_value(&self) -> ArgValue {
        // As `SignalArg`'s, written out rather than shared: a shared generic
        // helper is not inlined at `opt-level = "z"`, and cost demo-islands
        // 67 bytes (C1's record).
        let value = if Observer::get().is_some() {
            self.0.try_get()
        } else {
            self.0.try_get_untracked()
        };
        match value {
            Some(value) => value.into_arg(),
            None => ArgValue::Unset,
        }
    }
}

/// Any signal whose value is an argument, as an [`ArgValue`] — the escape
/// hatch for a signal type the `From` impls below do not name.
#[must_use]
pub fn signal_arg<S>(signal: S) -> ArgValue
where
    S: Get + GetUntracked<Value = <S as Get>::Value> + Send + Sync + 'static,
    <S as Get>::Value: Into<ArgValue>,
{
    ArgValue::source(SignalArg::new(signal))
}

/// `From` for each of `reactive_graph`'s readable signals, so that a call site
/// writes the signal itself. Each is one instantiation per value type.
macro_rules! signal_arg_from {
    ($($ty:ident),* $(,)?) => {$(
        impl<T> From<$ty<T>> for ArgValue
        where
            T: Into<ArgValue> + Clone + Send + Sync + 'static,
        {
            fn from(signal: $ty<T>) -> ArgValue {
                signal_arg(signal)
            }
        }
    )*};
}

signal_arg_from!(
    Signal,
    ReadSignal,
    RwSignal,
    Memo,
    ArcSignal,
    ArcReadSignal,
    ArcRwSignal,
    ArcMemo,
);

/// [`IntoArg`] for each of the same signals, over any `T: IntoArg`: what
/// `tr!` takes first. One instantiation per value type, as above.
macro_rules! signal_into_arg {
    ($($ty:ident),* $(,)?) => {$(
        impl<T> IntoArg for $ty<T>
        where
            T: IntoArg + Clone + Send + Sync + 'static,
        {
            fn into_arg(self) -> ArgValue {
                ArgValue::source(SignalIntoArg(self))
            }
        }
    )*};
}

signal_into_arg!(
    Signal,
    ReadSignal,
    RwSignal,
    Memo,
    ArcSignal,
    ArcReadSignal,
    ArcRwSignal,
    ArcMemo,
);
