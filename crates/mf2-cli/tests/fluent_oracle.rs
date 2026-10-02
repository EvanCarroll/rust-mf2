//! Converted catalogs format as the Fluent originals (Phase 8 A3;
//! the Phase 8 work order, the tooling design §6.1).
//!
//! In one process, each original is formatted with **`fluent-bundle`** — what
//! a Fluent application formats with, a dev-dependency here and nothing
//! else's — and each converted message with `mf2`, from a catalog
//! `mf2-build` wrote from `mf2 convert --from fluent`'s output, over a
//! sampled argument set. Two corpora: A2's reference workload, and the
//! mapped part of the construct corpus (`tests/fluent/constructs/`).
//!
//! **Settings.** Isolation on both sides or off on both; and the functions
//! the catalogs are built for — none (the core `:number`, as a converted
//! corpus that needs no feature builds by default) or `fn-number`
//! (localized numbers, what `mf2 convert` names when `:percent` or
//! `:currency` appear). A corpus that needs `fn-number` is not built
//! without it.
//!
//! **Arguments.** A variable Fluent reads as a number — the operand of
//! `NUMBER`, or a bare selector with a number or plural-category key — gets
//! numbers: integers at the plural boundaries of every locale here, negatives,
//! decimals and large values. Every other variable gets strings (Latin, RTL,
//! empty) and those numbers too, since an application may pass either.
//! Each variable sees every value of its set; the others rotate beside it.
//! A message with a `DATETIME` has no oracle (`fluent-bundle` 0.16 has none
//! built in) and is counted, not compared.
//!
//! **Classes.** A pair that differs is explained by the first rule that
//! makes it equal: isolation marks removed (**isolation**); then every
//! number, with its grouping, symbols, sign and percent sign, read as one
//! token, a currency mark beside it removed (**number rendering**,
//! sub-classed by where the number sits in the Fluent message and how its
//! text differs). A different variant is **selection** only when it is
//! shown to be one of two causes: a selector's number has more than three
//! decimals and Fluent, given it rounded as `:number` rounds it, agrees; or
//! the two sides put the number in different plural categories (a probe
//! message on each side names the category). What no rule explains is
//! **unexplained**, printed with the message and the arguments, and fails
//! the test; so does a class the owner has not approved ([`APPROVED`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use fluent_syntax::ast;
use mf2::{Arg, BidiStrategy, Catalog, FormatContext, Formatter, Function, MsgId, Registry};
use mf2_build::{Build, Config, Features};

// ---------------------------------------------------------------------------
// The two registries: what a generated module gives a corpus with and
// without `fn-number` (`mf2-build`'s codegen), every function it could name.

static CORE_FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("string", &mf2::functions::STRING),
    ("number", &mf2::functions::NUMBER),
    ("integer", &mf2::functions::INTEGER),
    ("offset", &mf2::functions::OFFSET),
];
static CORE: Registry = Registry::new(&CORE_FUNCTIONS);

static FN_NUMBER_FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("string", &mf2::functions::STRING),
    ("number", &mf2::fn_number::NUMBER),
    ("integer", &mf2::fn_number::INTEGER),
    ("offset", &mf2::fn_number::OFFSET),
    ("percent", &mf2::fn_number::PERCENT),
    ("currency", &mf2::fn_number::CURRENCY),
    ("unit", &mf2::fn_number::UNIT),
];
static FN_NUMBER: Registry =
    Registry::new(&FN_NUMBER_FUNCTIONS).with_numbers(&mf2::fn_number::NUMBERS);

static ISOLATING: FormatContext = FormatContext::new(&mf2::host_std::HOST);
static PLAIN: FormatContext = {
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = BidiStrategy::None;
    cx
};

// ---------------------------------------------------------------------------
// The sampled argument set.

#[derive(Clone, Copy, Debug)]
enum Val {
    Str(&'static str),
    Int(i64),
    Float(f64),
}

impl Val {
    fn fluent(self) -> FluentValue<'static> {
        match self {
            Val::Str(s) => FluentValue::from(s),
            Val::Int(n) => FluentValue::from(n),
            Val::Float(x) => FluentValue::from(x),
        }
    }

    fn mf2(self) -> Arg<'static> {
        match self {
            Val::Str(s) => Arg::Str(s),
            Val::Int(n) => Arg::Int(n),
            Val::Float(x) => Arg::Float(x),
        }
    }

    fn is_number(self) -> bool {
        !matches!(self, Val::Str(_))
    }
}

