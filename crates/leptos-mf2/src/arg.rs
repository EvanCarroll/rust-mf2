//! The owned call-site argument value (`plans/04-leptos-integration.md`
//! §2.1): what `tr!` builds, borrowed into the runtime's [`Arg`] at format
//! time.
//!
//! Client-path code: no `core::fmt`, no panicking operation, no allocation
//! that is not the value's own.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;

use mf2_runtime::{Arg, CustomValue, Date, DateTime, Time};

/// Text a call site owns: a literal costs nothing, anything else is counted,
/// so cloning a description — which every re-format after a locale change
/// does — never copies the text.
#[derive(Clone)]
pub enum Text {
    /// A `&'static str`: the program's own text.
    Static(&'static str),
    /// Counted text, shared by every clone.
    Shared(Arc<str>),
}

impl Text {
    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Text::Static(s) => s,
            Text::Shared(s) => s,
        }
    }
}

impl From<&'static str> for Text {
    fn from(s: &'static str) -> Text {
        Text::Static(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text::Shared(Arc::from(s))
    }
}

impl From<Arc<str>> for Text {
    fn from(s: Arc<str>) -> Text {
        Text::Shared(s)
    }
}

/// A value read at format time — the extension point for a reactive
/// argument (`plans/04-leptos-integration.md` §2.1).
///
/// `leptos-mf2` implements it for a wrapped `Signal<T>`: [`arg_value`] is
/// called inside whatever reactive context is formatting, which is exactly
/// where a signal has to be read for the node to subscribe to it. The
/// runtime never sees a signal — it sees what this returned.
///
/// [`arg_value`]: ArgSource::arg_value
pub trait ArgSource: Send + Sync {
    /// Its value now. Called once per format.
    fn arg_value(&self) -> ArgValue;
}

/// A date/time a call site owns: [`Arg::DateTime`] with its zone and
/// calendar owned (`plans/03-runtime.md` §2.7).
#[derive(Clone)]
pub struct DateTimeValue {
    date: Date,
    time: Time,
    offset: Option<i32>,
    zone: Option<Text>,
    calendar: Option<Text>,
}

impl DateTimeValue {
    /// The instant `epoch_ms` milliseconds after 1970-01-01T00:00:00Z;
    /// `None` past the year limit of [`Date`].
    #[must_use]
    pub fn instant(epoch_ms: i64) -> Option<DateTimeValue> {
        DateTime::from_epoch_ms(epoch_ms).map(DateTimeValue::from)
    }

    /// A floating date and time: no offset, so the formatting time zone
    /// applies without conversion.
    #[must_use]
    pub fn floating(date: Date, time: Time) -> DateTimeValue {
        DateTimeValue {
            date,
            time,
            offset: None,
            zone: None,
            calendar: None,
        }
    }

    /// The wall time `date` `time` in the IANA zone `zone` — 09:00 in
    /// Paris, whatever instant that is. A date/time function finds its
    /// offset through the host's zone data, and converts it from there to
    /// the zone it shows.
    #[must_use]
    pub fn wall_time(date: Date, time: Time, zone: impl Into<Text>) -> DateTimeValue {
        DateTimeValue::floating(date, time).with_zone(zone)
    }

    /// The same instant, in the IANA zone `zone`: `timeZone=input` shows it
    /// at that zone's wall time, which a date/time function works out
    /// through the host's zone data. A floating value has no instant to
    /// convert: its wall time is placed in `zone`, as
    /// [`DateTimeValue::wall_time`] does.
    #[must_use]
    pub fn with_zone(mut self, zone: impl Into<Text>) -> DateTimeValue {
        self.zone = Some(zone.into());
        self
    }

    /// The same value with a `calendar` override (a Unicode calendar
    /// identifier), which travels with it into the date/time functions.
    #[must_use]
    pub fn with_calendar(mut self, calendar: impl Into<Text>) -> DateTimeValue {
        self.calendar = Some(calendar.into());
        self
    }

