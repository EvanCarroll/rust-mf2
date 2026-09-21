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

## 2. API sketch

```rust
pub struct Formatter<'c> { catalog: &'c Catalog, registry: &'c Registry, cx: &'c FormatContext }

pub struct FormatContext {
    pub bidi: BidiStrategy,          // Default (spec default) | None
    pub time_zone: Option<TimeZoneId>, // see §6
    pub host: &'static dyn Host,     // NFC; and the `Intl` date backend when `datetime-intl` is on
}

impl Formatter<'_> {
    /// Fast path: `Some(&str)` when the message is `simple` — no evaluator, no alloc.
    pub fn simple(&self, id: MsgId) -> Option<&str>;
    pub fn write(&self, id: MsgId, args: &[Arg<'_>], out: &mut impl Sink, errs: &mut impl ErrorSink);
    pub fn parts(&self, id: MsgId, args: &[Arg<'_>], out: &mut impl PartSink, errs: &mut impl ErrorSink);
    /// Dynamic, named arguments (CLI, server-side lookup-by-name, conformance `dyn` cases).
    pub fn write_named(&self, id: MsgId, args: &[(&str, Arg<'_>)], …);
}
```

* `args` are **positional slots** assigned by the manifest (see
  [05-tooling](05-tooling.md) §3). Names exist only in the catalog's NAMES
  section, used for `write_named` and for fallback output such as `{$name}`.
* `Sink` is a minimal `push_str` trait (not `core::fmt::Write`), implemented for
  `String` and for the Leptos SSR buffer adapter.
* `PartSink` receives `Text`, `BidiIsolation`, `Expression{source, parts…}`,
  `MarkupOpen/Standalone/Close{name, options}`, `Fallback{source}` — the shape
  `expParts` asserts. The Leptos layer consumes this to turn markup into elements.
* `ErrorSink` receives small `enum` values (the 13 suite error kinds plus
  `UnsupportedOperation` and `MessageFunctionError`). No strings. A
  `diagnostics` feature (server/dev only) adds message id, source span and
  human-readable text.

### Values

```rust
pub enum Arg<'a> {
    Str(&'a str), Int(i64), Float(f64), Decimal(&'a str), // number-literal text, exact
    DateTime(DateTimeValue<'a>),                          // Instant(epoch ms) | Floating{date, time} — the spec treats
                                                          // an operand without an offset as floating; + optional zone
    Custom(&'a dyn CustomValue),
    Unset,                                                // → unresolved-variable
}
```

Kept deliberately small: every variant is a code path in the wasm. Conversions
from Rust types (`From<i32>`, `From<&String>`, …) are `#[inline]` and live on
the call-site side.

## 3. Function registry — full spec, closed world

Every default function in the pinned spec is implemented, REQUIRED and
RECOMMENDED, Stable and Draft:

`:string` · `:number` · `:integer` · `:offset` · `:percent` · `:currency` ·
`:unit` · `:datetime` · `:date` · `:time` — plus `u:id`, `u:dir`, custom
functions, and namespaces.

What keeps this from bloating the client is **closed-world registration**
(budget B13): `mf2-build` records the set of functions the corpus uses; the
generated code builds the `Registry` from exactly those handlers; handlers are
ordinary `fn` items referenced from nowhere else, so dead-code elimination drops
the rest. The *library* is full-spec; each *app* links what it uses. Catalogs
resolve function names to registry indices once at load; a name the registry
lacks yields `unknown-function` + fallback, exactly as the spec says.

```rust
pub trait Function: Sync {
    fn resolve<'a>(&self, cx: &FnContext<'a>, operand: Option<Resolved<'a>>, opts: &Options<'a>)
        -> Result<Resolved<'a>, FnError>;
}
pub trait ResolvedValue {                 // what a function returns
    fn write(&self, out: &mut dyn Sink) -> Result<(), FnError>;
    fn parts(&self, out: &mut dyn PartSink) -> Result<(), FnError>;
    fn matches(&self, key: &str) -> Result<bool, FnError>;      // spec Match
    fn better_than(&self, k1: &str, k2: &str) -> bool;          // spec BetterThan
    fn as_operand(&self) -> OperandView<'_>;                    // value + inheritable options
    fn dir(&self) -> Dir;
}
```

The three `:test:*` functions of the suite are implemented in the conformance
crate against this public trait and nowhere else; if they cannot be, the trait
is wrong.

