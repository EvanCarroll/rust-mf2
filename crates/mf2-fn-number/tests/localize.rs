//! The localized handlers over hand-built `number.*` entries (the encoders of
//! `mf2_catalog::writer::number`): separators, grouping sizes and minimum
//! grouping digits, `useGrouping`, native digits, the percent pattern, signs,
//! sub-parts, the unannotated-number hook, and that selection is the core's.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mf2::{Arg, BidiStrategy, FormatContext, FormatError, Formatter, Function, Registry};
use mf2::{Part, PartSink, SubPartSink};
use mf2_catalog::format::locale_key;
use mf2_catalog::number::{AffixPart, Grouping, Style};
use mf2_catalog::writer::number::{PatternSpec, SymbolsSpec, patterns, symbols};
use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Catalog, Dir, MsgId};

static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &mf2::functions::STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_numbers(&mf2_fn_number::NUMBERS);

/// en cardinal: `one: i = 1 and v = 0`.
const EN_CARDINAL: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];

struct Loc {
    tag: &'static str,
    grouping: Grouping,
    min: u8,
    decimal: &'static str,
    group: &'static str,
    minus: &'static str,
    percent_suffix: &'static str,
    digits: Option<&'static str>,
}

const G3: Grouping = Grouping {
    primary: 3,
    secondary: 3,
};

const EN: Loc = Loc {
    tag: "en",
    grouping: G3,
    min: 1,
    decimal: ".",
    group: ",",
    minus: "-",
    percent_suffix: "",
    digits: None,
};

const FR: Loc = Loc {
    tag: "fr",
    grouping: G3,
    min: 1,
    decimal: ",",
    group: "\u{202f}",
    minus: "-",
    percent_suffix: "\u{202f}",
    digits: None,
};

const ES: Loc = Loc {
    tag: "es",
    grouping: G3,
    min: 2,
    decimal: ",",
    group: ".",
    minus: "-",
    percent_suffix: "\u{a0}",
    digits: None,
};

const HI_DEVA: Loc = Loc {
    tag: "hi-u-nu-deva",
    grouping: Grouping {
        primary: 3,
        secondary: 2,
    },
    min: 1,
    decimal: ".",
    group: ",",
    minus: "-",
    percent_suffix: "",
    digits: Some("०१२३४५६७८९"),
};

fn entries(l: &Loc) -> Vec<(u32, Vec<u8>)> {
    let sym = symbols(&SymbolsSpec {
        grouping: l.grouping,
        minimum_grouping_digits: l.min,
        decimal: l.decimal,
        group: l.group,
        minus: l.minus,
        plus: "+",
        percent: "%",
        digits: l.digits,
    })
    .expect("symbols");
    let mut suffix = Vec::new();
    if !l.percent_suffix.is_empty() {
        suffix.push(AffixPart::Text(l.percent_suffix));
    }
    suffix.push(AffixPart::Percent);
    let pat = patterns(&[(
        Style::Percent,
        PatternSpec {
            grouping: l.grouping,
            positive: (Vec::new(), suffix),
            negative: None,
        },
    )])
    .expect("patterns");
    vec![
        (locale_key::PLURAL_CARDINAL, EN_CARDINAL.to_vec()),
        (locale_key::NUMBER_SYMBOLS, sym),
        (locale_key::NUMBER_PATTERNS, pat),
    ]
}

fn catalog(src: &str, l: &Loc, data: bool) -> Catalog {
    let parsed = mf2_syntax::parse_model(src);
    let model = parsed.message.expect("parses");
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = Options::new(l.tag, Dir::Ltr);
    options.locale_entries = if data {
        entries(l)
    } else {
        vec![(locale_key::PLURAL_CARDINAL, EN_CARDINAL.to_vec())]
    };
    let (bytes, manifest) = writer::single(&model, &slots, &options).expect("writes");
    Catalog::new(bytes, manifest.hash()).expect("loads")
}

fn cx() -> FormatContext {
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = BidiStrategy::None;
    cx
}