    /// The runtime's borrowed form.
    pub(crate) fn borrow(&self) -> DateTime<'_> {
        let mut dt = DateTime::floating(self.date, self.time);
        if let Some(seconds) = self.offset
            && let Some(with) = dt.with_offset(seconds)
        {
            dt = with;
        }
        if let Some(zone) = &self.zone {
            dt = dt.in_zone(zone.as_str());
        }
        if let Some(calendar) = &self.calendar {
            dt.options.calendar = Some(calendar.as_str());
        }
        dt
    }
}

impl From<DateTime<'_>> for DateTimeValue {
    fn from(d: DateTime<'_>) -> DateTimeValue {
        DateTimeValue {
            date: d.date,
            time: d.time,
            offset: d.offset,
            zone: d.zone.map(|z| Text::Shared(Arc::from(z))),
            calendar: d.options.calendar.map(|c| Text::Shared(Arc::from(c))),
        }
    }
}

/// One positional argument of a call site: owned, `'static` and
/// `Send + Sync`, so a description can sit in a `const`, cross a thread, or
/// wait in a registry until something renders it.
///
/// Every variant is borrowed into one [`Arg`]
/// (`plans/04-leptos-integration.md` §2.1).
#[derive(Clone)]
#[non_exhaustive]
pub enum ArgValue {
    /// A string.
    Str(Text),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// An exact decimal as `number-literal` text (`-12.50`, `1e3`).
    Decimal(Text),
    /// A date/time.
    DateTime(Arc<DateTimeValue>),
    /// An application value: what carries a measure, a date or a value a
    /// custom function downcasts ([`CustomValue`]).
    Custom(Arc<dyn CustomValue + Send + Sync>),
    /// A value read at format time ([`ArgSource`]).
    Source(Arc<dyn ArgSource>),
    /// No value: the message reports an Unresolved Variable, as the spec
    /// requires.
    Unset,
}

impl ArgValue {
    /// A `&'static str` argument without an allocation — what the macro
    /// emits for a string literal at a call site.
    #[must_use]
    pub const fn str_static(s: &'static str) -> ArgValue {
        ArgValue::Str(Text::Static(s))
    }

    /// An exact decimal from its `number-literal` text. The runtime formats
    /// and selects on the digits as written, with no float in between.
    #[must_use]
    pub fn decimal(text: impl Into<Text>) -> ArgValue {
        ArgValue::Decimal(text.into())
    }

    /// An application value.
    #[must_use]
    pub fn custom<C: CustomValue + Send + Sync + 'static>(value: C) -> ArgValue {
        ArgValue::Custom(Arc::new(value))
    }

    /// A value read at format time ([`ArgSource`]).
    #[must_use]
    pub fn source<S: ArgSource + 'static>(source: S) -> ArgValue {
        ArgValue::Source(Arc::new(source))
    }

    /// Itself, unless it is an [`ArgValue::Source`]: then what the source
    /// gives now, resolved until it is not a source (bounded, so a source
    /// that returns itself cannot loop).
    pub(crate) fn resolve(&self) -> ArgValue {
        let mut value = self.clone();
        for _ in 0..4 {
            let ArgValue::Source(source) = &value else {
                return value;
            };
            value = source.arg_value();
        }
        ArgValue::Unset
    }
}

impl From<Text> for ArgValue {
    fn from(t: Text) -> ArgValue {
        ArgValue::Str(t)
    }
}

/// Any string slice, copied: a call site's `&str` is rarely `'static`
/// (`user.name()`), and a description outlives the call. A literal keeps its
/// `&'static str` through [`ArgValue::str_static`], which is what the macro
/// emits for one.
impl From<&str> for ArgValue {
    fn from(s: &str) -> ArgValue {
        ArgValue::Str(Text::Shared(Arc::from(s)))
    }
}

impl From<String> for ArgValue {
    fn from(s: String) -> ArgValue {
        ArgValue::Str(Text::Shared(Arc::from(s)))
    }
}

impl From<&String> for ArgValue {
    fn from(s: &String) -> ArgValue {
        ArgValue::Str(Text::Shared(Arc::from(s.as_str())))
    }
}

impl From<Arc<str>> for ArgValue {
    fn from(s: Arc<str>) -> ArgValue {
        ArgValue::Str(Text::Shared(s))
    }
}

