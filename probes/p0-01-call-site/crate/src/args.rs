//! Owned, `'static` call-site argument values (plans/04 §2).

use leptos::oco::Oco;
use leptos::prelude::{Get, Memo, ReadSignal, RwSignal, Signal};

/// One positional argument. Borrowed into the runtime's `Arg<'a>` at format
/// time; the runtime never sees a signal.
///
/// The probe carries the variants the reference workload uses; the real type
/// adds `Decimal`, `DateTime` and `Custom` (same size class).
#[derive(Clone)]
pub enum ArgValue {
    /// A string.
    Str(Oco<'static, str>),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A signal-valued argument: the node subscribes to it.
    Reactive(Signal<ArgValue>),
}

impl ArgValue {
    #[cfg_attr(any(feature = "effect", feature = "static-locale"), allow(dead_code))]
    pub(crate) fn is_reactive(&self) -> bool {
        matches!(self, Self::Reactive(_))
    }
}

impl From<i64> for ArgValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<i32> for ArgValue {
    fn from(v: i32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl From<u32> for ArgValue {
    fn from(v: u32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl From<f64> for ArgValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<&'static str> for ArgValue {
    fn from(v: &'static str) -> Self {
        Self::Str(Oco::Borrowed(v))
    }
}

impl From<String> for ArgValue {
    fn from(v: String) -> Self {
        Self::Str(Oco::Owned(v))
    }
}

impl From<Oco<'static, str>> for ArgValue {
    fn from(v: Oco<'static, str>) -> Self {
        Self::Str(v)
    }
}

/// One instance per signal *type* (not per call site).
macro_rules! reactive_from {
    ($($sig:ident),*) => {$(
        impl<T> From<$sig<T>> for ArgValue
        where
            T: Clone + Into<ArgValue> + Send + Sync + 'static,
        {
            #[inline(never)]
            fn from(s: $sig<T>) -> Self {
                Self::Reactive(Signal::derive(move || s.get().into()))
            }
        }
    )*};
}
reactive_from!(ReadSignal, RwSignal, Memo);

impl<T> From<Signal<T>> for ArgValue
where
    T: Clone + Into<ArgValue> + Send + Sync + 'static,
    Signal<T>: Get<Value = T>,
{
    #[inline(never)]
    fn from(s: Signal<T>) -> Self {
        Self::Reactive(Signal::derive(move || s.get().into()))
    }
}

/// Inline capacity: the reference workload's maximum variable count.
const INLINE: usize = 4;

/// Up to four values inline, a boxed slice beyond. Never a const generic.
#[derive(Clone)]
pub struct ArgList(Repr);

#[derive(Clone)]
enum Repr {
    Inline(u8, [ArgValue; INLINE]),
    Spill(Box<[ArgValue]>),
}

impl ArgList {
    pub(crate) fn from_array<const N: usize>(args: [ArgValue; N]) -> Self {
        if N <= INLINE {
            let mut it = args.into_iter();
            let vals = core::array::from_fn(|_| it.next().unwrap_or(ArgValue::Int(0)));
            #[allow(clippy::cast_possible_truncation)]
            Self(Repr::Inline(N as u8, vals))
        } else {
            Self(Repr::Spill(Box::from(args)))
        }
    }

    /// The values in slot order.
    pub fn as_slice(&self) -> &[ArgValue] {
        match &self.0 {
            Repr::Inline(n, vals) => vals.get(..usize::from(*n)).unwrap_or(&[]),
            Repr::Spill(b) => b,
        }
    }

    pub(crate) fn into_boxed(self) -> Box<[ArgValue]> {
        match self.0 {
            Repr::Inline(n, vals) => vals.into_iter().take(usize::from(n)).collect(),
            Repr::Spill(b) => b,
        }
    }
}
