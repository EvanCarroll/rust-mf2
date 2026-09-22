//! Conformance layer L4, the client side (`plans/01-conformance.md` §3,
//! `plans/10-phase-3-work-order.md` A9): what runs from a compiled catalog —
//! natively inside the `mf2-conformance` harness, and as `mf2-l4-wasi`
//! under wasmtime on `wasm32-wasip1` — so the two can be compared byte for
//! byte.
//!
//! * [`test_functions`]: `:test:function`, `:test:select`, `:test:format`,
//!   through `mf2-runtime`'s public `Function` trait only;
//! * [`run`]: one [`Case`] (catalog bytes, arguments, bidi strategy) →
//!   one canonical [`Record`] (string output, sorted error names, parts as
//!   canonical JSON), formatting with named arguments, positional arguments
//!   and to parts;
//! * [`encode_cases`] / [`decode_cases`]: the bundle the native side hands
//!   the wasm side.

pub mod test_functions;

use std::fmt::Write as _;

use mf2_catalog::{Catalog, Entry};
use mf2_runtime::functions::STRING;
use mf2_runtime::{
    Arg, BidiStrategy, CustomValue, Date, DateTime, Dir, FormatContext, FormatError, Formatter,
    Function, Host, MarkupKind, MsgId, Part, PartSink, Registry, SubPartSink, Time, Value,
};

/// The handlers L4 formats with — the all-features configuration
/// (`plans/01-conformance.md` §3): `:string`, the localized numeric
/// functions with `:percent`, `:currency` and `:unit` (`fn-number`), the
/// date/time functions (`fn-datetime`) and the test functions.
pub static FUNCTIONS: [(&str, &dyn Function); 13] = [
    ("currency", &mf2_fn_number::CURRENCY),
    ("date", &mf2_fn_datetime::DATE),
    ("datetime", &mf2_fn_datetime::DATETIME),
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &STRING),
    ("test:format", &test_functions::FORMAT),
    ("test:function", &test_functions::FUNCTION),
    ("test:select", &test_functions::SELECT),
    ("time", &mf2_fn_datetime::TIME),
    ("unit", &mf2_fn_number::UNIT),
];

/// The registry over [`FUNCTIONS`], formatting unannotated numbers with the
/// locale's symbols and unannotated date/time values as `:datetime`.
pub static REGISTRY: Registry = Registry::new(&FUNCTIONS)
    .with_numbers(&mf2_fn_number::NUMBERS)
    .with_dates(&mf2_fn_datetime::DATES);

/// The handlers of the **default** configuration (L4d, `plans/01-conformance.md`
/// §3): what an application's generated registry holds with `fn-number` and
/// `fn-datetime` off — `:string` and the core's neutral `:number`,
/// `:integer`, `:offset` — plus the test functions. The gated functions
/// (`:percent`, `:currency`, `:unit`, `:datetime`, `:date`, `:time`) are
/// absent, so they are Unknown Functions.
pub static DEFAULT_FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("integer", &mf2_runtime::functions::INTEGER),
    ("number", &mf2_runtime::functions::NUMBER),
    ("offset", &mf2_runtime::functions::OFFSET),
    ("string", &STRING),
    ("test:format", &test_functions::FORMAT),
    ("test:function", &test_functions::FUNCTION),
    ("test:select", &test_functions::SELECT),
];

/// The registry over [`DEFAULT_FUNCTIONS`]: no hooks, so unannotated numbers
/// are neutral and unannotated dates a Bad Operand.
pub static DEFAULT_REGISTRY: Registry = Registry::new(&DEFAULT_FUNCTIONS);

/// Which configuration a case formats in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Config {
    /// Every feature on (layer L4): [`REGISTRY`].
    #[default]
    All,
    /// The default features (layer L4d): [`DEFAULT_REGISTRY`].
    Default,
}

impl Config {
    fn registry(self) -> &'static Registry {
        match self {
            Config::All => &REGISTRY,
            Config::Default => &DEFAULT_REGISTRY,
        }
    }
}

