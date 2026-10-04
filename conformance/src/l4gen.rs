//! Layer L4 on generated input: a message generated from the
//! vendored `message.abnf`, **steered** towards the functions the runtime
//! has, compiled for a random locale, unstripped and stripped, with
//! generated arguments — one [`Generated`] per seed, deterministically.
//!
//! The grammar alone almost never spells `:number` or one of its options, so
//! the parsed model is rewritten before it is written: most function names
//! become one of [`mf2_l4_runner::FUNCTIONS`] (a few stay unknown), most
//! options one that function reads, with a value drawn from the values it
//! takes and some it rejects; literal operands often become numbers, and
//! variant keys plural keywords or small integers; unannotated placeholders
//! and `.input`s sometimes gain a function; placeholders and selectors often
//! refer to a declared name (so declarations are evaluated, and selectors
//! select on an annotated value); variants usually get one key per selector
//! and a catch-all. Markup, attributes and the text between stay as
//! generated.
//!
//! Every generated message is written, **valid or not**: the writer accepts
//! models with data-model errors (a build refuses to ship them; the format
//! does not), so the runtime must format those without panicking too.

use std::borrow::Cow;
use std::collections::BTreeSet;

use mf2_catalog::format::locale_key::ICU_BLOB;
use mf2_catalog::writer::{self, Options as WriterOptions};
use mf2_l4_runner::{ArgSpec, Case, Config};
use mf2_locale_data::icu_blob::{DateNeeds, IcuBlobSpec, icu_blob};
use mf2_locale_data::number::{NumberNeeds, number_locale_entries};
use mf2_locale_data::{PluralKind, Selection, direction, plural_locale_entries};
use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, FunctionRef, Key, Literal, LiteralExpression,
    Message, OptionValue, Options, Pattern, PatternPart, VariableExpression, VariableRef, Variant,
};
use mf2_runtime::BidiStrategy;

use crate::abnf::{Generator, Grammar, Rng};

/// One generated case.
#[derive(Clone, Debug)]
pub struct Generated {
    /// The steered message, serialized (for reports).
    pub source: String,
    /// Whether the steered model is free of data-model errors (only valid
    /// messages ship; the others are formatted all the same).
    pub valid: bool,
    /// The catalog's locale.
    pub locale: &'static str,
    /// The case from the unstripped catalog.
    pub unstripped: Case,
    /// The same case from the stripped catalog.
    pub stripped: Case,
    /// When the catalog carries `icu.blob`: the same case from a catalog
    /// whose blob has the data of every shape (`DateNeeds::all`), which
    /// must format as the sliced one.
    pub all_dates: Option<Case>,
}

/// The locales generated messages are compiled for: Latin and non-Latin,
/// both directions, simple and complex plural rules, `und` (root), the
/// goldens' locale panel (`goldens::PANEL`), and two non-Latin numbering
/// systems (Phase 4, A9).
pub const LOCALES: [&str; 18] = [
    "en",
    "en",
    "en",
    "pl",
    "ar",
    "he",
    "cy",
    "ja",
    "fr-CA",
    "und",
    "es",
    "de",
    "fr",
    "hi",
    "ru",
    "ar-EG",
    "hi-u-nu-deva",
    "en-US",
];

/// A generated message, before any catalog: what the seed produced and how
/// to format it. Layer L5 needs exactly this — it compiles the source
/// through `mf2-build` into a corpus of its own — and it costs none of the
/// locale data and catalog writing [`case`] does.
#[derive(Clone, Debug)]
pub struct GeneratedMessage {
    /// The steered message, serialized.
    pub source: String,
    /// Whether the steered model is free of data-model errors.
    pub valid: bool,
    /// The locale the seed chose.
    pub locale: &'static str,
    /// The bidi strategy the seed chose.
    pub bidi: BidiStrategy,
    /// The arguments to format it with — some names deliberately left unset,
    /// some spelled in another normalization form.
    pub args: Vec<(String, ArgSpec)>,
}

/// The message for `seed`, without compiling it.
pub fn message(grammar: &Grammar, seed: u64) -> Result<GeneratedMessage, String> {
    steered(grammar, seed).map(|(_, m)| m)
}

