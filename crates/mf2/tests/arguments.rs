//! What a `tr!` argument may be (`plans/19-native-and-terminal.md` §7):
//! each [`IntoArg`] conversion, exact, formatted against a catalog
//! `compile_str` wrote; and the dispatch `tr!` expands an argument to, step
//! by step. Signals are
//! `tests/render.rs`'s.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::net::Ipv4Addr;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use mf2::{
    ArgValue, Compiled, CustomValue, Date, DateTime, DateTimeValue, FormatContext, Formatter,
    Function, IntoArg, Number, Registry, Text, Time, functions, tr_args1,
};

static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("datetime", &mf2::fn_datetime::DATETIME),
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("string", &functions::STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static CX: FormatContext = FormatContext::new(&mf2::host_std::ZONES_HOST);

/// `value` in a one-variable message, with the errors discarded.
fn formatted(source: &str, value: ArgValue) -> String {
    let c = mf2::compile_str(source, "en").expect("the message compiles");
    tr_args1(Compiled::ID, value).format(&Formatter::new(&c.catalog, &REGISTRY, &CX))
}

/// What `tr!` expands `name = e` to, as the macro writes it: the argument
/// through the first step its type allows.
macro_rules! arg {
    ($e:expr) => {{
        #[allow(unused_imports)]
        use mf2::__arg::{KindDisplay as _, KindFrom as _, KindIntoArg as _, KindNeither as _};
        mf2::__arg::convert($e, |probe| (&&&probe).__mf2_kind())
    }};
}

/// Its `Debug` form, which shows the variant and the value exactly.
fn shown(value: &ArgValue) -> String {
    format!("{value:?}")
}

#[test]
fn integers_are_exact_to_128_bits() {
    // Within `i64`, an integer; past it, the exact decimal, written without
    // `core::fmt`, which `:integer` formats and a `.match` selects on as it
    // is (core output never groups).
    let cases: [(ArgValue, &str, &str); 16] = [
        (i8::MIN.into_arg(), "Int(-128)", "-128"),
        (i16::MAX.into_arg(), "Int(32767)", "32767"),
        (i32::MIN.into_arg(), "Int(-2147483648)", "-2147483648"),
        (
            i64::MIN.into_arg(),
            "Int(-9223372036854775808)",
            "-9223372036854775808",
        ),
        (u8::MAX.into_arg(), "Int(255)", "255"),
        (u16::MAX.into_arg(), "Int(65535)", "65535"),
        (u32::MAX.into_arg(), "Int(4294967295)", "4294967295"),
        (
            9_223_372_036_854_775_807_u64.into_arg(),
            "Int(9223372036854775807)",
            "9223372036854775807",
        ),
        (
            9_223_372_036_854_775_808_u64.into_arg(),
            "Decimal(\"9223372036854775808\")",
            "9223372036854775808",
        ),
        (
            u64::MAX.into_arg(),
            "Decimal(\"18446744073709551615\")",
            "18446744073709551615",
        ),
        (
            u128::MAX.into_arg(),
            "Decimal(\"340282366920938463463374607431768211455\")",
            "340282366920938463463374607431768211455",
        ),
        (
            i128::MIN.into_arg(),
            "Decimal(\"-170141183460469231731687303715884105728\")",
            "-170141183460469231731687303715884105728",
        ),
        (
            (i128::from(i64::MIN) - 1).into_arg(),
            "Decimal(\"-9223372036854775809\")",
            "-9223372036854775809",
        ),
        (i128::from(-7).into_arg(), "Int(-7)", "-7"),
        (0_u128.into_arg(), "Int(0)", "0"),
        (
            isize::MIN.into_arg(),
            "Int(-9223372036854775808)",
            "-9223372036854775808",
        ),
    ];
    for (value, debug, text) in cases {
        assert_eq!(shown(&value), debug);
        assert_eq!(formatted("{$n :integer}", value), text, "{debug}");
    }

    // `usize` does not saturate, where 1.x's `ArgValue::from` does (and
    // still does: `From` stays as it was).
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(
            shown(&usize::MAX.into_arg()),
            "Decimal(\"18446744073709551615\")"
        );
        assert_eq!(
            shown(&ArgValue::from(usize::MAX)),
            "Int(9223372036854775807)"
        );
    }
    assert_eq!(shown(&7_usize.into_arg()), "Int(7)");

    // The `NonZero` forms are their value.
    assert_eq!(shown(&NonZero::new(3_u8).unwrap().into_arg()), "Int(3)");
    assert_eq!(
        shown(&NonZero::new(u64::MAX).unwrap().into_arg()),
        "Decimal(\"18446744073709551615\")"
    );
    assert_eq!(
        shown(&NonZero::new(i128::MIN).unwrap().into_arg()),
        "Decimal(\"-170141183460469231731687303715884105728\")"
    );

    // A number past `i64` still selects, as a number: `one` is exactly 1.
    let plural = ".input {$n :integer} .match $n 1 {{one}} * {{{$n} many}}";
    assert_eq!(
        formatted(plural, u64::MAX.into_arg()),
        "18446744073709551615 many"
    );
    assert_eq!(
        formatted(plural, NonZero::new(1_u128).unwrap().into_arg()),
        "one"
    );
}