static DEFAULT_BIDI: FormatContext = FormatContext::new(&mf2_host_std::HOST);

fn context(bidi: BidiStrategy) -> &'static FormatContext {
    static NO_BIDI: std::sync::OnceLock<FormatContext> = std::sync::OnceLock::new();
    match bidi {
        BidiStrategy::Default => &DEFAULT_BIDI,
        BidiStrategy::None => NO_BIDI.get_or_init(|| {
            let mut cx = FormatContext::new(&mf2_host_std::HOST);
            cx.bidi = BidiStrategy::None;
            cx
        }),
    }
}

/// A suite parameter value.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgSpec {
    Str(String),
    Int(i64),
    Float(f64),
    /// An exact decimal as text ([`Arg::Decimal`]; generated input only —
    /// the suite has none).
    Decimal(String),
    /// A date/time ([`Arg::DateTime`]): the suite's `{"type": "datetime"}`
    /// parameters ([`ArgSpec::date_time`]). No zone name (the bundle
    /// carries none).
    DateTime(DateTime<'static>),
    /// A value no core function takes (a boolean, an object, …).
    Other,
}

/// An application value with no conversions (the suite's `true`).
struct Opaque;

impl CustomValue for Opaque {}

static OPAQUE: Opaque = Opaque;

impl ArgSpec {
    /// A typed `datetime` parameter: its value parsed as a date/time literal
    /// (`mf2_fn_datetime::parse_literal`); a value that is not one is
    /// [`ArgSpec::Other`].
    pub fn date_time(value: &str) -> ArgSpec {
        mf2_fn_datetime::parse_literal(value).map_or(ArgSpec::Other, ArgSpec::DateTime)
    }

    fn arg(&self) -> Arg<'_> {
        match self {
            ArgSpec::Str(s) => Arg::Str(s),
            ArgSpec::Int(n) => Arg::Int(*n),
            ArgSpec::Float(x) => Arg::Float(*x),
            ArgSpec::Decimal(d) => Arg::Decimal(d),
            ArgSpec::DateTime(d) => Arg::DateTime(d),
            ArgSpec::Other => Arg::Custom(&OPAQUE),
        }
    }
}

/// One L4 case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The test's ledger key, for reports.
    pub id: String,
    /// The one-message catalog.
    pub catalog: Vec<u8>,
    pub manifest_hash: u64,
    pub bidi: BidiStrategy,
    /// Named arguments, as the suite's `params`.
    pub args: Vec<(String, ArgSpec)>,
    /// The configuration (the registry) it formats in.
    pub config: Config,
}

/// What formatting a case produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// String output.
    pub text: String,
    /// The suite names of the errors, sorted (the spec fixes no order).
    pub errors: Vec<String>,
    /// Parts output, as canonical JSON (`expParts`' shape).
    pub parts: String,
    /// The error names of the parts run, sorted.
    pub parts_errors: Vec<String>,
}

impl Record {
    /// One line of canonical text (for the native/wasm comparison).
    pub fn line(&self) -> String {
        let mut s = String::new();
        json_string(&mut s, &self.text);
        s.push('\t');
        s.push_str(&self.errors.join(","));
        s.push('\t');
        s.push_str(&self.parts);
        s.push('\t');
        s.push_str(&self.parts_errors.join(","));
        s
    }
}

/// The message of a one-message catalog.
const ID: MsgId = MsgId::from_raw(0);

fn names(errors: &[FormatError]) -> Vec<String> {
    let mut v: Vec<String> = errors
        .iter()
        .map(|e| match e.kind() {
            Some(k) => k.suite_name().to_owned(),
            None => format!("{e:?}"),
        })
        .collect();
    v.sort();
    v
}