/// The steered model and everything derived from the seed. [`case`] goes on
/// to compile it; [`message`] stops here.
fn steered(grammar: &Grammar, seed: u64) -> Result<(Message<'static>, GeneratedMessage), String> {
    let src = Generator::new(grammar, seed).generate("message");
    let parsed = mf2_syntax::parse_model(&src);
    let Some(model) = parsed.message else {
        return Err(format!("no model for generated {src:?}"));
    };
    let mut r = Rng::new(seed ^ 0x4c34_6765_6e21_5f5f);
    let model = steer(model.into_owned(), &mut r);
    let source = mf2_syntax::serialize(&model).map_err(|e| format!("serialize: {e}"))?;
    let valid = mf2_syntax::validate(&model).is_empty();
    let locale = LOCALES[index(&mut r, LOCALES.len())];
    let bidi = if r.chance(1, 4) {
        BidiStrategy::None
    } else {
        BidiStrategy::Default
    };
    let analysis = mf2_syntax::analyze(&model);
    let dated = date_operands(&model);
    let mut args: Vec<(String, ArgSpec)> = Vec::new();
    for n in &analysis.externals {
        // Some left unset; the caller may spell a name in any normalization
        // form.
        if r.chance(1, 10) {
            continue;
        }
        let name = if r.chance(1, 2) { n.spelling } else { &n.nfc };
        // A variable a date/time function takes is mostly given a date.
        let value = if dated.contains(n.spelling) && r.chance(3, 4) {
            date_arg(&mut r)
        } else {
            arg(&mut r)
        };
        args.push((name.to_owned(), value));
    }
    Ok((
        model,
        GeneratedMessage {
            source,
            valid,
            locale,
            bidi,
            args,
        },
    ))
}

/// The case for `seed`: the message is `Generator::new(grammar,
/// seed).generate("message")` (so `generated.rs`'s case `n` and this one
/// start from the same message), steered with a second stream of the same
/// seed. `Err` only if the generated text does not parse (a parser bug that
/// `generated.rs` reports) or the writer fails.
pub fn case(grammar: &Grammar, seed: u64) -> Result<Generated, String> {
    let (model, generated) = steered(grammar, seed)?;
    let GeneratedMessage {
        source,
        valid,
        locale,
        bidi,
        args,
    } = generated;
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = WriterOptions::new(locale, direction(locale).map_err(|e| e.to_string())?);
    options.cldr_version = Some(mf2_locale_data::CLDR_VERSION);
    options.locale_entries =
        plural_locale_entries(locale, &[PluralKind::Cardinal, PluralKind::Ordinal])
            .map_err(|e| e.to_string())?;
    // The locale's number data, as `mf2::compile_str` writes it: symbols
    // always, the patterns of the numeric functions the message names.
    let mut needs = NumberNeeds::from_functions(analysis.functions.iter().map(|f| &*f.nfc));
    needs.symbols = true;
    options
        .locale_entries
        .extend(number_locale_entries(locale, &needs).map_err(|e| e.to_string())?);
    // And the date data of the `icu` date formatter, as `compile_str`
    // writes it: what the message formats (02 §4.4). The same catalog with
    // every shape's data — and each of the steering's other calendars —
    // must format alike: the slicing rule misses nothing.
    let mut dates = DateNeeds::default();
    dates.add_message(&model);
    let mut full_options = None;
    if !dates.is_empty() {
        let mut every = options.clone();
        let mut all = DateNeeds::all();
        all.calendars = Selection::Listed(STEERED_CALENDARS.map(String::from).into());
        every.locale_entries.push((
            ICU_BLOB,
            icu_blob(locale, &IcuBlobSpec::every_variant(all)).map_err(|e| e.to_string())?,
        ));
        full_options = Some(every);
        options.locale_entries.push((
            ICU_BLOB,
            icu_blob(locale, &IcuBlobSpec::every_variant(dates)).map_err(|e| e.to_string())?,
        ));
    }
    let make = |options: &WriterOptions, id: String| -> Result<Case, String> {
        let (catalog, manifest) =
            writer::single(&model, &slots, options).map_err(|e| format!("writer::single: {e}"))?;
        Ok(Case {
            id,
            catalog,
            manifest_hash: manifest.hash(),
            bidi,
            args: args.clone(),
            config: Config::All,
        })
    };
    Ok(Generated {
        all_dates: full_options
            .map(|o| make(&o, format!("gen#{seed:x}/all-dates")))
            .transpose()?,
        unstripped: make(&options, format!("gen#{seed:x}/unstripped"))?,
        stripped: make(
            &options.clone().stripped(),
            format!("gen#{seed:x}/stripped"),
        )?,
        source,
        valid,
        locale,
    })
}