#[test]
fn floats_and_chars_are_what_1x_made_of_them() {
    assert_eq!(shown(&2.5_f64.into_arg()), "Float(2.5)");
    assert_eq!(shown(&0.5_f32.into_arg()), "Float(0.5)");
    assert_eq!(shown(&'é'.into_arg()), "Str(\"é\")");
    assert_eq!(formatted("{$n :number}", 2.5_f64.into_arg()), "2.5");
}

#[test]
fn a_bool_is_the_string_a_match_selects_on() {
    let ArgValue::Str(Text::Static(on)) = true.into_arg() else {
        panic!("a bool is a static string: no allocation");
    };
    assert_eq!(on, "true");
    assert_eq!(shown(&false.into_arg()), "Str(\"false\")");
    let switch = ".input {$on :string} .match $on true {{on}} false {{off}} * {{other}}";
    assert_eq!(formatted(switch, true.into_arg()), "on");
    assert_eq!(formatted(switch, false.into_arg()), "off");
}

#[test]
fn text_is_shared_copied_or_kept_static() {
    static HELLO: &str = "hello";
    // Borrowed static text stays borrowed: the same bytes, not a copy.
    let ArgValue::Str(Text::Static(s)) = Cow::Borrowed(HELLO).into_arg() else {
        panic!("a borrowed Cow<'static, str> stays static");
    };
    assert!(std::ptr::eq(s, HELLO));
    // Owned text is counted…
    let ArgValue::Str(Text::Shared(owned)) = Cow::<'static, str>::Owned("x".into()).into_arg()
    else {
        panic!("an owned Cow is shared");
    };
    assert_eq!(&*owned, "x");
    // …an `Arc<str>` is shared as it is…
    let arc: Arc<str> = Arc::from("shared");
    let ArgValue::Str(Text::Shared(same)) = arc.clone().into_arg() else {
        panic!("an Arc<str> is shared");
    };
    assert!(Arc::ptr_eq(&arc, &same));
    // …and a `&str`, a `String` and a `Text` are what 1.x made of them.
    let name = String::from("Ada");
    assert_eq!(shown(&name.as_str().into_arg()), "Str(\"Ada\")");
    assert_eq!(shown(&name.clone().into_arg()), "Str(\"Ada\")");
    assert_eq!(shown(&Text::Static("Ada").into_arg()), "Str(\"Ada\")");
    assert_eq!(
        formatted("Hello, {$name}!", name.into_arg()),
        "Hello, \u{2068}Ada\u{2069}!"
    );
}