/// Formats `case`: named arguments (string and parts) and positional
/// arguments (string), which must agree.
pub fn run(case: &Case) -> Result<Record, String> {
    let catalog = Catalog::new(case.catalog.clone(), case.manifest_hash)
        .map_err(|e| format!("Catalog::new: {e:?}"))?;
    let f = Formatter::new(&catalog, case.config.registry(), context(case.bidi));
    let named: Vec<(&str, Arg<'_>)> = case
        .args
        .iter()
        .map(|(n, a)| (n.as_str(), a.arg()))
        .collect();

    let mut text = String::new();
    let mut errors = Vec::new();
    f.write_named(ID, &named, &mut text, &mut errors);

    // Positional: the slots' names are the catalog's NAMES.
    let slot_names = match catalog.get(ID) {
        Entry::Pattern(m) | Entry::Select(m) => {
            let n = m.names();
            (0..n.external_count())
                .map(|s| n.external(s).and_then(|r| catalog.text(r)).unwrap_or(""))
                .collect()
        }
        _ => Vec::new(),
    };
    // Names compare under NFC (plans/03-runtime.md §7).
    let nfc_names: Vec<String> = case
        .args
        .iter()
        .map(|(n, _)| {
            let mut buf = String::new();
            mf2_host_std::HOST.nfc(n, &mut buf).to_owned()
        })
        .collect();
    let positional: Vec<Arg<'_>> = slot_names
        .iter()
        .map(|name| {
            nfc_names
                .iter()
                .position(|n| n == name)
                .map_or(Arg::Unset, |i| named[i].1)
        })
        .collect();
    let mut text2 = String::new();
    let mut errors2 = Vec::new();
    f.write(ID, &positional, &mut text2, &mut errors2);
    if (&text2, &errors2) != (&text, &errors) {
        return Err(format!(
            "positional and named arguments disagree: {text2:?} {errors2:?} vs {text:?} {errors:?}"
        ));
    }

    let mut parts = JsonParts::default();
    let mut parts_errors = Vec::new();
    f.parts_named(ID, &named, &mut parts, &mut parts_errors);
    let mut concatenated = String::new();
    for p in &parts.text {
        concatenated.push_str(p);
    }
    if concatenated != text {
        return Err(format!(
            "the parts do not concatenate to the string: {concatenated:?} vs {text:?}"
        ));
    }
    Ok(Record {
        text,
        errors: names(&errors),
        parts: format!("[{}]", parts.json.join(",")),
        parts_errors: names(&parts_errors),
    })
}

/// Parts as canonical JSON objects (and their text, to check the
/// concatenation).
#[derive(Default)]
struct JsonParts {
    json: Vec<String>,
    text: Vec<String>,
}

struct SubParts<'s> {
    json: &'s mut Vec<String>,
}

impl SubPartSink for SubParts<'_> {
    fn sub_part(&mut self, kind: &str, text: &str) {
        let mut o = String::from("{\"type\":");
        json_string(&mut o, kind);
        o.push_str(",\"value\":");
        json_string(&mut o, text);
        o.push('}');
        self.json.push(o);
    }
}

fn value_text(v: &Value<'_>) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Number(n) => {
            let mut s = String::new();
            n.write_plain(&mut s);
            s
        }
        Value::Float(x) => x.to_string(),
        other => other.as_str().unwrap_or("").to_owned(),
    }
}