/// Integers at every plural boundary of the locales here (`en`, `pl`, `fr`,
/// `pt`, and `ar` for the pseudo-locale `ar-XB`), negatives, decimals and
/// large values.
const NUMBERS: [Val; 41] = [
    Val::Int(0),
    Val::Int(1),
    Val::Int(2),
    Val::Int(3),
    Val::Int(4),
    Val::Int(5),
    Val::Int(6),
    Val::Int(10),
    Val::Int(11),
    Val::Int(12),
    Val::Int(13),
    Val::Int(14),
    Val::Int(15),
    Val::Int(21),
    Val::Int(22),
    Val::Int(25),
    Val::Int(99),
    Val::Int(100),
    Val::Int(101),
    Val::Int(102),
    Val::Int(103),
    Val::Int(111),
    Val::Int(112),
    Val::Int(1000),
    Val::Int(1001),
    Val::Int(12345),
    Val::Int(1_000_000),
    Val::Int(1_234_567),
    Val::Int(12_345_678_901),
    Val::Int(-1),
    Val::Int(-2),
    Val::Int(-5),
    Val::Int(-12345),
    Val::Float(0.5),
    Val::Float(1.5),
    Val::Float(2.25),
    Val::Float(-1.5),
    Val::Float(0.1),
    Val::Float(1234.5),
    Val::Float(1.0e-7),
    Val::Float(1.0004),
];

const STRINGS: [Val; 3] = [Val::Str("Ada"), Val::Str("مرحبا"), Val::Str("")];

// ---------------------------------------------------------------------------
// What a Fluent message reads.

/// Where a variable is written in a message's text.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Place {
    /// `{ NUMBER($x, …) }`.
    NumberCall,
    /// `{ $x }`, where `$x` is also a selector of the message.
    UnderSelector,
    /// `{ $x }`, nothing else.
    Bare,
}

#[derive(Default, Debug)]
struct Reads {
    /// Variables Fluent reads as a number.
    numeric: BTreeSet<String>,
    /// Every variable the message reads, and where it is written.
    places: BTreeMap<String, BTreeSet<Place>>,
    /// Variables that select.
    selectors: BTreeSet<String>,
    /// Of those, the ones selecting on a cardinal plural category, and on an
    /// ordinal one.
    cardinal: BTreeSet<String>,
    ordinal: BTreeSet<String>,
    /// A `DATETIME` somewhere in it (no oracle).
    datetime: bool,
}

const CATEGORIES: [&str; 5] = ["zero", "one", "two", "few", "many"];

struct Walker<'a> {
    messages: &'a BTreeMap<&'a str, &'a ast::Message<&'a str>>,
    terms: &'a BTreeMap<&'a str, &'a ast::Term<&'a str>>,
    reads: Reads,
    seen: BTreeSet<String>,
}