fn index(r: &mut Rng, n: usize) -> usize {
    usize::try_from(r.below(n as u64)).unwrap_or(0)
}

fn pick<T: Copy>(r: &mut Rng, xs: &[T]) -> T {
    xs[index(r, xs.len())]
}

// ───────────────────────────────────────────────────────── arguments ──

/// Number-literal text, well-formed or not, as an operand or a `Decimal`.
const NUMBERS: [&str; 24] = [
    "0",
    "-0",
    "1",
    "2",
    "3",
    "5",
    "11",
    "-1",
    "0.5",
    "1.50",
    "2.5",
    "-2.5",
    "0.05",
    "9.995",
    "1e3",
    "1E-7",
    "123456789.987654321",
    "12345678901234567890123456789012345678901234567890",
    "1e9999",
    "1e10000",
    "0.1e-9999",
    "1.",
    "abc",
    "",
];

/// Strings: numbers, text in both directions, combining marks, markup-like
/// text, and characters the fallback escapes.
const STRINGS: [&str; 14] = [
    "1",
    "-2.5",
    "1e3",
    "hello",
    "",
    "e\u{301}",
    "\u{1e0a}\u{323}",
    "שלום",
    "مرحبا",
    "a|b\\c",
    "{$x}",
    "\u{2067}x\u{2069}",
    "one",
    "*",
];

/// Date/time literal text, well-formed or not (Phase 4: `:datetime`,
/// `:date`, `:time` operands, string and typed arguments) — among them two
/// floating wall times New York skips and repeats (2021's daylight-saving
/// changes), for the zone search.
const DATES: [&str; 16] = [
    "2006-01-02",
    "2006-01-02T15:04:06",
    "2021-03-14T02:30:00",
    "2021-11-07T01:30:00",
    "2006-01-02T15:04:06Z",
    "2006-01-02T15:04:06.789+05:30",
    "2006-01-02T15:04:06-14:00",
    "1970-01-01T00:00:00Z",
    "0001-01-01",
    "9999-12-31T23:59:59.999Z",
    "2024-02-29",
    "2023-02-29",
    "2006-1-2",
    "2006-01-02T24:00:00",
    "horse",
    "",
];

/// A date/time argument: typed, or as text.
fn date_arg(r: &mut Rng) -> ArgSpec {
    let d = pick(r, &DATES);
    if r.chance(1, 2) {
        ArgSpec::date_time(d)
    } else {
        ArgSpec::Str(d.to_owned())
    }
}

/// The variables `model` hands to a date/time function as its operand
/// (as written).
fn date_operands(model: &Message<'_>) -> BTreeSet<String> {
    fn expression(e: &Expression<'_>, out: &mut BTreeSet<String>) {
        if let Expression::Variable(v) = e
            && v.function
                .as_ref()
                .is_some_and(|f| is_date_function(&f.name))
        {
            out.insert(v.arg.name.to_string());
        }
    }
    let mut out = BTreeSet::new();
    for d in model.declarations() {
        match d {
            Declaration::Input(i) => {
                if i.value
                    .function
                    .as_ref()
                    .is_some_and(|f| is_date_function(&f.name))
                {
                    out.insert(i.value.arg.name.to_string());
                }
            }
            Declaration::Local(l) => expression(&l.value, &mut out),
            _ => {}
        }
    }
    let patterns: Vec<&Pattern<'_>> = match model {
        Message::Pattern(p) => vec![&p.pattern],
        Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
        _ => Vec::new(),
    };
    for p in patterns {
        for part in p.parts() {
            if let PatternPart::Expression(e) = part {
                expression(e, &mut out);
            }
        }
    }
    out
}

