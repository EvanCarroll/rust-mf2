//! What a `tr!` argument may be, and how the macro converts one.
//!
//! [`IntoArg`] is the typed conversion: numbers, text, dates, paths, and an
//! application's own types that implement it. Any other type with a
//! `Display` is an argument too, as its text. The macro picks the first of
//! four steps that the argument's type allows, where that type is known
//! (`crate::__arg`); a type that allows none gets [`IntoArg`]'s own message.
//!
//! Client-path code: the typed conversions write no `core::fmt` and take no
//! panicking path. An integer past `i64` is written digit by digit. Only the
//! `Display` step formats, and only where a call site takes it: it links the
//! type's own `Display`, as 1.x's `.to_string()` at the call site did.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::sync::Arc;
use core::num::NonZero;

use mf2_runtime::{CustomValue, DateTime};

use crate::arg::{ArgValue, DateTimeValue, Text};

/// A value a `tr!` argument converts from, by its type: a number, a string, a
/// date, a path, or an application's own type.
///
/// `tr!("id", name = value)` converts `value` by the first of these its type
/// allows:
///
/// 1. `IntoArg` — the typed conversion, below;
/// 2. a `From<T> for ArgValue` of the application's own, as 1.x's `tr!`
///    converted every argument (and generic code bounded on it);
/// 3. `Display` — any other type with a text is an argument as that text,
///    made when the description is built: an `io::Error`, an `Ipv4Addr`, a
///    key binding. The text is not translated; for a word that needs one,
///    pass a string and select on it in the message (`.match $state`);
/// 4. none of these — a compile error at the argument, in this trait's words.
///
/// | Type | Is |
/// |---|---|
/// | `i8` … `i128`, `u8` … `u128`, `isize`, `usize`, and their `NonZero` forms | an integer, exactly: past `i64`, the exact decimal |
/// | `f32`, `f64` | a floating-point number |
/// | `bool` | the string `true` or `false`, which `.match` selects on |
/// | `char`, `&str`, `String`, `Arc<str>`, [`Text`] | a string: an `Arc<str>` is shared as it is, other text is copied once into a shared string (a literal is kept as it is) |
/// | `Cow<'static, str>` | a string; borrowed text stays borrowed |
/// | `&Path`, `PathBuf`, `&OsStr`, `OsString` (with `std`: `host-std`, `native` or a Leptos mode) | its text, lossy where it is not UTF-8 |
/// | `SystemTime` (with `std`) | an instant, to the millisecond; nothing past the years a date can hold |
/// | jiff's `Timestamp`, `Zoned`, `civil::Date`, `civil::DateTime` (with `host-std` and `datetime`) | a date: an instant, an instant with its zone, or a floating date and time |
/// | [`DateTimeValue`], the runtime's [`DateTime`] | a date and time |
/// | [`ArgValue`] | itself: an exact decimal, a value read at format time, … |
/// | `Arc<C>` for a [`CustomValue`] | an application value a function reads |
/// | a signal of any of these (with a Leptos mode) | its value, read when the message is formatted |
/// | `&T` for any of these that is `Copy` | what `T` is |
///
/// `&String`, `&PathBuf`, `&OsString` and jiff's `&Zoned` are arguments too,
/// through `From<&T> for ArgValue`: a reference to a type that is not `Copy`
/// cannot also be an `IntoArg` beside the `&T` rule. A `Cow` that borrows for
/// less than `'static` is refused by the borrow checker (the impl is for
/// `'static` text, which it keeps): pass `&*cow`, or the `Cow` owned.
///
/// An application implements it for a type that is a number, a date or a
/// value a custom function reads, which its `Display` text would not be:
///
/// ```
/// use mf2::{ArgValue, IntoArg};
///
/// struct Celsius(f64);
///
/// impl IntoArg for Celsius {
///     fn into_arg(self) -> ArgValue {
///         self.0.into_arg()
///     }
/// }
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a message argument",
    label = "neither a number, a string, a date, a path nor a type with `Display`",
    note = "a `tr!` argument is an integer, a float, a `bool`, a `char`, text (`&str`, `String`, \
            `Cow<'static, str>`, `Arc<str>`), a date (`DateTimeValue`, `SystemTime`, jiff's \
            types with `native`), a path, a signal of one of these, or any type with `Display`, \
            as its text",
    note = "for a type of your own, implement `Display` to pass its text, or `mf2::IntoArg` to \
            pass it as a number, a date or a value a custom function reads"
)]
pub trait IntoArg {
    /// The argument's value.
    fn into_arg(self) -> ArgValue;
}

