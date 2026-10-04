//! The A0 wasm harness: `mf2-runtime` in the browser with one registry per
//! variant (cargo feature) and **the same exports in every variant**, so a
//! size delta against `base` is the numeric functions (and their `Intl`
//! glue) alone, and the page can drive `rust` and `intl` side by side.
//!
//! | Export (`Probe` method) | What |
//! |---|---|
//! | `load(bytes, hash)` | a catalog compiled natively (`intl-probe-native`); its index |
//! | `clear_args`, `arg_*` | positional arguments (slot order) for the next call |
//! | `format`, `parts`, `errors` | one message to string / to parts (JSON), and the errors of that call (suite names, sorted) |
//! | `format_all` | every message of a catalog, no arguments: `text U+001F errors U+001E …` |
//! | `bench_int`, `bench_float` | `iters` formats of one message in a loop inside wasm (no JS ↔ wasm crossing of the harness per format), `NoErrors`, a reused `String` |
//! | `bench_none` | the same for a message with no argument (the locale-symbol cases) |
//!
//! The harness is std (as a Leptos client is): the allocator, the panic
//! runtime and wasm-bindgen's own glue are in `base` and cancel in the
//! deltas.

use std::fmt::Write as _;

use mf2_catalog::{Catalog, Entry, MsgId};
use mf2_runtime::{
    Arg, BidiStrategy, CustomValue, Dir, FormatContext, FormatError, Formatter, Function,
    MarkupKind, NoErrors, Part, PartSink, Registry, SubPartSink, Value, functions,
};
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(not(any(
    feature = "base",
    feature = "rust",
    feature = "intl",
    feature = "intl-loc",
    feature = "intl-cu",
    feature = "intl-codes",
    feature = "rust-loc",
    feature = "rust-cu",
    feature = "rt-intl",
    feature = "rt-intl-loc",
    feature = "rt-intl-cu",
    feature = "rt-names-cu"
)))]
compile_error!("build intl-probe-wasm with exactly one variant feature (scripts/build.sh)");

/// The variant's registry: closed world, as `mf2-build` would generate it.
#[cfg(feature = "base")]
static FUNCTIONS: [(&str, &dyn Function); 1] = [("string", &functions::STRING)];
#[cfg(feature = "base")]
const VARIANT: &str = "base";

#[cfg(feature = "rust")]
static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("offset", &functions::OFFSET),
    ("string", &functions::STRING),
];
#[cfg(feature = "rust")]
const VARIANT: &str = "rust";

#[cfg(feature = "intl")]
static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &intl_probe_fn::INTEGER),
    ("number", &intl_probe_fn::NUMBER),
    ("offset", &intl_probe_fn::OFFSET),
    ("string", &functions::STRING),
];
#[cfg(feature = "intl")]
const VARIANT: &str = "intl";

#[cfg(feature = "intl-loc")]
static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("integer", &intl_probe_fn::INTEGER_LOC),
    ("number", &intl_probe_fn::NUMBER_LOC),
    ("offset", &intl_probe_fn::OFFSET_LOC),
    ("percent", &intl_probe_fn::PERCENT),
    ("string", &functions::STRING),
];
#[cfg(feature = "intl-loc")]
const VARIANT: &str = "intl-loc";

#[cfg(feature = "intl-cu")]
static FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("currency", &intl_probe_fn::CURRENCY),
    ("integer", &intl_probe_fn::INTEGER_LOC),
    ("number", &intl_probe_fn::NUMBER_LOC),
    ("offset", &intl_probe_fn::OFFSET_LOC),
    ("percent", &intl_probe_fn::PERCENT),
    ("string", &functions::STRING),
    ("unit", &intl_probe_fn::UNIT),
];
#[cfg(feature = "intl-cu")]
const VARIANT: &str = "intl-cu";

#[cfg(feature = "intl-codes")]
static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &intl_probe_fn::INTEGER),
    ("number", &intl_probe_fn::NUMBER),
    ("offset", &intl_probe_fn::OFFSET),
    ("string", &functions::STRING),
];
#[cfg(feature = "intl-codes")]
const VARIANT: &str = "intl-codes";

#[cfg(feature = "rust-loc")]
static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &functions::STRING),
];
#[cfg(feature = "rust-loc")]
const VARIANT: &str = "rust-loc";

#[cfg(any(feature = "rust-cu", feature = "rt-intl-cu", feature = "rt-names-cu"))]
static FUNCTIONS: [(&str, &dyn Function); 7] = [
    ("currency", &mf2_fn_number::CURRENCY),
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &functions::STRING),
    ("unit", &mf2_fn_number::UNIT),
];
#[cfg(feature = "rust-cu")]
const VARIANT: &str = "rust-cu";
#[cfg(feature = "rt-intl-cu")]
const VARIANT: &str = "rt-intl-cu";
#[cfg(feature = "rt-names-cu")]
const VARIANT: &str = "rt-names-cu";

#[cfg(feature = "rt-intl")]
static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("offset", &functions::OFFSET),
    ("string", &functions::STRING),
];
#[cfg(feature = "rt-intl")]
const VARIANT: &str = "rt-intl";

