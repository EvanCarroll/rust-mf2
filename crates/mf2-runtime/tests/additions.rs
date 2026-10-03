//! The Phase 4 additions of `plans/03-runtime.md` §2.7, through the public
//! API only — the way `mf2-fn-number` and `mf2-fn-datetime` use them: a
//! handler on `NumberSpec` reading `Digits`, a `Measure` value, the
//! unannotated-number hook, date/time arguments, the context's time zone.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::items_after_statements,
    clippy::trivially_copy_pass_by_ref
)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use mf2_catalog::writer::{self, Options as WriterOptions};
use mf2_catalog::{Catalog, Dir, MsgId};
use mf2_runtime::functions::{NUMBER, STRING};
use mf2_runtime::{
    Arg, BidiStrategy, Date, DateTime, Digits, ErrorSink, FnContext, FormatContext, FormatError,
    Formatter, Function, Host, Measure, MeasureUnit, Number, NumberSpec, Options, Part, PartSink,
    Registry, Sign, Sink, SubPartSink, Time, TimeZone, Value, ZoneOption,
};

struct TestHost;

impl Host for TestHost {
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        let mut b = ryu::Buffer::new();
        let s = b.format_finite(x).as_bytes();
        let out = buf.get_mut(..s.len())?;
        out.copy_from_slice(s);
        std::str::from_utf8(out).ok()
    }
}

static HOST: TestHost = TestHost;

/// en cardinal: `one: i = 1 and v = 0`.
const EN_CARDINAL: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];

/// A `:percent` over the public core: `NumberSpec::PERCENT`, digits written
/// as `<sign><digits>%` with `,` for the decimal separator (a stand-in for
/// locale symbols).
struct Percent;

fn localized(d: &Digits<'_>, out: &mut dyn Sink) {
    match d.sign() {
        Sign::Minus => out.push_str("−"),
        Sign::Plus => out.push_str("+"),
        Sign::None => {}
    }
    let mut buf = [0u8; 1];
    for m in (0..i16::try_from(d.integer_count()).unwrap()).rev() {
        buf[0] = b'0' + d.digit(m);
        out.push_str(std::str::from_utf8(&buf).unwrap());
    }
    if d.fraction_count() > 0 {
        out.push_str(",");
        for m in 1..=i16::try_from(d.fraction_count()).unwrap() {
            buf[0] = b'0' + d.digit(-m);
            out.push_str(std::str::from_utf8(&buf).unwrap());
        }
    }
}

impl Function for Percent {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        Number::resolve(NumberSpec::PERCENT, cx, operand, options, errs).map(Value::Number)
    }

    fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Number(n) = value
            && let Some(d) = n.digits()
        {
            localized(&d, out);
            out.push_str("%");
        }
    }

    fn selectable(&self, value: &Value<'_>) -> bool {
        matches!(value, Value::Number(n) if n.is_selectable())
    }

    fn matches(
        &self,
        cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        errs: &mut dyn ErrorSink,
    ) -> bool {
        matches!(value, Value::Number(n) if n.matches(cx, key, errs))
    }

    fn better_than(&self, _cx: &FnContext<'_>, _value: &Value<'_>, k1: &str, k2: &str) -> bool {
        Number::better_than(k1, k2)
    }
}

/// A toy `:currency`: `code=` required, two fraction digits, a `Measure`.
struct Currency;

impl Function for Currency {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        let inherited = match operand {
            Some(Value::Measure(m)) => Some((m.unit, m.flags)),
            _ => None,
        };
        let code = options
            .get("code")
            .and_then(|o| o.value.as_str())
            .and_then(|s| <[u8; 3]>::try_from(s.as_bytes()).ok())
            .map(MeasureUnit::Currency);
        let (unit, flags) = match (code, inherited) {
            (Some(u), _) => (u, 1),
            (None, Some(i)) => i,
            (None, None) => {
                errs.error(FormatError::BadOperand);
                return None;
            }
        };
        let number = Number::resolve(NumberSpec::currency(2), cx, operand, options, errs)?;
        Some(Value::Measure(Measure::new(number, unit, flags + 1)))
    }

    fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Measure(m) = value
            && let Some(d) = m.number.digits()
        {
            d.write_neutral(out);
            out.push_str(" ");
            out.push_str(m.unit.as_str());
            out.push_str(if m.flags > 2 { " (inherited)" } else { "" });
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }
}

/// The unannotated-number hook: `⟨exact value⟩`.
struct Plain;