impl PartSink for JsonParts {
    fn part(&mut self, part: Part<'_>) {
        let mut o = String::from("{\"type\":");
        let mut text = String::new();
        match part {
            Part::Text(t) => {
                json_string(&mut o, "text");
                o.push_str(",\"value\":");
                json_string(&mut o, t);
                text.push_str(t);
            }
            Part::BidiIsolation(i) => {
                json_string(&mut o, "bidiIsolation");
                o.push_str(",\"value\":");
                json_string(&mut o, i.as_str());
                text.push_str(i.as_str());
            }
            Part::Expression(e) => {
                json_string(&mut o, e.kind());
                o.push_str(",\"locale\":");
                json_string(&mut o, e.locale());
                o.push_str(",\"dir\":");
                json_string(
                    &mut o,
                    match e.dir() {
                        Dir::Ltr => "ltr",
                        Dir::Rtl => "rtl",
                        Dir::Auto => "auto",
                    },
                );
                if let Some(id) = e.id() {
                    o.push_str(",\"id\":");
                    json_string(&mut o, id);
                }
                e.write(&mut text);
                o.push_str(",\"value\":");
                json_string(&mut o, &text);
                let mut subs = Vec::new();
                e.sub_parts(&mut SubParts { json: &mut subs });
                if !subs.is_empty() {
                    let _ = write!(o, ",\"parts\":[{}]", subs.join(","));
                }
            }
            Part::Markup(m) => {
                json_string(&mut o, "markup");
                o.push_str(",\"kind\":");
                json_string(
                    &mut o,
                    match m.kind() {
                        MarkupKind::Open => "open",
                        MarkupKind::Standalone => "standalone",
                        MarkupKind::Close => "close",
                    },
                );
                o.push_str(",\"name\":");
                json_string(&mut o, m.name());
                if let Some(id) = m.id() {
                    o.push_str(",\"id\":");
                    json_string(&mut o, id);
                }
                let options: Vec<String> = m
                    .options()
                    .map(|(k, v)| {
                        let mut s = String::new();
                        json_string(&mut s, k);
                        s.push(':');
                        json_string(&mut s, &value_text(v));
                        s
                    })
                    .collect();
                if !options.is_empty() {
                    let _ = write!(o, ",\"options\":{{{}}}", options.join(","));
                }
            }
            Part::Fallback(src) => {
                json_string(&mut o, "fallback");
                let mut s = String::new();
                src.write(&mut s);
                o.push_str(",\"source\":");
                json_string(&mut o, &s);
                text.push('{');
                text.push_str(&s);
                text.push('}');
            }
        }
        o.push('}');
        self.json.push(o);
        self.text.push(text);
    }
}

/// Appends `s` as a JSON string: `"`, `\` and control characters escaped,
/// everything else as itself.
pub fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ─────────────────────────────────────────────────────────── the bundle ──

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&u32::try_from(b.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(b);
}

/// Serializes cases for the wasm side.
pub fn encode_cases(cases: &[Case]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&u32::try_from(cases.len()).unwrap_or(u32::MAX).to_le_bytes());
    for c in cases {
        put_bytes(&mut out, c.id.as_bytes());
        put_bytes(&mut out, &c.catalog);
        out.extend_from_slice(&c.manifest_hash.to_le_bytes());
        // Bit 0: no bidi isolation; bit 1: the default configuration.
        out.push(
            u8::from(c.bidi == BidiStrategy::None) | u8::from(c.config == Config::Default) << 1,
        );
        out.extend_from_slice(
            &u32::try_from(c.args.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for (name, arg) in &c.args {
            put_bytes(&mut out, name.as_bytes());
            match arg {
                ArgSpec::Str(s) => {
                    out.push(0);
                    put_bytes(&mut out, s.as_bytes());
                }
                ArgSpec::Int(n) => {
                    out.push(1);
                    out.extend_from_slice(&n.to_le_bytes());
                }
                ArgSpec::Float(x) => {
                    out.push(2);
                    out.extend_from_slice(&x.to_bits().to_le_bytes());
                }
                ArgSpec::Other => out.push(3),
                ArgSpec::Decimal(d) => {
                    out.push(4);
                    put_bytes(&mut out, d.as_bytes());
                }
                ArgSpec::DateTime(d) => {
                    out.push(5);
                    put_date_time(&mut out, d);
                }
            }
        }
    }
    out
}

/// A date/time's fields: year, month, day, hour, minute, second,
/// millisecond, then the offset (a flag and seconds). The zone is not
/// carried.
fn put_date_time(out: &mut Vec<u8>, d: &DateTime<'_>) {
    out.extend_from_slice(&d.date.year().to_le_bytes());
    out.extend_from_slice(&[
        d.date.month(),
        d.date.day(),
        d.time.hour(),
        d.time.minute(),
        d.time.second(),
    ]);
    out.extend_from_slice(&d.time.millisecond().to_le_bytes());
    out.push(u8::from(d.offset.is_some()));
    out.extend_from_slice(&d.offset.unwrap_or(0).to_le_bytes());
}

struct Reader<'b> {
    b: &'b [u8],
}