#[cfg(feature = "rt-intl-loc")]
static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &functions::STRING),
];
#[cfg(feature = "rt-intl-loc")]
const VARIANT: &str = "rt-intl-loc";

static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// The host: the browser's, and for the option as built (`rt-intl*`) and
/// the number split (`rt-names-cu`) the same with its number formatter
/// (`Intl`).
#[cfg(not(any(
    feature = "rt-intl",
    feature = "rt-intl-loc",
    feature = "rt-intl-cu",
    feature = "rt-names-cu"
)))]
const HOST: &dyn mf2_runtime::Host = &mf2_host_web::HOST;
#[cfg(any(
    feature = "rt-intl",
    feature = "rt-intl-loc",
    feature = "rt-intl-cu",
    feature = "rt-names-cu"
))]
const HOST: &dyn mf2_runtime::Host = &mf2_host_web::NUMBERS_HOST;

static CX_DEFAULT: FormatContext = FormatContext::new(HOST);
static CX_NONE: FormatContext = {
    let mut cx = FormatContext::new(HOST);
    cx.bidi = BidiStrategy::None;
    cx
};

fn context(bidi_none: bool) -> &'static FormatContext {
    if bidi_none { &CX_NONE } else { &CX_DEFAULT }
}

/// The variant this module was built as.
#[wasm_bindgen]
pub fn variant() -> String {
    VARIANT.to_owned()
}

/// An argument, owned until the call.
enum OwnedArg {
    Str(String),
    Int(i64),
    Float(f64),
    Decimal(String),
    Other,
    Unset,
}

/// An application value with no conversions (the suite's `true`).
struct Opaque;

impl CustomValue for Opaque {}

static OPAQUE: Opaque = Opaque;

impl OwnedArg {
    fn arg(&self) -> Arg<'_> {
        match self {
            OwnedArg::Str(s) => Arg::Str(s),
            OwnedArg::Int(n) => Arg::Int(*n),
            OwnedArg::Float(x) => Arg::Float(*x),
            OwnedArg::Decimal(d) => Arg::Decimal(d),
            OwnedArg::Other => Arg::Custom(&OPAQUE),
            OwnedArg::Unset => Arg::Unset,
        }
    }
}

/// The harness state: loaded catalogs, the next call's arguments, the last
/// call's errors.
#[wasm_bindgen]
pub struct Probe {
    catalogs: Vec<Catalog>,
    args: Vec<OwnedArg>,
    errors: Vec<FormatError>,
}

impl Default for Probe {
    fn default() -> Probe {
        Probe::new()
    }
}

fn names(errors: &[FormatError]) -> String {
    let mut v: Vec<&str> = errors
        .iter()
        .map(|e| e.kind().map_or("other", mf2_model::ErrorKind::suite_name))
        .collect();
    v.sort_unstable();
    v.join(",")
}