impl Function for Plain {
    fn resolve<'a>(
        &self,
        _cx: &FnContext<'_>,
        _operand: Option<&Value<'a>>,
        _options: &Options<'_, 'a>,
        _errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        None
    }

    fn formattable(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Result<(), FormatError> {
        // Never asked: the evaluator checks unannotated values itself.
        Err(FormatError::MessageFunctionError)
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Some(n) = value.to_number(cx.host()) {
            out.push_str("⟨");
            n.write_plain(out);
            out.push_str("⟩");
        }
    }

    fn part_kind(&self) -> &'static str {
        "localized"
    }
}

/// `{:zone}`: the context's time zone.
struct Zone;

impl Function for Zone {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        _operand: Option<&Value<'a>>,
        _options: &Options<'_, 'a>,
        _errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        Some(Value::Str(match cx.time_zone().as_option() {
            ZoneOption::Utc => "UTC",
            ZoneOption::Named(_) => "named",
            _ => "other",
        }))
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let (Value::Str(s), ZoneOption::Named(n)) = (value, cx.time_zone().as_option()) {
            out.push_str(s);
            out.push_str(" ");
            out.push_str(n);
        } else if let Value::Str(s) = value {
            out.push_str(s);
        }
    }
}

static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("cur", &Currency),
    ("number", &NUMBER),
    ("percent", &Percent),
    ("string", &STRING),
    ("zone", &Zone),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static LOCALIZED: Registry = Registry::new(&FUNCTIONS).with_numbers(&Plain);

fn catalog(src: &str) -> Catalog {
    let parsed = mf2_syntax::parse_model(src);
    let model = parsed.message.expect("parses");
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = WriterOptions::new("en", Dir::Ltr);
    options.locale_entries = vec![(1, EN_CARDINAL.to_vec())];
    let (bytes, manifest) = writer::single(&model, &slots, &options).expect("writes");
    Catalog::new(bytes, manifest.hash()).expect("loads")
}

fn run(
    registry: &Registry,
    cx: &FormatContext,
    src: &str,
    args: &[(&str, Arg<'_>)],
) -> (String, Vec<FormatError>) {
    let cat = catalog(src);
    let f = Formatter::new(&cat, registry, cx);
    let mut out = String::new();
    let mut errs = Vec::new();
    f.write_named(MsgId::from_raw(0), args, &mut out, &mut errs);
    (out, errs)
}

fn cx() -> FormatContext {
    let mut cx = FormatContext::new(&HOST);
    cx.bidi = BidiStrategy::None;
    cx
}

fn ok(registry: &Registry, src: &str, args: &[(&str, Arg<'_>)]) -> String {
    let (s, e) = run(registry, &cx(), src, args);
    assert!(e.is_empty(), "{src}: unexpected errors {e:?}");
    s
}

#[test]
fn percent_on_the_public_core() {
    assert_eq!(
        ok(
            &REGISTRY,
            "{0.12345678 :percent maximumFractionDigits=1}",
            &[]
        ),
        "12,3%"
    );
    assert_eq!(ok(&REGISTRY, "{-1 :percent}", &[]), "−100%");
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {0.01 :percent} {{{$n :percent}}}",
            &[]
        ),
        "1%"
    );
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {0.42 :number} {{{$n :percent}}}",
            &[]
        ),
        "42%"
    );
    // The value is not multiplied: `:number` of a percent is the operand.
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {0.5 :percent} {{{$n :number}}}",
            &[]
        ),
        "0.5"
    );
    let sel = ".input {$n :percent} .match $n one {{one}} 100 {{hundred}} * {{other}}";
    assert_eq!(ok(&REGISTRY, sel, &[("n", Arg::Float(0.01))]), "one");
    assert_eq!(ok(&REGISTRY, sel, &[("n", Arg::Int(1))]), "hundred");
    assert_eq!(ok(&REGISTRY, sel, &[("n", Arg::Int(2))]), "other");
    // `:percent` has no `select`: ignored, not an error.
    assert_eq!(ok(&REGISTRY, "{1 :percent select=exact}", &[]), "100%");
    let (s, e) = run(&REGISTRY, &cx(), "{x :percent}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{|x|}", &[FormatError::BadOperand][..])
    );
}