impl Walker<'_> {
    fn pattern(&mut self, p: &ast::Pattern<&str>, in_term: bool) {
        for e in &p.elements {
            if let ast::PatternElement::Placeable { expression } = e {
                self.expression(expression, in_term, true);
            }
        }
    }

    fn expression(&mut self, e: &ast::Expression<&str>, in_term: bool, placed: bool) {
        match e {
            ast::Expression::Inline(i) => self.inline(i, in_term, placed),
            ast::Expression::Select { selector, variants } => {
                if !in_term {
                    let numeric_keys = variants.iter().any(|v| match &v.key {
                        ast::VariantKey::NumberLiteral { .. } => true,
                        ast::VariantKey::Identifier { name } => CATEGORIES.contains(name),
                    });
                    match selector {
                        ast::InlineExpression::VariableReference { id } => {
                            self.reads.selectors.insert(id.name.to_owned());
                            self.reads.places.entry(id.name.to_owned()).or_default();
                            if numeric_keys {
                                self.reads.numeric.insert(id.name.to_owned());
                                self.reads.cardinal.insert(id.name.to_owned());
                            }
                        }
                        ast::InlineExpression::FunctionReference { id, arguments } => {
                            if id.name == "NUMBER"
                                && let Some(ast::InlineExpression::VariableReference { id: var }) =
                                    arguments.positional.first()
                            {
                                let ordinal = arguments.named.iter().any(|a| {
                                    a.name.name == "type"
                                        && matches!(
                                            a.value,
                                            ast::InlineExpression::StringLiteral {
                                                value: "ordinal"
                                            }
                                        )
                                });
                                self.reads.selectors.insert(var.name.to_owned());
                                let set = if ordinal {
                                    &mut self.reads.ordinal
                                } else {
                                    &mut self.reads.cardinal
                                };
                                set.insert(var.name.to_owned());
                            }
                            self.inline(selector, in_term, false);
                        }
                        _ => {}
                    }
                }
                for v in variants {
                    self.pattern(&v.value, in_term);
                }
            }
        }
    }

    fn inline(&mut self, i: &ast::InlineExpression<&str>, in_term: bool, placed: bool) {
        match i {
            ast::InlineExpression::VariableReference { id } if !in_term => {
                let places = self.reads.places.entry(id.name.to_owned()).or_default();
                if placed {
                    places.insert(Place::Bare);
                }
            }
            ast::InlineExpression::FunctionReference { id, arguments } => {
                if id.name == "DATETIME" {
                    self.reads.datetime = true;
                }
                if id.name == "NUMBER"
                    && !in_term
                    && let Some(ast::InlineExpression::VariableReference { id: var }) =
                        arguments.positional.first()
                {
                    self.reads.numeric.insert(var.name.to_owned());
                    let places = self.reads.places.entry(var.name.to_owned()).or_default();
                    if placed {
                        places.insert(Place::NumberCall);
                    }
                    return;
                }
                for a in &arguments.positional {
                    self.inline(a, in_term, false);
                }
            }
            ast::InlineExpression::MessageReference { id, attribute } => {
                let key = format!("{}.{:?}", id.name, attribute.as_ref().map(|a| a.name));
                if !self.seen.insert(key) {
                    return;
                }
                if let Some(m) = self.messages.get(id.name) {
                    let pattern = match attribute {
                        Some(a) => m
                            .attributes
                            .iter()
                            .find(|x| x.id.name == a.name)
                            .map(|x| &x.value),
                        None => m.value.as_ref(),
                    };
                    if let Some(p) = pattern {
                        self.pattern(p, in_term);
                    }
                }
            }
            ast::InlineExpression::TermReference { id, attribute, .. } => {
                let key = format!("-{}.{:?}", id.name, attribute.as_ref().map(|a| a.name));
                if !self.seen.insert(key) {
                    return;
                }
                if let Some(t) = self.terms.get(id.name) {
                    let pattern = match attribute {
                        Some(a) => t
                            .attributes
                            .iter()
                            .find(|x| x.id.name == a.name)
                            .map(|x| &x.value),
                        None => Some(&t.value),
                    };
                    if let Some(p) = pattern {
                        self.pattern(p, true);
                    }
                }
            }
            ast::InlineExpression::Placeable { expression } => {
                self.expression(expression, in_term, placed);
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Reads {
        for (name, places) in &mut self.reads.places {
            if self.reads.selectors.contains(name) && places.remove(&Place::Bare) {
                places.insert(Place::UnderSelector);
            }
        }
        self.reads
    }
}

// ---------------------------------------------------------------------------
// One locale on each side.

/// A message or attribute of one locale: the MF2 id and the Fluent place.
struct Unit {
    id: String,
    message: String,
    attribute: Option<String>,
    reads: Reads,
}

struct Locale {
    tag: String,
    bundle: FluentBundle<FluentResource>,
    units: Vec<Unit>,
    /// The plural category of a number on each side, cardinal and ordinal.
    probe: Probe,
}

/// The plural category each side gives a number: a message on each side
/// that selects on it and writes the category's name.
struct Probe {
    fluent: FluentBundle<FluentResource>,
    /// Cardinal and ordinal; `None` when `compile_str` has no rules for the
    /// locale.
    mf2: [Option<mf2::Compiled>; 2],
}

const PROBE_FTL: &str = "\
cardinal = { $n ->
    [zero] zero
    [one] one
    [two] two
    [few] few
    [many] many
   *[other] other
}
ordinal = { NUMBER($n, type: \"ordinal\") ->
    [zero] zero
    [one] one
    [two] two
    [few] few
    [many] many
   *[other] other
}
";

impl Probe {
    fn new(tag: &str) -> Probe {
        let langid: unic_langid::LanguageIdentifier = tag.parse().expect("a language identifier");
        let mut fluent = FluentBundle::new(vec![langid]);
        fluent.set_use_isolating(false);
        fluent.add_builtins().expect("NUMBER");
        fluent
            .add_resource(FluentResource::try_new(PROBE_FTL.to_owned()).expect("parses"))
            .expect("adds");
        let keys = "zero {{zero}} one {{one}} two {{two}} few {{few}} many {{many}} * {{other}}";
        let mf2 = [
            mf2::compile_str(&format!(".input {{$n :number}} .match $n {keys}"), tag).ok(),
            mf2::compile_str(
                &format!(".input {{$n :number select=ordinal}} .match $n {keys}"),
                tag,
            )
            .ok(),
        ];
        Probe { fluent, mf2 }
    }

    /// `(fluent-bundle's, mf2's)` category of `v`.
    fn categories(&self, v: Val, ordinal: bool) -> (String, String) {
        let id = if ordinal { "ordinal" } else { "cardinal" };
        let message = self.fluent.get_message(id).expect("the probe");
        let mut args = FluentArgs::new();
        args.set("n", v.fluent());
        let mut errors = Vec::new();
        let fluent = self
            .fluent
            .format_pattern(message.value().expect("a value"), Some(&args), &mut errors)
            .into_owned();
        let mf2 = match &self.mf2[usize::from(ordinal)] {
            Some(c) => {
                let f = Formatter::new(&c.catalog, &FN_NUMBER, &PLAIN);
                let mut out = String::new();
                let mut errors = Vec::new();
                f.write_named(mf2::Compiled::ID, &[("n", v.mf2())], &mut out, &mut errors);
                out
            }
            None => "(no rules)".to_owned(),
        };
        (fluent, mf2)
    }
}

fn ftl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ftl_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ftl") {
            out.push(path);
        }
    }
}