fn arg(r: &mut Rng) -> ArgSpec {
    if r.chance(1, 8) {
        return date_arg(r);
    }
    match r.below(6) {
        0 | 1 => ArgSpec::Str(pick(r, &STRINGS).to_owned()),
        2 if r.chance(1, 2) => ArgSpec::Str(pick(r, &NUMBERS).to_owned()),
        2 => ArgSpec::Int(pick(
            r,
            &[0, 1, 2, 3, 5, 11, 21, -1, -7, 1_000_000, i64::MAX, i64::MIN],
        )),
        3 => ArgSpec::Float(pick(
            r,
            &[
                0.0,
                -0.0,
                0.5,
                1.0,
                -2.5,
                1e21,
                1e-7,
                f64::MAX,
                f64::MIN_POSITIVE,
                f64::NAN,
                f64::INFINITY,
            ],
        )),
        4 => ArgSpec::Decimal(pick(r, &NUMBERS).to_owned()),
        _ => ArgSpec::Other,
    }
}

// ────────────────────────────────────────────────────────── steering ──

/// The functions a steered annotation names (weighted by repetition), and
/// `None`: keep the generated name (almost always an unknown function).
/// Phase 4 adds the percent, currency, unit and date/time functions (A9).
const FUNCTION_NAMES: [Option<&str>; 18] = [
    Some("number"),
    Some("number"),
    Some("number"),
    Some("integer"),
    Some("integer"),
    Some("offset"),
    Some("percent"),
    Some("currency"),
    Some("unit"),
    Some("datetime"),
    Some("date"),
    Some("time"),
    Some("string"),
    Some("string"),
    Some("test:function"),
    Some("test:select"),
    Some("test:format"),
    None,
];

/// The calendars the steering names besides `gregory` (kept in step with
/// `fuzz/fuzz_targets/format.rs`).
pub const STEERED_CALENDARS: [&str; 3] = ["buddhist", "hebrew", "japanese"];

/// Whether `function` takes a date/time operand.
fn is_date_function(function: &str) -> bool {
    matches!(function, "datetime" | "date" | "time")
}

const DIGITS: &[&str] = &["0", "1", "2", "3", "5", "20", "21", "100", "-1", "1.5", "x"];