impl IntoArg for ArgValue {
    #[inline]
    fn into_arg(self) -> ArgValue {
        self
    }
}

/// A reference to a `Copy` argument is what the value is: `&count`.
#[diagnostic::do_not_recommend]
impl<T: IntoArg + Copy> IntoArg for &T {
    #[inline]
    fn into_arg(self) -> ArgValue {
        (*self).into_arg()
    }
}

// 1.x's conversions, each the same `ArgValue::from`, so a call site compiles
// to what it did.
macro_rules! as_from {
    ($($t:ty),* $(,)?) => {$(
        impl IntoArg for $t {
            #[inline]
            fn into_arg(self) -> ArgValue {
                ArgValue::from(self)
            }
        }
    )*};
}
as_from!(
    i8,
    i16,
    i32,
    i64,
    u8,
    u16,
    u32,
    f32,
    f64,
    char,
    &str,
    String,
    Arc<str>,
    Text,
    DateTimeValue,
);

impl IntoArg for DateTime<'_> {
    #[inline]
    fn into_arg(self) -> ArgValue {
        ArgValue::from(self)
    }
}

impl<C: CustomValue + Send + Sync + 'static> IntoArg for Arc<C> {
    #[inline]
    fn into_arg(self) -> ArgValue {
        ArgValue::from(self)
    }
}

/// An integer that may not fit `i64`: exact either way. These five are out
/// of line, and share one decimal writer: inline, the branch and the call
/// went into every `usize` argument of a terminal UI (C1's record).
impl IntoArg for u64 {
    fn into_arg(self) -> ArgValue {
        match i64::try_from(self) {
            Ok(n) => ArgValue::Int(n),
            Err(_) => exact(false, u128::from(self)),
        }
    }
}

impl IntoArg for i128 {
    fn into_arg(self) -> ArgValue {
        match i64::try_from(self) {
            Ok(n) => ArgValue::Int(n),
            Err(_) => exact(self < 0, self.unsigned_abs()),
        }
    }
}

impl IntoArg for u128 {
    fn into_arg(self) -> ArgValue {
        match i64::try_from(self) {
            Ok(n) => ArgValue::Int(n),
            Err(_) => exact(false, self),
        }
    }
}

/// Without saturating, where `ArgValue::from` (1.x) does: a `usize` past
/// `i64::MAX` is its exact decimal. On a 32-bit target (the browser's) every
/// `usize` fits, and nothing else is compiled.
impl IntoArg for usize {
    fn into_arg(self) -> ArgValue {
        match i64::try_from(self) {
            Ok(n) => ArgValue::Int(n),
            Err(_) => exact(false, u128::try_from(self).unwrap_or(u128::MAX)),
        }
    }
}

impl IntoArg for isize {
    fn into_arg(self) -> ArgValue {
        match i64::try_from(self) {
            Ok(n) => ArgValue::Int(n),
            Err(_) => exact(
                self < 0,
                u128::try_from(self.unsigned_abs()).unwrap_or(u128::MAX),
            ),
        }
    }
}

macro_rules! non_zero {
    ($($t:ty),* $(,)?) => {$(
        impl IntoArg for NonZero<$t> {
            #[inline]
            fn into_arg(self) -> ArgValue {
                self.get().into_arg()
            }
        }
    )*};
}
non_zero!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