fn load_locale(dir: &Path) -> Locale {
    let tag = dir
        .file_name()
        .and_then(|n| n.to_str())
        .expect("a locale directory")
        .replace('_', "-");
    let langid: unic_langid::LanguageIdentifier = tag.parse().expect("a language identifier");
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle.add_builtins().expect("NUMBER");
    let mut files = Vec::new();
    ftl_files(dir, &mut files);
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read");
        let resource = FluentResource::try_new(source)
            .unwrap_or_else(|(_, e)| panic!("{}: {e:?}", file.display()));
        bundle
            .add_resource(resource)
            .unwrap_or_else(|e| panic!("{}: {e:?}", file.display()));
    }
    // The bundle owns the resources; read them again for the AST walk (the
    // same text, so the same entries).
    let sources: Vec<String> = files
        .iter()
        .map(|f| std::fs::read_to_string(f).expect("read"))
        .collect();
    let parsed: Vec<ast::Resource<&str>> = sources
        .iter()
        .map(|s| fluent_syntax::parser::parse(s.as_str()).expect("parses"))
        .collect();
    let mut messages = BTreeMap::new();
    let mut terms = BTreeMap::new();
    for r in &parsed {
        for e in &r.body {
            match e {
                ast::Entry::Message(m) => {
                    messages.insert(m.id.name, m);
                }
                ast::Entry::Term(t) => {
                    terms.insert(t.id.name, t);
                }
                _ => {}
            }
        }
    }
    let mut units = Vec::new();
    for (name, m) in &messages {
        let mut walk = |p: &ast::Pattern<&str>, attribute: Option<&str>| {
            let mut w = Walker {
                messages: &messages,
                terms: &terms,
                reads: Reads::default(),
                seen: BTreeSet::new(),
            };
            w.pattern(p, false);
            units.push(Unit {
                id: match attribute {
                    Some(a) => format!("{name}.{a}"),
                    None => (*name).to_owned(),
                },
                message: (*name).to_owned(),
                attribute: attribute.map(str::to_owned),
                reads: w.finish(),
            });
        };
        if let Some(v) = &m.value {
            walk(v, None);
        }
        for a in &m.attributes {
            walk(&a.value, Some(a.id.name));
        }
    }
    let probe = Probe::new(&tag);
    Locale {
        tag,
        bundle,
        units,
        probe,
    }
}

/// Every locale of a Fluent directory, in tag order.
fn load(ftl: &Path) -> Vec<Locale> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(ftl)
        .expect("read_dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.iter().map(|d| load_locale(d)).collect()
}

/// The MF2 side of one build: every locale's catalog and the ids.
struct Built {
    ids: BTreeMap<String, MsgId>,
    catalogs: BTreeMap<String, Catalog>,
}

/// `mf2 convert --from fluent FTL` into `out`, which must not exist.
fn convert(ftl: &Path, out: &Path) {
    std::fs::create_dir_all(out).expect("mkdir");
    let output = Command::new(env!("CARGO_BIN_EXE_mf2"))
        .arg("-C")
        .arg(out)
        .args(["convert", "--from", "fluent"])
        .arg(ftl)
        .output()
        .expect("the mf2 binary runs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(out.join("mf2.toml"), "source_locale = \"en\"\n").expect("write");
}

/// The catalogs of a converted directory, built for `features`; `None` when
/// the corpus needs a function they leave out.
fn build(root: &Path, features: &[&str]) -> Option<Built> {
    let config = Config::load(root).expect("mf2.toml");
    let outcome = Build::at(root, root.join("target"))
        .config(config)
        .features(Features::from_names(features.iter().copied()))
        .check()
        .expect("the build runs");
    if !outcome.report.is_clean() {
        return None;
    }
    let ids = outcome
        .manifest
        .ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let index = u32::try_from(i).expect("fits");
            (id.clone(), MsgId::new(0, index).expect("an id"))
        })
        .collect();
    let catalogs = outcome
        .catalogs
        .iter()
        .map(|c| {
            let catalog = Catalog::new(c.bytes.clone(), outcome.manifest_hash).expect("loads");
            (c.tag.clone(), catalog)
        })
        .collect();
    Some(Built { ids, catalogs })
}

// ---------------------------------------------------------------------------
// Comparison and classification.

/// The report: per setting and class, a count and the first examples.
#[derive(Default)]
struct Report {
    compared: BTreeMap<String, usize>,
    no_oracle: BTreeMap<String, usize>,
    classes: BTreeMap<(String, String), (usize, Vec<String>)>,
}

impl Report {
    fn add(&mut self, setting: &str, class: String, example: impl FnOnce() -> String) {
        let class_is_unexplained = class.starts_with("unexplained");
        let (n, examples) = self.classes.entry((setting.to_owned(), class)).or_default();
        *n += 1;
        let keep = if class_is_unexplained { 1000 } else { 3 };
        if examples.len() < keep {
            examples.push(example());
        }
    }