#[test]
fn a_reference_to_a_copy_argument_is_the_argument() {
    let n = 3_i64;
    let big = u64::MAX;
    let s = "Ada";
    assert_eq!(shown(&(&n).into_arg()), "Int(3)");
    assert_eq!(
        shown(&(&big).into_arg()),
        "Decimal(\"18446744073709551615\")"
    );
    assert_eq!(shown(&(&s).into_arg()), "Str(\"Ada\")");
    assert_eq!(shown(&(&&true).into_arg()), "Str(\"true\")");
}

#[test]
fn paths_and_os_strings_are_their_text() {
    let path = PathBuf::from("locales/en");
    let text = "Str(\"locales/en\")";
    assert_eq!(shown(&path.as_path().into_arg()), text);
    assert_eq!(shown(&path.as_os_str().into_arg()), text);
    assert_eq!(shown(&path.clone().into_os_string().into_arg()), text);
    assert_eq!(shown(&(&path.as_path()).into_arg()), text);
    // A reference to a type that is not `Copy` is `From<&T>`'s, which the
    // dispatch takes before `Display`.
    assert_eq!(shown(&ArgValue::from(&path)), text);
    assert_eq!(shown(&ArgValue::from(&OsString::from("locales/en"))), text);
    assert_eq!(shown(&path.into_arg()), text);

    // Not UTF-8: lossy, as `Path::display` shows it.
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let odd = OsStr::from_bytes(b"caf\xe9");
        assert_eq!(shown(&odd.into_arg()), "Str(\"caf\u{FFFD}\")");
        assert_eq!(
            shown(&odd.to_os_string().into_arg()),
            "Str(\"caf\u{FFFD}\")"
        );
        assert_eq!(shown(&Path::new(odd).into_arg()), "Str(\"caf\u{FFFD}\")");
    }
}

#[test]
fn a_system_time_is_an_instant_to_the_millisecond() {
    let at = |d: DateTimeValue| shown(&ArgValue::from(d));
    let instant = |ms| at(DateTimeValue::instant(ms).expect("an instant"));
    let after = UNIX_EPOCH + Duration::from_micros(1_500_900);
    assert_eq!(shown(&after.into_arg()), instant(1_500));
    assert_eq!(
        shown(&after.into_arg()),
        "DateTime(DateTimeValue(1970-01-01T00:00:01.500 offset 0))"
    );
    // Before the epoch the millisecond is floored, as after it: 1.5 ms
    // before midnight is in the millisecond that starts 2 ms before.
    let before = UNIX_EPOCH - Duration::from_micros(1_500);
    assert_eq!(shown(&before.into_arg()), instant(-2));
    // A reference is the value (`SystemTime` is `Copy`).
    assert_eq!(shown(&(&UNIX_EPOCH).into_arg()), instant(0));
    // Past the years a date can hold, nothing: an Unresolved Variable, as a
    // missing value is.
    if let Some(far) = UNIX_EPOCH.checked_add(Duration::from_secs(40_000_000_000_000)) {
        assert_eq!(shown(&far.into_arg()), "Unset");
    }
    // It formats as the instant it is.
    let message = "{$when :datetime timeZone=|UTC|}";
    let noon = UNIX_EPOCH + Duration::from_secs(1_767_268_800);
    assert_eq!(
        formatted(message, noon.into_arg()),
        formatted(
            message,
            ArgValue::from(DateTimeValue::instant(1_767_268_800_000).expect("an instant"))
        )
    );
}