impl<'b> Reader<'b> {
    fn take(&mut self, n: usize) -> Result<&'b [u8], String> {
        if self.b.len() < n {
            return Err("truncated bundle".into());
        }
        let (a, rest) = self.b.split_at(n);
        self.b = rest;
        Ok(a)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.u32()?.to_le_bytes()))
    }

    fn date_time(&mut self) -> Result<DateTime<'static>, String> {
        let bad = || String::from("bad date/time in bundle");
        let year = self.i32()?;
        let f = self.take(5)?;
        let ms = self.take(2)?;
        let has_offset = self.u8()? == 1;
        let offset = self.i32()?;
        let date = Date::new(year, f[0], f[1]).ok_or_else(bad)?;
        let time =
            Time::new(f[2], f[3], f[4], u16::from_le_bytes([ms[0], ms[1]])).ok_or_else(bad)?;
        let d = DateTime::floating(date, time);
        if has_offset {
            d.with_offset(offset).ok_or_else(bad)
        } else {
            Ok(d)
        }
    }

    fn bytes(&mut self) -> Result<&'b [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    fn string(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|e| e.to_string())
    }
}

/// Reads cases written by [`encode_cases`].
pub fn decode_cases(b: &[u8]) -> Result<Vec<Case>, String> {
    let mut r = Reader { b };
    let n = r.u32()?;
    let mut cases = Vec::new();
    for _ in 0..n {
        let id = r.string()?;
        let catalog = r.bytes()?.to_vec();
        let manifest_hash = r.u64()?;
        let flags = r.u8()?;
        let bidi = if flags & 1 == 1 {
            BidiStrategy::None
        } else {
            BidiStrategy::Default
        };
        let config = if flags & 2 == 2 {
            Config::Default
        } else {
            Config::All
        };
        let nargs = r.u32()?;
        let mut args = Vec::new();
        for _ in 0..nargs {
            let name = r.string()?;
            let arg = match r.u8()? {
                0 => ArgSpec::Str(r.string()?),
                1 => ArgSpec::Int(i64::from_le_bytes(r.u64()?.to_le_bytes())),
                2 => ArgSpec::Float(f64::from_bits(r.u64()?)),
                4 => ArgSpec::Decimal(r.string()?),
                5 => ArgSpec::DateTime(r.date_time()?),
                _ => ArgSpec::Other,
            };
            args.push((name, arg));
        }
        cases.push(Case {
            id,
            catalog,
            manifest_hash,
            bidi,
            args,
            config,
        });
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_round_trip() {
        let far = DateTime::floating(
            mf2_runtime::Date::new(-44, 3, 15).unwrap(),
            mf2_runtime::Time::new(9, 5, 0, 7).unwrap(),
        )
        .with_offset(-(5 * 3600 + 30 * 60))
        .unwrap();
        let cases = vec![Case {
            id: "t#0".into(),
            catalog: vec![1, 2, 3],
            manifest_hash: 42,
            bidi: BidiStrategy::None,
            args: vec![
                ("s".into(), ArgSpec::Str("x".into())),
                ("i".into(), ArgSpec::Int(-7)),
                ("f".into(), ArgSpec::Float(1.5)),
                ("d".into(), ArgSpec::Decimal("1.50".into())),
                (
                    "a".into(),
                    ArgSpec::date_time("2006-01-02T15:04:06.25+01:00"),
                ),
                ("b".into(), ArgSpec::date_time("2006-01-02")),
                ("c".into(), ArgSpec::DateTime(far)),
                ("o".into(), ArgSpec::Other),
            ],
            config: Config::Default,
        }];
        assert!(matches!(cases[0].args[4].1, ArgSpec::DateTime(_)));
        assert_eq!(ArgSpec::date_time("2006-02-30"), ArgSpec::Other);
        assert_eq!(decode_cases(&encode_cases(&cases)).unwrap(), cases);
    }
}
