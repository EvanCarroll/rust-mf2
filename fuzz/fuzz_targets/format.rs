//! Fuzz target `format`: `mf2-runtime` formatting every message of arbitrary
//! and of writer-made catalogs with arbitrary arguments (A10,
//! `plans/10-phase-3-work-order.md`).
//!
//! The input is `[flags] [n] [n argument bytes] [payload]`:
//!
//! * **flags** — bit 0: the bidi strategy `None` (else `Default`); bit 1:
//!   the default configuration's registry (`fn-number` and `datetime`
//!   off: layer L4d) instead of the all-features one;
//! * **arguments** — a sequence of `[tag] …`: `tag % 8` = 0 a string
//!   (`[len]` bytes, lossy UTF-8), 1 an `i64` (8 bytes LE), 2 an `f64`
//!   (8 bytes LE bits), 3 a decimal as text (`[len]` bytes), 4 an
//!   application value with no conversions, 5 a date/time from literal text
//!   (`[len]` bytes; unset when it is not one), 6 a date/time instant (an
//!   `i64` of epoch milliseconds, 8 bytes LE; unset past the range), 7
//!   unset; a truncated argument takes what is there;
//! * **payload** — starting with the magic `MF2B`: a catalog, loaded with
//!   the manifest hash its own header carries (as in `catalog`), so a damaged
//!   catalog gets past the skew check to the formatter. Otherwise MF2 source
//!   up to the first NUL and a locale byte: a source that parses — valid or
//!   not (the writer accepts models with data-model errors) — is written as a
//!   one-message catalog for that locale (direction, plural rules and number
//!   data from `mf2-locale-data`, as `mf2::compile_str` writes them, and —
//!   when the message formats a date or can receive one — the locale's
//!   `icu.blob` with every shape's data, built once per locale, off the
//!   clock), which must load.
//!
//! Every message of a catalog that loads is formatted with the arguments as
//! positional (slot) arguments — to a string, again to a string, and to
//! parts — and as named arguments, under the message's own slot names.
//! The functions are the L4 registry's — all features (`:string`, the
//! localized numeric functions, `:percent`, `:currency`, `:unit`, the
//! date/time functions over ICU4X from the catalog's `icu.blob`
//! (`icu`) — in catalog mode over the neutral backend, since ICU4X
//! does not promise to survive a damaged blob (`CATALOG_REGISTRY`) — the
//! unannotated hooks, the suite's `:test:*`) or the default configuration's
//! (flags bit 1) — the host `mf2-host-std`'s, with `jiff`'s zone data.
//! Checked:
//!
//! * no panic, no out-of-bounds access (the address sanitizer);
//! * the output is deterministic (the second run is the first);
//! * the parts concatenate to the string;
//! * named arguments format as positional ones, when the message's slot
//!   names are distinct and in NFC (a damaged catalog may break either);
//! * an id past the last message formats as a Missing Message;
//! * **linear time**: the run — a catalog's load and every format, not
//!   source mode's compile, which is build side (the `parse` and `catalog`
//!   targets hold it) — takes at most 50 ms + 50 µs per input byte + 100 ns
//!   per byte of text written. Text is charged because one
//!   catalog string may be referenced from many places, so a message's
//!   output can be quadratic in the catalog's size; for the same reason a
//!   string sink stops resolving catalog text past 16 MiB (it still counts
//!   what the runtime hands it), and a message whose string output reached
//!   that cap is not formatted again (parts, named).

#![no_main]

use std::sync::OnceLock;
use std::time::Duration;

#[path = "../common/budget.rs"]
mod budget;