/// The options a function reads, and values for each (valid and not).
fn options_of(function: &str) -> &'static [(&'static str, &'static [&'static str])] {
    const NUMBER: &[(&str, &[&str])] = &[
        ("minimumIntegerDigits", DIGITS),
        ("minimumFractionDigits", DIGITS),
        ("maximumFractionDigits", DIGITS),
        ("minimumSignificantDigits", DIGITS),
        ("maximumSignificantDigits", DIGITS),
        (
            "roundingPriority",
            &["auto", "morePrecision", "lessPrecision", "x"],
        ),
        (
            "roundingIncrement",
            &["1", "2", "5", "10", "25", "50", "2500", "3", "0"],
        ),
        (
            "roundingMode",
            &[
                "ceil",
                "floor",
                "expand",
                "trunc",
                "halfCeil",
                "halfFloor",
                "halfExpand",
                "halfTrunc",
                "halfEven",
                "x",
            ],
        ),
        ("trailingZeroDisplay", &["auto", "stripIfInteger", "x"]),
        (
            "signDisplay",
            &["auto", "always", "exceptZero", "negative", "never", "x"],
        ),
        ("useGrouping", &["auto", "always", "min2", "never", "x"]),
        ("select", &["exact", "plural", "ordinal", "x"]),
        ("u:dir", &["ltr", "rtl", "auto", "inherit", "x"]),
        ("u:id", &["id", ""]),
    ];
    const OFFSET: &[(&str, &[&str])] = &[
        (
            "add",
            &["1", "2", "-1", "0.5", "1e3", "99999999999999999999", "x"],
        ),
        ("subtract", &["1", "2", "-1", "0.5", "1e3", "x"]),
        ("u:dir", &["ltr", "rtl", "auto"]),
    ];
    const STRING: &[(&str, &[&str])] = &[
        ("u:dir", &["ltr", "rtl", "auto", "inherit", "x"]),
        ("u:id", &["id", ""]),
    ];
    const TEST: &[(&str, &[&str])] = &[
        ("decimalPlaces", &["0", "1", "2", "3", "x"]),
        ("fails", &["never", "format", "select", "always", "x"]),
        ("u:dir", &["ltr", "rtl"]),
    ];
    const CURRENCY: &[(&str, &[&str])] = &[
        (
            "currency",
            &["EUR", "USD", "JPY", "GBP", "eur", "XXX", "EURO", "x"],
        ),
        (
            "currencyDisplay",
            &["narrowSymbol", "symbol", "name", "code", "never", "x"],
        ),
        ("currencySign", &["accounting", "standard", "x"]),
        ("fractionDigits", &["auto", "0", "2", "3", "x"]),
        ("useGrouping", &["auto", "always", "min2", "never"]),
        ("minimumIntegerDigits", DIGITS),
        ("maximumSignificantDigits", DIGITS),
        ("trailingZeroDisplay", &["auto", "stripIfInteger"]),
        ("roundingMode", &["halfEven", "ceil", "x"]),
        ("select", &["plural", "x"]),
    ];
    const UNIT: &[(&str, &[&str])] = &[
        (
            "unit",
            &[
                "kilometer",
                "meter-per-second",
                "celsius",
                "liter-per-100-kilometer",
                "furlong",
                "x-y",
                "Meter",
                "",
            ],
        ),
        ("unitDisplay", &["short", "narrow", "long", "x"]),
        ("usage", &["road", "x"]),
        ("signDisplay", &["always", "never"]),
        ("maximumFractionDigits", DIGITS),
        ("useGrouping", &["auto", "never"]),
    ];
    const DATE_TIME: &[(&str, &[&str])] = &[
        (
            "dateFields",
            &["weekday", "month-day", "year-month-day-weekday", "x"],
        ),
        ("dateLength", &["long", "medium", "short", "x"]),
        ("timePrecision", &["hour", "minute", "second", "x"]),
        ("fields", &["day-weekday", "year-month-day", "x"]),
        ("length", &["long", "short", "x"]),
        ("precision", &["hour", "second", "x"]),
        ("timeZoneStyle", &["long", "short", "x"]),
        (
            "timeZone",
            &[
                "UTC",
                "input",
                "+05:30",
                "-14:00",
                "America/New_York",
                "Europe/Paris",
                "Asia/Kathmandu",
                "Australia/Lord_Howe",
                "Etc/Unknown",
                "25:00",
                "x/",
            ],
        ),
        ("hour12", &["true", "false", "x"]),
        (
            "calendar",
            &["gregory", "japanese", "buddhist", "hebrew", "x_y", "x"],
        ),
        ("u:dir", &["ltr", "rtl", "auto"]),
    ];
    match function {
        "number" | "integer" | "percent" => NUMBER,
        "offset" => OFFSET,
        "string" => STRING,
        "currency" => CURRENCY,
        "unit" => UNIT,
        "datetime" | "date" | "time" => DATE_TIME,
        _ => TEST,
    }
}

/// Keys that match something: plural keywords and small integers.
const KEYS: [&str; 10] = [
    "0", "1", "2", "one", "few", "many", "other", "zero", "two", "1.0",
];