#[test]
fn measure_values() {
    assert_eq!(ok(&REGISTRY, "{42 :cur code=EUR}", &[]), "42.00 EUR");
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {42 :cur code=EUR} {{{$n :cur}}}",
            &[]
        ),
        "42.00 EUR (inherited)"
    );
    // A measure is a numeric operand, with its digit options.
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {4.5 :cur code=EUR} {{{$n :number}}}",
            &[]
        ),
        "4.5"
    );
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {4.5 :cur code=EUR minimumIntegerDigits=3} {{{$n :number}}}",
            &[]
        ),
        "004.5"
    );
    // `:currency` does not take `:number`'s fraction digits from its operand.
    assert_eq!(
        ok(
            &REGISTRY,
            ".local $n = {4.5 :number minimumFractionDigits=4} {{{$n :cur code=USD}}}",
            &[]
        ),
        "4.50 USD"
    );
    // Not selectable: Bad Selector.
    let (s, e) = run(
        &REGISTRY,
        &cx(),
        ".local $n = {42 :cur code=EUR} .match $n * {{other}}",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("other", &[FormatError::BadSelector][..])
    );
}

#[test]
fn unannotated_numbers_through_the_hook() {
    let args = [
        ("i", Arg::Int(-7)),
        ("f", Arg::Float(1.3)),
        ("d", Arg::Decimal("4.20")),
        ("s", Arg::Str("x")),
    ];
    let src = "{$i} {$f} {$d} {$s}";
    assert_eq!(ok(&REGISTRY, src, &args), "-7 1.3 4.2 x");
    assert_eq!(ok(&LOCALIZED, src, &args), "⟨-7⟩ ⟨1.3⟩ ⟨4.2⟩ x");
    // Through a declaration, too; annotated values keep their handler.
    assert_eq!(
        ok(&LOCALIZED, ".local $x = {$i} {{{$x} {$i :number}}}", &args),
        "⟨-7⟩ -7"
    );
    // The evaluator's errors, as without the hook; the part kind; still no
    // selection.
    let (s, e) = run(&LOCALIZED, &cx(), "{$x}", &[("x", Arg::Float(f64::NAN))]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{$x}", &[FormatError::BadOperand][..])
    );
    let long = "1".repeat(41);
    let (s, e) = run(&LOCALIZED, &cx(), "{$x}", &[("x", Arg::Decimal(&long))]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{$x}", &[FormatError::UnsupportedOperation][..])
    );
    let (s, e) = run(&LOCALIZED, &cx(), ".match $i * {{any}}", &args);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("any", &[FormatError::BadSelector][..])
    );

    struct Kinds(Vec<String>);
    impl PartSink for Kinds {
        fn part(&mut self, part: Part<'_>) {
            if let Part::Expression(e) = part {
                self.0.push(e.kind().to_owned());
            }
        }
    }
    let cat = catalog("{$i}{$s}");
    let cx = cx();
    let f = Formatter::new(&cat, &LOCALIZED, &cx);
    let mut kinds = Kinds(Vec::new());
    f.parts_named(MsgId::from_raw(0), &args, &mut kinds, &mut Vec::new());
    assert_eq!(kinds.0, ["localized", "string"]);
}

/// A date handler for unannotated dates: the ISO text.
struct Iso;

impl Function for Iso {
    fn resolve<'a>(
        &self,
        _cx: &FnContext<'_>,
        _operand: Option<&Value<'a>>,
        _options: &Options<'_, 'a>,
        _errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        None
    }

    fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::DateTime(d) = value {
            d.write_iso(out);
        }
    }

    fn part_kind(&self) -> &'static str {
        "datetime"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> mf2_runtime::Dir {
        mf2_runtime::Dir::Ltr
    }
}

static DATED: Registry = Registry::new(&FUNCTIONS).with_dates(&Iso);

