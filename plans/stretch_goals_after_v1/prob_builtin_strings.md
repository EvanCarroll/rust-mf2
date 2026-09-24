# Stretch goal after v1 — JS strings for catalog text (JS String Builtins)

Status: **deferred until after v1** (decided by the owner, 2026-09-21). Not part
of any phase. This document records the idea, what was verified, why it is
cheap to defer, the seams v1 keeps so it stays cheap, and how to re-evaluate
it. Part of the [master plan](../00-master-plan.md) (§9, "Later"); evidence
cited from [phase-0-results](../phase-0-results.md).

## 1. The idea

Today message text reaches the page as UTF-8 in wasm linear memory and is
converted to a JS string by wasm-bindgen's glue (`TextDecoder`) **each time**
the client writes it into the DOM. The idea: have catalog text exist as JS
strings, created once, so a DOM write passes a reference and converts nothing.

Two forms, in the order they should be considered:

| | A. Lazy `JsString` cache | B. Catalog v2 container: imported string constants |
|---|---|---|
| What | `leptos-mf2` (client only) keeps the JS string created by the first DOM write of each catalog string, keyed by `StrRef`; later writes reuse it; a locale switch replaces the cache | each locale's catalog is a tiny wasm module whose import section declares one `(import "s" "<text>" (global (ref extern)))` per distinct text string; the engine creates the JS strings at instantiation |
| Catalog format | unchanged | new major `format_version` |
| When strings are created | on first client-side write, only for strings written | all at load, by the engine, from the file's UTF-8 import names |
| Needs | nothing new | JS String Builtins' *imported string constants* in the browser, or the fallback below |

## 2. How catalogs travel in v1 (the baseline)

([04](../04-leptos-integration.md) §6, [02](../02-catalog-format.md) §2–3,
[05](../05-tooling.md) §4; verified end to end by P0.2.)

`mf2-build` writes one binary `.mf2b` per locale (+ `.br`/`.gz`), embedded in
the server binary only; `mf2-axum` serves it at `/i18n/<locale>.<hash>.mf2b`,
immutable. The server renders the text into the HTML and emits `<link
rel="preload" as="fetch" data-mf2>`; the client `fetch()`es it (reusing the
preload), copies it into wasm memory, validates the structure in place,
installs it, and hydrates — adopting the server's text nodes.

| Step | Copy / conversion | Measured (P0) |
|---|---|---|
| network → `ArrayBuffer` | the browser's | — |
| `ArrayBuffer` → wasm memory | one memcpy per locale | 0.16 ms at 4× CPU throttle (P0.8) |
| `Catalog::new` | none (validated in place, 0 allocations) | 0.08 ms at 4× (P0.8) |
| hydration | **none** — server text adopted (P0.10) | — |
| a client-side DOM write of a message | one UTF-8 → JS decode of that string | share of a locale switch unknown (see §6) |
| `to_string()` (45 % of call sites) | a memcpy inside wasm memory; no JS | — |

So the only per-string cost either form removes is the decode on client-side
writes: locale switches, nodes created on the client (CSR, lazy-route
navigation), reactive updates, attributes. Page load with SSR has none today.

## 3. What was verified (2026-09-21)

**wasm-bindgen 0.2.128 / js-sys 0.3.105** (latest at the time), by building
small crates and reading the wasm import section and the JS glue (throwaway
crates in `target/scratch/`):

* `js_sys::JsString` does **not** use the builtins: `length`, `charCodeAt`,
  `slice`, `concat`, `fromCharCode` and `==` are each an ordinary import from
  the JS glue module; `JsString::from(&str)` goes through `TextDecoder`,
  `String::from(&JsString)` through `TextEncoder.encodeInto`. No
  `wasm:js-string` import, no `builtins` compile option.
* `#[wasm_bindgen(thread_local_v2, static_string)]` is wasm-bindgen's own
  mechanism, **not** the proposal: the literal moves into the generated JS
  (`function() { const ret = \`…\`; return ret; }`), is fetched once through an
  ordinary import and cached as an `externref`. It needs the literal at compile
  time, so it cannot carry lazily loaded, per-locale text (goal 2; B6).