#[test]
fn dates_values_and_application_values_are_themselves() {
    struct Money(i64);
    impl CustomValue for Money {
        fn as_number(&self) -> Option<Number> {
            Some(Number::from_i64(self.0))
        }
    }

    let date = Date::new(2026, 9, 29).expect("a real date");
    let time = Time::new(12, 30, 0, 0).expect("a real time");
    let floating = "DateTime(DateTimeValue(2026-09-29T12:30:00.000))";
    assert_eq!(
        shown(&DateTimeValue::floating(date, time).into_arg()),
        floating
    );
    assert_eq!(shown(&DateTime::floating(date, time).into_arg()), floating);
    assert_eq!(
        shown(&(&DateTime::floating(date, time)).into_arg()),
        floating
    );
    // `ArgValue` is itself: an exact decimal, for one.
    assert_eq!(
        shown(&ArgValue::decimal("19.99").into_arg()),
        "Decimal(\"19.99\")"
    );

    let money = Arc::new(Money(42)).into_arg();
    assert_eq!(shown(&money), "Custom(..)");
    assert_eq!(formatted("{$amount :integer}", money), "42");
}

/// An application's type with only a `Display`: trippy's key binding.
struct KeyBinding(char);

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ctrl+{}", self.0)
    }
}

/// An application's type with 1.x's conversion, and an English `Display`
/// that 1.x never used for it.
struct Meters(f64);

impl fmt::Display for Meters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} m", self.0)
    }
}

impl From<Meters> for ArgValue {
    fn from(m: Meters) -> ArgValue {
        ArgValue::Float(m.0)
    }
}

/// An application's type with `IntoArg` and a `Display`.
#[derive(Clone, Copy)]
struct Celsius(f64);

impl fmt::Display for Celsius {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} °C", self.0)
    }
}

impl IntoArg for Celsius {
    fn into_arg(self) -> ArgValue {
        self.0.into_arg()
    }
}

fn generic_display<T: fmt::Display>(t: T) -> ArgValue {
    arg!(t)
}

fn generic_from<T>(t: T) -> ArgValue
where
    ArgValue: From<T>,
{
    arg!(t)
}

fn generic_into_arg<T: IntoArg + fmt::Display>(t: T) -> ArgValue {
    arg!(t)
}

#[test]
fn the_dispatch_takes_the_first_step_the_type_allows() {
    // 1. `IntoArg`.
    assert_eq!(shown(&arg!(3_i64)), "Int(3)");
    assert_eq!(shown(&arg!(u64::MAX)), "Decimal(\"18446744073709551615\")");
    // An unsuffixed literal is an `i32`, as 1.x's `ArgValue::from(3)` was.
    assert_eq!(shown(&arg!(3)), "Int(3)");
    assert_eq!(shown(&arg!(2.5)), "Float(2.5)");
    assert_eq!(shown(&arg!(true)), "Str(\"true\")");
    assert_eq!(shown(&arg!(Celsius(21.5))), "Float(21.5)");
    assert_eq!(shown(&arg!(&Celsius(21.5))), "Float(21.5)");
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        shown(&arg!(usize::MAX)),
        "Decimal(\"18446744073709551615\")"
    );

    // 2. `From<T> for ArgValue`: 1.x's conversion, before the text.
    assert_eq!(shown(&arg!(Meters(2.5))), "Float(2.5)");
    let name = String::from("Ada");
    assert_eq!(shown(&arg!(&name)), "Str(\"Ada\")");
    let dir = PathBuf::from("locales");
    assert_eq!(shown(&arg!(&dir)), "Str(\"locales\")");

    // 3. `Display`: the text.
    assert_eq!(shown(&arg!(KeyBinding('q'))), "Str(\"Ctrl+q\")");
    assert_eq!(shown(&arg!(&KeyBinding('x'))), "Str(\"Ctrl+x\")");
    assert_eq!(shown(&arg!(Ipv4Addr::LOCALHOST)), "Str(\"127.0.0.1\")");
    let error = std::io::Error::other("no such file");
    assert_eq!(shown(&arg!(error)), "Str(\"no such file\")");

    // Generic code takes the step its bounds allow.
    assert_eq!(shown(&generic_display(5_i64)), "Str(\"5\")");
    assert_eq!(shown(&generic_from(5_i64)), "Int(5)");
    assert_eq!(shown(&generic_from(Meters(1.5))), "Float(1.5)");
    assert_eq!(shown(&generic_into_arg(Celsius(3.0))), "Float(3.0)");
}