use libfuzzer_sys::fuzz_target;
use mf2_catalog::format::locale_key::ICU_BLOB;
use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Catalog, Entry, MsgId, StrRef};
use mf2_fn_datetime::{DateTimeFunction, Neutral};
use mf2_locale_data::icu_blob::{DateNeeds, IcuBlobSpec, icu_blob};
use mf2_locale_data::number::{NumberNeeds, number_locale_entries};
use mf2_locale_data::{PluralKind, Selection, direction, plural_locale_entries};
use mf2_runtime::{
    Arg, BidiStrategy, CustomValue, DateTime, FormatContext, FormatError, Formatter, Function,
    Host, Part, PartSink, Registry, Sink, SubPartSink,
};

/// A string sink resolves catalog text up to this much output.
const TEXT_CAP: usize = 16 << 20;
/// Time charged per byte of output.
const NANOS_PER_TEXT_BYTE: u64 = 100;
/// Source mode's locales, by `locale byte % len`: those of Phase 3, the
/// locale panel and two non-Latin numbering systems (Phase 4, A9). Kept in
/// step with `xtask/src/fuzz_seed.rs`.
const LOCALES: [&str; 17] = [
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
    "sr-Latn",
];

fuzz_target!(|data: &[u8]| {
    let [flags, n, rest @ ..] = data else {
        return;
    };
    let (arg_bytes, payload) = rest.split_at(usize::from(*n).min(rest.len()));
    // Source mode is the build side — parse, write, locale data (cached per
    // process, a locale's blob built on first use) — off the clock: the
    // `parse` and `catalog` targets hold the parser and the writer to
    // linear time. A catalog's load is the client's, on the clock.
    let compiled = if payload.starts_with(b"MF2B") {
        None
    } else {
        match compile(payload) {
            Some(c) => Some(c),
            None => return,
        }
    };
    let cpu = budget::Cpu::start();
    let mut work = 0usize;
    run(*flags, arg_bytes, payload, compiled, &mut work);
    let budget = budget::budget(
        data.len(),
        Duration::from_nanos(
            u64::try_from(work)
                .unwrap_or(u64::MAX)
                .saturating_mul(NANOS_PER_TEXT_BYTE),
        ),
    );
    let used = cpu.elapsed();
    assert!(
        used <= budget,
        "{used:?} of CPU for {} bytes and {work} bytes of output (budget {budget:?})",
        data.len()
    );
});

fn run(flags: u8, arg_bytes: &[u8], payload: &[u8], compiled: Option<Catalog>, work: &mut usize) {
    let values = values(arg_bytes);
    let bidi = if flags & 1 != 0 {
        BidiStrategy::None
    } else {
        BidiStrategy::Default
    };
    let registry = if flags & 2 != 0 {
        &mf2_l4_runner::DEFAULT_REGISTRY
    } else if compiled.is_none() {
        // A damaged catalog's `icu.blob` is not handed to ICU4X (see
        // `CATALOG_REGISTRY`).
        &CATALOG_REGISTRY
    } else {
        &mf2_l4_runner::REGISTRY
    };
    let catalog = match compiled {
        Some(c) => c,
        None => {
            let hash = payload
                .get(8..16)
                .and_then(|h| h.try_into().ok())
                .map_or(0, u64::from_le_bytes);
            match Catalog::new(payload.to_vec(), hash) {
                Ok(c) => c,
                Err(_) => return,
            }
        }
    };
    format_all(&catalog, registry, &values, bidi, work);
}

// ── registries ───────────────────────────────────────────────────────────────

static N_DATE: DateTimeFunction<Neutral> = DateTimeFunction::date(Neutral);
static N_DATETIME: DateTimeFunction<Neutral> = DateTimeFunction::datetime(Neutral);
static N_TIME: DateTimeFunction<Neutral> = DateTimeFunction::time(Neutral);
static N_DATES: DateTimeFunction<Neutral> = DateTimeFunction::unannotated(Neutral);