#[test]
fn date_time_arguments() {
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
    let floating = DateTime::floating(
        Date::new(2006, 1, 2).unwrap(),
        Time::new(15, 4, 6, 250).unwrap(),
    );
    let zoned = instant.with_offset(3600).unwrap().in_zone("Europe/Paris");
    let args = [
        ("a", Arg::from(&instant)),
        ("b", Arg::from(&floating)),
        ("c", Arg::DateTime(&zoned)),
    ];
    // Without a date handler: Bad Operand, no date code.
    let (s, e) = run(&REGISTRY, &cx(), "{$a} | {$b}", &args);
    assert_eq!(s, "{$a} | {$b}");
    assert_eq!(e, [FormatError::BadOperand, FormatError::BadOperand]);
    // With one (fn-datetime's place).
    assert_eq!(
        ok(&DATED, "{$a} | {$b} | {$c}", &args),
        "2006-01-02T15:04:06Z | 2006-01-02T15:04:06.250 | 2006-01-02T15:04:06+01:00[Europe/Paris]"
    );
    // Neither a string (`:string`) nor a numeric operand.
    for src in ["{$a :string}", "{$a :number}"] {
        let (_, e) = run(&DATED, &cx(), src, &args);
        assert_eq!(e, [FormatError::BadOperand], "{src}");
    }
    // Not selectable, even with a handler.
    let (s, e) = run(&DATED, &cx(), ".match $a * {{any}}", &args);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("any", &[FormatError::BadSelector][..])
    );

    struct Kind(Vec<(String, mf2_runtime::Dir)>);
    impl PartSink for Kind {
        fn part(&mut self, part: Part<'_>) {
            if let Part::Expression(e) = part {
                self.0.push((e.kind().to_owned(), e.dir()));
            }
        }
    }
    let cat = catalog("{$a}");
    let cx = cx();
    let f = Formatter::new(&cat, &DATED, &cx);
    let mut k = Kind(Vec::new());
    f.parts_named(MsgId::from_raw(0), &args, &mut k, &mut Vec::new());
    assert_eq!(k.0, [("datetime".to_owned(), mf2_runtime::Dir::Ltr)]);
}

#[test]
fn the_context_time_zone() {
    assert_eq!(ok(&REGISTRY, "{:zone}", &[]), "UTC");
    let mut cx = cx();
    cx.time_zone = TimeZone::named("Asia/Kolkata").unwrap();
    let (s, e) = run(&REGISTRY, &cx, "{:zone}", &[]);
    assert!(e.is_empty());
    assert_eq!(s, "named Asia/Kolkata");
    // The host defaults: no zone data, no date formatter.
    assert_eq!(HOST.zone_offset("Asia/Kolkata", 0), None);
}

#[test]
fn digits_view() {
    struct Sub(Vec<(String, String)>);
    impl SubPartSink for Sub {
        fn sub_part(&mut self, kind: &str, text: &str) {
            self.0.push((kind.to_owned(), text.to_owned()));
        }
    }
    // Resolve through a handler and read the digits back.
    struct Probe;
    impl Function for Probe {
        fn resolve<'a>(
            &self,
            cx: &FnContext<'_>,
            operand: Option<&Value<'a>>,
            options: &Options<'_, 'a>,
            errs: &mut dyn ErrorSink,
        ) -> Option<Value<'a>> {
            Number::resolve(NumberSpec::NUMBER, cx, operand, options, errs).map(Value::Number)
        }
        fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
            if let Value::Number(n) = value
                && let Some(d) = n.digits()
            {
                let mut s = String::new();
                d.write_neutral(&mut s);
                let mut sub = Sub(Vec::new());
                d.neutral_parts(&mut sub);
                let desc = format!(
                    "{s} int={} frac={} zero={} d0={} d-1={} d5={} g={:?} parts={:?}",
                    d.integer_count(),
                    d.fraction_count(),
                    d.is_zero(),
                    d.digit(0),
                    d.digit(-1),
                    d.digit(5),
                    n.grouping(),
                    sub.0
                );
                out.push_str(&desc);
            }
        }
    }
    static PROBE: [(&str, &dyn Function); 1] = [("probe", &Probe)];
    static R: Registry = Registry::new(&PROBE);
    assert_eq!(
        ok(
            &R,
            "{-1234.5 :probe minimumIntegerDigits=6 minimumFractionDigits=2 useGrouping=min2 signDisplay=always}",
            &[]
        ),
        "-001234.50 int=6 frac=2 zero=false d0=4 d-1=5 d5=0 g=Some(Min2) \
         parts=[(\"minusSign\", \"-\"), (\"integer\", \"001234\"), (\"decimal\", \".\"), (\"fraction\", \"50\")]"
    );
    assert_eq!(
        ok(&R, "{0 :probe signDisplay=always}", &[]),
        "+0 int=1 frac=0 zero=true d0=0 d-1=0 d5=0 g=None parts=[(\"plusSign\", \"+\"), (\"integer\", \"0\")]"
    );
}

#[test]
fn exact_digits() {
    let n = Number::parse("-1234.50").unwrap();
    let d = n.exact_digits();
    let mut s = String::new();
    d.write_neutral(&mut s);
    assert_eq!(s, "-1234.5");
    assert_eq!((d.integer_count(), d.fraction_count()), (4, 1));
    assert_eq!(
        (d.digit(3), d.digit(0), d.digit(-1), d.digit(-2)),
        (1, 4, 5, 0)
    );
    assert!(n.digits().is_none());
    let z = Number::parse("0.001").unwrap();
    let d = z.exact_digits();
    assert_eq!(
        (d.integer_count(), d.fraction_count(), d.sign()),
        (1, 3, Sign::None)
    );
    let m0 = Number::from_f64(-0.0, &HOST).unwrap();
    assert_eq!(m0.exact_digits().sign(), Sign::Minus);
    assert!(m0.exact_digits().is_zero());
}