## 4. Crates

| Crate | Contents | Client? |
|---|---|---|
| `mf2-runtime` | evaluator, selection, fallback, bidi, parts, registry, the `Host` trait, `:string`, the plural-rule evaluator, and the **core numeric semantics**: `:number` / `:integer` / `:offset` — operand parsing, every digit and rounding option (over `fixed_decimal`), `signDisplay`, `select` with `exact` / `plural` / `ordinal`, and locale-neutral output. Depends on `mf2-model`, `mf2-catalog`, `fixed_decimal`. Numeric code is only linked when the corpus uses a numeric function (closed world) | yes |
| `mf2-fn-number` | the *localization* of numbers — symbols, grouping, numbering systems — plus `:percent`, `:currency`, `:unit`. Depends on `mf2-runtime` | feature `fn-number`, when used |
| `mf2-fn-datetime` | `:datetime :date :time`: semantics in Rust; text from ICU4X (`datetime-icu`, optional dependency `icu_datetime`) or from `Host` (`datetime-intl`). Depends on `mf2-runtime` only — it never names a host crate | feature `fn-datetime`, when used |
| `mf2-host-web` | `Host` for the browser: `String.prototype.normalize`; `Intl.DateTimeFormat` glue behind `datetime-intl`. Depends on `mf2-runtime`, `js-sys`, `web-sys` | client only |
| `mf2-host-std` | `Host` for native and for the `wasm32-wasip1` test run: a pure-Rust NFC normalizer. Depends on `mf2-runtime` | server / tests |

Function crates are split by **dependency weight**, not per function; within a
crate, closed-world linking does the fine-grained pruning.

## 5. Backends per function family (decision D4)

Measured by Phase 0 probes P0.4–P0.6 (deltas against a fair base,
`wasm-release`, `wasm-opt -Oz`, gzip -9; details in
[phase-0-results](phase-0-results.md)):

| Family | Own Rust, data in catalog | ICU4X in the browser | Browser `Intl` glue |
|---|---|---|---|
| Plural rules | **0.43 KB gz** evaluator, correct on all 15,041 CLDR 48 samples. Data per locale 0–84 B | 14.5 KB gz (runtime blob) – 17.9 KB gz (compiled data, which also lacks 75 CLDR locales) | not used: selection never uses the host |
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
| `fn-number` | the two options that are *only* about locale — `numberingSystem`, and `useGrouping` other than `auto`/`never` — emit *Unsupported Operation* and render neutrally. Any numeric formatting at all is a `check` **warning** (`neutral-numbers`, configurable to allow/deny). `:percent` / `:currency` / `:unit` in the corpus are a **build error** | locale symbols, grouping, numbering systems, `:percent`, `:currency`, `:unit`; data arrives lazily in the catalog. Measured 1.7 KB gz (+2.9 with `:currency` + `:unit`; P0.5) |
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
  panic formatting into std builds. Options — accept, fix upstream, or an own
  panic-free digit buffer (which D1's rule requires to come with a baseline,
  a gate and a fallback) — are an open owner decision; Phase 3 cannot exit with
  B12 red.
* **Dates (`fn-datetime`)**: the server always formats with ICU4X — so on a
  server build `fn-datetime` pulls in `icu_datetime` whichever client backend was
  chosen. The client backend is the app's
  choice between exactness (`datetime-icu`) and size (`datetime-intl`); P0.6
  and P0.10 measured both (above) and showed that text differences never break
  hydration. Three consequences: (1) `mf2-build` links ICU4X's **no-zone field
  set** whenever the corpus never uses `timeZoneStyle` — −29 KB gz of code and
  ≈ −85 % of `icu.blob`, since zone names dominate it; (2) ICU4X 2.x has **no
  time-zone transition rules**, so named zones (including the visitor's zone
  from the cookie, §6) need a tz database beside it on the server — which one
  is an open owner decision; (3) for `datetime-icu` byte identity the server
  formats from the same per-locale data the catalog's `icu.blob` carries.

Whatever is enabled, **the function semantics (options, operand rules, errors,
selection) are implemented once in Rust** in `mf2-fn-*`; a backend only supplies
the final "digits + symbols → text" or "instant + skeleton → text" step. Option
validation, inheritance, `select` handling and error emission never fork.

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
