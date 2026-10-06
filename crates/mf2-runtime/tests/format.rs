//! The evaluator on hand-written messages: compile with `writer::single`,
//! format from the catalog, check text, parts and errors.

use std::fmt::Write as _;

use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Catalog, Dir, MsgId};
use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{
    Arg, BidiStrategy, FormatContext, FormatError, Formatter, Function, Host, Part, PartSink,
    Registry, SubPartSink,
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

static CORE: [(&str, &dyn Function); 4] = [
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("string", &STRING),
];
static REGISTRY: Registry = Registry::new(&CORE);

/// en cardinal: `one: i = 1 and v = 0`.
const EN_CARDINAL: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];

fn catalog(src: &str, dir: Dir) -> Catalog {
    let parsed = mf2_syntax::parse_model(src);
    let model = parsed.message.expect("parses");
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = Options::new("en", dir);
    options.locale_entries = vec![(1, EN_CARDINAL.to_vec())];
    let (bytes, manifest) = writer::single(&model, &slots, &options).expect("writes");
    Catalog::new(bytes, manifest.hash()).expect("loads")
}

fn format_with(
    src: &str,
    args: &[(&str, Arg<'_>)],
    bidi: BidiStrategy,
    dir: Dir,
) -> (String, Vec<FormatError>) {
    let cat = catalog(src, dir);
    let mut cx = FormatContext::new(&HOST);
    cx.bidi = bidi;
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    let mut out = String::new();
    let mut errs = Vec::new();
    f.write_named(MsgId::from_raw(0), args, &mut out, &mut errs);
    (out, errs)
}

fn format(src: &str, args: &[(&str, Arg<'_>)]) -> (String, Vec<FormatError>) {
    format_with(src, args, BidiStrategy::None, Dir::Ltr)
}

fn ok(src: &str, args: &[(&str, Arg<'_>)]) -> String {
    let (s, e) = format(src, args);
    assert!(e.is_empty(), "{src}: unexpected errors {e:?}");
    s
}

struct Parts(String);

struct Sub<'s>(&'s mut String);

impl SubPartSink for Sub<'_> {
    fn sub_part(&mut self, kind: &str, text: &str) {
        let _ = write!(self.0, " {kind}={text:?}");
    }
}

impl PartSink for Parts {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(t) => {
                let _ = write!(self.0, "[text {t:?}]");
            }
            Part::BidiIsolation(i) => {
                let _ = write!(self.0, "[bidi {i:?}]");
            }
            Part::Expression(e) => {
                let mut v = String::new();
                e.write(&mut v);
                let _ = write!(self.0, "[{} {v:?} {:?}", e.kind(), e.dir());
                if let Some(id) = e.id() {
                    let _ = write!(self.0, " id={id}");
                }
                e.sub_parts(&mut Sub(&mut self.0));
                self.0.push(']');
            }
            Part::Markup(m) => {
                let _ = write!(self.0, "[markup {:?} {}", m.kind(), m.name());
                if let Some(id) = m.id() {
                    let _ = write!(self.0, " id={id}");
                }
                for (k, v) in m.options() {
                    let _ = write!(self.0, " {k}={:?}", v.as_str());
                }
                self.0.push(']');
            }
            Part::Fallback(src) => {
                let mut s = String::new();
                src.write(&mut s);
                let _ = write!(self.0, "[fallback {s}]");
            }
            _ => self.0.push_str("[unknown]"),
        }
    }
}

fn parts(src: &str, args: &[(&str, Arg<'_>)]) -> (String, Vec<FormatError>) {
    let cat = catalog(src, Dir::Ltr);
    let cx = FormatContext::new(&HOST);
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    let mut out = Parts(String::new());
    let mut errs = Vec::new();
    f.parts_named(MsgId::from_raw(0), args, &mut out, &mut errs);
    (out.0, errs)
}

#[test]
fn simple_and_patterns() {
    assert_eq!(ok("Hello", &[]), "Hello");
    let cat = catalog("Hello", Dir::Ltr);
    let cx = FormatContext::new(&HOST);
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    assert_eq!(f.simple(MsgId::from_raw(0)), Some("Hello"));
    assert_eq!(ok("Hi {$user}!", &[("user", Arg::Str("Ann"))]), "Hi Ann!");
    assert_eq!(
        ok(
            "{$n} and {$x}",
            &[("n", Arg::Int(-42)), ("x", Arg::Float(4.2))]
        ),
        "-42 and 4.2"
    );
    assert_eq!(ok("{|lit|} {42}", &[]), "lit 42");
    assert_eq!(ok("{$d}", &[("d", Arg::Decimal("1.50e2"))]), "150");
}

#[test]
fn declarations_resolve_once() {
    assert_eq!(
        ok(
            ".local $x = {$y :string} .local $z = {$x} {{{$z}-{$x}}}",
            &[("y", Arg::Str("a"))]
        ),
        "a-a"
    );
    // An unused declaration is never resolved: no error for it.
    assert_eq!(ok(".local $x = {$missing} {{ok}}", &[]), "ok");
    // A failing declaration reports once, however often it is used.
    let (s, e) = format(
        ".input {$foo :number} {{{$foo} {$foo}}}",
        &[("foo", Arg::Str("foo"))],
    );
    assert_eq!(s, "{$foo} {$foo}");
    assert_eq!(e, [FormatError::BadOperand]);
    // A long chain does not recurse.
    let mut src = String::from(".local $a0 = {1 :integer}\n");
    for i in 1..5000 {
        let _ = writeln!(src, ".local $a{i} = {{$a{} :integer}}", i - 1);
    }
    src.push_str("{{{$a4999}}}");
    assert_eq!(ok(&src, &[]), "1");
}

#[test]
fn fallbacks() {
    let (s, e) = format("{$var}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{$var}", &[FormatError::UnresolvedVariable][..])
    );
    let (s, e) = format("{$var :number}", &[]);
    assert_eq!(s, "{$var}");
    assert_eq!(
        e,
        [FormatError::UnresolvedVariable, FormatError::BadOperand]
    );
    let (s, e) = format("{|C:\\\\| :test:function}", &[]);
    assert_eq!(s, "{|C:\\\\|}");
    assert_eq!(e, [FormatError::UnknownFunction]);
    let (s, e) = format("{:test:undefined}", &[]);
    assert_eq!(s, "{:test:undefined}");
    assert_eq!(e, [FormatError::UnknownFunction]);
    let (s, e) = format(
        ".local $var = {|val| :test:undefined} {{{$var :number}}}",
        &[],
    );
    assert_eq!(s, "{$var}");
    assert_eq!(e, [FormatError::UnknownFunction, FormatError::BadOperand]);
}

#[test]
fn numbers() {
    assert_eq!(ok("{4.2 :number}", &[]), "4.2");
    assert_eq!(ok("{-4.20 :number}", &[]), "-4.2");
    assert_eq!(ok("{|0.42e+1| :number}", &[]), "4.2");
    assert_eq!(ok("{4.2 :number minimumFractionDigits=2}", &[]), "4.20");
    assert_eq!(ok("{1.23456 :number}", &[]), "1.235");
    assert_eq!(ok("{4.2 :integer}", &[]), "4");
    assert_eq!(ok("{41 :offset add=1}", &[]), "42");
    assert_eq!(
        ok(
            ".local $x = {41 :integer signDisplay=always} {{{$x :offset add=1}}}",
            &[]
        ),
        "+42"
    );
    assert_eq!(
        ok(
            ".local $x = {1.25 :integer} .local $y = {$x :number} {{{$y}}}",
            &[]
        ),
        "1"
    );
    assert_eq!(
        ok(
            ".input {$n :number minimumFractionDigits=2 signDisplay=always} {{{$n :number minimumFractionDigits=1}}}",
            &[("n", Arg::Int(5))]
        ),
        "+5.0"
    );
    let (s, e) = format("hello {042 :number}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("hello {|042|}", &[FormatError::BadOperand][..])
    );
    let (s, e) = format("{42 :offset}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{|42|}", &[FormatError::BadOption][..])
    );
}

#[test]
fn selection() {
    let m = ".input {$n :integer} .match $n 1 {{one exactly}} one {{one}} * {{other {$n}}}";
    assert_eq!(ok(m, &[("n", Arg::Int(1))]), "one exactly");
    assert_eq!(ok(m, &[("n", Arg::Float(1.2))]), "one exactly");
    assert_eq!(ok(m, &[("n", Arg::Int(7))]), "other 7");
    let m = ".input {$n :number} .match $n one {{one}} * {{other}}";
    assert_eq!(ok(m, &[("n", Arg::Int(1))]), "one");
    assert_eq!(ok(m, &[("n", Arg::Decimal("1.0"))]), "one");
    let m = ".input {$n :number minimumFractionDigits=1} .match $n one {{one}} * {{other}}";
    assert_eq!(ok(m, &[("n", Arg::Int(1))]), "other");
    let m = ".input {$s :string} .match $s |a b| {{space}} * {{other}}";
    assert_eq!(ok(m, &[("s", Arg::Str("a b"))]), "space");
    // NFC on the operand.
    let m =
        ".local $x = {\u{1E0A}\u{0323} :string} .match $x \u{1E0C}\u{0307} {{Right}} * {{Wrong}}";
    assert_eq!(ok(m, &[]), "Right");
    let (s, e) = format(
        ".local $x = {1 :number select=$bad} .match $x 1 {{ONE}} * {{other}}",
        &[("bad", Arg::Str("exact"))],
    );
    assert_eq!(s, "other");
    assert_eq!(e, [FormatError::BadOption, FormatError::BadSelector]);
    let (s, e) = format(
        ".input {$x :number} .match $x horse {{h}} * {{other}}",
        &[("x", Arg::Int(1))],
    );
    assert_eq!(s, "other");
    assert_eq!(e, [FormatError::BadVariantKey]);
    let m = ".input {$a :string} .input {$b :string} .match $a $b x x {{xx}} x * {{x*}} * x {{*x}} * * {{**}}";
    assert_eq!(ok(m, &[("a", Arg::Str("x")), ("b", Arg::Str("x"))]), "xx");
    assert_eq!(ok(m, &[("a", Arg::Str("x")), ("b", Arg::Str("y"))]), "x*");
    assert_eq!(ok(m, &[("a", Arg::Str("y")), ("b", Arg::Str("x"))]), "*x");
    assert_eq!(ok(m, &[("a", Arg::Str("y")), ("b", Arg::Str("y"))]), "**");
}

#[test]
fn bidi() {
    let d = |src: &str, dir| format_with(src, &[], BidiStrategy::Default, dir).0;
    assert_eq!(
        d(".local $x = {1} {{ {$x}}}", Dir::Ltr),
        " \u{2068}1\u{2069}"
    );
    assert_eq!(d("{1 :number}", Dir::Ltr), "1");
    assert_eq!(d("{1 :number}", Dir::Rtl), "\u{2066}1\u{2069}");
    assert_eq!(
        d("hello {world :string u:dir=ltr u:id=foo}", Dir::Ltr),
        "hello \u{2066}world\u{2069}"
    );
    assert_eq!(
        d("hello {world :string u:dir=rtl}", Dir::Ltr),
        "hello \u{2067}world\u{2069}"
    );
    assert_eq!(
        d("hello {world :string u:dir=auto}", Dir::Ltr),
        "hello \u{2068}world\u{2069}"
    );
    assert_eq!(
        d(
            ".local $world = {world :string u:dir=ltr u:id=foo} {{hello {$world}}}",
            Dir::Ltr
        ),
        "hello \u{2066}world\u{2069}"
    );
    assert_eq!(d("{$x}", Dir::Ltr), "\u{2068}{$x}\u{2069}");
}

#[test]
fn parts_output() {
    let (p, e) = parts("{#tag foo=bar u:id=x}content{/tag}{#img/}", &[]);
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(
        p,
        "[markup Open tag id=x foo=Some(\"bar\")][text \"content\"][markup Close tag][markup Standalone img]"
    );
    let (p, e) = parts("{#tag u:dir=rtl}x", &[]);
    assert_eq!(e, [FormatError::BadOption]);
    assert_eq!(p, "[markup Open tag][text \"x\"]");
    let (p, _) = parts("{-42.5 :number}", &[]);
    assert_eq!(
        p,
        "[number \"-42.5\" Ltr minusSign=\"-\" integer=\"42\" decimal=\".\" fraction=\"5\"]"
    );
    let (p, _) = parts("hello {world :string u:dir=ltr u:id=foo}", &[]);
    assert_eq!(
        p,
        "[text \"hello \"][bidi Lri][string \"world\" Ltr id=foo][bidi Pdi]"
    );
    let (p, e) = parts("{$x}", &[]);
    assert_eq!(e, [FormatError::UnresolvedVariable]);
    assert_eq!(p, "[bidi Fsi][fallback $x][bidi Pdi]");
}

/// A damaged catalog (a markup name that is not UTF-8) stops string and
/// parts output at the same place, with the same errors: the parts still
/// concatenate to the string (found by the `format` fuzz target).
#[test]
fn malformed_markup_name() {
    let cat = catalog("a{#zqzq}b{/zqzq}c", Dir::Ltr);
    let mut bytes = cat.as_bytes().to_vec();
    let mut damaged = 0;
    for i in 0..bytes.len().saturating_sub(3) {
        if &bytes[i..i + 4] == b"zqzq" {
            bytes[i] = 0xFF;
            damaged += 1;
        }
    }
    assert!(damaged > 0);
    let hash = cat.manifest_hash();
    let cat = Catalog::new(bytes, hash).expect("still loads");
    let cx = FormatContext::new(&HOST);
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    let mut text = String::new();
    let mut errs = Vec::new();
    f.write(MsgId::from_raw(0), &[], &mut text, &mut errs);
    assert_eq!(text, "a\u{2068}{\u{FFFD}}\u{2069}");
    assert_eq!(errs, [FormatError::Malformed]);
    let mut p = Parts(String::new());
    let mut perrs = Vec::new();
    f.parts(MsgId::from_raw(0), &[], &mut p, &mut perrs);
    assert_eq!(p.0, "[text \"a\"][bidi Fsi][fallback \u{FFFD}][bidi Pdi]");
    assert_eq!(perrs, errs);
}

#[test]
fn neutral_grouping_is_unsupported() {
    // The locale-only grouping values.
    for (src, want) in [
        (
            "{12345 :number useGrouping=always}",
            vec![FormatError::UnsupportedOperation],
        ),
        (
            "{12345 :integer useGrouping=min2}",
            vec![FormatError::UnsupportedOperation],
        ),
        ("{12345 :number useGrouping=auto}", vec![]),
        ("{12345 :number useGrouping=never}", vec![]),
    ] {
        let (s, e) = format(src, &[]);
        assert_eq!(s, "12345", "{src}");
        assert_eq!(e, want, "{src}");
    }
}

#[test]
fn offset_keeps_the_sign_of_a_nonzero_side() {
    // Found by the Phase 4 intl probe (A0): a zero addend took the sign of
    // zero + zero. `-0` + `-0` alone stays `-0`.
    for (src, want) in [
        ("{0 :offset subtract=5}", "-5"),
        ("{-5 :offset add=0}", "-5"),
        ("{-5 :offset subtract=0}", "-5"),
        ("{0 :offset add=5}", "5"),
        ("{-0 :offset subtract=5}", "-5"),
        ("{-0 :offset add=5}", "5"),
        ("{5 :offset subtract=5}", "0"),
        ("{-0 :offset add=0}", "0"),
        ("{-0 :offset subtract=0}", "-0"),
        ("{-0.5 :offset add=0}", "-0.5"),
    ] {
        assert_eq!(ok(src, &[]), want, "{src}");
    }
}

/// u-namespace.md, `u:dir`: removed from the resolved options before the
/// handler is called (and `u:id` with it); the value stays on the resolved
/// value, so it still isolates — directly and through a declaration.
#[test]
fn u_options_are_removed_before_the_handler() {
    use mf2_runtime::{ErrorSink, FnContext, Options, Sink, Value};

    /// Formats the names of the options it was given.
    struct Names;
    impl Function for Names {
        fn resolve<'a>(
            &self,
            _cx: &FnContext<'_>,
            _operand: Option<&Value<'a>>,
            options: &Options<'_, 'a>,
            _errs: &mut dyn ErrorSink,
        ) -> Option<Value<'a>> {
            let mut names: Vec<&str> = options.iter().map(|(n, _)| n).collect();
            names.sort_unstable();
            Some(Value::Boxed(Box::new(names.join(","))))
        }
        fn format(&self, _cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
            if let Some(s) = value.downcast_ref::<String>() {
                out.push_str(s);
            }
        }
    }
    static NAMES: [(&str, &dyn Function); 1] = [("names", &Names)];
    static R: Registry = Registry::new(&NAMES);

    let run = |src: &str| {
        let cat = catalog(src, Dir::Ltr);
        let mut cx = FormatContext::new(&HOST);
        cx.bidi = BidiStrategy::Default;
        let f = Formatter::new(&cat, &R, &cx);
        let mut out = String::new();
        let mut errs = Vec::new();
        f.write_named(MsgId::from_raw(0), &[], &mut out, &mut errs);
        assert!(errs.is_empty(), "{src}: {errs:?}");
        out
    };
    // The handler saw `a` and `b` only; `u:dir=rtl` still isolated the value.
    assert_eq!(
        run("{1 :names a=1 u:dir=rtl u:id=x b=2}"),
        "\u{2067}a,b\u{2069}"
    );
    // Without `u:dir` the handler's `Auto` direction gives FSI: the control.
    assert_eq!(run("{1 :names a=1 b=2}"), "\u{2068}a,b\u{2069}");
    // Through a declaration the value keeps its direction.
    assert_eq!(
        run(".local $x = {1 :names u:dir=rtl} {{{$x}}}"),
        "\u{2067}\u{2069}"
    );
}
