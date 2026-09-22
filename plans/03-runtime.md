# 03 — Runtime: evaluator, functions, locale data

Part of the [master plan](00-master-plan.md). RFC 2119 keywords apply.

The vendored spec (`third_party/message-format-wg/spec/`) is normative for
behaviour. This document only fixes *structure*: what the crates are, what the
APIs look like, and how full-spec support coexists with a tiny wasm.

**There is no Rust MF2 formatter to build on.** The planning-session audit
(2026-09-20) found no published crate that formats spec MF2, and ICU4X has no
MessageFormat component (epic open, a third-party PR closed unmerged in 2026-05).
The evaluator and the function handlers are written here.

## 1. Ground rules for `mf2-runtime`

* `#![no_std]` + `alloc`. `forbid(unsafe_code)`.
* **One evaluation path**: from a `mf2-catalog` view. To format an ad-hoc source
  string (tests, CLI, server-side dynamic text) compile it to an in-memory
  one-message catalog first. There is no "interpret the AST" path to drift.
* **fmt-free and panic-free on the client path** (budget B12 in
  [06](06-size-and-perf.md)): no `format!`, no `Debug`/`Display` use, no
  panicking index/`unwrap`; structural impossibilities return the fallback
  representation and push an error.
* **Formatting never fails.** Per the spec, a message always yields output;
  errors are *collected alongside* it. APIs return `(output, errors)` or take an
  error sink.
* **Byte-identical on native and wasm32** for every deterministic function
  (conformance L4 runs on both).
* **Allocation**: the per-call lists — declarations, `.input` slots,
  `.local`s, option lists, selectors — are heap `Vec`s grown only through
  `try_reserve`, so a list that stays empty allocates nothing: a simple or
  declaration-free message formats without allocating, a select with one
  `.input` allocates 4 times (B10-P3). Measured alternatives (10 A11:
  alternating builds): an inline tier of four items gives 0 allocations but
  +811 B gz and a 1-argument pattern 35–45 ns slower; inline index lists
  only give 1 allocation and no faster select. A formatter-owned arena
  reused across calls is the remaining candidate (P6).

## 2. The API of `mf2-runtime`

Written in Phase 3 (task A1) before the code and **frozen at the exit of
Phase 3** (2026-09-21, as below); Phases 4 (functions), 5b (macros) and 6 (Leptos) are written
against it. Methods, trait methods with a default, enum variants of the
`#[non_exhaustive]` enums and fields of the `#[non_exhaustive]` structs may be
added; changing a signature after the exit needs the current work order changed
in the same commit. Every item is `no_std` + `alloc` and client-path code
(§1; 05 §8).

### 2.1 Formatting

```rust
#[derive(Clone, Copy)]
pub struct Formatter<'c> { /* catalog: &'c Catalog, registry: &'c Registry, cx: &'c FormatContext */ }

#[non_exhaustive]                       // P4 adds the time zone (§6); build it with `new`
pub struct FormatContext {
    pub bidi: BidiStrategy,             // Default (the spec's default) | None
    pub host: &'static dyn Host,        // NFC; f64 → shortest text; P4: the `Intl` date backend
}
impl FormatContext { pub const fn new(host: &'static dyn Host) -> FormatContext; } // bidi = Default

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum BidiStrategy { #[default] Default, None }

impl<'c> Formatter<'c> {
    pub const fn new(catalog: &'c Catalog, registry: &'c Registry, cx: &'c FormatContext) -> Formatter<'c>;
    pub fn catalog(&self) -> &'c Catalog;
    /// `Some` when the message is `simple` and its text is valid: no evaluator, no allocation.
    pub fn simple(&self, id: MsgId) -> Option<&'c str>;
    /// The same, unresolved — the seam for catalog text as JS strings.
    pub fn simple_ref(&self, id: MsgId) -> Option<StrRef>;
    pub fn write(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn Sink, errs: &mut dyn ErrorSink);
    pub fn parts(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn PartSink, errs: &mut dyn ErrorSink);
    /// Arguments by name (CLI, server-side dynamic text, conformance `dyn` cases):
    /// each slot takes the argument whose NFC name equals the slot's NAMES entry.
    pub fn write_named(&self, id: MsgId, args: &[(&str, Arg<'_>)], out: &mut dyn Sink, errs: &mut dyn ErrorSink);
    pub fn parts_named(&self, id: MsgId, args: &[(&str, Arg<'_>)], out: &mut dyn PartSink, errs: &mut dyn ErrorSink);
}
```

* `args` are **positional slots** assigned by the manifest ([05](05-tooling.md)
  §3): `args[slot]`, a missing slot is `Arg::Unset`, and arguments past the
  message's last slot are ignored — a reference past it (only a damaged
  catalog has one) is unresolved, as with named arguments (found by the
  `format` fuzz target, 10 A10). Names exist only in the
  catalog's NAMES section, used by `*_named` and by fallback output (`{$name}`).
* Every method takes `dyn` sinks: one copy of the walker in the wasm (B1).
  Formatting never fails (§1): output and errors always both arrive.

### 2.2 Output

```rust
pub trait Sink {
    fn push_str(&mut self, s: &str);
    /// Catalog text — the seam for catalog text as JS strings: the evaluator writes
    /// catalog text only through this. `false`: the string is invalid (F4), nothing written.
    fn push_catalog_text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        match catalog.text(r) { Some(s) => { self.push_str(s); true } None => false }
    }
}
impl Sink for String { /* growth through `try_reserve`; on failure the text is dropped */ }

pub trait ErrorSink { fn error(&mut self, e: FormatError); }
impl ErrorSink for Vec<FormatError> { /* `try_reserve` */ }
pub struct NoErrors;                    // impl ErrorSink: discards (the release client, §8)

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, thiserror::Error)]
#[repr(u8)]
#[non_exhaustive]
pub enum FormatError {
    // Resolution Errors                // Message Function Errors
    UnresolvedVariable,                 BadOperand,
    UnknownFunction,                    BadOption,
    BadSelector,                        BadVariantKey,
                                        UnsupportedOperation,
                                        MessageFunctionError,  // any other handler failure
    /// No variant matched: a catalog built from an invalid model (no fallback variant,
    /// or key counts that differ from the selector count). The message formats as `{�}`.
    MissingFallbackVariant,
    /// The id is not in this catalog (`Entry::Absent`, another chunk, out of range). Nothing is written.
    MissingMessage,
    /// The message's record is malformed (a view yielded `Err`, a string is not UTF-8):
    /// `{�}` in its place, and the message ends there.
    Malformed,
}
impl FormatError { pub const fn kind(self) -> Option<mf2_model::ErrorKind>; } // None for the last two

pub trait PartSink { fn part(&mut self, part: Part<'_>); }

pub enum Part<'p> {
    Text(&'p str),
    BidiIsolation(Isolation),           // Lri U+2066 | Rli U+2067 | Fsi U+2068 | Pdi U+2069; `as_str()`
    Expression(ExpressionPart<'p>),     // a formatted placeholder
    Markup(MarkupPart<'p>),
    Fallback(FallbackSource<'p>),       // a placeholder that resolved to a fallback value
}

impl<'p> ExpressionPart<'p> {
    pub fn kind(&self) -> &'p str;              // `Function::part_kind`: "string", "number", "test", …
    pub fn locale(&self) -> &'p str;            // the catalog's locale
    pub fn dir(&self) -> Dir;                   // after `u:dir`; `Auto` = unknown
    pub fn id(&self) -> Option<&'p str>;        // `u:id`
    pub fn value(&self) -> &'p Value<'p>;       // the resolved value
    pub fn write(&self, out: &mut dyn Sink);    // its formatted text (what string output shows)
    pub fn sub_parts(&self, out: &mut dyn SubPartSink); // e.g. a number's minusSign / integer / decimal / fraction
}
pub trait SubPartSink { fn sub_part(&mut self, kind: &str, text: &str); }

impl<'p> MarkupPart<'p> {
    pub fn kind(&self) -> MarkupKind;           // Open | Standalone | Close
    pub fn name(&self) -> &'p str;              // NFC
    pub fn id(&self) -> Option<&'p str>;        // `u:id`
    pub fn options(&self) -> MarkupOptions<'p>; // (&'p str, &'p Value<'p>), resolved; `u:` options removed
}

pub enum FallbackSource<'p> { Variable(&'p str), Literal(&'p str), Function(&'p str), Unknown }
impl FallbackSource<'_> { pub fn write(&self, out: &mut dyn Sink); } // `$x`, `|a\|b|`, `:ns:fn`, `�` — no braces
```