#[wasm_bindgen]
impl Probe {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Probe {
        Probe {
            catalogs: Vec::new(),
            args: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Loads a catalog; its index, or -1 if it does not load.
    pub fn load(&mut self, bytes: Vec<u8>, hash: u64) -> i32 {
        match Catalog::new(bytes, hash) {
            Ok(c) => {
                self.catalogs.push(c);
                i32::try_from(self.catalogs.len() - 1).unwrap_or(-1)
            }
            Err(_) => -1,
        }
    }

    /// The number of messages of catalog `cat`.
    pub fn messages(&self, cat: u32) -> u32 {
        self.catalogs
            .get(cat as usize)
            .map_or(0, Catalog::message_count)
    }

    pub fn clear_args(&mut self) {
        self.args.clear();
    }

    pub fn arg_str(&mut self, s: String) {
        self.args.push(OwnedArg::Str(s));
    }

    /// An integer (a JS safe integer).
    #[allow(clippy::cast_possible_truncation)]
    pub fn arg_int(&mut self, n: f64) {
        self.args.push(OwnedArg::Int(n as i64));
    }

    pub fn arg_float(&mut self, x: f64) {
        self.args.push(OwnedArg::Float(x));
    }

    pub fn arg_decimal(&mut self, s: String) {
        self.args.push(OwnedArg::Decimal(s));
    }

    pub fn arg_other(&mut self) {
        self.args.push(OwnedArg::Other);
    }

    pub fn arg_unset(&mut self) {
        self.args.push(OwnedArg::Unset);
    }

    /// Message `id` of catalog `cat` to a string, with the arguments set.
    pub fn format(&mut self, cat: u32, id: u32, bidi_none: bool) -> String {
        self.errors.clear();
        let mut out = String::new();
        if let Some(c) = self.catalogs.get(cat as usize) {
            let f = Formatter::new(c, &REGISTRY, context(bidi_none));
            let args: Vec<Arg<'_>> = self.args.iter().map(OwnedArg::arg).collect();
            f.write(MsgId::from_raw(id), &args, &mut out, &mut self.errors);
        }
        out
    }

    /// The same to parts, as JSON in the suite's `expParts` shape.
    pub fn parts(&mut self, cat: u32, id: u32, bidi_none: bool) -> String {
        self.errors.clear();
        let mut parts = JsonParts::default();
        if let Some(c) = self.catalogs.get(cat as usize) {
            let f = Formatter::new(c, &REGISTRY, context(bidi_none));
            let args: Vec<Arg<'_>> = self.args.iter().map(OwnedArg::arg).collect();
            f.parts(MsgId::from_raw(id), &args, &mut parts, &mut self.errors);
        }
        format!("[{}]", parts.json.join(","))
    }

    /// The errors of the last `format` / `parts`: suite names, sorted.
    pub fn errors(&self) -> String {
        names(&self.errors)
    }

    /// Every message of catalog `cat` to a string, no arguments:
    /// `text U+001F errors`, records separated by U+001E.
    pub fn format_all(&mut self, cat: u32, bidi_none: bool) -> String {
        let mut all = String::new();
        let Some(c) = self.catalogs.get(cat as usize) else {
            return all;
        };
        let f = Formatter::new(c, &REGISTRY, context(bidi_none));
        let mut errors = Vec::new();
        for id in 0..c.message_count() {
            if id > 0 {
                all.push('\u{1e}');
            }
            errors.clear();
            f.write(MsgId::from_raw(id), &[], &mut all, &mut errors);
            all.push('\u{1f}');
            all.push_str(&names(&errors));
        }
        all
    }

    /// `iters` formats of message `id` with `Arg::Int(start + i % modulo)`;
    /// the total output length (keeps the work alive).
    pub fn bench_int(&mut self, cat: u32, id: u32, iters: u32, start: i32, modulo: u32) -> u32 {
        let Some(c) = self.catalogs.get(cat as usize) else {
            return 0;
        };
        let f = Formatter::new(c, &REGISTRY, &CX_DEFAULT);
        let mut out = String::with_capacity(256);
        let mut total = 0u32;
        let m = modulo.max(1);
        for i in 0..iters {
            let n = i64::from(start) + i64::from(i % m);
            out.clear();
            f.write(MsgId::from_raw(id), &[Arg::Int(n)], &mut out, &mut NoErrors);
            total = total.wrapping_add(u32::try_from(out.len()).unwrap_or(0));
        }
        total
    }

    /// `iters` formats of message `id` with no argument (the locale-symbol
    /// cases' literal operands); the total output length.
    pub fn bench_none(&mut self, cat: u32, id: u32, iters: u32) -> u32 {
        let Some(c) = self.catalogs.get(cat as usize) else {
            return 0;
        };
        let f = Formatter::new(c, &REGISTRY, &CX_DEFAULT);
        let mut out = String::with_capacity(256);
        let mut total = 0u32;
        for _ in 0..iters {
            out.clear();
            f.write(MsgId::from_raw(id), &[], &mut out, &mut NoErrors);
            total = total.wrapping_add(u32::try_from(out.len()).unwrap_or(0));
        }
        total
    }

    /// `iters` formats of message `id` with `Arg::Float(base + (i % modulo) / 4)`.
    pub fn bench_float(&mut self, cat: u32, id: u32, iters: u32, base: f64, modulo: u32) -> u32 {
        let Some(c) = self.catalogs.get(cat as usize) else {
            return 0;
        };
        let f = Formatter::new(c, &REGISTRY, &CX_DEFAULT);
        let mut out = String::with_capacity(256);
        let mut total = 0u32;
        let m = modulo.max(1);
        for i in 0..iters {
            let x = base + f64::from(i % m) * 0.25;
            out.clear();
            f.write(
                MsgId::from_raw(id),
                &[Arg::Float(x)],
                &mut out,
                &mut NoErrors,
            );
            total = total.wrapping_add(u32::try_from(out.len()).unwrap_or(0));
        }
        total
    }

    /// The slot names of message `id` (the page maps named params onto
    /// them), U+001F-separated.
    pub fn slots(&self, cat: u32, id: u32) -> String {
        let Some(c) = self.catalogs.get(cat as usize) else {
            return String::new();
        };
        match c.get(MsgId::from_raw(id)) {
            Entry::Pattern(m) | Entry::Select(m) => {
                let n = m.names();
                (0..n.external_count())
                    .map(|s| n.external(s).and_then(|r| c.text(r)).unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("\u{1f}")
            }
            _ => String::new(),
        }
    }
}

// ─────────────────────────────────────────── parts as the suite's JSON ──
// (conformance/l4-runner's `JsonParts`, trimmed.)

#[derive(Default)]
struct JsonParts {
    json: Vec<String>,
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
        match part {
            Part::Text(t) => {
                json_string(&mut o, "text");
                o.push_str(",\"value\":");
                json_string(&mut o, t);
            }
            Part::BidiIsolation(i) => {
                json_string(&mut o, "bidiIsolation");
                o.push_str(",\"value\":");
                json_string(&mut o, i.as_str());
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
                let mut text = String::new();
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
            }
            _ => json_string(&mut o, "unknown"),
        }
        o.push('}');
        self.json.push(o);
    }
}

fn json_string(out: &mut String, s: &str) {
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