/// The L4 registry with the date functions over the neutral backend, for
/// catalog mode: ICU4X does not promise to survive a damaged `icu.blob` —
/// a smoke run found `DecimalFormatter::try_new` panicking on a damaged
/// numbering-system name (`DataMarkerAttributes::from_str_or_panic`), in
/// release builds too — so only the pristine blobs of source mode reach
/// it. The date semantics, and every other function, still meet the
/// damaged catalog.
static CATALOG_FUNCTIONS: [(&str, &dyn Function); 13] = [
    ("currency", &mf2_fn_number::CURRENCY),
    ("date", &N_DATE),
    ("datetime", &N_DATETIME),
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("offset", &mf2_fn_number::OFFSET),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &mf2_runtime::functions::STRING),
    ("test:format", &mf2_l4_runner::test_functions::FORMAT),
    ("test:function", &mf2_l4_runner::test_functions::FUNCTION),
    ("test:select", &mf2_l4_runner::test_functions::SELECT),
    ("time", &N_TIME),
    ("unit", &mf2_fn_number::UNIT),
];
static CATALOG_REGISTRY: Registry = Registry::new(&CATALOG_FUNCTIONS)
    .with_numbers(&mf2_fn_number::NUMBERS)
    .with_dates(&N_DATES);

// ── arguments ────────────────────────────────────────────────────────────────

/// An argument, owned.
enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Decimal(String),
    Opaque,
    DateTime(DateTime<'static>),
    Unset,
}

struct Opaque;

impl CustomValue for Opaque {}

static OPAQUE: Opaque = Opaque;

impl Value {
    fn arg(&self) -> Arg<'_> {
        match self {
            Value::Str(s) => Arg::Str(s),
            Value::Int(n) => Arg::Int(*n),
            Value::Float(x) => Arg::Float(*x),
            Value::Decimal(d) => Arg::Decimal(d),
            Value::Opaque => Arg::Custom(&OPAQUE),
            Value::DateTime(d) => Arg::DateTime(d),
            Value::Unset => Arg::Unset,
        }
    }
}

fn values(mut b: &[u8]) -> Vec<Value> {
    fn take<'a>(b: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, rest) = b.split_at(n.min(b.len()));
        *b = rest;
        a
    }
    fn eight(b: &mut &[u8]) -> [u8; 8] {
        let mut a = [0u8; 8];
        let t = take(b, 8);
        a[..t.len()].copy_from_slice(t);
        a
    }
    fn text(b: &mut &[u8]) -> String {
        let n = take(b, 1).first().copied().unwrap_or(0);
        String::from_utf8_lossy(take(b, usize::from(n))).into_owned()
    }
    let mut out = Vec::new();
    while let Some((&tag, rest)) = b.split_first() {
        b = rest;
        out.push(match tag % 8 {
            0 => Value::Str(text(&mut b)),
            1 => Value::Int(i64::from_le_bytes(eight(&mut b))),
            2 => Value::Float(f64::from_bits(u64::from_le_bytes(eight(&mut b)))),
            3 => Value::Decimal(text(&mut b)),
            4 => Value::Opaque,
            5 => {
                mf2_fn_datetime::parse_literal(&text(&mut b)).map_or(Value::Unset, Value::DateTime)
            }
            6 => DateTime::from_epoch_ms(i64::from_le_bytes(eight(&mut b)))
                .map_or(Value::Unset, Value::DateTime),
            _ => Value::Unset,
        });
    }
    out
}

// ── source mode ──────────────────────────────────────────────────────────────

/// The `icu.blob` of `LOCALES[locale]` with every shape's data, every
/// variant, and the calendars the steering names (`buddhist`, `hebrew`,
/// `japanese`) — built on first use (off the clock, as all of `compile`).
fn blob(locale: usize) -> &'static [u8] {
    static BLOBS: [OnceLock<Vec<u8>>; LOCALES.len()] = [const { OnceLock::new() }; LOCALES.len()];
    BLOBS[locale].get_or_init(|| {
        let mut all = DateNeeds::all();
        // `mf2_conformance::l4gen::STEERED_CALENDARS`'s.
        all.calendars =
            Selection::Listed(["buddhist", "hebrew", "japanese"].map(String::from).into());
        icu_blob(LOCALES[locale], &IcuBlobSpec::every_variant(all))
            .expect("icu.blob of a known locale")
    })
}

