//! A reactive argument (`plans/04-leptos-integration.md` §2.1, §4).
//!
//! `tr!("users-online", count = count)` with `count: Signal<i32>` expands to
//! `ArgValue::from(count)` like any other argument. The signal is **not**
//! read at the call site: it is wrapped in a [`SignalArg`], and the core
//! reads it through [`ArgSource::arg_value`] once per format — inside
//! whatever reactive context is formatting, which is exactly where it has to
//! be read for the node to subscribe.
//!
//! The wrapper is generic over the signal, not over the call site: one
//! [`SignalArg`] instance per signal *type*, so 200 call sites reading
//! `Signal<i32>` share one. That is the difference P0.1 measured against the
//! `move || tr!(…, count = count.get())` form, which costs a closure type
//! per site.
//!
//! A disposed signal reads as [`ArgValue::Unset`] rather than panicking
//! ([`Get::get`] would): the placeholder then reports an Unresolved
//! Variable, which is a defined outcome, and the client path takes no panic.

use reactive_graph::computed::{ArcMemo, Memo};
use reactive_graph::signal::{ArcReadSignal, ArcRwSignal, ReadSignal, RwSignal};
use reactive_graph::traits::Get;
use reactive_graph::wrappers::read::{ArcSignal, Signal};

use crate::arg::{ArgSource, ArgValue};

/// A signal as an argument: read at format time, never before.
pub struct SignalArg<S>(S);

impl<S> SignalArg<S> {
    /// Wraps `signal`.
    pub const fn new(signal: S) -> SignalArg<S> {
        SignalArg(signal)
    }
}

impl<S> ArgSource for SignalArg<S>
where
    S: Get + Send + Sync + 'static,
    S::Value: Into<ArgValue>,
{
    fn arg_value(&self) -> ArgValue {
        match self.0.try_get() {
            Some(value) => value.into(),
            None => ArgValue::Unset,
        }
    }
}

/// Any signal whose value is an argument, as an [`ArgValue`] — the escape
/// hatch for a signal type the `From` impls below do not name.
#[must_use]
pub fn signal_arg<S>(signal: S) -> ArgValue
where
    S: Get + Send + Sync + 'static,
    S::Value: Into<ArgValue>,
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