impl From<char> for ArgValue {
    fn from(c: char) -> ArgValue {
        let mut buf = [0u8; 4];
        ArgValue::Str(Text::Shared(Arc::from(&*c.encode_utf8(&mut buf))))
    }
}

macro_rules! int_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for ArgValue {
            #[inline]
            fn from(n: $t) -> ArgValue {
                ArgValue::Int(i64::from(n))
            }
        }
    )*};
}
int_arg!(i8, i16, i32, i64, u8, u16, u32);

/// A count: `usize` is 32-bit on the client, and no collection a server
/// holds is longer than `i64::MAX`.
impl From<usize> for ArgValue {
    #[inline]
    fn from(n: usize) -> ArgValue {
        ArgValue::Int(i64::try_from(n).unwrap_or(i64::MAX))
    }
}

impl From<f64> for ArgValue {
    #[inline]
    fn from(x: f64) -> ArgValue {
        ArgValue::Float(x)
    }
}

impl From<f32> for ArgValue {
    #[inline]
    fn from(x: f32) -> ArgValue {
        ArgValue::Float(f64::from(x))
    }
}

impl From<DateTimeValue> for ArgValue {
    fn from(d: DateTimeValue) -> ArgValue {
        ArgValue::DateTime(Arc::new(d))
    }
}

impl From<DateTime<'_>> for ArgValue {
    fn from(d: DateTime<'_>) -> ArgValue {
        ArgValue::DateTime(Arc::new(DateTimeValue::from(d)))
    }
}

impl<C: CustomValue + Send + Sync + 'static> From<Arc<C>> for ArgValue {
    fn from(c: Arc<C>) -> ArgValue {
        ArgValue::Custom(c)
    }
}

/// The values of a call site, in slot order: up to four inline — the
/// reference workload's maximum — and a boxed slice beyond that. Never a
/// const generic: one concrete type, whatever the arity.
#[derive(Clone)]
pub struct ArgList(Repr);

/// Inline capacity.
pub(crate) const INLINE: usize = 4;

#[derive(Clone)]
enum Repr {
    Inline(u8, [ArgValue; INLINE]),
    Spill(Box<[ArgValue]>),
}

impl ArgList {
    /// No arguments.
    pub(crate) const EMPTY: ArgList = ArgList(Repr::Inline(
        0,
        [
            ArgValue::Unset,
            ArgValue::Unset,
            ArgValue::Unset,
            ArgValue::Unset,
        ],
    ));

    /// Up to [`INLINE`] values, in slot order; `n` says how many of `values`
    /// are real.
    pub(crate) const fn inline(n: u8, values: [ArgValue; INLINE]) -> ArgList {
        ArgList(Repr::Inline(n, values))
    }

    /// Five or more values.
    pub(crate) fn spill(values: Box<[ArgValue]>) -> ArgList {
        ArgList(Repr::Spill(values))
    }

    /// The values, in slot order.
    #[must_use]
    pub fn as_slice(&self) -> &[ArgValue] {
        match &self.0 {
            Repr::Inline(n, values) => values.get(..usize::from(*n)).unwrap_or(&[]),
            Repr::Spill(values) => values,
        }
    }
}

/// The borrowed form of one value: [`Arg`] borrows the text, the application
/// value and — through `date` — the date/time the caller resolved.
pub(crate) fn to_arg<'a>(value: &'a ArgValue, date: Option<&'a DateTime<'a>>) -> Arg<'a> {
    match value {
        ArgValue::Str(t) => Arg::Str(t.as_str()),
        ArgValue::Int(n) => Arg::Int(*n),
        ArgValue::Float(x) => Arg::Float(*x),
        ArgValue::Decimal(t) => Arg::Decimal(t.as_str()),
        ArgValue::DateTime(_) => match date {
            Some(d) => Arg::DateTime(d),
            None => Arg::Unset,
        },
        ArgValue::Custom(c) => Arg::Custom(&**c),
        // A source is resolved before this point; one that is still here
        // resolved to nothing.
        ArgValue::Source(_) | ArgValue::Unset => Arg::Unset,
    }
}