/// The string `true` or `false`, as MF2 has no boolean: `.match $flag` selects
/// on it, and no allocation is made.
impl IntoArg for bool {
    #[inline]
    fn into_arg(self) -> ArgValue {
        ArgValue::str_static(if self { "true" } else { "false" })
    }
}

/// Borrowed text stays borrowed, and owned text is shared.
impl IntoArg for Cow<'static, str> {
    #[inline]
    fn into_arg(self) -> ArgValue {
        match self {
            Cow::Borrowed(s) => ArgValue::str_static(s),
            Cow::Owned(s) => ArgValue::from(s),
        }
    }
}

/// `n`, negated when `negative`, as the `number-literal` text the runtime
/// formats and selects on exactly (`ArgValue::Decimal`): the digits written
/// one by one, so that no `core::fmt` is linked. Never inlined, so that the
/// wide integers' conversions share it.
#[inline(never)]
fn exact(negative: bool, mut n: u128) -> ArgValue {
    // `u128::MAX` has 39 digits; one more for the sign.
    let mut buf = [b'0'; 40];
    let mut len = 0;
    for slot in buf.iter_mut().rev() {
        *slot = b'0' | u8::try_from(n % 10).unwrap_or(0);
        n /= 10;
        len += 1;
        if n == 0 {
            break;
        }
    }
    if negative
        && let Some(slot) = buf
            .len()
            .checked_sub(len + 1)
            .and_then(|at| buf.get_mut(at))
    {
        *slot = b'-';
        len += 1;
    }
    let digits = buf
        .get(buf.len().saturating_sub(len)..)
        .and_then(|d| core::str::from_utf8(d).ok())
        .unwrap_or("0");
    ArgValue::Decimal(Text::Shared(Arc::from(digits)))
}

/// The text of a value that is an argument only through its `Display`: made
/// now, once — what 1.x's `.to_string()` at the call site made. Generic, so
/// that nothing of it is compiled into a build whose call sites never take
/// this step: a function here that formats would reach `String`'s own code
/// in every client (C1's record: demo-islands' `String::push`).
#[inline]
pub(crate) fn display<T: core::fmt::Display + ?Sized>(value: &T) -> ArgValue {
    use alloc::string::ToString;
    ArgValue::from(value.to_string())
}

/// Paths, OS strings and the system clock: their text, lossy where it is not
/// UTF-8, and an instant.
#[cfg(any(
    feature = "host-std",
    feature = "native",
    all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    )
))]
mod with_std {
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ArgValue, Cow, DateTimeValue, IntoArg};

    /// `s` as text: borrowed when it is UTF-8, else with U+FFFD in place of
    /// what is not.
    fn lossy(s: &OsStr) -> ArgValue {
        match s.to_string_lossy() {
            Cow::Borrowed(s) => ArgValue::from(s),
            Cow::Owned(s) => ArgValue::from(s),
        }
    }

    /// `s` as text, its buffer reused when it is UTF-8.
    fn owned(s: OsString) -> ArgValue {
        match s.into_string() {
            Ok(s) => ArgValue::from(s),
            Err(s) => lossy(&s),
        }
    }

    impl IntoArg for &Path {
        fn into_arg(self) -> ArgValue {
            lossy(self.as_os_str())
        }
    }

    impl IntoArg for PathBuf {
        fn into_arg(self) -> ArgValue {
            owned(self.into_os_string())
        }
    }

    impl IntoArg for &OsStr {
        fn into_arg(self) -> ArgValue {
            lossy(self)
        }
    }

    impl IntoArg for OsString {
        fn into_arg(self) -> ArgValue {
            owned(self)
        }
    }

    /// Beside `&T` for `T: IntoArg + Copy`, a reference to a type that is not
    /// `Copy` cannot be an `IntoArg`; `tr!` takes it through this.
    impl From<&PathBuf> for ArgValue {
        fn from(p: &PathBuf) -> ArgValue {
            lossy(p.as_os_str())
        }
    }

    /// As `&PathBuf`.
    impl From<&OsString> for ArgValue {
        fn from(s: &OsString) -> ArgValue {
            lossy(s)
        }
    }

    /// The instant, to the millisecond (floored, as a date/time value holds
    /// milliseconds); nothing ([`ArgValue::Unset`]) past the years a date
    /// can hold.
    impl IntoArg for SystemTime {
        fn into_arg(self) -> ArgValue {
            let ms = match self.duration_since(UNIX_EPOCH) {
                Ok(after) => i64::try_from(after.as_millis()).ok(),
                Err(before) => {
                    let d = before.duration();
                    let ms = d.as_millis() + u128::from(d.subsec_nanos() % 1_000_000 != 0);
                    i64::try_from(ms).ok().map(|ms| -ms)
                }
            };
            ms.and_then(DateTimeValue::instant)
                .map_or(ArgValue::Unset, ArgValue::from)
        }
    }
}

