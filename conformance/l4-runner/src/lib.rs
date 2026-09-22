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
use mf2_runtime::functions::{INTEGER, NUMBER, OFFSET, STRING};
use mf2_runtime::{
    Arg, BidiStrategy, CustomValue, Dir, FormatContext, FormatError, Formatter, Function, Host,
    MarkupKind, MsgId, Part, PartSink, Registry, SubPartSink, Value,
};

/// The handlers L4 formats with: the core functions and the test functions.
pub static FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("integer", &INTEGER),
    ("number", &NUMBER),
    ("offset", &OFFSET),
    ("string", &STRING),
    ("test:format", &test_functions::FORMAT),
    ("test:function", &test_functions::FUNCTION),
    ("test:select", &test_functions::SELECT),
];

/// The registry over [`FUNCTIONS`].
pub static REGISTRY: Registry = Registry::new(&FUNCTIONS);

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
    /// A value no core function takes (a boolean, an object, …).
    Other,
}

/// An application value with no conversions (the suite's `true`).
struct Opaque;

impl CustomValue for Opaque {}

static OPAQUE: Opaque = Opaque;

impl ArgSpec {
    fn arg(&self) -> Arg<'_> {
        match self {
            ArgSpec::Str(s) => Arg::Str(s),
            ArgSpec::Int(n) => Arg::Int(*n),
            ArgSpec::Float(x) => Arg::Float(*x),
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
    let f = Formatter::new(&catalog, &REGISTRY, context(case.bidi));
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
        out.push(u8::from(c.bidi == BidiStrategy::None));
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
            }
        }
    }
    out
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
        let bidi = if r.u8()? == 1 {
            BidiStrategy::None
        } else {
            BidiStrategy::Default
        };
        let nargs = r.u32()?;
        let mut args = Vec::new();
        for _ in 0..nargs {
            let name = r.string()?;
            let arg = match r.u8()? {
                0 => ArgSpec::Str(r.string()?),
                1 => ArgSpec::Int(i64::from_le_bytes(r.u64()?.to_le_bytes())),
                2 => ArgSpec::Float(f64::from_bits(r.u64()?)),
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
        });
    }
    Ok(cases)
}