Errors are plain values, never strings; a later `diagnostics` feature
(server/dev only) adds message id, source span and human-readable text.

The shape is what the suite's `expParts` asserts: a `text` part, a
`bidiIsolation` part, an expression part with `type`, `locale`, `dir`, `id`,
`value` and sub-`parts` (the harness compares the keys a test names), a
`markup` part with `kind`, `name`, `id`, `options`, and a `fallback` part with
its `source`. The parts concatenate to the string output. The Leptos layer
builds elements from `Markup` parts ([04](04-leptos-integration.md) §7).

### 2.3 Values

```rust
#[derive(Clone, Copy)]
#[non_exhaustive]                       // so P4's `DateTime` is additive (frozen at P3's exit)
pub enum Arg<'a> {
    Str(&'a str), Int(i64), Float(f64),
    Decimal(&'a str),                   // number-literal text, exact
    Custom(&'a dyn CustomValue),
    Unset,                              // → Unresolved Variable
    // P4, additive: DateTime(DateTimeValue<'a>) — Instant(epoch ms) | Floating{date, time}
    // (the spec treats an operand without an offset as floating) + optional zone
}
// From<&str>, From<&String>, From<i32>, From<i64>, From<u32>, From<f64>: `#[inline]`, call-site side.

pub trait CustomValue {                 // an application's own argument type
    fn as_str(&self) -> Option<&str> { None }        // string conversion (placeholders, `:string`)
    fn as_number(&self) -> Option<Number> { None }   // numeric conversion (numeric operands)
    fn as_any(&self) -> Option<&dyn core::any::Any> { None } // for handlers that know the type
}