    fn unexplained(&self) -> Vec<&String> {
        self.classes
            .iter()
            .filter(|((_, c), _)| c.starts_with("unexplained"))
            .flat_map(|(_, (_, e))| e)
            .collect()
    }

    fn text(&self) -> String {
        let mut s = String::new();
        for (setting, n) in &self.compared {
            let _ = writeln!(
                s,
                "{setting}: {n} pairs compared, {} units without an oracle",
                self.no_oracle.get(setting).copied().unwrap_or(0)
            );
            for ((set, class), (count, examples)) in &self.classes {
                if set != setting {
                    continue;
                }
                let _ = writeln!(s, "  {count:>7}  {class}");
                for e in examples {
                    let _ = writeln!(s, "             {e}");
                }
            }
        }
        s
    }
}

const ISOLATES: [char; 4] = ['\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}'];

fn strip_isolates(s: &str) -> String {
    s.chars().filter(|c| !ISOLATES.contains(c)).collect()
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
        || ('\u{0660}'..='\u{0669}').contains(&c)
        || ('\u{06F0}'..='\u{06F9}').contains(&c)
}

/// Inside a number, between digits: grouping and decimal separators.
fn is_inner(c: char) -> bool {
    matches!(
        c,
        ',' | '.' | '\u{00A0}' | '\u{202F}' | '\u{066B}' | '\u{066C}' | '\'' | '\u{2019}'
    )
}

/// Around a number: signs, the percent sign, bidi marks.
fn is_affix(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{2212}'
            | '+'
            | '%'
            | '\u{066A}'
            | '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | 'E'
            | 'e'
    )
}

/// `s` with every number — digits, their separators and affixes — replaced
/// by `#`, and the numbers in order.
fn numbers_out(s: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        // A number starts at a digit, or at an affix directly before one.
        let starts = is_digit(chars[i])
            || (is_affix(chars[i])
                && !matches!(chars[i], 'E' | 'e')
                && chars[i + 1..]
                    .iter()
                    .take_while(|c| is_affix(**c))
                    .count()
                    .checked_add(i + 1)
                    .and_then(|j| chars.get(j))
                    .is_some_and(|c| is_digit(*c)));
        if !starts {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_affix(chars[i]) && !is_digit(chars[i]) {
            i += 1;
        }
        loop {
            if i < chars.len() && is_digit(chars[i]) {
                i += 1;
            } else if i + 1 < chars.len()
                && (is_inner(chars[i]) || matches!(chars[i], 'E' | 'e'))
                && (is_digit(chars[i + 1])
                    || chars[i + 1] == '-' && chars.get(i + 2).is_some_and(|c| is_digit(*c)))
            {
                i += if chars[i + 1] == '-' { 2 } else { 1 };
            } else {
                break;
            }
        }
        while i < chars.len()
            && matches!(
                chars[i],
                '%' | '\u{066A}' | '\u{061C}' | '\u{200E}' | '\u{200F}'
            )
        {
            i += 1;
        }
        found.push(chars[start..i].iter().collect());
        out.push('#');
    }
    (out, found)
}

/// How a number's text differs between the two sides: the steps that take
/// the MF2 text to Fluent's, in order, or what is left when they do not.
fn number_kinds(fluent: &str, mf2: &str, kinds: &mut BTreeSet<&'static str>) {
    let mut m: String = mf2
        .chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => char::from_u32(c as u32 - 0x0660 + 0x30).unwrap_or(c),
            '\u{06F0}'..='\u{06F9}' => char::from_u32(c as u32 - 0x06F0 + 0x30).unwrap_or(c),
            _ => c,
        })
        .collect();
    if m != mf2 {
        kinds.insert("native digits");
    }
    let unmarked: String = m
        .chars()
        .filter(|c| !matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}'))
        .map(|c| if c == '\u{2212}' { '-' } else { c })
        .collect();
    if unmarked != m {
        kinds.insert("sign with a bidi mark or U+2212");
        m = unmarked;
    }
    if m.contains(['%', '\u{066A}']) {
        kinds.insert("percent");
        return;
    }
    // The decimal separator is where Fluent's '.' is, counted from the end.
    if let Some(dot) = fluent.find('.') {
        let fraction = fluent.len() - dot - 1;
        let chars: Vec<char> = m.chars().collect();
        if let Some(sep) = chars.len().checked_sub(fraction + 1)
            && let Some(&c) = chars.get(sep)
            && c != '.'
            && !c.is_ascii_digit()
        {
            kinds.insert("decimal separator");
            m = chars[..sep]
                .iter()
                .chain(['.'].iter())
                .chain(&chars[sep + 1..])
                .collect();
        }
    }
    let (int, frac) = match m.find('.') {
        Some(i) => m.split_at(i),
        None => (m.as_str(), ""),
    };
    let ungrouped: String = int
        .chars()
        .filter(|c| {
            !matches!(
                c,
                ',' | '.' | '\u{00A0}' | '\u{202F}' | '\u{066C}' | '\'' | '\u{2019}'
            )
        })
        .chain(frac.chars())
        .collect();
    if ungrouped != m {
        kinds.insert("grouping");
        m = ungrouped;
    }
    if m != fluent {
        kinds.insert("digits: rounding or precision");
    }
}