fn steer(message: Message<'static>, r: &mut Rng) -> Message<'static> {
    // References are often redirected to a declared name, so declarations
    // are evaluated and selectors select on an annotated value.
    let declared: Vec<String> = message
        .declarations()
        .iter()
        .map(|d| d.name().to_owned())
        .collect();
    let mut s = Steer { r, declared };
    match message {
        Message::Pattern(mut m) => {
            s.declarations(&mut m.declarations);
            m.pattern = s.pattern(core::mem::take(&mut m.pattern));
            Message::Pattern(m)
        }
        Message::Select(mut m) => {
            s.declarations(&mut m.declarations);
            for sel in &mut m.selectors {
                if let Some(name) = s.declared_name(2, 3) {
                    sel.name = Cow::Owned(name);
                }
            }
            let width = m.selectors.len();
            for v in &mut m.variants {
                for k in &mut v.keys {
                    if let Key::Literal(l) = k
                        && s.r.chance(1, 2)
                    {
                        l.value = Cow::Borrowed(pick(s.r, &KEYS));
                    }
                }
                // Usually one key per selector (a mismatch is a data-model
                // error the generator reaches on its own).
                if s.r.chance(3, 4) {
                    v.keys.truncate(width);
                    while v.keys.len() < width {
                        v.keys.push(catch_all());
                    }
                }
                v.value = s.pattern(core::mem::take(&mut v.value));
            }
            // Usually a catch-all variant (its absence is a data-model error).
            let has_fallback = m
                .variants
                .iter()
                .any(|v| v.keys.iter().all(|k| matches!(k, Key::CatchAll(_))));
            if !has_fallback && s.r.chance(3, 4) {
                m.variants.push(Variant {
                    keys: (0..width).map(|_| catch_all()).collect(),
                    value: Pattern::from_text(Cow::Borrowed("other")),
                });
            }
            Message::Select(m)
        }
        other => other,
    }
}

/// An annotation with no options, for [`Steer::function`] to fill.
fn new_function() -> FunctionRef<'static> {
    FunctionRef {
        name: Cow::Borrowed("number"),
        options: Options::default(),
    }
}

fn catch_all() -> Key<'static> {
    Key::CatchAll(CatchAllKey { value: None })
}

struct Steer<'r> {
    r: &'r mut Rng,
    declared: Vec<String>,
}