#[test]
fn a_display_argument_is_its_text_when_the_description_is_built() {
    use std::cell::Cell;
    struct Ticks<'a>(&'a Cell<u32>);
    impl fmt::Display for Ticks<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.set(self.0.get() + 1);
            write!(f, "tick {}", self.0.get())
        }
    }
    let count = Cell::new(0);
    let description = tr_args1(Compiled::ID, arg!(Ticks(&count)));
    assert_eq!(count.get(), 1);
    let c = mf2::compile_str("{$t}", "en").expect("the message compiles");
    let f = Formatter::new(&c.catalog, &REGISTRY, &CX);
    // Formatted twice, made once: the text is the argument, not the value.
    assert_eq!(description.format(&f), "\u{2068}tick 1\u{2069}");
    assert_eq!(description.format(&f), "\u{2068}tick 1\u{2069}");
    assert_eq!(count.get(), 1);
}

/// jiff's instants and civil dates as `tr!` arguments (`IntoArg`, with
/// `host-std` and `fn-datetime`): each is the date/time value it names, to
/// the millisecond, as the value's `Debug` shows exactly. jiff comes from
/// the native host's re-export, as an application's does, so this crate has
/// no jiff dependency of its own (`plan/01` §4.1).
#[test]
fn jiffs_instants_and_civil_dates_are_date_arguments() {
    use mf2::host_std::jiff;
    use mf2::host_std::jiff::civil::date;
    use mf2::host_std::jiff::tz::{Offset, TimeZone};
    use mf2::{ArgValue, IntoArg};

    let shown = |value: ArgValue| format!("{value:?}");
    // A timestamp is the instant, in UTC, floored to the millisecond — also
    // before the epoch.
    let ts = jiff::Timestamp::new(1, 500_900_000).expect("a timestamp");
    let one_and_a_half = "DateTime(DateTimeValue(1970-01-01T00:00:01.500 offset 0))";
    assert_eq!(shown(ts.into_arg()), one_and_a_half);
    assert_eq!(shown((&ts).into_arg()), one_and_a_half);
    let before = jiff::Timestamp::new(0, -1_500_000).expect("a timestamp");
    assert_eq!(
        shown(before.into_arg()),
        "DateTime(DateTimeValue(1969-12-31T23:59:59.998 offset 0))"
    );

    // A civil date and time is floating: the formatting zone applies.
    let dt = date(2026, 9, 29).at(12, 30, 5, 123_456_789);
    assert_eq!(
        shown(dt.into_arg()),
        "DateTime(DateTimeValue(2026-09-29T12:30:05.123))"
    );
    assert_eq!(
        shown(date(2026, 9, 29).into_arg()),
        "DateTime(DateTimeValue(2026-09-29T00:00:00.000))"
    );

    // A zoned value keeps its offset, and its zone when that has an IANA
    // name, so that `timeZone=input` shows it there.
    let paris = dt
        .in_tz("Europe/Paris")
        .expect("the zone database has Paris");
    let in_paris = "DateTime(DateTimeValue(2026-09-29T12:30:05.123 offset 7200 Europe/Paris))";
    assert_eq!(shown(paris.clone().into_arg()), in_paris);
    // `Zoned` is not `Copy`: its reference is `From<&Zoned>`'s, which `tr!`
    // takes before the value's text.
    assert_eq!(shown(ArgValue::from(&paris)), in_paris);
    let fixed = dt
        .to_zoned(TimeZone::fixed(Offset::constant(-5)))
        .expect("a fixed offset");
    assert_eq!(
        shown(fixed.into_arg()),
        "DateTime(DateTimeValue(2026-09-29T12:30:05.123 offset -18000))"
    );
}