fn describe(args: &[(&str, Val)]) -> String {
    let parts: Vec<String> = args.iter().map(|(n, v)| format!("${n}={v:?}")).collect();
    parts.join(" ")
}

/// Removes a currency code or symbol written beside a number (`#`).
fn currency_out(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let space = |c: &char| matches!(c, ' ' | '\u{00A0}' | '\u{202F}');
    while i < chars.len() {
        let code = |j: usize| {
            chars
                .get(j..j + 3)
                .is_some_and(|w| w.iter().all(char::is_ascii_uppercase))
        };
        let symbol = |j: usize| {
            chars
                .get(j)
                .is_some_and(|c| matches!(c, '€' | '$' | '£' | '¥'))
        };
        // The width of a code or symbol at `j`, or 0.
        let mark = |j: usize| match (code(j), symbol(j)) {
            (true, _) => 3,
            (false, true) => 1,
            (false, false) => 0,
        };
        // Before the number: `EUR #`, `€#`, `-EUR #`.
        let width = mark(i);
        if width > 0 {
            let mut j = i + width;
            while chars.get(j).is_some_and(space) {
                j += 1;
            }
            if chars.get(j) == Some(&'#') {
                if out.ends_with('-') {
                    out.pop();
                }
                i = j;
                continue;
            }
        }
        // After it: `# EUR`, `# €`.
        if chars[i] == '#' {
            let mut j = i + 1;
            while chars.get(j).is_some_and(space) {
                j += 1;
            }
            let width = mark(j);
            if width > 0 {
                out.push('#');
                i = j + width;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Text with isolation marks removed (when they are on), numbers read as
/// one token each, and currency marks beside them removed.
fn shape(s: &str, isolating: bool) -> (String, Vec<String>) {
    let s = if isolating {
        strip_isolates(s)
    } else {
        s.to_owned()
    };
    let (text, numbers) = numbers_out(&s);
    (currency_out(&text), numbers)
}

/// Whether `:number`'s default `maximumFractionDigits` (3) changes `v`.
fn rounds(v: Val) -> bool {
    matches!(v, Val::Float(x) if (x * 1000.0).fract() != 0.0)
}

/// `v` as `:number`'s default `maximumFractionDigits` rounds it.
fn rounded(v: Val) -> Val {
    match v {
        Val::Float(x) if rounds(v) => Val::Float((x * 1000.0).round() / 1000.0),
        _ => v,
    }
}

/// One pair that differs, and what it is known to be.
struct Pair<'a> {
    setting: &'a str,
    loc: &'a Locale,
    unit: &'a Unit,
    pattern: &'a ast::Pattern<&'a str>,
    args: &'a [(&'a str, Val)],
    fluent: &'a str,
    mf2: &'a str,
    isolating: bool,
}

fn classify(report: &mut Report, p: &Pair<'_>) {
    let example = || {
        format!(
            "{} {}  {}  fluent={:?} mf2={:?}",
            p.loc.tag,
            p.unit.id,
            describe(p.args),
            p.fluent,
            p.mf2
        )
    };
    if p.isolating && strip_isolates(p.fluent) == strip_isolates(p.mf2) {
        report.add(
            p.setting,
            "isolation: where FSI/PDI are placed".into(),
            example,
        );
        return;
    }
    let (fs, fnums) = shape(p.fluent, p.isolating);
    let (ms, mnums) = shape(p.mf2, p.isolating);
    if fs == ms && fnums.len() == mnums.len() {
        // Where the numbers that differ sit in the Fluent message.
        let mut places = BTreeSet::new();
        for (name, v) in p.args {
            if v.is_number()
                && let Some(pl) = p.unit.reads.places.get(*name)
            {
                places.extend(pl.iter().copied());
            }
        }
        let place = match places.iter().collect::<Vec<_>>().as_slice() {
            [] => "a number literal",
            [Place::NumberCall] => "NUMBER() placeholder",
            [Place::UnderSelector] => "placeholder of a selector variable",
            [Place::Bare] => "unannotated placeholder",
            _ => "several kinds of placeholder",
        };
        let mut kinds = BTreeSet::new();
        if strip_isolates(p.mf2).contains(|c: char| c.is_ascii_uppercase() || "€$£¥".contains(c))
            && ms
                != numbers_out(&if p.isolating {
                    strip_isolates(p.mf2)
                } else {
                    p.mf2.to_owned()
                })
                .0
        {
            kinds.insert("currency code or symbol");
        }
        for (a, b) in fnums.iter().zip(&mnums) {
            if a != b {
                number_kinds(a, b, &mut kinds);
            }
        }
        let kinds: Vec<&str> = kinds.into_iter().collect();
        report.add(
            p.setting,
            format!("number rendering: {place}: {}", kinds.join(" + ")),
            example,
        );
        return;
    }
    // A different variant. Selecting on a number with more than three
    // decimals: MF2 selects on the value `:number` shows, rounded to three;
    // Fluent on the value given. Given the rounded value, Fluent agrees.
    let selecting = |name: &str| p.unit.reads.selectors.contains(name);
    if p.args.iter().any(|(n, v)| selecting(n) && rounds(*v)) {
        let mut fargs = FluentArgs::new();
        for (name, v) in p.args {
            let v = if selecting(name) { rounded(*v) } else { *v };
            fargs.set(*name, v.fluent());
        }
        let mut errors = Vec::new();
        let again = p
            .loc
            .bundle
            .format_pattern(p.pattern, Some(&fargs), &mut errors);
        if shape(&again, p.isolating).0 == ms {
            report.add(
                p.setting,
                "selection: a number with more than three decimals selects as :number rounds it"
                    .into(),
                example,
            );
            return;
        }
    }
    // The two sides put the number in different plural categories.
    for (name, v) in p.args {
        for (ordinal, set) in [
            (false, &p.unit.reads.cardinal),
            (true, &p.unit.reads.ordinal),
        ] {
            if !v.is_number() || !set.contains(*name) {
                continue;
            }
            let (f, m) = p.loc.probe.categories(*v, ordinal);
            if f != m {
                let why = match (ordinal, v) {
                    (true, Val::Float(x)) if x.fract() != 0.0 => {
                        "ordinal category of a non-integer"
                    }
                    (true, _) => "ordinal rules differ",
                    (false, _) => "cardinal rules differ",
                };
                report.add(
                    p.setting,
                    format!(
                        "selection: plural category: {why} ({} {v:?}: fluent-bundle {f}, mf2 {m})",
                        p.loc.tag
                    ),
                    example,
                );
                return;
            }
        }
    }
    report.add(p.setting, "unexplained".into(), example);
}

/// One comparison run: every unit of every locale, every case, one setting.
fn compare(
    report: &mut Report,
    corpus: &str,
    locales: &mut [Locale],
    built: &Built,
    features: &str,
    isolating: bool,
) {
    let setting = format!(
        "{corpus} [{features}, isolation {}]",
        if isolating { "on" } else { "off" }
    );
    let (registry, cx) = (
        if features == "fn-number" {
            &FN_NUMBER
        } else {
            &CORE
        },
        if isolating { &ISOLATING } else { &PLAIN },
    );
    for loc in locales.iter_mut() {
        loc.bundle.set_use_isolating(isolating);
        let catalog = built
            .catalogs
            .get(&loc.tag)
            .unwrap_or_else(|| panic!("no catalog for {}", loc.tag));
        let formatter = Formatter::new(catalog, registry, cx);
        for unit in &loc.units {
            if unit.reads.datetime {
                *report.no_oracle.entry(setting.clone()).or_default() += 1;
                continue;
            }
            let Some(&id) = built.ids.get(&unit.id) else {
                report.add(&setting, "unexplained: not converted".into(), || {
                    format!("{} {}", loc.tag, unit.id)
                });
                continue;
            };
            let message = loc.bundle.get_message(&unit.message).expect("the message");
            let pattern = match &unit.attribute {
                Some(a) => message.get_attribute(a).expect("the attribute").value(),
                None => message.value().expect("a value"),
            };
            let vars: Vec<(&str, &[Val])> = unit
                .reads
                .places
                .keys()
                .map(|name| {
                    let set: &[Val] = if unit.reads.numeric.contains(name) {
                        &NUMBERS
                    } else {
                        &ALL
                    };
                    (name.as_str(), set)
                })
                .collect();
            let cases = vars.iter().map(|(_, s)| s.len()).max().unwrap_or(1);
            for i in 0..cases {
                let args: Vec<(&str, Val)> = vars
                    .iter()
                    .enumerate()
                    .map(|(j, (name, set))| (*name, set[(i + 7 * j) % set.len()]))
                    .collect();
                let mut fargs = FluentArgs::new();
                for (name, v) in &args {
                    fargs.set(*name, v.fluent());
                }
                let mut ferrors = Vec::new();
                let fluent = loc
                    .bundle
                    .format_pattern(pattern, Some(&fargs), &mut ferrors);
                let margs: Vec<(&str, Arg<'_>)> = args.iter().map(|(n, v)| (*n, v.mf2())).collect();
                let mut mf2 = String::new();
                let mut merrors = Vec::new();
                formatter.write_named(id, &margs, &mut mf2, &mut merrors);
                *report.compared.entry(setting.clone()).or_default() += 1;
                if ferrors.is_empty() != merrors.is_empty() {
                    report.add(&setting, "unexplained: errors on one side".into(), || {
                        format!(
                            "{} {}  {}  fluent={fluent:?} {ferrors:?} mf2={mf2:?} {merrors:?}",
                            loc.tag,
                            unit.id,
                            describe(&args)
                        )
                    });
                    continue;
                }
                if fluent != mf2 {
                    classify(
                        report,
                        &Pair {
                            setting: &setting,
                            loc,
                            unit,
                            pattern,
                            args: &args,
                            fluent: &fluent,
                            mf2: &mf2,
                            isolating,
                        },
                    );
                }
            }
        }
    }
}

static ALL: [Val; 44] = {
    let mut all = [Val::Int(0); 44];
    let mut i = 0;
    while i < STRINGS.len() {
        all[i] = STRINGS[i];
        i += 1;
    }
    while i < 44 {
        all[i] = NUMBERS[i - STRINGS.len()];
        i += 1;
    }
    all
};

/// Converts `ftl`, builds it both ways and compares every setting.
fn run(report: &mut Report, corpus: &str, ftl: &Path, out: &Path) {
    let _ = std::fs::remove_dir_all(out);
    convert(ftl, out);
    let mut locales = load(ftl);
    for features in ["core", "fn-number"] {
        // The dates are built for, not compared: `fluent-bundle` has no
        // `DATETIME`.
        let names: &[&str] = if features == "core" {
            &[]
        } else {
            &["fn-number", "fn-datetime"]
        };
        let Some(built) = build(out, names) else {
            continue;
        };
        for isolating in [true, false] {
            compare(report, corpus, &mut locales, &built, features, isolating);
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fluent-oracle")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

/// A2's reference workload, as `.ftl`, under `dir/ftl`.
fn workload(dir: &Path) -> PathBuf {
    let knobs = workload_gen::Knobs::default();
    let files =
        workload_gen::generate_as(&knobs, &[workload_gen::Format::Ftl], &[]).expect("the workload");
    for (path, bytes) in files.iter() {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, bytes).expect("write");
    }
    dir.join("ftl")
}

#[test]
fn converted_catalogs_format_as_the_fluent_originals() {
    let dir = scratch("corpora");
    let mut report = Report::default();
    let ftl = workload(&dir.join("workload"));
    run(&mut report, "workload", &ftl, &dir.join("workload-out"));
    let constructs = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fluent/constructs");
    run(
        &mut report,
        "constructs",
        &constructs,
        &dir.join("constructs-out"),
    );
    let text = report.text();
    std::fs::write(dir.join("report.txt"), &text).expect("write");
    println!("{text}");
    let unexplained = report.unexplained();
    assert!(unexplained.is_empty(), "{text}");
    let unapproved: Vec<&String> = report
        .classes
        .keys()
        .map(|(_, class)| class)
        .filter(|class| !APPROVED.iter().any(|a| class.starts_with(a)))
        .collect();
    assert!(
        unapproved.is_empty(),
        "not approved: {unapproved:#?}\n{text}"
    );
}

/// The classes of difference the owner approved (2026-09-24,
/// the Phase 8 work order §A3): what "formats identically" allows.
const APPROVED: [&str; 5] = [
    // Localized numbers where `fluent-bundle` writes `f64`'s `Display`.
    "number rendering:",
    // The Default Bidi Strategy's isolates, not `fluent-bundle`'s.
    "isolation:",
    // `:number` selects on the value it shows, rounded to three decimals.
    "selection: a number with more than three decimals",
    // Current CLDR plural rules against `intl_pluralrules`' older ones.
    "selection: plural category: cardinal rules differ",
    "selection: plural category: ordinal category of a non-integer",
];

/// The negative control: a plural key dropped from one converted message
/// (Polish `one`, which only 1 reaches) fails the harness, naming the
/// message and the argument.
#[test]
fn a_dropped_plural_key_fails_naming_the_message_and_the_argument() {
    let dir = scratch("dropped-key");
    let ftl = workload(&dir.join("workload"));
    let out = dir.join("out");
    convert(&ftl, &out);
    let file = out.join("locales/pl/common.mf2");
    let source = std::fs::read_to_string(&file).expect("read");
    let (at, id) = {
        let lines: Vec<&str> = source.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.starts_with("  one {{"))
            .expect("a Polish `one` variant");
        let id = lines[..at]
            .iter()
            .rev()
            .find_map(|l| l.split_once(" =").map(|(id, _)| id.trim()))
            .expect("its message")
            .to_owned();
        (at, id)
    };
    let edited: Vec<&str> = source
        .lines()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, l)| l)
        .collect();
    std::fs::write(&file, edited.join("\n") + "\n").expect("write");

    let mut locales = load(&ftl);
    let built = build(&out, &["fn-number"]).expect("builds");
    let mut report = Report::default();
    compare(
        &mut report,
        "workload",
        &mut locales,
        &built,
        "fn-number",
        false,
    );
    let unexplained = report.unexplained();
    let named = format!("pl {id}  $count=Int(1)  ");
    assert!(
        unexplained.iter().any(|e| e.starts_with(&named)),
        "expected {named:?} among {unexplained:#?}"
    );
    assert!(
        unexplained
            .iter()
            .all(|e| e.starts_with(&format!("pl {id} "))),
        "{unexplained:#?}"
    );
}