/// jiff's instants and civil dates, wherever the native host carries jiff
/// — `host-std` with `datetime` — through that crate's re-export, so
/// that this one has no jiff dependency of its own (`plan/01` §4.1).
#[cfg(all(feature = "host-std", feature = "datetime"))]
mod with_jiff {
    use alloc::string::String;

    use mf2_host_std::jiff;
    use mf2_host_std::jiff::Zoned;
    use mf2_host_std::jiff::civil;
    use mf2_runtime::{Date, DateTime, Time};

    use super::{ArgValue, DateTimeValue, IntoArg};

    fn date(d: civil::Date) -> Option<Date> {
        Date::new(
            i32::from(d.year()),
            u8::try_from(d.month()).ok()?,
            u8::try_from(d.day()).ok()?,
        )
    }

    fn time(t: civil::Time) -> Option<Time> {
        Time::new(
            u8::try_from(t.hour()).ok()?,
            u8::try_from(t.minute()).ok()?,
            u8::try_from(t.second()).ok()?,
            u16::try_from(t.millisecond()).ok()?,
        )
    }

    fn floating(dt: civil::DateTime) -> Option<DateTime<'static>> {
        Some(DateTime::floating(date(dt.date())?, time(dt.time())?))
    }

    /// The instant, in UTC, to the millisecond (floored).
    impl IntoArg for jiff::Timestamp {
        fn into_arg(self) -> ArgValue {
            i64::try_from(self.as_nanosecond().div_euclid(1_000_000))
                .ok()
                .and_then(DateTimeValue::instant)
                .map_or(ArgValue::Unset, ArgValue::from)
        }
    }

    /// The instant at its offset, in its IANA zone when it has one, so that
    /// `timeZone=input` shows it there.
    impl IntoArg for Zoned {
        fn into_arg(self) -> ArgValue {
            zoned(&self)
        }
    }

    /// As `Zoned`: `Zoned` is not `Copy`, so its reference is not an `IntoArg`
    /// (and would otherwise be taken as its text).
    impl From<&Zoned> for ArgValue {
        fn from(z: &Zoned) -> ArgValue {
            zoned(z)
        }
    }

    fn zoned(z: &Zoned) -> ArgValue {
        let at = floating(z.datetime()).and_then(|dt| dt.with_offset(z.offset().seconds()));
        let Some(at) = at else {
            // An offset a date/time value cannot hold: the same instant in UTC.
            return z.timestamp().into_arg();
        };
        let value = DateTimeValue::from(at);
        match z.time_zone().iana_name() {
            Some(name) => ArgValue::from(value.with_zone(String::from(name))),
            None => ArgValue::from(value),
        }
    }

    /// A floating date and time: no offset, so the formatting time zone
    /// applies.
    impl IntoArg for civil::DateTime {
        fn into_arg(self) -> ArgValue {
            floating(self).map_or(ArgValue::Unset, ArgValue::from)
        }
    }

    /// A floating date, at 00:00 (a date literal's time).
    impl IntoArg for civil::Date {
        fn into_arg(self) -> ArgValue {
            date(self).map_or(ArgValue::Unset, |d| {
                ArgValue::from(DateTime::floating(d, Time::MIDNIGHT))
            })
        }
    }
}