fn compile(data: &[u8]) -> Option<Catalog> {
    let (src, tail) = match data.iter().position(|&b| b == 0) {
        Some(i) => (&data[..i], &data[i + 1..]),
        None => (data, &[][..]),
    };
    let src = String::from_utf8_lossy(src);
    let model = mf2_syntax::parse_model(&src).message?;
    let locale = LOCALES[usize::from(tail.first().copied().unwrap_or(0)) % LOCALES.len()];
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = Options::new(locale, direction(locale).expect("a known locale"));
    options.cldr_version = Some(mf2_locale_data::CLDR_VERSION);
    options.locale_entries =
        plural_locale_entries(locale, &[PluralKind::Cardinal, PluralKind::Ordinal])
            .expect("plural rules of a known locale");
    let mut needs = NumberNeeds::from_functions(analysis.functions.iter().map(|f| &*f.nfc));
    needs.symbols = true;
    options
        .locale_entries
        .extend(number_locale_entries(locale, &needs).expect("number data of a known locale"));
    // The `icu` data when the message formats a date or can
    // receive one (02 §4.4): the locale's blob for every shape, the
    // steering's calendars too (built once per locale: `blob`), which the
    // sliced blob `mf2::compile_str` writes is a subset of (`generated_l4`
    // checks the slicing).
    let mut dates = DateNeeds::default();
    dates.add_message(&model);
    if !dates.is_empty() {
        let locale = usize::from(tail.first().copied().unwrap_or(0)) % LOCALES.len();
        options
            .locale_entries
            .push((ICU_BLOB, blob(locale).to_vec()));
    }
    if tail.get(1).is_some_and(|b| b & 1 != 0) {
        options = options.stripped();
    }
    let (bytes, manifest) = writer::single(&model, &slots, &options)
        .unwrap_or_else(|e| panic!("a parsed model is not writable: {e:?}"));
    Some(
        Catalog::new(bytes, manifest.hash())
            .unwrap_or_else(|e| panic!("writer output does not load: {e:?}")),
    )
}

// ── formatting ───────────────────────────────────────────────────────────────

/// A string sink that stops resolving catalog text past [`TEXT_CAP`], and
/// counts every byte it is handed.
#[derive(Default, PartialEq)]
struct Capped {
    text: String,
    total: usize,
    capped: bool,
}

impl Sink for Capped {
    fn push_str(&mut self, s: &str) {
        self.total += s.len();
        if self.text.len() + s.len() <= TEXT_CAP {
            self.text.push_str(s);
        } else {
            self.capped = true;
        }
    }

    fn push_catalog_text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        if self.total > TEXT_CAP {
            self.capped = true;
            return true;
        }
        match catalog.text(r) {
            Some(s) => {
                self.push_str(s);
                true
            }
            None => false,
        }
    }
}

/// Parts, reduced to the text they stand for (every accessor called).
#[derive(Default)]
struct Parts {
    text: String,
}

struct Subs<'s>(&'s mut usize);

impl SubPartSink for Subs<'_> {
    fn sub_part(&mut self, kind: &str, text: &str) {
        *self.0 += kind.len() + text.len();
    }
}

impl PartSink for Parts {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(t) => self.text.push_str(t),
            Part::BidiIsolation(i) => self.text.push_str(i.as_str()),
            Part::Expression(e) => {
                let _ = (e.kind(), e.locale(), e.dir(), e.id());
                e.write(&mut self.text);
                let mut n = 0;
                e.sub_parts(&mut Subs(&mut n));
            }
            Part::Markup(m) => {
                let _ = (m.kind(), m.name(), m.id());
                for (k, v) in m.options() {
                    let _ = (k, v.as_str());
                }
            }
            Part::Fallback(src) => {
                self.text.push('{');
                src.write(&mut self.text);
                self.text.push('}');
            }
            _ => {}
        }
    }
}