impl Steer<'_> {
    /// With probability `num / den`, a declared name (if there is one).
    fn declared_name(&mut self, num: u64, den: u64) -> Option<String> {
        if self.declared.is_empty() || !self.r.chance(num, den) {
            return None;
        }
        Some(self.declared[index(self.r, self.declared.len())].clone())
    }

    fn declarations(&mut self, declarations: &mut [Declaration<'static>]) {
        for d in declarations {
            match d {
                Declaration::Input(i) => match &mut i.value.function {
                    Some(f) => self.function(f),
                    None => {
                        if self.r.chance(1, 2) {
                            i.value.function = Some(new_function());
                        }
                    }
                },
                Declaration::Local(l) => self.expression(&mut l.value),
                _ => {}
            }
        }
    }

    fn pattern(&mut self, pattern: Pattern<'static>) -> Pattern<'static> {
        let mut out = Pattern::default();
        for mut part in pattern.into_parts() {
            if let PatternPart::Expression(e) = &mut part {
                self.expression(e);
            }
            out.push(part);
        }
        // Sometimes a placeholder more: a declaration's value, or a number.
        if self.r.chance(1, 3) {
            let e = match self.declared_name(3, 4) {
                Some(name) => Expression::Variable(VariableExpression {
                    arg: VariableRef {
                        name: Cow::Owned(name),
                    },
                    function: None,
                    attributes: Attributes::default(),
                }),
                None => Expression::Literal(LiteralExpression {
                    arg: Literal {
                        value: Cow::Borrowed(pick(self.r, &NUMBERS)),
                    },
                    function: Some(new_function()),
                    attributes: Attributes::default(),
                }),
            };
            let mut e = e;
            self.expression(&mut e);
            out.push(PatternPart::Expression(e));
        }
        out
    }

    fn expression(&mut self, e: &mut Expression<'static>) {
        match e {
            Expression::Literal(l) => {
                if l.function.is_none() && self.r.chance(1, 3) {
                    l.function = Some(new_function());
                }
                if let Some(f) = &mut l.function {
                    self.function(f);
                    // A date/time function mostly gets date/time text.
                    let odds = if is_date_function(&f.name) { 3 } else { 2 };
                    if self.r.chance(odds - 1, odds) {
                        l.arg = Literal {
                            value: Cow::Borrowed(self.operand(&f.name)),
                        };
                    }
                }
            }
            Expression::Variable(v) => {
                if let Some(name) = self.declared_name(1, 3) {
                    v.arg.name = Cow::Owned(name);
                }
                if v.function.is_none() && self.r.chance(1, 3) {
                    v.function = Some(new_function());
                }
                if let Some(f) = &mut v.function {
                    self.function(f);
                }
            }
            Expression::Function(f) => {
                self.function(&mut f.function);
                // Often an operand: a number, or a declaration's value.
                if self.r.chance(2, 3) {
                    let function = Some(f.function.clone());
                    let attributes = f.attributes.clone();
                    *e = match self.declared_name(1, 2) {
                        Some(name) => Expression::Variable(VariableExpression {
                            arg: VariableRef {
                                name: Cow::Owned(name),
                            },
                            function,
                            attributes,
                        }),
                        None => Expression::Literal(LiteralExpression {
                            arg: Literal {
                                value: Cow::Borrowed(self.operand(&f.function.name)),
                            },
                            function,
                            attributes,
                        }),
                    };
                }
            }
            _ => {}
        }
    }

    /// A literal operand for `function`: date/time text for the date/time
    /// functions (sometimes a number), number text for the others.
    fn operand(&mut self, function: &str) -> &'static str {
        if is_date_function(function) && self.r.chance(4, 5) {
            pick(self.r, &DATES)
        } else {
            pick(self.r, &NUMBERS)
        }
    }

    fn function(&mut self, f: &mut FunctionRef<'static>) {
        let Some(name) = pick(self.r, &FUNCTION_NAMES) else {
            return;
        };
        f.name = Cow::Borrowed(name);
        let known = options_of(name);
        let mut options = Options::with_capacity(f.options.len() + 2);
        let mut taken: Vec<&str> = Vec::new();
        for (k, v) in f.options.iter() {
            let (k, v) = if self.r.chance(3, 4) {
                let (k, values) = pick(self.r, known);
                // A variable value usually stays a variable (bound to an
                // argument or a local); a literal becomes one of the
                // option's values.
                let v = match v {
                    OptionValue::Variable(_) if self.r.chance(3, 4) => v.clone(),
                    _ => OptionValue::Literal(Literal {
                        value: Cow::Borrowed(pick(self.r, values)),
                    }),
                };
                (k, v)
            } else {
                (k, v.clone())
            };
            // Renaming must not make a duplicate option (a data-model error
            // the generator reaches on its own).
            if taken.contains(&k) {
                continue;
            }
            taken.push(k);
            options.push(Cow::Owned(k.to_owned()), v);
        }
        // Sometimes an option more.
        for _ in 0..self.r.below(3) {
            let (k, values) = pick(self.r, known);
            if !taken.contains(&k) {
                taken.push(k);
                let v = OptionValue::Literal(Literal {
                    value: Cow::Borrowed(pick(self.r, values)),
                });
                options.push(Cow::Borrowed(k), v);
            }
        }
        f.options = options;
    }
}

#[cfg(test)]
mod tests {
    use super::{LOCALES, case};
    use crate::abnf::Grammar;
    use crate::spec::{ABNF, read_spec};

    fn grammar() -> Grammar {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("conformance/ has a parent")
            .to_path_buf();
        let text = read_spec(&root, ABNF).unwrap_or_else(|e| panic!("{e}"));
        Grammar::parse(&text).expect("the spec's ABNF parses")
    }

    #[test]
    fn deterministic_and_steered() {
        let g = grammar();
        let (mut number, mut locales) = (0, std::collections::BTreeSet::new());
        for seed in 0..300 {
            let a = case(&g, seed).expect("generated case");
            let b = case(&g, seed).expect("generated case");
            assert_eq!(a.source, b.source);
            // As bytes: a `Float(NaN)` argument is not `==` itself.
            let bytes = |g: &super::Generated| {
                mf2_l4_runner::encode_cases(&[g.unstripped.clone(), g.stripped.clone()])
            };
            assert_eq!(bytes(&a), bytes(&b));
            number += usize::from(a.source.contains(":number"));
            locales.insert(a.locale);
        }
        assert!(number > 30, "only {number} messages call :number");
        assert_eq!(locales.len(), LOCALES.len() - 2, "{locales:?}");
    }
}