/// A resolved value's data. Which handler resolved it decides how it formats and selects (§2.4).
#[non_exhaustive]
pub enum Value<'a> {
    Str(&'a str),                       // a literal, a string argument, `:string`'s operand
    Int(i64), Float(f64), Decimal(&'a str), // unannotated numeric arguments
    Number(Number),                     // `:number`, `:integer`, `:offset` (P4: `:percent`, …)
    Custom(&'a dyn CustomValue),
    Boxed(Box<dyn core::any::Any>),     // a custom handler's own data (the only allocation a handler makes)
    Fallback(FallbackSource<'a>),       // an operand that failed to resolve, as a handler receives it (§2.6)
}
impl<'a> Value<'a> {
    pub fn from_arg(arg: Arg<'a>) -> Option<Value<'a>>;                   // `Unset` → None
    pub fn as_str(&self) -> Option<&str>;                                  // Str, Custom::as_str
    pub fn to_number(&self, host: &dyn Host) -> Option<Number>;           // the numeric-operand rules
    pub fn downcast_ref<T: core::any::Any>(&self) -> Option<&T>;          // Boxed, Custom::as_any
}

/// An exact decimal and, once a numeric handler resolved it, its resolved options. Opaque:
/// the digit backend is internal (owner decision 1, 10 §"State at the start").
pub struct Number { /* private */ }
impl Number {
    pub fn parse(number_literal: &str) -> Option<Number>;   // `["-"] (0 / [1-9]*DIGIT) ["." 1*DIGIT] [e ["-"/"+"] 1*DIGIT]`
    pub fn from_i64(n: i64) -> Number;
    pub fn from_f64(x: f64, host: &dyn Host) -> Option<Number>;  // finite only; shortest round trip
    pub fn is_negative(&self) -> bool;
    pub fn is_integer(&self) -> bool;
    pub fn to_i64(&self) -> Option<i64>;
    pub fn write_plain(&self, out: &mut dyn Sink);           // the exact value: `-1234.5`, no exponent
}
```

`Arg` is small on purpose — every variant is a code path in the wasm.
`Value::Boxed` is the escape hatch that lets the public API express any custom
function (the `:test:*` functions use it); built-in handlers never allocate.

### 2.4 Functions and the registry

```rust
pub trait Function: Sync {
    /// Function resolution (formatting.md): `operand` resolved — a `Value::Fallback` when it
    /// failed (§2.6) — and `options` resolved with `u:id`/`u:dir` removed. `None` = a
    /// fallback value; the handler has reported why through `errs`.
    fn resolve<'a>(&self, cx: &FnContext<'_>, operand: Option<&Value<'a>>,
                   options: &Options<'_, 'a>, errs: &mut dyn ErrorSink) -> Option<Value<'a>>;
    /// Whether `value` can be formatted; `Err(e)`: the placeholder is a fallback value and `e` is reported.
    fn formattable(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> { Ok(()) }
    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink);
    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {}
    fn part_kind(&self) -> &'static str { "string" }
    /// The directionality of the formatted value, for the Default Bidi Strategy.
    fn dir(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Dir { Dir::Auto }
    /// Selection (formatting.md, "Resolve Selectors", Match, BetterThan). `matches`
    /// gets the key in NFC and reports e.g. Bad Variant Key through `errs`.
    fn selectable(&self, value: &Value<'_>) -> bool { false }
    fn matches(&self, cx: &FnContext<'_>, value: &Value<'_>, key: &str, errs: &mut dyn ErrorSink) -> bool { false }
    fn better_than(&self, cx: &FnContext<'_>, value: &Value<'_>, key1: &str, key2: &str) -> bool { false }
}

impl<'x> FnContext<'x> {                // read-only (formatting.md: access MUST be minimal)
    pub fn locale(&self) -> &'x str;
    pub fn dir(&self) -> Option<Dir>;   // the expression's `u:dir`, if any
    pub fn host(&self) -> &'static dyn Host;
    pub fn catalog(&self) -> &'x Catalog; // locale data: `catalog.locale_entry(key)`
}

impl<'o, 'a> Options<'o, 'a> {         // order is not significant; a repeated name: the last wins
    pub fn get(&self, name: &str) -> Option<OptionValue<'o, 'a>>;
    pub fn iter(&self) -> impl Iterator<Item = (&'a str, OptionValue<'o, 'a>)>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
#[derive(Clone, Copy)]
pub struct OptionValue<'o, 'a> { pub value: &'o Value<'a>, pub literal: bool } // `literal`: set by a literal

/// Closed world (B13): exactly the handlers the corpus uses, built by generated code.
pub struct Registry { /* &'static [(&'static str, &'static dyn Function)] */ }
impl Registry {
    pub const EMPTY: Registry;
    pub const fn new(functions: &'static [(&'static str, &'static dyn Function)]) -> Registry;
    pub fn get(&self, name: &str) -> Option<&'static dyn Function>; // `ns:name`, NFC
}
pub mod functions { pub static STRING; pub static NUMBER; pub static INTEGER; pub static OFFSET; }
```

`mf2-build` (P5a) generates `static REGISTRY: Registry = Registry::new(&[("integer",
&mf2::functions::INTEGER)]);` — handlers are statics referenced only from that
table, so an unused one is never linked. Function names are looked up per call
(`catalog.function(index)` → `Registry::get`, a few short compares). A11
measured it: 12.6 ns per resolution against 2.8 ns from a load-time table,
next to a 433 ns select — so it **stays per call**, and a `Formatter` needs no
per-catalog setup. A name the registry lacks is *Unknown Function* and a
fallback value.

### 2.5 The host

```rust
pub trait Host: Sync {
    /// The NFC form of `s`, which failed the quick check (a code point ≥ U+0300).
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str;
    /// The shortest text that round-trips the finite `x`, in any form `number-literal`
    /// accepts with an optional `+` in the exponent (`ryu`'s and JavaScript's `String(x)` both do).
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str>;
}
```

`mf2-host-std` (native, `wasm32-wasip1`: `unicode-normalization`, `ryu`) and
`mf2-host-web` (the browser: `String.prototype.normalize`, `String(x)`). Float
text through the host keeps `ryu` out of the client wasm (06 §3, "known
savings"); the runtime parses the text, so both hosts give the same digits.

### 2.6 Resolution rules the suite fixes

Where the spec leaves a choice, or the suite asserts more than the spec text:

* **Declarations** are resolved lazily, each at most once (call-by-need),
  without recursion (a chain of 10,000 `.local`s must not grow the stack). A
  reference `External(slot)` inside or after an `.input` for that slot is the
  `.input`'s value; inside the `.input` itself it is the argument. A reference
  to a declaration that is not before it (only a malformed catalog has one) is
  unresolved.
* **A fallback operand is given to the handler**, as `Value::Fallback`, after
  the function is found — formatting.md's step 1 would return a fallback value
  at once, but the suite expects what a handler does with it: an unknown
  function is still *Unknown Function* (`syntax.json` #15, #16), the numeric
  functions and `:test:*` report *Bad Operand* (`fallback.json` #3, #5,
  `pattern-selection.json` #11, #15), and `:string` takes the text of its
  representation, `{$x}`, and selects on it with no further error
  (`functions/string.json` #3). A fallback value's source is the expression's
  own (`$x`, `|lit|`, `:fn`).
* **Options** whose value is a fallback value are omitted and report *Bad
  Option*. `u:id` (a string value) and `u:dir` (`ltr`, `rtl`, `auto`,
  `inherit`) are removed before the handler is called and kept on the resolved
  value, so a variable carries them into a later placeholder
  (`u-options.json` #5). Any other value: *Bad Option*, ignored. `u:dir` on
  markup: *Bad Option*, ignored.
* **Direction**: `u:dir=ltr`/`rtl`/`auto` sets it and isolates; otherwise the
  handler's `dir` — `:string` and unannotated strings are unknown (`Auto`),
  numbers `Ltr` (neutral digits), fallback values unknown. The Default Bidi
  Strategy then follows formatting.md exactly; markup is never isolated.
* **Unannotated values**: a string formats as itself (part kind `string`); an
  integer, float or decimal argument formats as its exact value in plain
  neutral digits (kind `number`, `Ltr`) — no rounding, so the numeric handlers
  are not linked unless the corpus uses them; an application value through
  `CustomValue::as_str`, else *Bad Operand* and a fallback. None of them is
  selectable (*Bad Selector*).
* **Selection** is the spec's algorithm. A selector that resolved to a fallback
  value or is not `selectable` matches only `*` and reports *Bad Selector* once
  (`pattern-selection.json` #12, #14: not the handler's own error). A variant
  whose key count differs from the selector count never matches; if no variant
  matches, `{�}` and `MissingFallbackVariant`.
* **Markup** writes nothing to a `Sink`; its options resolve like an
  expression's, and its name is checked in both modes, so a damaged catalog
  stops string and parts output at the same place (the parts concatenate to
  the string; found by the `format` fuzz target, 10 A10); pairing is never
  checked.
* **Malformed data** (a view's `Err`, a string that is not UTF-8): `{�}` in its
  place, `Malformed`, and the message stops there — output before it stays.

### 2.7 Phase 4 additions (A1)

Written before the code (11 A1); every item is additive under the rules of
§2 — new methods, trait methods with a default, variants of the
`#[non_exhaustive]` enums, private or `#[non_exhaustive]` fields — and the
frozen items above are unchanged. The function crates (`mf2-fn-number`,
`mf2-fn-datetime`) are written against these and the §2 items only.

**Date/time values** (for `mf2-fn-datetime`; `datetime.md`):

```rust
pub struct Date { /* private */ }        // civil, proleptic Gregorian (ISO 8601), |year| ≤ 999,999
impl Date { pub const fn new(year: i32, month: u8, day: u8) -> Option<Date>; year(); month(); day();
            days_since_epoch(); from_days_since_epoch(i64) -> Option<Date>; weekday() }   // ISO: 1 = Monday
pub struct Time { /* private */ }        // wall clock, millisecond precision (the spec's literal has ≤ 3 digits)
impl Time { pub const MIDNIGHT: Time; pub const fn new(h: u8, m: u8, s: u8, ms: u16) -> Option<Time>; … }

#[non_exhaustive]                        // build with the constructors
pub struct DateTime<'a> {
    pub date: Date,
    pub time: Time,
    pub offset: Option<i32>,             // seconds east of UTC; `None` = floating (datetime.md: no offset)
    pub zone: Option<&'a str>,           // the IANA zone an application value is in, if it names one
    pub options: DateTimeOptions<'a>,    // what `:datetime`/`:date`/`:time` resolved; empty for an argument
}
impl<'a> DateTime<'a> {
    pub const fn floating(date: Date, time: Time) -> DateTime<'a>;
    pub const fn from_epoch_ms(ms: i64) -> Option<DateTime<'a>>; // an instant: UTC, offset 0
    pub const fn to_epoch_ms(&self) -> Option<i64>;              // `None` when floating
    pub const fn with_offset(self, seconds: i32) -> Option<Self>; // |seconds| < 86,400
    pub const fn in_zone(self, zone: &'a str) -> Self;
    pub fn write_iso(&self, out: &mut dyn Sink);                 // ISO 8601 / RFC 9557 text
}

#[non_exhaustive] #[derive(Default)]
pub struct DateTimeOptions<'a> {
    pub date: Option<DateStyle>,         // fields + length; `None`: no date part (`:time`) or unresolved
    pub time: Option<TimePrecision>,     // `None`: no time part (`:date`) or unresolved
    pub time_zone_style: Option<ZoneStyle>,
    pub time_zone: Option<ZoneOption<'a>>,   // the override options travel with the value
    pub hour12: Option<bool>,
    pub calendar: Option<&'a str>,
}
pub struct DateStyle { pub fields: DateFields, pub length: DateLength }
pub enum DateFields { Weekday, DayWeekday, MonthDay, MonthDayWeekday, YearMonthDay, YearMonthDayWeekday }
pub enum DateLength { Long, Medium, Short }
pub enum TimePrecision { Hour, Minute, Second }
pub enum ZoneStyle { Long, Short }
pub enum ZoneOption<'a> { Input, Utc, Offset(i32), Named(&'a str) }

pub enum Arg<'a> { …, DateTime(&'a DateTime<'a>) }   // a reference: `Arg` stays 24 bytes
pub enum Value<'a> { …, DateTime(DateTime<'a>) }     // an argument, or a date/time function's result
pub trait CustomValue { …, fn as_date_time(&self) -> Option<DateTime<'_>> { None } }
```

An application converts its own type (`jiff`, `chrono`, `time`, a JS
`Date`) into a `DateTime`; `mf2-fn-datetime` parses date/time literals and
strings into one. **Unannotated**, a `DateTime` is formatted only by the
registry's date handler, `Registry::with_dates` (below) — `mf2-fn-datetime`
formats it as `:datetime` with its defaults; without one it is *Bad Operand*,
so a client whose corpus uses no date function links no date code. (A first
cut formatted it as ISO 8601 text in the core: +~350 B gz of B1 for every
client, measured by `bench/b12/check.sh`, so it moved behind the hook.)
`DateTime::write_iso` stays as a helper. Neither a date/time nor a measure
has a string form: `:string` of one is *Bad Operand*.

**The time zone of the formatting context** (§6; the default of `timeZone`):

```rust
pub struct TimeZone { /* private: UTC, an offset, or an IANA name held inline (≤ 64 bytes) */ }
impl TimeZone {
    pub const UTC: TimeZone;
    pub const fn offset(seconds: i32) -> Option<TimeZone>;   // |seconds| < 86,400
    pub fn named(name: &str) -> Option<TimeZone>;            // a well-formed RFC 9557 `time-zone-name`
    pub fn as_option(&self) -> ZoneOption<'_>;               // Utc | Offset | Named
}
pub fn is_zone_name(s: &str) -> bool;                        // RFC 9557 `time-zone-name`
pub struct FormatContext { …, pub time_zone: TimeZone }      // `new`: UTC
impl FnContext<'_> { pub fn time_zone(&self) -> &TimeZone; }
```

Owned, not borrowed — `FormatContext` has no lifetime to add, and a
per-request zone (the visitor's cookie, §6) is not `'static`.

**`Host` methods for dates**, both with defaults (so `mf2-host-std` and any
existing host compile unchanged):

```rust
pub trait Host: Sync {
    …
    /// The UTC offset, in seconds, of the IANA zone `zone` at the instant `epoch_ms`;
    /// `None`: no zone data, or no such zone (the default). A date/time function that
    /// must convert an instant to a named zone then reports *Bad Option* (§5.4).
    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> { None }
    /// `datetime-intl`: formats `request` with the host's date formatter (the browser's
    /// `Intl.DateTimeFormat`) and returns `true`; `false` (the default) when it has none.
    fn format_date_time(&self, locale: &str, request: &DateTimeRequest<'_>, out: &mut dyn Sink) -> bool { false }
}
pub struct DateTimeRequest<'r> {
    pub epoch_ms: i64,                   // the instant (a floating value: its wall time read as UTC)
    pub zone: ZoneOption<'r>,            // where to show it (never `Input`)
    pub options: &'r DateTimeOptions<'r>,
}
```

`zone_offset` is how named zones reach both backends: `mf2-host-std` answers
it from a tz database (owner decision 1, 11 §"Carried"), `mf2-host-web` from
the browser's own (`Intl.DateTimeFormat` with `timeZone`), so `datetime-icu`
on the client needs no tz data of its own. The date semantics (operand and
option rules, the zone conversions — including the two-pass search that puts
a floating wall time into a named zone) stay in `mf2-fn-datetime`; a backend
only turns an instant plus a style into text.

**The numeric core, public for `mf2-fn-number`**:

```rust
#[derive(Clone, Copy)]
pub struct NumberSpec { /* private: option set, fraction defaults, integer rounding, selectable, scale */ }
impl NumberSpec {
    pub const NUMBER: NumberSpec; pub const INTEGER: NumberSpec; pub const OFFSET: NumberSpec;
    pub const PERCENT: NumberSpec;       // `:percent`'s options, 0–0 fraction digits, scale 2, plural
    pub const UNIT: NumberSpec;          // `:unit`'s options, not selectable
    /// `:currency` whose currency shows `fraction_digits` (its own for `fractionDigits=auto`).
    pub const fn currency(fraction_digits: u8) -> NumberSpec;
}
impl Number {
    /// What `:number` does under `spec`: operand rules, the spec's options (others are
    /// ignored), inheritance from a number or measure operand, the digit plan, rounding.
    pub fn resolve(spec: NumberSpec, cx: &FnContext<'_>, operand: Option<&Value<'_>>,
                   options: &Options<'_, '_>, errs: &mut dyn ErrorSink) -> Option<Number>;
    pub fn digits(&self) -> Option<Digits<'_>>;   // the rounded display digits; `None` for a bare number
    pub fn exact_digits(&self) -> Digits<'_>;     // the exact value, unrounded (unannotated numbers)
    pub fn grouping(&self) -> Option<Grouping>;   // the resolved `useGrouping`; `None` = not set
    pub fn is_selectable(&self) -> bool;
    pub fn matches(&self, cx: &FnContext<'_>, key: &str, errs: &mut dyn ErrorSink) -> bool;
    pub fn better_than(key1: &str, key2: &str) -> bool;
}
impl Digits<'_> {
    pub fn sign(&self) -> Sign;                   // None | Minus | Plus, after `signDisplay`
    pub fn integer_count(&self) -> u16;           // ≥ 1, after `minimumIntegerDigits`
    pub fn fraction_count(&self) -> u16;
    pub fn digit(&self, magnitude: i16) -> u8;    // 0–9; integers at 0.., fractions at -1..
    pub fn is_zero(&self) -> bool;
    pub fn write_neutral(&self, out: &mut dyn Sink);         // the core's output
    pub fn neutral_parts(&self, out: &mut dyn SubPartSink);
    pub fn operands(&self) -> Operands;           // CLDR operands as shown: a unit's plural form
}
pub enum Sign { None, Minus, Plus }
pub enum Grouping { Auto, Always, Never, Min2 }

/// `:currency` and `:unit` values (`mf2-fn-number`); a numeric operand for every numeric function.
#[non_exhaustive] #[derive(Clone)]
pub struct Measure<'a> { pub number: Number, pub unit: MeasureUnit<'a>, pub flags: u32 }
impl<'a> Measure<'a> { pub fn new(number: Number, unit: MeasureUnit<'a>, flags: u32) -> Self; }
pub enum MeasureUnit<'a> { Currency([u8; 3]), Unit(&'a str) }   // currency code upper-cased
pub enum Value<'a> { …, Measure(Measure<'a>) }
impl Value<'_> { pub fn digit_size(&self) -> Option<u8>; }   // a digit size option's value (number.md): `fractionDigits`
pub trait CustomValue { …, fn as_measure(&self) -> Option<Measure<'_>> { None } }
```

`flags` is the resolving crate's own encoding of the options it adds
(`currencyDisplay`, `currencySign`, `fractionDigits`, `unitDisplay`), so a
later `:currency` / `:unit` inherits them from its operand; the runtime never
reads it. Selection and exact-match keys stay the core's: a localized handler
delegates `matches` / `better_than` to `Number`, so plural operands still
come from the formatted digits and keys stay in neutral digits.

**Unannotated numbers with `fn-number` on** (`syntax.json` #90), **and
unannotated dates with `fn-datetime` on**:

```rust
impl Registry {
    /// This registry, with `f` formatting unannotated numeric values (integer, float and
    /// decimal arguments): `mf2-fn-number`'s localized exact value. Without it they are
    /// neutral (§2.6).
    pub const fn with_numbers(self, f: &'static dyn Function) -> Registry;
    /// This registry, with `f` formatting unannotated date/time values (`mf2-fn-datetime`:
    /// as `:datetime` with its defaults). Without it they are *Bad Operand*.
    pub const fn with_dates(self, f: &'static dyn Function) -> Registry;
}
```

The evaluator checks an unannotated number as any unannotated value (§2.6:
a non-finite float is *Bad Operand*, an over-long decimal *Unsupported
Operation*), so the errors do not depend on the feature, and then asks `f`
for `dir`, `format`, `format_parts` and `part_kind`; a date/time it hands to
its handler entirely (`formattable` too). Neither selects (*Bad Selector*). Generated registries add it whenever `fn-number`
is on (05 §3.1), so the default configuration links nothing new.

**LOCALE entries** are read as before, `FnContext::catalog().locale_entry(key)`,
with the keys of `mf2_catalog::format::locale_key` and the entry views
`mf2-catalog` provides for them (02 §4).

**The `intl` option** (§5.3; owner decision 4, 11 §"Carried"): the
numeric functions' final step — display, `:integer`'s rounding, the plural
category — from the host's number formatter, on `wasm32-unknown-unknown`
with feature `intl` only:

```rust
/// Feature `intl` on wasm32-unknown-unknown; `false` on every other build (servers,
/// wasm32-wasip1, native tests: the Rust path). The one switch: mf2-fn-number reads it too.
pub const INTL_NUMBERS: bool;

pub trait Host: Sync {
    …
    /// The host's number formatter; `None` (the default) when it has none.
    fn numbers(&self) -> Option<&dyn NumberFormatter> { None }
}
pub trait NumberFormatter: Sync {       // mf2-host-web: Intl.NumberFormat, Intl.PluralRules
    /// Writes `request` for `locale` (neutral symbols when `request.neutral`); `false`: cannot.
    fn format(&self, locale: &str, request: &NumberRequest<'_>, out: NumberOut<'_>) -> bool;
    /// The plural category of `request.value` under its digit options (`request.ordinal`: ordinal).
    fn plural(&self, locale: &str, request: &NumberRequest<'_>) -> Option<Category>;
}
pub enum NumberOut<'o> { Text(&'o mut dyn Sink), Parts(&'o mut dyn SubPartSink) }

#[non_exhaustive] #[derive(Clone, Copy)]
pub struct NumberRequest<'r> {
    pub value: &'r str,                  // the exact value in plain digits: `-1234.5`, `-0`, no exponent
    pub style: NumberStyle<'r>,          // unscaled for `Percent` (the style multiplies)
    pub neutral: bool,                   // the core's output: `en`, `latn`, no grouping
    pub digits: DigitOptions,
    pub sign: SignDisplay,
    pub grouping: Grouping,
    pub ordinal: bool,
}
#[non_exhaustive] #[derive(Clone, Copy)]
pub struct DigitOptions {                // ECMA-402's, as SetNumberFormatDigitOptions resolved them
    pub minimum_integer: u8,             // 1–21
    pub fraction: Option<(u8, u8)>,      // 0–100; None: the style's defaults (or significant alone)
    pub significant: Option<(u8, u8)>,   // 1–21; None: fraction digits alone
    pub priority: RoundingPriority,      // Auto | MorePrecision | LessPrecision
    pub increment: u16,                  // 1 = none
    pub mode: RoundingMode,              // Ceil | Floor | Expand | Trunc | HalfCeil | HalfFloor | HalfExpand | HalfTrunc | HalfEven
    pub strip_if_integer: bool,          // trailingZeroDisplay
}
#[non_exhaustive]
pub enum NumberStyle<'a> {
    Decimal, Percent,
    Currency { code: &'a str, display: CurrencyDisplay, accounting: bool,
               own_digits: bool },           // fractionDigits unset/auto: `fraction` None, the formatter's
    Unit { unit: &'a str, display: UnitDisplay },
}
pub enum CurrencyDisplay { Symbol, NarrowSymbol, Name, Code, Never }
pub enum UnitDisplay { Short, Narrow, Long }
pub enum SignDisplay { Auto, Always, ExceptZero, Negative, Never }   // were pub(crate)
impl Number {
    /// This number through the host's formatter in `style`, neutral or with the locale's
    /// symbols: a resolved number's display, a bare number's exact value. `false`: no formatter.
    pub fn format_by_host(&self, cx: &FnContext<'_>, style: NumberStyle<'_>, neutral: bool,
                          out: NumberOut<'_>) -> bool;
}
// mf2-host-web, feature `intl`:
pub struct IntlNumbers(pub &'static dyn Host);   // another host + `numbers()` (Intl, when v3)
pub static NUMBERS_HOST: IntlNumbers;            // IntlNumbers(&HOST)
```

* **What moves to the host.** With `INTL_NUMBERS` the numeric core keeps
  option validation and its errors, the operand rules, inheritance,
  `:offset`'s exact addition, `select` and the exact-match keys, and asks
  the formatter for the display (`format`), `:integer`'s rounded value (a
  neutral format with 0 fraction digits), a non-integer exact key's
  display (neutral), and the plural category (`plural`, once per selector
  value). `Number::digits` is then always `None`: the Rust rounding, digit
  output and plural evaluator are not linked. On every other build nothing
  changes; `format_by_host` exists there too (and only asks the host).
* **Limits.** The formatter's digit sizes are ECMA-402's: with
  `INTL_NUMBERS`, `minimumIntegerDigits`, `minimumSignificantDigits` and
  `maximumSignificantDigits` above 21 are *Bad Option* and replaced by 21
  (number.md, "Digit Size Options", allows the replacement); the Rust path
  takes 0–99.
* **No formatter** (`numbers()` is `None`: a host without one, or a
  browser without `Intl.NumberFormat` v3 — no Rust fallback in the client,
  owner decision 4): a numeric function's resolution reports *Unsupported
  Operation*, the value shows its exact digits in neutral symbols, and
  keyword selection matches only `other`.
* **One slot, not three.** `numbers()` is a trait method with a default, on
  every build, so a client that turns `intl` on and formats no number links
  nothing new (B1′ = +0 B); the price is one `Host` vtable slot for every
  client: +17 B raw / +15 B gz of B1's runtime part (`b12-runtime`,
  `bench/b12/check.sh`; three methods measured +38 / +31). A host gets the
  formatter as a separate static (`NUMBERS_HOST`) so that `HOST` itself does
  not grow with the feature.

## 3. Function registry — full spec, closed world

Every default function in the pinned spec is implemented, REQUIRED and
RECOMMENDED, Stable and Draft:

`:string` · `:number` · `:integer` · `:offset` · `:percent` · `:currency` ·
`:unit` · `:datetime` · `:date` · `:time` — plus `u:id`, `u:dir`, custom
functions, and namespaces.

What keeps this from bloating the client is **closed-world registration**
(budget B13): `mf2-build` records the set of functions the corpus uses; the
generated code builds the `Registry` from exactly those handlers; handlers are
statics referenced from nowhere else, so dead-code elimination drops the rest.
The *library* is full-spec; each *app* links what it uses. A name the registry
lacks yields `unknown-function` + fallback, exactly as the spec says. The trait
and the registry are in §2.4: a handler resolves an expression to a [`Value`]
and is then asked, for that value, whether and how it formats, its direction,
and whether it selects and how (Match, BetterThan).

The three `:test:*` functions of the suite are implemented in the conformance
crate against this public trait and nowhere else; if they cannot be, the trait
is wrong. (A1 checked it: `resolve` keeps `Input`, `DecimalPlaces`,
`FailsFormat`, `FailsSelect` in a `Value::Boxed`, which a later `:test:*`
expression reads back with `downcast_ref`; `formattable` fails for
`:test:select` and `fails=format`; `selectable` is false for `:test:format` and
`fails=select`, which yields exactly *Bad Selector*.)

## 4. Crates

| Crate | Contents | Client? |
|---|---|---|
| `mf2-runtime` | evaluator, selection, fallback, bidi, parts, registry, the `Host` trait, `:string`, the plural-rule evaluator, and the **core numeric semantics**: `:number` / `:integer` / `:offset` — operand parsing, every digit and rounding option (over `fixed_decimal`), `signDisplay`, `select` with `exact` / `plural` / `ordinal`, and locale-neutral output. Depends on `mf2-model`, `mf2-catalog`, `fixed_decimal`. Numeric code is only linked when the corpus uses a numeric function (closed world) | yes |
| `mf2-fn-number` | the *localization* of numbers — symbols, grouping, numbering systems — plus `:percent`, `:currency`, `:unit`. Depends on `mf2-runtime` | feature `fn-number`, when used |
| `mf2-fn-datetime` | `:datetime :date :time`: semantics in Rust; text from ICU4X (`datetime-icu`, optional dependency `icu_datetime`; data from the catalog's `icu.blob`) or from `Host` (`datetime-intl`; ICU4X with compiled data off the browser). Depends on `mf2-runtime` (and `mf2-catalog` for the entry's key) only — it never names a host crate | feature `fn-datetime`, when used |
| `mf2-host-web` | `Host` for the browser: `String.prototype.normalize`, `String(x)` for float text (`HOST`); with feature `time-zones`, `ZONES_HOST` also answers `zone_offset` from the browser's zone data; with `datetime-intl`, `INTL_HOST` also formats dates with `Intl.DateTimeFormat` (one inline-JS module, `wasm-bindgen`). Separate statics, since a host method is linked whenever its host is (B1′). Depends on `mf2-runtime`, `js-sys` | client only |
| `mf2-host-std` | `Host` for native and for the `wasm32-wasip1` test run: a pure-Rust NFC normalizer (`unicode-normalization`), float text (`ryu`) and zone offsets from `jiff`'s bundled IANA database (owner decision 1). Depends on `mf2-runtime` | server / tests |

Function crates are split by **dependency weight**, not per function; within a
crate, closed-world linking does the fine-grained pruning.

## 5. Backends per function family (decision D4)

Measured by Phase 0 probes P0.4–P0.6 (deltas against a fair base,
`wasm-release`, `wasm-opt -Oz`, gzip -9; details in
[phase-0-results](phase-0-results.md)):

| Family | Own Rust, data in catalog | ICU4X in the browser | Browser `Intl` glue |
|---|---|---|---|
| Plural rules | **0.43 KB gz** evaluator, correct on all 15,041 CLDR 48 samples. Data per locale 0–84 B | 14.5 KB gz (runtime blob) – 17.9 KB gz (compiled data, which also lacks 75 CLDR locales) | not used by default: selection never uses the host (the proposed `intl` option, §5.3, would take the category from `Intl.PluralRules`) |
| Decimal numbers | core semantics over `fixed_decimal` **10.2 KB gz** (always needed for MF2 semantics, whatever the backend) + localization **1.7 KB gz** incl. `:percent`; symbols ≤ 0.2 KB gz per locale | `icu_decimal` + blob +15.9 KB gz over the core, and cannot express `useGrouping=always` | +1.6 KB gz wasm + 0.6 KB gz JS over the core |
| Currency / unit | +2.9 KB gz; data for the codes used 0.2–0.6 KB per locale (all codes: 4–10 KB gz each) | only in `icu_experimental` (unstable) | ≈ glue only |
| Date / time | semantics 3.4 KB gz (needed by both backends); formatting itself not realistic (calendars, zones, skeleton matching) | `icu_datetime` + blob **93 KB gz** Gregorian (64 without zone styles, 105 any calendar); blob 2.2–2.6 KB gz per locale without zone names, 17–23 with | **5.4 KB gz** incl. the semantics + 0.6 KB gz JS |

"Browser `Intl`" means the **ECMAScript Internationalization API (ECMA-402)**:
the `Intl` global object built into every browser and JS runtime
(`Intl.NumberFormat`, `Intl.DateTimeFormat`, `Intl.PluralRules`), backed by the
browser's own ICU/CLDR data and reachable from Rust through `js_sys::Intl`. Code
and locale data it uses are not in the wasm. MF2's numeric and date option names
were deliberately derived from it.

### 5.1 Owner's decision: number and date internationalization are opt-in features

Locale-aware number and date formatting is behind cargo features, **off by
default**. An application that values the smallest wasm leaves them off; one that
needs them turns them on knowing the cost.

**A feature is a property of the application, not of a target.** The i18n crate
declares it once and it applies to the server build and the wasm build alike, so
the server renders exactly what the client would (with `fn-number` off, both
produce neutral digits). The only sanctioned difference is `datetime-intl`, where
the server has to use ICU4X because there is no `Intl` on the server.

| Client feature | Off (default) | On |
|---|---|---|
| *(core, always)* | `:string`; `:number` / `:integer` / `:offset` with their **complete semantics** — operand rules, all digit-size options, `roundingMode` / `roundingIncrement` / `roundingPriority`, `trailingZeroDisplay`, `signDisplay`, `minimumIntegerDigits`, option inheritance, and `select` = `exact` / `plural` / `ordinal` (rules from the catalog) — rendered with **neutral symbols**: ASCII digits, `.`, `-`/`+`, no grouping | — |
| `fn-number` | the option values that are *only* about locale — `useGrouping` other than `auto`/`never` — emit *Unsupported Operation* and render neutrally, and the locale's numbering system (a non-Latin default such as `ar-EG`'s, or a `-u-nu-` tag) is not applied: ASCII digits. (The pinned spec has no `numberingSystem` option; a catalog's system comes from its locale, 02 §4.) Any numeric formatting at all is a `check` **warning** (`neutral-numbers`, configurable to allow/deny). `:percent` / `:currency` / `:unit` in the corpus are a **build error** | locale symbols, grouping, numbering systems, `:percent`, `:currency`, `:unit`; data arrives lazily in the catalog. Measured 1.7 KB gz (+2.9 with `:currency` + `:unit`; P0.5) |
| `fn-datetime` + one backend | `:datetime` / `:date` / `:time` in the corpus are a **build error** | `datetime-icu`: ICU4X **code** in the wasm, its **data** in the catalog (`icu.blob`) — identical to server output, 93 KB gz of code (Gregorian; P0.6). `datetime-intl`: the browser's built-in API — 5.4 KB gz incl. the shared date semantics + 0.6 KB gz JS; output differs cosmetically from the server's (at least U+202F vs U+0020, and zone-styled layouts) |

Two mechanisms, two jobs:

* **The feature flag is the guard.** A translator adding `{$d :date}` cannot
  silently add 100 KB to every visitor's download: `mf2 check` and the build fail
  with "corpus uses `:date` (locales/es/profile.mf2:41) but feature
  `fn-datetime` is off" until a developer opts in. Off never means *silently
  wrong*: every degradation is reported **at build time** by `mf2 check` (an
  error for gated functions, the `neutral-numbers` warning for plain digits) —
  which matters because the release client's error sink is a no-op (§8).
* **Closed-world linking is the pruner.** With a feature on, handlers the corpus
  does not reference are still eliminated (B13). Turning a feature on costs
  nothing until a message uses it.

Plural selection is **not** behind a flag: it is core MF2, costs about 1 KB, and
its data is lazy.

Conformance implications: the suite runs in the **all-features** configuration
(that is the "full MF2" claim, L4), and again in the **default** configuration,
where the ledger records the documented degradations (`unsupported-operation`,
build-time rejections) explicitly — never as silent skips.

### 5.2 Implementation choices that follow

* **Plural: own evaluator, rules baked per locale into the catalog.** 34–42×
  smaller than `icu_plurals` (0.43 vs 14.5–17.9 KB gz), locale count costs
  nothing, deterministic on both targets.
  Rules come from pinned CLDR JSON (`cldr-json` 48.2.1) at build time. CLDR's
  own `@integer` / `@decimal` samples are an exhaustive, free test corpus for
  every locale and MUST be run against the evaluator.
* **Numbers: `fixed_decimal` carries the arithmetic** (arbitrary-precision
  rounding modes and increments) in core; `fn-number` layers symbols, grouping
  and digits from the catalog's LOCALE section on top. Same code on server and
  client, so output is byte-identical. The split follows one rule: *semantics
  are core, localization is the feature* — plural selection depends on the
  formatted digits, so the digit logic cannot be optional. P0.5 found that
  `fixed_decimal` 0.7.2 (and its `smallvec`) keeps six panic entry points
  reachable, which breaks B12 for the numeric path and pulls ≈ 3.6 KB raw of
  panic formatting into std builds. **Owner decision (2026-09-21, D15): an own
  panic-free, allocation-free digit buffer** inside `mf2-runtime`
  (`number/decimal/own.rs`), under D1's rule: `fixed_decimal` stays behind the
  same internal interface (feature `fixed-decimal`, never a client's) as the
  baseline of an A/B — the suite, a differential on the same random corpus,
  size, allocations, B12 and speed (10 A5b) — and as the fallback if the own
  buffer does not pass. **A5b (2026-09-21): it passes every row** — identical
  output on P0.5's 100,000-case corpus, the same ECMA-402 result (95,675
  identical, 0 different), 5,142 vs 7,305 B gz for the numeric share, B12
  clean vs a surviving panic import, 0 vs 0.5 allocations per format, speed
  equal within noise (`bench/runtime-bench/NUMBER-AB-P3.md`).
* **Dates (`fn-datetime`)**: the server always formats with ICU4X — so on a
  server build `fn-datetime` pulls in `icu_datetime` whichever client backend was
  chosen. The client backend is the app's
  choice between exactness (`datetime-icu`) and size (`datetime-intl`); P0.6
  and P0.10 measured both (above) and showed that text differences never break
  hydration. Three consequences: (1) `mf2-build` links ICU4X's **no-zone field
  set** whenever the corpus never uses `timeZoneStyle` — −29 KB gz of code and
  ≈ −85 % of `icu.blob`, since zone names dominate it; (2) ICU4X 2.x has **no
  time-zone transition rules**, so named zones (including the visitor's zone
  from the cookie, §6) need a tz database beside it on the server — **owner
  decision 1 (2026-09-21): `jiff` with its bundled IANA database**, compiled
  into `mf2-host-std` (deterministic, the same native and on `wasm32-wasip1`),
  reached through `Host::zone_offset`; in the browser `mf2-host-web` answers
  from the browser's own data; (3) for `datetime-icu` byte identity the server
  formats from the same per-locale data the catalog's `icu.blob` carries.

**The date backends as built (Phase 4, A6).** Both plug into
`mf2-fn-datetime`'s `Backend` seam: the semantics resolve a value, a `Plan`
(the wall date and time in the zone shown, the offset, the zone, the resolved
options) is what a backend formats.

* **`datetime-icu`: `mf2_fn_datetime::icu::Icu<C, Z, D>`.** The type
  parameters are what closed-world linking prunes by, chosen by the build
  from the corpus (`mf2-build`, P5a): `C` = `AnyCalendar` (the
  `DateTimeFormatter`: a locale's non-Gregorian default such as `th`'s
  Buddhist, and `calendar=`) or `GregorianOnly` (the
  `FixedCalendarDateTimeFormatter<Gregorian>`; another calendar is an
  *Unsupported Operation*); `Z` = `WithZones` (the composite field set with
  zone styles) or `NoZones` (`timeZoneStyle` is an *Unsupported Operation*);
  `D` = `Blob` (the catalog's `icu.blob`, 02 §4.9 — client and server alike,
  so the same bytes) or `Compiled` (ICU4X's compiled data, never in the
  browser: the server side of `datetime-intl`). The statics `DATETIME`,
  `DATE`, `TIME`, `DATES` are the widest, `Icu<AnyCalendar, WithZones,
  Blob>`. A plan becomes a semantic skeleton at run time (`FieldSetBuilder`):
  the date fields → `E` / `DE` / `MD` / `MDE` / `YMD` / `YMDE`, the length,
  the time precision, `timeZoneStyle` long / short → the *specific* zone
  names (`SpecificLong` / `SpecificShort`: what `Intl.DateTimeFormat`'s
  `timeZoneName` long / short show); `hour12` → hour cycle `h12` / `h23`;
  `calendar` → the calendar algorithm. The zone shown: UTC as ICU4X's `utc`,
  an offset as an unknown zone with that offset ("GMT+05:30"), a named zone
  as its BCP-47 id through ICU4X's IANA parser, with the plan's offset — the
  conversion itself is the semantics', through `Host::zone_offset`, since
  ICU4X has no transition rules. Per format the blob is copied into a
  `BlobDataProvider` and the formatter built from it (no cache: the handlers
  are `no_std` statics), twice (`formattable`, then `format`): **2.7–4.5 µs**
  a date placeholder without zones, **38 µs** with a zone style (the zoned
  blob is ~28 KB: its copy and the IANA parser), against 0.3–0.6 µs for the
  neutral backend (`cargo run --release -p runtime-bench --example
  date_cost`, native, 2026-09-22) — a per-catalog provider cache is the known
  improvement, not needed by any budget. *Unsupported Operation* (and a
  fallback value): no `icu.blob`, a calendar or shape the blob does not
  carry, a date outside ICU4X's range, a zone style with `NoZones`, a calendar
  other than `gregory` with `GregorianOnly`. The text's direction is the
  catalog's (an Arabic date is right-to-left). ICU4X writes through
  `core::fmt::Write` and keeps its own `core::fmt` and panic paths: B12 holds
  for our crates, and this code is the feature's documented cost (06 B4).
* **Named zones.** Server: `mf2-host-std` answers `Host::zone_offset` from
  `jiff`'s bundled database (features `std`, `tzdb-bundle-always`: never the
  system's), the same natively and on `wasm32-wasip1` (owner decision 1).
  Browser: `mf2_host_web::ZONES_HOST` (`time-zones`, which `datetime-icu`
  turns on with `host-web`) and `INTL_HOST` answer it from the browser's
  data — the `longOffset` zone name of an `Intl.DateTimeFormat` for that
  zone, parsed — so the client wasm carries no tz database. The plain `HOST`
  has no zone data (a host method is linked with its host: B1′).
* **`datetime-intl`: `mf2_fn_datetime::Intl`** hands `Plan::request` (the
  instant, the zone, the options) to `Host::format_date_time`;
  `mf2_host_web::INTL_HOST` formats it with one cached `Intl.DateTimeFormat`
  per locale and option set (an inline-JS module; the options travel as a
  JSON string built in a fixed buffer). The mapping onto ECMA-402: `dateStyle`
  / `timeStyle` when the plan is expressible with styles (the date
  `year-month-day` or none, the time to the minute or second or none, no zone
  style, not a time alone with `hour12`: V8 and JavaScriptCore apply
  `hour12` to a 24-hour locale's `timeStyle` pattern by swapping its hour
  field, "03:04 PM"), otherwise components (`year`, `month` long / short /
  numeric by length, `day`, `weekday`, `hour`, `minute`, `second`,
  `timeZoneName`); `hour12`, `calendar` and the zone pass through. A host
  without a formatter (`HOST`), or one that rejects the request, gives the
  neutral text; an instant outside ECMA-402's range (±8.64 × 10¹⁵ ms) is an
  *Unsupported Operation*. Off the browser `datetime-intl` formats with
  `Icu<AnyCalendar, WithZones, Compiled>`, and the catalog carries no
  `icu.blob`.
* **`datetime-intl` against ICU4X** (`tools/e2e` check `datetime`: the
  panel's 11 locales × 60 messages, one-message catalogs, the server's ICU4X
  text beside the browser's, P0.10's tolerance: U+202F and U+00A0 read as
  U+0020). Where the mapping is exact (styles, 187 cases per engine) every
  case is identical, tolerated or a named engine–CLDR divergence: Chromium
  143 — 143 identical, 20 tolerated, 24 known (Polish short dates, Spanish
  and Arabic date–time glue, no Welsh data in the headless shell); Firefox
  155 — 164, 20, 3 (Polish short dates); WebKit 26.6 — 162, 20, 5. Mapped to
  components (253) and with zone styles (220) the engines differ more
  (Chromium 46 and 60 different, Firefox 32 and 39, WebKit 35 and 40): a
  locale's semantic skeleton picks, e.g., a numeric month where a component
  asks for an abbreviated one (`de` medium month–day: ICU4X "02.01.",
  `Intl` "2. Jan.") and no ECMA-402 option set reproduces CLDR's semantic
  skeletons per locale; the zone names themselves agree. Cosmetic, and
  harmless to hydration (P0.10) — the known cost of the option (§5.1).

Whatever is enabled, **the function semantics (options, operand rules, errors,
selection) are implemented once in Rust** in `mf2-fn-*`; a backend only supplies
the final "digits + symbols → text" or "instant + skeleton → text" step. Option
validation, inheritance, `select` handling and error emission never fork.

### 5.3 Owner request (2026-09-21): an `intl` client option — adopted after the Phase 4 probe

The owner asked to use the browser's `Intl` wherever it can do the work,
with Rust kept for server rendering, so that the client wasm need not carry
number and date code. Phase 4 probes it first ([11](11-phase-4-work-order.md)
A0); it becomes an opt-in client feature (`intl`, off by default) only if the
probe's numbers hold. The design it would follow:

* **The split stays "semantics in Rust, final step from a backend"** (§5.2).
  Rust keeps what `Intl` does not do MF2's way: option validation and MF2's
  errors (`Intl` throws where MF2 reports *Bad Option* and continues), the
  operand rules, exact-match keys, `:offset`'s exact decimal addition, and the
  whole evaluator. On the client, `Intl.NumberFormat` (`formatToParts`, with
  the digit options, `useGrouping: false` and `numberingSystem: latn` for
  neutral output) replaces the digit plan, rounding and digit output;
  `Intl.PluralRules` with the same digit options gives the plural category;
  `Intl.DateTimeFormat` the dates (`datetime-intl` folds into the option).
* **Through the `Host`** (03 §2.5): new methods with default bodies (the
  frozen API allows them) that `mf2-host-web` implements with `Intl`, caching
  one formatter per locale and option set; `mf2-host-std` keeps the Rust path,
  so a server renders with the same Rust code as today.
* **Expected gain** (estimates from measured pieces; the probe measures it):
  about 3 of the core numbers' 5.1 KB gz and the 0.43 KB plural evaluator,
  less ~1 KB gz of JS glue; about 2–3 KB more when `fn-number`, `:currency`
  and `:unit` are used (P0.5: 1.7 + 2.9 KB gz of Rust against 1.6 KB wasm +
  0.6 KB JS of `Intl` glue); and each catalog drops its number, currency, unit
  and plural entries (0.2–0.7 KB a locale).
* **Known costs**: server and client text differ where the browser's CLDR
  version or engine conventions differ from our pinned CLDR (P0.10: harmless
  to hydration — the server's text stays until the node's next update);
  MF2's options need `Intl.NumberFormat` v3 (2022–23 in all three engines),
  and supporting older browsers would mean shipping the Rust fallback anyway;
  `Intl.PluralRules.select` takes a JS number, so decimals past ~15
  significant digits can select differently from the exact value;
  conformance for this option is per engine (Chromium, Firefox, WebKit),
  as good as each engine's `Intl`; every numeric placeholder crosses from wasm
  to JS.

**The probe (11 A0; `bench/intl-probe/RESULTS.md`) and the decision.**
Measured in Chromium 143 and Firefox 155 (WebKit could not start then), B gz
of wasm + JS against the same harness without numbers: the core numeric
functions cost 5,382 on the Rust path and **7,541** through `Intl` (774 of it
JS) — the option is *larger*, because MF2's option validation, operand
rules, `:offset` and inheritance stay in Rust and the glue outweighs the
rounding, digit output and plural code it replaces; with locale symbols and
`:percent` the two are 7,103 and 7,557; only `:currency` + `:unit` may favour
it (≈ −1.8 KB, an estimate until A4 lands). Every numeric placeholder is
2–6× slower (+2–10 µs; +10–40 µs at 4× CPU throttle). Where the engines have
the data they agree with the Rust path — P0.5's 100,000 cases 95,675 / 0,
the panel's 4,004 localized cases all identical — but `Intl.PluralRules`
answers with en-US rules for a locale it lacks (Firefox lacks 30 of CLDR's,
Chromium 5), silently. **Owner decision 4 (2026-09-22): adopted as an opt-in
client feature, off by default, requiring `Intl.NumberFormat` v3** — no Rust
fallback in the client; an engine without it formats numbers neutrally and
reports *Unsupported Operation*. The probe stays as the A/B baseline.

**What is built (Phase 4).** Features `intl` on `mf2-runtime`,
`mf2-fn-number`, `mf2-host-web` and the `mf2` facade; the switch is the
runtime's `INTL_NUMBERS` (the feature *and* `wasm32-unknown-unknown`), which
`mf2-fn-number` reads too, so the two crates cannot disagree; the API is
§2.7's.

* **The runtime** has two backends behind one internal interface
  (`number/display.rs`, the Rust one; `number/intl.rs`): what a resolved
  number keeps, `:integer`'s rounding, the neutral display, the display an
  exact key compares with, the plural category. Everything else —
  `number.rs`'s resolution, the option tables and errors, `:offset`, the
  exact-key rules — is shared, so the two builds report the same errors
  (except the digit-size limit of 21, §2.7).
* **`mf2-fn-number`** writes `:number`, `:integer`, `:offset`, `:percent`,
  unannotated numbers, `:currency` and `:unit` through
  `Number::format_by_host` with the style (`intl.rs`; hooks in `lib.rs` and
  `measure.rs`). `measure.rs` still resolves the currency or unit and its
  options; `fractionDigits` unset or `auto` asks for the currency's own
  digits (the engine's), `currencyDisplay=never` — which `Intl` lacks — is
  formatted with `code`, the code and the blank next to it dropped; a unit
  `Intl` does not sanction (it sanctions 45 and their `-per-` compounds) is
  *Unsupported Operation* at `formattable`, as the Rust path says for a
  unit the catalog lacks.
* **`mf2-host-web`** (`numbers.rs`): `NUMBERS_HOST` = `IntlNumbers(&HOST)`,
  whose `numbers()` is `Intl` once `Intl.NumberFormat` v3 is detected (by
  behaviour: `roundingPriority`, `roundingMode`, `roundingIncrement`,
  `trailingZeroDisplay`, `useGrouping: "min2"`, `signDisplay: "negative"`,
  exact decimal strings, `PluralRules` with `roundingMode`, `formatToParts`
  — Chromium 143 resolves `roundingPriority` to `"auto"` in
  `resolvedOptions()` while honouring it). One inline-JS module caches one
  `NumberFormat` / `PluralRules` per kind, locale and option set (FIFO,
  256); the options cross as JSON built in a fixed 512-byte buffer (no
  allocation, no `core::fmt`), the value as its exact decimal text, parts
  back as `type U+001F value` records separated by U+001E.
* **Conformance** ([01](01-conformance.md) §3–§4, `cargo xtask l4-web`):
  L4 **324 / 324** runtime tests in Chromium 143, Firefox 155 and WebKit
  26.6, and no suite test formats otherwise than the Rust path, in either
  configuration. Over the goldens the text is the Rust path's except
  Chromium's Welsh currency and unit names (English; 72 cases) and ar-EG's
  `currencyDisplay=never` (a U+200F left of the number, 9 cases, all
  engines); negative numbers in ar, ar-EG and he split the minus sign's bidi
  mark into its own sub-part (all engines). All of it is in the ledger's
  `[[intl]]` tables.
* **Cost** ([06](06-size-and-perf.md) §3, "Phase 4: the `intl` client
  option"; `bench/intl-probe/RESULTS.md` §8): B gz of wasm + JS against the
  Rust path, core numbers +2,205, with `fn-number` +389, with `:currency`
  and `:unit` **−3,643** (JS 983 of it); per numeric placeholder or select
  1.7–6× the Rust path (+1 to +7 µs; +4 to +27 µs at 4× throttle) in all three
  engines, and faster than A0's probe handlers; B1′ −69 B gz; one `Host`
  vtable slot (+15 B gz) for every client.
* **Catalogs.** An `intl` client reads no `number.*`, `currency.*`,
  `unit.*` or `plural.*` entry; leaving them out of the catalogs it
  downloads is `mf2-build`'s slicing (P5a), which then serves the client a
  catalog without them and keeps the server's (which renders with the Rust
  path) whole.

### 5.4 Dates: the semantics' open points (Phase 4, A5)

`mf2-fn-datetime` decides what datetime.md leaves open as follows (its crate
docs and tests hold the details):

* **Operands**: a `Value::DateTime` (an argument, or an earlier date/time
  value with its options), `CustomValue::as_date_time`, or a string — a
  literal, a string argument, `CustomValue::as_str` — that matches the
  spec's regular expression exactly, over the whole string, and names a real
  day (2023-02-29 is rejected; the MAY to accept other ISO 8601 forms is not
  taken). No time → 00:00:00; no offset → floating; `-00:00` is offset 0.
  Anything else is *Bad Operand* with a fallback value. A `:date` value is
  accepted as a `:time` operand and the reverse (the spec's MAY *Bad
  Operand* is not taken): a resolved value keeps its operand's whole
  date/time.
* **Options**: each function takes exactly the options datetime.md lists for
  it; any other is ignored without an error. Values compare
  case-sensitively; an invalid value is *Bad Option* and ignored; a
  non-override option set by a variable is *Bad Option* and ignored.
  `calendar` accepts `3*8alphanum *("-" 3*8alphanum)` (UTS #35 also allows
  `_`, which `Intl.DateTimeFormat` rejects); whether the calendar is known is
  the backend's to report.
* **Inheritance**: only the override options (`timeZone`, `hour12`,
  `calendar`) travel from a date/time operand; the expression's own win. So
  over `.local $d = {|…| :date length=long}`, `{$d}` formats `$d` as resolved
  (long) and `{$d :date}` is a new `:date` (medium).
* **Resolved value**: `Value::DateTime` whose `options` carry the parts,
  styles and override options, with `time_zone` naming the zone the value is
  now in — a resolved value is always in the zone its `time_zone` names, so
  an inherited zone never converts twice.
* **Time zones**: the default is the context's (`FnContext::time_zone`).
  `timeZone=input` on a floating operand is *Bad Operand* and the context's
  zone is used. A floating value takes the target zone without conversion; a
  value with an offset or zone converts — to UTC or an offset by arithmetic,
  to a named zone through `Host::zone_offset`. A floating wall time placed in
  a named zone finds its offset from the zone's offsets a day before and
  after: in an overlap the earlier instant, in a gap the wall time moves
  forward by the gap (java.time's and Temporal's `compatible`), at most six
  host calls. Without zone data, converting an instant to a named zone is
  *Bad Option* and a fallback value (the alternative datetime.md allows) —
  including to the context's zone when it is a named one, so a server whose
  context names a zone needs `mf2-host-std`'s tz database (owner decision 1).
* **Unannotated** date/time values format as `:datetime` with no options
  (`DATES`, for `Registry::with_dates`), with the value's own override
  options.
* **The neutral stub backend** (no backend feature, and tests): the same text
  in every locale — `2006-01-02`, `--01-02`, `---02`, `Mon`…`Sun`, `15` /
  `15:04` / `15:04:06`, `03:04 PM` for `hour12=true`, `Z` / `±hh:mm` / the
  zone name for `timeZoneStyle` — pieces joined by a space; no sub-parts.
  A6's backends (`datetime-icu`, `datetime-intl`) plug into the same
  `Backend` seam and show `Plan::zone` (§5.2, "The date backends as
  built"); a backend adds only its own *Unsupported Operation*s (data it
  lacks, a range it cannot show), never a semantic error.
* **Without a backend feature** (`fn-datetime` alone) the statics format
  with the neutral backend; with `datetime-icu` the ICU4X backend over the
  catalog's `icu.blob`, which `mf2::compile_str` (and `mf2-build`) writes
  when the message formats a date or can receive one (02 §4.4); with
  `datetime-intl` alone, `Intl.DateTimeFormat` in the browser and ICU4X
  with compiled data elsewhere. With both, `datetime-icu` wins (the same
  text on server and client).

## 6. Formatting context and SSR parity

Hydration-safe output needs server and client to agree on *inputs*, not just
code:

* **Locale and direction** — negotiated on the server, stated in the boot data,
  carried in the catalog header.
* **Time zone** — the server cannot know it. `leptos-mf2` persists the client's
  IANA zone in a cookie on first load; SSR uses it when present and otherwise
  formats date/time in UTC **and marks the node for client re-render after
  hydration**. This holds for any backend.
* **"Now"** never enters a message implicitly; date arguments are always explicit.

## 7. Spec obligations with structural impact

Listed in [01-conformance](01-conformance.md) §6; the ones that shape code:

* **NFC via `Host`**: quick check (all code points < U+0300) then
  `host.nfc()`. No normalization tables in the wasm.
* **Default Bidi Strategy** is the default; `None` is offered. Needs message
  `dir` (catalog header) and each resolved value's `dir`.
* **Plural operands come from the formatted number** (`1` vs `1.0`), so number
  selection calls the same digit-resolution code as formatting.
* **Markup** produces no text in string output and structured parts in parts
  output; options on markup resolve like expression options. `u:id` is carried
  onto the part; **`u:dir` on markup emits *Bad Option* and is ignored**. The
  runtime never requires markup to be paired.
* **Names compare under NFC** (slots, NAMES, `write_named`), like keys.
* **Fallback output** uses NAMES from the catalog.

## 8. Error policy in applications

`leptos-mf2` installs an `ErrorSink` that is a no-op in release client builds
(errors cost nothing), logs to the console with the `diagnostics` feature in
dev, and logs through `tracing` on the server. Missing-message policy
(`Entry::Absent`) is configurable: render the fallback-locale text (default,
already flattened in at build time), render the id (dev), or render empty.