fn context(bidi: BidiStrategy) -> FormatContext {
    let mut cx = FormatContext::new(&mf2_host_std::HOST);
    cx.bidi = bidi;
    cx
}

fn format_all(
    catalog: &Catalog,
    registry: &Registry,
    values: &[Value],
    bidi: BidiStrategy,
    work: &mut usize,
) {
    let cx = context(bidi);
    let f = Formatter::new(catalog, registry, &cx);
    let args: Vec<Arg<'_>> = values.iter().map(Value::arg).collect();
    let count = catalog.message_count();
    for i in 0..count {
        let Some(id) = MsgId::new(catalog.chunk(), i) else {
            break;
        };
        if *work > 4 * TEXT_CAP {
            break;
        }
        format_one(catalog, &f, id, &args, work);
    }
    // One past the end: a Missing Message, never a panic.
    if let Some(id) = MsgId::new(catalog.chunk(), count) {
        let mut out = Capped::default();
        let mut errs: Vec<FormatError> = Vec::new();
        f.write(id, &args, &mut out, &mut errs);
        *work += out.total;
        assert!(
            errs.iter()
                .any(|e| matches!(e, FormatError::MissingMessage)),
            "an absent id formatted without a Missing Message: {errs:?}"
        );
    }
}

fn format_one(catalog: &Catalog, f: &Formatter<'_>, id: MsgId, args: &[Arg<'_>], work: &mut usize) {
    let mut first = Capped::default();
    let mut first_errs: Vec<FormatError> = Vec::new();
    f.write(id, args, &mut first, &mut first_errs);
    *work += first.total;
    if first.capped {
        return;
    }
    let mut again = Capped::default();
    let mut again_errs: Vec<FormatError> = Vec::new();
    f.write(id, args, &mut again, &mut again_errs);
    *work += again.total;
    assert!(
        again.text == first.text && again_errs == first_errs,
        "formatting is not deterministic"
    );

    let mut parts = Parts::default();
    let mut parts_errs: Vec<FormatError> = Vec::new();
    f.parts(id, args, &mut parts, &mut parts_errs);
    *work += parts.text.len();
    assert_eq!(
        parts.text, first.text,
        "the parts do not concatenate to the string"
    );
    assert_eq!(parts_errs, first_errs, "parts report other errors");

    // Named: the message's slot names, when they are distinct and NFC.
    let names: Vec<&str> = match catalog.get(id) {
        Entry::Pattern(m) | Entry::Select(m) => {
            let n = m.names();
            (0..n.external_count())
                .map_while(|s| n.external(s).and_then(|r| catalog.text(r)))
                .collect()
        }
        _ => Vec::new(),
    };
    let nfc = names.iter().all(|n| {
        use unicode_normalization::UnicodeNormalization;
        n.nfc().eq(n.chars())
    });
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    let comparable = nfc && sorted.len() == names.len() && names.len() == slot_count(catalog, id);
    let named: Vec<(&str, Arg<'_>)> = names
        .iter()
        .zip(args.iter().copied().chain(std::iter::repeat(Arg::Unset)))
        .map(|(n, a)| (*n, a))
        .collect();
    let mut by_name = Capped::default();
    let mut by_name_errs: Vec<FormatError> = Vec::new();
    f.write_named(id, &named, &mut by_name, &mut by_name_errs);
    *work += by_name.total;
    if comparable && !by_name.capped {
        assert!(
            by_name.text == first.text && by_name_errs == first_errs,
            "named and positional arguments disagree: {:?} {by_name_errs:?} vs {:?} {first_errs:?}",
            by_name.text,
            first.text
        );
    }
}

fn slot_count(catalog: &Catalog, id: MsgId) -> usize {
    match catalog.get(id) {
        Entry::Pattern(m) | Entry::Select(m) => {
            usize::try_from(m.names().external_count()).unwrap_or(usize::MAX)
        }
        _ => 0,
    }
}