fn run(src: &str, l: &Loc, args: &[(&str, Arg<'_>)]) -> (String, Vec<FormatError>) {
    let cat = catalog(src, l, true);
    let cx = cx();
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    let mut out = String::new();
    let mut errs = Vec::new();
    f.write_named(MsgId::from_raw(0), args, &mut out, &mut errs);
    (out, errs)
}

fn ok(src: &str, l: &Loc, args: &[(&str, Arg<'_>)]) -> String {
    let (s, e) = run(src, l, args);
    assert!(e.is_empty(), "{src}: unexpected errors {e:?}");
    s
}

#[test]
fn separators_and_grouping() {
    assert_eq!(ok("{1234567.891 :number}", &EN, &[]), "1,234,567.891");
    assert_eq!(ok("{1234.5 :number}", &FR, &[]), "1\u{202f}234,5");
    assert_eq!(ok("{-1234.5 :number}", &FR, &[]), "-1\u{202f}234,5");
    assert_eq!(ok("{999 :number}", &EN, &[]), "999");
    // Minimum grouping digits 2 (es): four digits are not grouped …
    assert_eq!(ok("{1234 :number}", &ES, &[]), "1234");
    assert_eq!(ok("{12345 :number}", &ES, &[]), "12.345");
    // … unless `useGrouping=always`; `min2` asks for two in any locale.
    assert_eq!(ok("{1234 :number useGrouping=always}", &ES, &[]), "1.234");
    assert_eq!(ok("{1234 :number useGrouping=min2}", &EN, &[]), "1234");
    assert_eq!(ok("{12345 :number useGrouping=min2}", &EN, &[]), "12,345");
    assert_eq!(
        ok("{1234567 :number useGrouping=never}", &EN, &[]),
        "1234567"
    );
    // Grouping covers `minimumIntegerDigits`' padding, as in ECMA-402.
    assert_eq!(
        ok("{1234 :number minimumIntegerDigits=6}", &EN, &[]),
        "001,234"
    );
    // Primary 3, secondary 2, and the numbering system's digits.
    assert_eq!(ok("{1234567 :integer}", &HI_DEVA, &[]), "१२,३४,५६७");
    assert_eq!(ok("{-0.5 :number}", &HI_DEVA, &[]), "-०.५");
    assert_eq!(ok("{1 :offset add=2}", &HI_DEVA, &[]), "३");
}

#[test]
fn signs() {
    assert_eq!(ok("{5 :number signDisplay=always}", &EN, &[]), "+5");
    assert_eq!(ok("{0 :number signDisplay=exceptZero}", &EN, &[]), "0");
    assert_eq!(ok("{-5 :number signDisplay=never}", &EN, &[]), "5");
    assert_eq!(ok("{-0 :number signDisplay=negative}", &EN, &[]), "0");
    assert_eq!(ok("{-0 :number}", &EN, &[]), "-0");
}

#[test]
fn percent() {
    assert_eq!(ok("{0.12345678 :percent}", &EN, &[]), "12%");
    assert_eq!(
        ok("{0.12345678 :percent maximumFractionDigits=1}", &FR, &[]),
        "12,3\u{202f}%"
    );
    assert_eq!(ok("{-0.5 :percent}", &ES, &[]), "-50\u{a0}%");
    assert_eq!(ok("{12.5 :percent}", &ES, &[]), "1250\u{a0}%");
    assert_eq!(ok("{125 :percent}", &ES, &[]), "12.500\u{a0}%");
    assert_eq!(ok("{0.5 :percent signDisplay=always}", &EN, &[]), "+50%");
    // Selection on the scaled value, keys in neutral digits.
    let sel = ".input {$n :percent} .match $n one {{one}} 100 {{hundred}} * {{other}}";
    assert_eq!(ok(sel, &EN, &[("n", Arg::Float(0.01))]), "one");
    assert_eq!(ok(sel, &FR, &[("n", Arg::Int(1))]), "hundred");
}

#[test]
fn unannotated_numbers() {
    // syntax.json #90.
    let args = [("one", Arg::Float(1.3)), ("two", Arg::Float(4.2))];
    assert_eq!(ok("{$one} et {$two}", &FR, &args), "1,3 et 4,2");
    // The exact value, no rounding; grouping `auto`.
    let args = [
        ("i", Arg::Int(-1_234_567)),
        ("d", Arg::Decimal("1234.56789")),
        ("s", Arg::Str("1234")),
    ];
    assert_eq!(
        ok("{$i} {$d} {$s}", &ES, &args),
        "-1.234.567 1234,56789 1234"
    );
    assert_eq!(ok("{$i}", &HI_DEVA, &args), "-१२,३४,५६७");
    // The same errors as without fn-number; still no selection.
    let (s, e) = run("{$x}", &EN, &[("x", Arg::Float(f64::INFINITY))]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{$x}", &[FormatError::BadOperand][..])
    );
    let (s, e) = run(".match $i * {{any}}", &EN, &args);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("any", &[FormatError::BadSelector][..])
    );
}

#[test]
fn selection_is_the_cores() {
    let sel = ".input {$n :number} .match $n 1234.5 {{exact}} one {{one}} * {{other}}";
    // The exact key is in neutral digits whatever the locale shows.
    assert_eq!(ok(sel, &FR, &[("n", Arg::Float(1234.5))]), "exact");
    assert_eq!(ok(sel, &FR, &[("n", Arg::Int(1))]), "one");
    assert_eq!(ok(sel, &HI_DEVA, &[("n", Arg::Int(7))]), "other");
}

struct Parts(Vec<(String, String)>);

impl SubPartSink for Parts {
    fn sub_part(&mut self, kind: &str, text: &str) {
        self.0.push((kind.to_owned(), text.to_owned()));
    }
}

struct Collect(Vec<(String, String)>, Vec<String>);

impl PartSink for Collect {
    fn part(&mut self, part: Part<'_>) {
        if let Part::Expression(e) = part {
            self.1.push(e.kind().to_owned());
            let mut p = Parts(Vec::new());
            e.sub_parts(&mut p);
            self.0.extend(p.0);
        }
    }
}

fn parts(src: &str, l: &Loc, args: &[(&str, Arg<'_>)]) -> (Vec<(String, String)>, Vec<String>) {
    let cat = catalog(src, l, true);
    let cx = cx();
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    let mut c = Collect(Vec::new(), Vec::new());
    f.parts_named(MsgId::from_raw(0), args, &mut c, &mut Vec::new());
    (c.0, c.1)
}

fn owned(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(k, t)| ((*k).to_owned(), (*t).to_owned()))
        .collect()
}

#[test]
fn sub_parts() {
    let (p, kinds) = parts("{-1234.5 :number}", &EN, &[]);
    assert_eq!(kinds, ["number"]);
    assert_eq!(
        p,
        owned(&[
            ("minusSign", "-"),
            ("integer", "1"),
            ("group", ","),
            ("integer", "234"),
            ("decimal", "."),
            ("fraction", "5"),
        ])
    );
    let (p, _) = parts("{0.125 :percent maximumFractionDigits=1}", &FR, &[]);
    assert_eq!(
        p,
        owned(&[
            ("integer", "12"),
            ("decimal", ","),
            ("fraction", "5"),
            ("literal", "\u{202f}"),
            ("percentSign", "%"),
        ])
    );
    let (p, kinds) = parts("{$n}", &HI_DEVA, &[("n", Arg::Int(123_456))]);
    assert_eq!(kinds, ["number"]);
    assert_eq!(
        p,
        owned(&[
            ("integer", "१"),
            ("group", ","),
            ("integer", "२३"),
            ("group", ","),
            ("integer", "४५६"),
        ])
    );
}

#[test]
fn no_symbols_entry_means_neutral() {
    // The wasm holds no symbols (B6): without the entry, the core's output.
    let cat = catalog("{1234.5 :number} {$n} {0.5 :percent}", &FR, false);
    let cx = cx();
    let f = Formatter::new(&cat, &REGISTRY, &cx);
    let mut out = String::new();
    f.write_named(
        MsgId::from_raw(0),
        &[("n", Arg::Int(-12345))],
        &mut out,
        &mut Vec::new(),
    );
    assert_eq!(out, "1234.5 -12345 50%");
}