#[test]
fn digits_operands() {
    // `1.0` is `other` in English, `1` is `one`: operands as shown.
    struct Cat;
    impl Function for Cat {
        fn resolve<'a>(
            &self,
            cx: &FnContext<'_>,
            operand: Option<&Value<'a>>,
            options: &Options<'_, 'a>,
            errs: &mut dyn ErrorSink,
        ) -> Option<Value<'a>> {
            Number::resolve(NumberSpec::UNIT, cx, operand, options, errs).map(Value::Number)
        }
        fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
            if let Value::Number(n) = value
                && let Some(d) = n.digits()
            {
                let rules = cx
                    .catalog()
                    .locale_entry(mf2_catalog::format::locale_key::PLURAL_CARDINAL)
                    .unwrap_or(&[]);
                let c = mf2_runtime::plural_category(rules, &d.operands());
                out.push_str(if c == mf2_runtime::Category::One {
                    "one"
                } else {
                    "other"
                });
            }
        }
    }
    static CAT: [(&str, &dyn Function); 1] = [("cat", &Cat)];
    static R: Registry = Registry::new(&CAT);
    assert_eq!(ok(&R, "{1 :cat}", &[]), "one");
    assert_eq!(ok(&R, "{1 :cat minimumFractionDigits=1}", &[]), "other");
    assert_eq!(ok(&R, "{1.04 :cat maximumFractionDigits=1}", &[]), "one"); // shows 1
    assert_eq!(
        ok(
            &R,
            "{1.04 :cat minimumFractionDigits=1 maximumFractionDigits=1}",
            &[]
        ),
        "other"
    ); // shows 1.0
    assert_eq!(ok(&R, "{1.04 :cat maximumFractionDigits=0}", &[]), "one");
}

/// `{:probe}`: counts its calls and remembers whether it was shown a `u:`
/// option. formatting.md: every expression is evaluated at most once
/// (call-by-need, not call-by-name), and `u:` options may be taken out of
/// the options a handler gets — ours always are.
struct Probe {
    calls: AtomicUsize,
    saw_u: AtomicBool,
}

impl Function for Probe {
    fn resolve<'a>(
        &self,
        _cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        _errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if options.iter().any(|(name, _)| name.starts_with("u:")) {
            self.saw_u.store(true, Ordering::SeqCst);
        }
        match operand {
            Some(Value::Str(s)) => Some(Value::Str(s)),
            _ => Some(Value::Str("")),
        }
    }

    fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Str(s) = value {
            out.push_str(s);
        }
    }

    fn selectable(&self, _value: &Value<'_>) -> bool {
        true
    }

    fn matches(
        &self,
        _cx: &FnContext<'_>,
        value: &Value<'_>,
        key: &str,
        _errs: &mut dyn ErrorSink,
    ) -> bool {
        matches!(value, Value::Str(s) if *s == key)
    }
}

#[test]
fn a_declaration_is_resolved_once_and_a_handler_sees_no_u_options() {
    static PROBE: Probe = Probe {
        calls: AtomicUsize::new(0),
        saw_u: AtomicBool::new(false),
    };
    static PROBED: [(&str, &dyn Function); 2] = [("probe", &PROBE), ("string", &STRING)];
    static PROBE_REGISTRY: Registry = Registry::new(&PROBED);

    // One declaration used as the selector, twice as a placeholder, as an
    // operand and as an option value: one call, and no `u:` option seen.
    let src = ".local $x = {a :probe u:id=i u:dir=ltr} .match $x \
               a {{{$x} {$x} {$x :string} {b :string k=$x}}} * {{other}}";
    let (s, e) = run(&PROBE_REGISTRY, &cx(), src, &[]);
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(s, "a a a b");
    assert_eq!(PROBE.calls.load(Ordering::SeqCst), 1);
    assert!(!PROBE.saw_u.load(Ordering::SeqCst));

    // A declaration nothing uses is never resolved.
    let (s, e) = run(&PROBE_REGISTRY, &cx(), ".local $y = {a :probe} {{ok}}", &[]);
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(s, "ok");
    assert_eq!(PROBE.calls.load(Ordering::SeqCst), 1);
}