* wasm-bindgen's changelog and the `JsString` docs mention no builtins support;
  the one related upstream item is open enhancement issue #4285 (2024-11-22,
  "Directly import `JsString` literals from JavaScript").

**The proposal** — `WebAssembly/js-string-builtins`, `proposals/js-string-builtins/Overview.md`
(read at the owner's request; the repository boundary in `CLAUDE.md` otherwise
excludes it, so re-reading it needs the owner's OK):

* `wasm:js-string` builtins: `cast`, `test`, `fromCharCodeArray`,
  `intoCharCodeArray`, `fromCharCode`, `fromCodePoint`, `charCodeAt`,
  `codePointAt`, `length`, `concat`, `substring`, `equals`, `compare`. The array
  conversions take **WasmGC `(array (mut i16))`**, not linear memory; UTF-8 and
  linear-memory variants are deferred to future namespaces
  (`wasm:text-decoder`/`wasm:text-encoder`). So the builtins do not convert
  Rust's UTF-8 in linear memory.
* **Imported string constants**: compile option `importedStringConstants: "<ns>"`;
  a module declares `(import "<ns>" "<the string>" (global (ref extern)))` —
  the field name *is* the string — and the engine creates the global's value.
  "If all imports in a module are from the imported string namespace, no import
  object needs to be provided." **Any module** can declare them — including a
  separately, lazily fetched catalog module. This is what makes form B possible.
* Options go in the compile-options dictionary of `compile`,
  `compileStreaming`, `instantiate(Streaming)` and `validate`
  (`{ builtins: ['js-string'], importedStringConstants: 's' }`); engines without
  support ignore them (WebIDL extra-member rules), so a polyfill path is always
  possible.

Not verified: current browser support for either feature; engine limits on the
number of imports and on instantiation cost with thousands of string-constant
globals.

## 4. Form B, sketched

* `mf2-build` emits each locale as a small wasm module (plain bytes; no Rust
  compile): an import section with one string-constant global per distinct
  **text** string (message text, text parts — identifiers, keys and names stay
  in the binary pool, since Rust compares them), an element segment + exported
  `externref` table over those globals, and the v1 structure (INDEX, MESSAGES,
  NAMES, FUNCS, LOCALE, …) in a custom section. `StrRef` for text becomes the
  import index.
* The client loads it with `WebAssembly.compileStreaming(fetch(url), { builtins:
  ['js-string'], importedStringConstants: 's' })` (served as
  `application/wasm`; the preload stays `as="fetch"`), reads the structure from
  the custom section, and fetches string handles through the exported table.
* **Fallback** for engines without support (an inference to be tested, not
  something the proposal specifies): build the import object from
  `WebAssembly.Module.imports(module)`, whose names the engine has already
  decoded — `new WebAssembly.Global({ value: 'externref' }, name)` per entry.
* **One format for all targets**: the import section *is* a length-prefixed
  UTF-8 string pool, so the server and the native runtime (and L4 on native and
  `wasm32-wasip1`) read it as their pool, and the browser client may also copy
  the file into wasm memory for the `String` path. Text then exists twice at
  runtime (wasm memory + JS heap) but travels once.

## 5. Benefits, costs, unknowns

Benefits (B over v1): text converted once, by the engine, straight from the
file; a DOM write passes a reference (no glue decode); no per-string work in
our code at load. A over v1: the same per-write saving for every string written
more than once, with no format change.

Costs and unknowns:

1. **B creates every string at load**; with SSR, hydration needs none of them —
   page load gets a little worse, switches/CSR/reactive updates better. A has
   no load cost.
2. **Rust cannot hold `externref` directly on stable**, and wasm-bindgen does
   not support the builtins or string constants: each access from Rust is a
   glue call returning a handle (cheaper than a decode, not free); pattern
   output built with `concat` is a glue call per join until wasm-bindgen
   supports the builtins.
3. **The `String` path** (45 % of call sites) needs UTF-8 in wasm memory —
   keep a copy (B's "one format" layout) or pay `TextEncoder` per call.
4. **Catalog size (B7)**: import names are length-prefixed; P0.7 measured
   length-prefixed strings at +1.5–3.2 KB gz per locale against NUL-terminated
   ones, and the synthetic `pl` already sits at 26.6 KB gz (B7's scaled rule
   holds; the 25 KB reference figure is `en`'s 20.5 KB). Plus a few constant
   bytes per import.
5. **Load time (B9)** with ~1,600 string-constant imports at 4× throttle, and
   engine import-count limits for very large apps.
6. **Leptos glue**: DOM writes by handle instead of delegating to tachys'
   `&str` impls (04 §3's principle) — our own write path, in the one glue
   module; server rendering is unaffected (no JS on the server).
7. **Browser support** and the fallback must pass L6 in Chromium and Firefox,
   including a forced-fallback run.
8. **Headroom is unmeasured**: P0.11's locale switch took 6.8 ms of script
   for 2,000 live nodes at 4× throttle, against 57 ms of layout; how much of
   the 6.8 ms is string conversion is not known.

## 6. Why deferring is cheap

* The catalog is an internal build artifact: a stable public binary format is
  a non-goal (00 §1); readers reject unknown `format_version` majors (02 F9);
  catalogs are rebuilt with the app, content-hashed, and checked against the
  wasm's `manifest_hash`. A v2 container needs **no data migration** — old
  cached files are simply never requested again.
* The evaluator, the `tr!` expansion and per-call-site cost (B5) are untouched
  by either form.

## 7. Seams v1 keeps (the insurance)

Designed in during Phases 2–6; each costs close to nothing if done from the
start and makes a later switch local:

1. **`StrRef` is opaque** in `mf2-catalog`'s public API — nothing outside the
   reader assumes "byte offset into a NUL-terminated pool" (02 §5).
2. **`Sink` has a catalog-text method** — e.g. `push_catalog_text(&Catalog,
   StrRef)`, defaulting to `push_str(catalog.text(r))` — so a JS-string sink or
   the cache (form A) needs no evaluator change (03 §2). `Formatter::simple`
   gets a variant that yields the `StrRef`, not only the `&str`.
3. **All DOM writes stay in one glue module** (04 §3, `glue/view.rs`) —
   already the plan.
4. **Catalog loading is one function** behind `hydrate_body` / `set_locale` —
   the only place that knows "fetch + copy + `Catalog::new`".

Also: commit before C5, so P0.7's encoder and P0.8's browser harness stay
recoverable from history as starting points.

## 8. When and how to re-evaluate

**Triggers** (any one):

* wasm-bindgen gains support for the builtins or direct `externref` (then
  Rust-side access and `concat` get cheap);
* Phase 6 measurements show string conversion is a meaningful share of
  locale-switch or CSR-mount time;
* a user workload with heavy client-side rendering (CSR, islands, long lists)
  makes per-write conversion visible.

**Order**: form A first (a Phase 6/7 optimization, no format change), then
form B as the step after, if A's measurements leave something to gain.

**Probe** (throwaway, `probes/`, built on the real `mf2-catalog`
writer/reader; roughly the size of P0.7 + P0.8 together), each against v1:

| Measure | Where | Gate |
|---|---|---|
| catalog size per locale, raw / gz / br | size gate, reference + generated locales | B7 (scaled rule) |
| load: `compileStreaming` + instantiate + structure install, desktop and 4× | browser harness (P0.8 style) | B9 |
| locale switch, 2,000 live nodes, script and layout, 4× | P0.11 harness | better than v1 |
| CSR mount and lazy-route navigation, reference app | P0.8/P0.11 harness | better than v1 |
| `to_string()` cost and allocations | native + browser | no worse than v1 |
| fixed wasm + JS glue size | size gate | B1 |
| correctness incl. forced fallback | conformance L6, Chromium + Firefox; L3/L4 for the container | 100 % |

**Adopt only if no worse than what exists** (the project's rule): v1's path
stays as the measured baseline and as the fallback.

**If adopted**, the work touches known places: `mf2-catalog` writer and reader
(new major version), `mf2-build` (emit), `mf2-axum` (`application/wasm`),
`leptos-mf2` boot (`compileStreaming`) and DOM-write glue (handles), conformance
reruns of L3, L4 and L6, and the size gate. `tr!`, the evaluator and B5 do not
change.
