# 04 — Leptos integration (`leptos-mf2`, `mf2-macros`, `mf2-axum`)

Part of the [master plan](00-master-plan.md). RFC 2119 keywords apply.

"Seamless" means: one macro, usable anywhere a string is usable in Leptos —
text child, attribute, component prop, plain `String` — with no `tr!` vs.
`move_tr!` distinction, reactive to locale changes by default, correct under
SSR, hydration, lazy routes and islands, and costing ≈ 25 B gz per call site on
the reference mix (P0.1; budget B5 ≤ 40).

## 1. What Phase 0 established (Leptos 0.8.20 / tachys 0.2.18)

Facts from reading the sources, the planning audit's type-checked prototype,
and Phase 0's browser-run probes P0.1, P0.2, P0.10 and P0.11
([phase-0-results](phase-0-results.md)):

| Finding | Consequence |
|---|---|
| A single concrete, non-generic `Tr` type implements `Render`, `RenderHtml`, `AddAnyAttr` (`no_attrs!`), `AttributeValue`, and `From<Tr>` for `TextProp` / `Signal<String>` / `Oco` / `String`, using public API only — and **runs**: SSR → preload → hydrate → switch → lazy route, 67/67 browser checks, zero console warnings (P0.2). | The whole design rests on this. |
| Marginal wasm per call site, weighted by the real mix, after `wasm-opt -Oz` (P0.1): **24.5 B gz** (35.7 against the harshest baseline) vs **97 B gz** for the closure-per-site control. The non-view `String` path (45 % of sites) and the prop/row shapes cost ≈ 0; **view positions cost 46–102 B gz** (text child 101.9, attribute 85.6, both about halved without arguments). The audit's "≈ 3 B gz" per view site holds only in raw bytes. | B5 is met. The view-position cost is tachys' per-block async hydration code compressing worse around a non-`&str` leaf, not our code; **P6 work item**: measure with `--cfg erase_components`, propose a tachys leaf hook that reuses `&str`'s state and async path, re-measure on tachys 0.3. |
| Hydrating from server HTML never compares or sets text or attribute content — confirmed in Chromium and Firefox, debug and release (P0.10). A **structural** difference (node is not a `Text`) makes stock tachys panic: the wasm traps and the page goes inert. | A text difference between server and client cannot break hydration (removes the risk from `datetime-intl`). `leptos-mf2` must walk the cursor itself and never panic (§3); markup structure must match (§7). |
| Effect per node (A): 427 B and 10 allocations per node on wasm32, and dropped effects stay subscribed until the source next fires (+72 B per churned node). Library registry (B): 44.8 B and ≈ 0 allocations per node, flat heap (P0.11). | D7 = B (§4). |
| A locale switch with 2,000 live nodes: 2.3–3.5 ms of script (6.8 ms at 4× CPU throttle) with the registry, 22.5 µs natively; the page's relayout of 2,000 changed texts costs more than the update itself. | Fine-grained update is fast enough; no need to remount the app. |
| Code-split chunks (`#[lazy]`, `cargo leptos --split`) share linear memory, statics, `TypeId`s and the `Owner` — confirmed: a lazy chunk renders `Tr` and switches live, registry slots are freed on drop inside the chunk (P0.2). | A root-level i18n state is visible inside lazy routes. |
| `shell()` runs inside the request owner after `additional_context`; it renders `<html lang dir>` and `<link rel="preload">` directly; route-list generation runs the app with mock request parts, the file handler in a bare owner (P0.2). | Negotiated locale comes from context; lookup falls back to the default locale rather than panic. |
| Render-time context lookup works for every `Tr` in all four `SsrMode`s; leptos_meta evaluates `<Title text>` outside the request owner (P0.2). | D9 as narrowed: derived conversions capture under `ssr` (§5). |
| Leptos 0.9 is in beta. The only rendering-trait break is `to_html_with_buf` taking a `RenderFlags` struct; text-separator rules move from `escape` to `flags.hydrate`. | Isolate all tachys coupling in one module; delegate the actual write to tachys' own `&str` impls so its rules stay its problem. |

## 2. Call-site API

```rust
// text child
view! { <h1>{tr!("welcome-title")}</h1> }

// attribute — any key: placeholder, title, aria-label, attr:…
view! { <input placeholder=tr!("search-placeholder")/> }

// component props: #[prop(into)] TextProp | Signal<String> | Oco<'static, str> | String
view! { <Button label=tr!("chat-send")/> }

// arguments are named at the call site, positional in the wasm
view! { <p>{tr!("greeting", name = user.name())}</p> }

// reactive argument
view! { <p>{tr!("users-online", count = count)}</p> }        // count: a signal

// plain String, anywhere (event handlers, server fns, format!)
let s: String = tr!("cmd-unknown", typed = input).to_string();

// markup → elements (§7)
view! { <p>{tr!("hotkey-release", kbd = |c| view! { <kbd>{c}</kbd> })}</p> }
```

`tr!` always returns a small **description** of a message (`MsgId` + positional
args). Nothing is formatted until something renders or stringifies it, in
whatever reactive context that happens. That is why one macro suffices.

**Where call sites really are** (measured on the reference workload, heuristic
classification, ± 5 points): about **half are non-view code that needs a plain
`String`** — match arms and function returns, error values set from event
handlers, deferred label closures in registries; about 20 % are text children
(most of them wrapped in `move ||` only because the old stack needed a closure
for reactivity); about 20 % are component props or HTML attributes; about 10 %
choose between two messages with `if`/`else`. So the `String` path is as
important as the view path, and gets the same per-site scrutiny (P0.1).

**Descriptions are data.** Because `Tr` is `Copy` and `const`-constructible, a
message can sit in a `const` or a static table:

```rust
const COMMANDS: &[Command] = &[Command { name: "pause", label: tr!("cmd.pause.label") }, …];
```

which replaces the "registry of closures that each return a `String`" pattern
with plain data, formatted when shown.

Compile-time checks (proc-macro, against the manifest — see
[05-tooling](05-tooling.md) §4): the id exists (did-you-mean otherwise); the
argument names equal the message's variables exactly; markup handlers, if the
call site gives any, cover every markup name of the message (§2.1); argument
types convert to `Arg`.

Expansion, by shape:

| Message | Expands to | Type |
|---|---|---|
| no variables, no markup | `$crate::__mf2::tr($crate::__mf2::MsgId::from_raw(123u32))` — a `const fn` | `Tr` — `Copy`, 4 bytes, identical on every target |
| variables | `$crate::__mf2::tr_args2(id, ArgValue::from(a), ArgValue::from(b))` | `TrArgs` — **one** concrete type: `MsgId` + an `ArgList` holding up to 4 values inline (the reference workload's maximum) and spilling to a boxed slice beyond that (`tr_args_n(id, [_; N].into())`). (P0.1's `tr_args` constructor was generic over the array length — 3 instances in the workload; the arity-specific `tr_args1`…`tr_args4` avoid even that, and only a call site with five or more arguments instantiates a generic.) |
| markup handlers | `$crate::__mf2::tr_rich(tr_args…, [(0x…u64, $crate::__mf2::markup(h))].into())` | `TrRich` — `TrArgs` plus type-erased handlers, each keyed by the **hash** of its markup name, not the name (§2.1) |

`$crate::__mf2` is a re-export in the application's generated i18n module, so the
expansion never hard-codes a crate name and downstream crates need no direct
dependency on ours.

No closure, no `String`, no `Signal`, no `HashMap`, no id string and no argument
or markup name is emitted at the call site.

### 2.1 The Leptos-free core (`mf2`), and its two extension points

**Where these types live** (owner, 2026-09-23; Phase 5b's owner question 1,
[13](13-phase-5b-work-order.md)). `Tr`, `TrArgs`, `TrRich` and `ArgValue` are
a **Leptos-free core in the facade** — `mf2::Tr`, `mf2::TrArgs`,
`mf2::ArgValue` — formatting against a catalog the caller supplies, so a
server, a test and `mf2-cli` can use them with no Leptos in the tree. L5
needs them in Phase 5b, before `leptos-mf2` exists.

`leptos-mf2` (Phase 6) then *adds* rendering, the catalog context and the
reactive argument; it does **not** re-declare `ArgValue`. `Reactive` is
therefore not a variant of the core enum.

**A1 (2026-09-23): each extension point is a trait, not the `Custom`
variant.** `Custom` stays what it is — an application value the runtime
borrows as `Arg::Custom(&dyn CustomValue)`, which is how a call site passes a
measure (`CustomValue::as_measure`), a date or a downcastable value. It
cannot also carry a signal: every `CustomValue` method returns a borrow of
`&self` (`as_str(&self) -> Option<&str>`), and a signal's value does not
exist until it is read, inside the observer that is formatting. Reading it
early would defeat the point, and caching it behind a lock cannot hand out
`&str`. So:

```rust
pub trait ArgSource: Send + Sync {          // ArgValue::Source(Arc<dyn ArgSource>)
    /// Its value now — called once per format, inside whatever reactive
    /// context is formatting, which is how a signal subscribes.
    fn arg_value(&self) -> ArgValue;
}

pub trait MarkupHandler: Send + Sync {      // TrRich's handlers
    /// The rendering layer's own handler, for the layer that knows its type.
    fn as_any(&self) -> &dyn Any;
}
```

`leptos-mf2`'s `impl From<Signal<T>> for ArgValue` wraps the signal in its own
`ArgSource`. One `ArgValue` across both crates, and the core resolves a
`Source` the same way wherever it formats — bounded, so a source that returns
a source cannot loop; it resolves to `Unset`, which is an Unresolved
Variable.

Markup handlers go through `markup(h)`, which the expansion names and which
therefore has to exist in both phases: Phase 5b's core takes anything that
already implements `MarkupHandler`, and Phase 6 supplies the one that takes a
view closure (a blanket impl for closures is impossible — `leptos-mf2` cannot
implement a foreign trait for a bare `F`). The **expansion does not change**
between the two, which is the point of fixing it here.

The owned, `'static`, `Send + Sync` call-site value covers every `Arg`
variant ([03-runtime](03-runtime.md) §2), borrowed into `Arg<'a>` at format
time — the runtime never sees a signal:

| `ArgValue` | → `Arg<'a>` | From |
|---|---|---|
| `Str(Text)` | `Str(&'a str)` | `&str` (copied — a call site's `&str` is rarely `'static`), `String`, `&String`, `Arc<str>`, `char`; `ArgValue::str_static` keeps a literal's `&'static str`, and that is what the macro emits for one |
| `Int(i64)` | `Int(i64)` | `i8`…`i64`, `u8`…`u32`, and `usize` (32-bit on the client; no server holds `i64::MAX` items) |
| `Float(f64)` | `Float(f64)` | `f32`, `f64` |
| `Decimal(Text)` | `Decimal(&'a str)` | `ArgValue::decimal(s)` — exact `number-literal` text |
| `DateTime(Arc<DateTimeValue>)` | `DateTime(&'a DateTime<'a>)` | `DateTime<'_>`, `DateTimeValue` (instant or floating, optional zone and calendar) |
| `Custom(Arc<dyn CustomValue + Send + Sync>)` | `Custom(&'a dyn CustomValue)` | `Arc<C>` for any `C: CustomValue + Send + Sync` |
| `Source(Arc<dyn ArgSource>)` | the resolved value's `Arg` | the extension point above |
| `Unset` | `Unset` | — |

`Text` is `Static(&'static str) | Shared(Arc<str>)`: `&'static str` costs
nothing, everything else is counted so that cloning a description — which a
re-format does on every locale change — never copies text. (`Oco` is Leptos',
so the core cannot use it; `leptos-mf2` converts, `Counted` to `Shared`,
without copying.)

Formatting is the same three methods on each of the three types, taking the
`Formatter` the caller built from *its* catalog, registry and context:
`write(&Formatter, &mut dyn Sink, &mut dyn ErrorSink)`,
`parts(&Formatter, &mut dyn PartSink, &mut dyn ErrorSink)` and `format(&Formatter)
-> String` (errors discarded). Phase 6 adds the ambient-catalog forms
(`to_string()`, `From<Tr> for TextProp`, …) on top of these, not beside them.

**Markup handlers are positional and keyed by a hash.** A rich call site
emits `(FNV-1a 64 of the markup name, handler)` pairs in the manifest's
ascending markup order; `TrRich::handler(name)` hashes the name the catalog
gives at render time (`MarkupPart::name`) and finds the pair. That keeps
markup names out of the wasm exactly as argument names are kept out (B6), and
the macro — which has every markup name of the message — rejects the
(astronomically unlikely) message whose two markup names collide, so the
lookup cannot be wrong. Handlers are supplied **all or none**: a message's
markup formats to parts with no handler at all, which is what the suite's own
markup tests assert at L5, but a call site that handles one markup name and
not its sibling is an oversight the macro reports ([05](05-tooling.md) §4).

## 3. Rendering

Implemented **once**, in `leptos-mf2`, for the concrete types:

* **SSR** (`to_html_with_buf`): resolve the per-request catalog from context **at
  render time** (§5); `simple` messages hand the borrowed `&str` to
  tachys' `<&str as RenderHtml>` — no `String` is built; patterns format into a
  reused scratch `String` first, then delegate (piecewise writes would break
  tachys' empty-text and `<!>` separator rules). Attributes delegate to
  `<&str as AttributeValue>::to_html`, which escapes.
* **Hydrate**: walk the cursor exactly as `&str` does, adopt the existing `Text`
  node, do not read the catalog, register for updates. **Never panic on a node
  that is not a `Text`**: stock tachys panics there (debug: `Unrecoverable
  hydration error`; release: `entered unreachable code`), the wasm traps and
  the page goes inert (P0.10), and its `failed_to_cast_text_node` is
  `pub(crate)` — so `leptos-mf2` walks the cursor itself, logs one diagnostic,
  and degrades (P0.2's glue did; hydration continued).
* **Client build / rebuild**: look up, set text or attribute. `rebuild` makes
  `move || if x { tr!("a") } else { tr!("b") }` work with no extra machinery.
* **Conversions**: `From<Tr…>` for `TextProp`, `Signal<String>`,
  `Oco<'static, str>`, `String`, plus `Display`-free `to_string()`. Each is one
  function in the library, not one per call site. Components that want the
  cheapest path take `#[prop(into)] label: TextProp`.
* Also implemented: `IntoProperty` (`prop:value=`), and `InnerHtmlValue` only if
  a real use appears.

All tachys-facing code lives in one module per supported tachys line
(`glue/tachys_0_2.rs`, later `glue/tachys_0_3.rs`), selected by feature.

## 4. Reactivity to locale change (decision D7; probe P0.11)

Client-side state is one thread-local: the active `Arc<Catalog>` and a change
notifier. Candidate strategies for translated nodes:

| Strategy | Per node | Locale switch | Notes |
|---|---|---|---|
| A. `RenderEffect` tracking one global `ArcTrigger` | 10 allocs, 427 B on wasm32 (797 B native) | 12–17 ms script for 2,000 nodes at 4× throttle | simplest; **leaks dead subscribers under churn** (+72 B per churned node on wasm32) until the next switch |
| **B. Library-owned registry** (**decided**, P0.11) | one slab slot `{node, MsgId, args, optional arg effect}`, 44.8 B and ≈ 0 allocs on wasm32; the view state is the 4-byte slot index; `Drop` frees it in O(1) | walk the slab synchronously in `set_locale`, then notify conversions: 6.8 ms script for 2,000 nodes at 4× | no effect, no `Owner`, no task per node; flat heap under 100k churn; reactive args still work because an enclosing closure drives `rebuild`, or the node's own argument effect |
| C. No live update — switch = cookie + navigation | nothing | full reload | a feature (`static-locale`) for apps that prefer it; natural fit for islands |

Signal-valued arguments (`count = count`) are the one place a per-node
subscription is inherent; `TrArgs` creates a single effect only when at least one
argument is reactive, and that effect tracks **only the arguments** — the
registry handles the locale. P0.1: this costs one library effect and no per-site
code, whereas the user writing `move || tr!(…, count = count.get())` creates a
closure type per site and costs what the old stack did — the documentation
steers users to the signal-valued form. The conversions (`TextProp`,
`Signal<String>`, `to_string()` under an observer) still subscribe to the one
trigger through the consuming component's effect; their behaviour inside
churning rows is a P6 follow-up.

Reading outside a reactive observer (event handlers, `format!`) MUST NOT warn:
tracking is attempted only when an observer exists; `tr_untracked!` is available
for the explicit case.

## 5. Where the catalog lives

| Target | Storage | Why |
|---|---|---|
| Client (wasm32, single-threaded) | `thread_local!` active catalog + notifier | no context walk per lookup (context lookups cost a lock + hash lookup per owner level, and every effect adds a level); shared across lazy chunks and islands |
| Server | **context**, provided per request from `additional_context`; a plain `Arc<Catalog>` from a process-wide cache — no signal | requests share threads; a global would be wrong. `Tr` stays a 4-byte `Copy` value on the server too and looks the catalog up when it renders. **P0.2 verified** that the request `Owner` is current whenever tachys renders a `Tr` (text and attribute) in all four `SsrMode`s, `Suspend` chunks included. What is *not* safe is a derived value a third party evaluates outside the owner — leptos_meta reads `<Title text>` after rendering, so under `PartiallyBlocked`/`Async` it saw no context. Therefore, under `ssr` only, `From<Tr>` for `TextProp` and `Signal<String>` **captures the request's `Arc<Catalog>` at conversion** (inside the owner); `Tr` itself stays `Copy`. Lookups with no context fall back to the default locale, never panic |

`leptos-mf2` exposes one function to install per-request state, and the docs
insist it be passed to **every** leptos_axum `_with_context` entry point
(`generate_route_list_with_…_and_context`, `leptos_routes_with_context` — which
also registers server functions through `handle_server_fns_with_context` —
and `file_and_error_handler_with_context`; three calls in practice). The
provider must tolerate missing request `Parts`: route-list generation runs with
mock parts, and the file handler calls it in a bare owner. With no request the
default locale is used.

## 6. Request flow: zero extra round trips

**Server**

1. `mf2-axum` negotiates the locale — cookie → `Accept-Language` → default, or a
   path-prefix strategy (`/es/…`) for sites that want crawlable per-locale URLs.
   The strategy is a trait. Responses carry `Content-Language` and the right
   `Vary`.
2. The shell renders `<html lang=… dir=…>` and
   `<link rel="preload" as="fetch" crossorigin href="/i18n/es.3fa9c1.mf2b" data-mf2>`.
   That link **is** the boot data: the client reads the URL from it and the
   locale from `<html lang>`. No inline script, so no CSP nonce; no JSON, so no
   serde in the wasm. The catalog downloads in parallel with the wasm.
3. `mf2-axum` serves `/i18n/*` from catalogs embedded in the server binary,
   picking the precompressed `.br`/`.gz` variant, with
   `Cache-Control: public, max-age=31536000, immutable`. It MAY also send the
   preload as a `Link` header.

**Client**

```rust
#[wasm_bindgen]
pub fn hydrate() { leptos_mf2::hydrate_body(App); }     // or hydrate_lazy / hydrate_islands
```

which initialises the executor, fetches the preloaded catalog, validates it
against `MANIFEST_HASH`, installs it, and only then hydrates. The gate is not
needed to avoid text mismatches (§1) but it **is** needed for (a) anything that
reads a plain `String` during hydration — `<Title>` runs a client effect and
would blank the document title — and (b) markup messages, whose node *structure*
comes from the catalog.

**Switching**: `i18n.set_locale("fr")` → fetch → validate → swap the
thread-local → notify → update `<html lang dir>` and the cookie. On failure the
old catalog stays and the error is returned. `i18n.preload_locale()` lets a
language menu warm the cache on hover. A manifest-hash mismatch (deploy skew)
triggers a reload, never a misread. **How the client learns another locale's
hashed URL is an open owner decision.** P0.2 used `GET /i18n/<tag>` → `307` to
the immutable URL (response `Cache-Control: no-cache`): one extra round trip,
at switch time only, and nothing in the page. The alternative is a small
tag → URL map emitted by SSR into the DOM (no round trip, a few bytes per
locale in every page).

**Time zone** for date functions: see [03-runtime](03-runtime.md) §6.

## 7. Markup → elements

MF2 markup (`{#kbd}…{/kbd}`, `{#icon/}`) is how a sentence carries inline
elements without baking word order into code. `TrRich` formats **to parts**
and builds a fragment: text parts become text nodes; `open…close` spans call the
handler for that name with the inner fragment as children; standalone markup
calls the handler with none. Handlers are `Fn(AnyView) -> AnyView`, type-erased:
rich messages are rare, so erasure is cheaper than monomorphisation.

* MF2 does **not** require markup to be paired, and the suite tests lone opens and
  closes. Pairing is our convention for building elements: a `check` **warning**
  (`unpaired-markup`, configurable) rather than an error. At render time
  `TrRich` pairs with a stack: a close with no matching open is dropped; an open
  that is never closed is closed at the end of the pattern; both raise a dev
  diagnostic. A markup *name* the source message lacks is a `check` error
  ([05-tooling](05-tooling.md) §3).
* Two handler kinds: **nesting** handlers (`Fn(AnyView) -> AnyView`, above) for
  applications, and a **flat** handler (`Fn(&MarkupPart) -> AnyView`), which
  renders each markup part as its own node with no pairing at all. The flat form
  is public API and is what conformance L6 uses to compare against `expParts`.
* Markup options are passed to the handler as a small read-only map
  (`{#link href=$url}`). `u:id` is passed through; `u:dir` on markup is a *Bad
  Option* and is ignored, as the spec requires.
* Structure comes from the catalog, so rich messages rely on the hydration gate;
  inside islands they need the catalog before the island hydrates — an explicit
  work item of the islands phase, not something to discover late.

## 8. Delivery modes

| Mode | Support | Notes |
|---|---|---|
| SSR + hydrate (cargo-leptos, Axum) | first-class, Phase 6 | everything above |
| …with `#[lazy]` routes / `--split` | Phase 6 | `hydrate_lazy`; state is shared with chunks |
| Islands | Phase 7 | server-only components cost **zero** wasm and need no client catalog; `hydrate_islands` cannot be gated, so the catalog loads alongside and notifies; strategy C is the natural default |
| CSR only (trunk) | Phase 7 | no server: locale from storage/`navigator.languages`; catalog URLs from a tiny generated index (`i18n/index.json`, preloaded from `index.html`) |
| Non-Leptos hosts (CLI, workers, other servers) | `mf2-runtime` directly | catalogs from disk |

## 9. Accessibility and SEO (WCAG 2.2 AA is a requirement, not a nicety)

* `<html lang dir>` is always correct and updates on switch (WCAG 3.1.1).
* **Language of parts (3.1.2)**: text borrowed from a fallback locale is in a
  different language from the page. The catalog flags such messages
  ([02](02-catalog-format.md) F7); the `mark-fallback-lang` feature (opt-in) renders them
  inside `<span lang="…">`, identically on server and client. `mf2 stats`
  reports fallback counts so the gap is visible either way.
* A reference `<LocaleSwitcher>`: a labelled native control, each language named
  in its own language with its own `lang` attribute, flexbox layout, no inline
  SVG.
* Head helpers: `<link rel="alternate" hreflang>` for path-prefix strategies;
  guidance for schema.org `inLanguage` on pages that emit structured data.
* Bidi: the spec's Default Bidi Strategy is on by default so interpolated names
  cannot scramble an RTL sentence. **Open for Phase 6: which strategy applies
  in which position.** The isolating marks (U+2066–U+2069) belong in displayed
  text; in a value the user or another program consumes as plain text — `value=`
  / `prop:value`, text copied to the clipboard, a `String` handed to a server
  function — they are invisible junk. `BidiStrategy` is a `FormatContext`
  field, so a renderer can hold one formatter per strategy and pick by
  position at no per-call-site cost; §11 says why this is on the list.

## 10. Version policy

Target the latest **stable** Leptos (0.8.x today). Track 0.9 betas in CI as
allowed-to-fail from Phase 6; add the `tachys_0_3` glue when 0.9 is released.
`leptos-mf2`'s major version follows Leptos' supported line.

`leptos-mf2` and `mf2-host-web` only *call* `js-sys` / `web-sys` APIs and define
no `#[wasm_bindgen]` items of their own (the `hydrate` export belongs to the
application), so `forbid(unsafe_code)` holds — P0.2's client crate built with it
for `ssr` and `hydrate`, and P0.6 showed a `#[wasm_bindgen] extern` block also
compiles under `forbid`. Versions verified end to end: Leptos 0.8.20,
leptos_axum 0.8.10, leptos_router 0.8.15, leptos_meta 0.8.6, tachys 0.2.18;
older 0.8.x releases were not tested (owner question for P6).

## 11. Prior art: leptos-fluent (audited 2026-09-23)

The owner authorized one reading of `leptos-fluent` 0.3.1 (Leptos 0.8, two
crates: a 2.2k-line runtime and a 9.5k-line proc-macro) outside the repository
boundary, for architecture only — nothing was copied. It is the closest thing
to this library that exists, so what it settles is worth having written down.
Sources are its own; the claims below were read in its tree.

**What it confirms, and we keep.**

| Our decision | What theirs shows |
|---|---|
| One macro, one `Copy` description (§2) | They have `tr!` → `String` *and* `move_tr!` → `Signal<String>` = `Signal::derive(closure)`: an `Arc`'d, non-memoizing closure per call site, re-formatting on every read. The split is a documented tripwire — a signal argument in a bare `tr!` is silently non-reactive. Our single `tr!` and a 4-byte `Tr` exist to avoid exactly this. |
| Positional arguments, resolved at compile time (§2.1) | Theirs builds a `HashMap<Cow, FluentValue>` **per formatting call**, and the catalog list sits behind a derived signal whose closure allocates a `Vec` per lookup. |
| Never panic when no catalog is in scope (§5) | Their `tr!` hides `expect_context::<I18n>()`, which panics outside the reactive ownership tree; the fix is a documented "pass the context as the macro's first argument" workaround and a second grammar for every macro arm. Our client state is a `thread_local!` and a missing catalog falls back to the default locale. |
| Lazily-loaded per-locale binary catalogs (master plan §2) | Theirs compiles **every** locale into the wasm — `static_loader!` embeds the `.ftl` sources, and the lookup iterates all loaders because that *is* their fallback chain. Message text, ids and argument names are all in the client. `format!("Unknown localization {id}")` puts the fmt machinery on the client path and renders that string into the page. There is no size measurement anywhere in the project, against a stated goal of being "the most performant internationalization framework available". |
| The manifest's path baked in, invalidated by `build.rs` (D8, 05 §4) | They tried `proc_macro::tracked_path`; **the nightly API was removed** and they fell back to emitting `include_bytes!` of every `.ftl` purely for rebuild tracking. A build script's `cargo::rerun-if-changed` is the only durable mechanism, which is what D8 uses. |
| Checks in the call-site macro, spanned at the call site (05 §4) | Their checker re-parses every `.rs` file under the workspace root on every expansion, with no caching, and reports one aggregated error spanned at the init macro's `check_translations:` argument; the offending `tr!` is named in prose, with line:column only on nightly. Their bidirectional check (an unused argument is as much an error as a missing one) is right, and ours does it. |
| `mf2 check` / `mf2-cli` own corpus-wide questions (05 §5–§6) | Theirs live in the proc-macro, and one of them (`fill_translations`) **writes to the source tree during expansion**. |

**What it adds to our list.**

1. **Markup is a real advantage, and the gap is worse than it looks.** Fluent
   has no markup, and their tree contains no answer: no `inner_html`, no
   wrapper component, no message-splitting helper. A sentence with a link in
   it has to be split into fragments and reassembled in the view, which is
   precisely the word-order bug i18n exists to prevent. §7 is therefore not a
   nicety; it is the feature. Keep the flat handler public (L6 needs it) and
   keep handlers cheap enough that a rich site is not a reason to avoid markup.
2. **Bidi isolation needs per-position control** (§9). Theirs is global: a
   `customise` closure reaching into `fluent-bundle` to call
   `set_use_isolating(false)`, so a user who wants clean `title` text loses
   isolation everywhere, including in the RTL sentences that need it. We can
   do better for free, and Phase 6 should decide it rather than inherit a
   global switch.
3. **Serialize the negotiated locale; never re-negotiate on the client.** They
   serialize nothing — the client re-runs negotiation at hydration and has to
   agree by luck. Their changelog carries a `hydrate` feature added and
   removed, a "re-render on hydration" hack added and removed, and repeated
   "fix hydration mode" entries. §6 already reads the locale from
   `<html lang>` and the catalog URL from the preload link, which *is* the
   serialized answer — Phase 6 must keep it that way, and its e2e must assert
   it (theirs added SSR end-to-end tests only after the hydration bugs).
4. **Islands foreclose themselves if ignored.** Theirs cannot support islands:
   the loader static and the context live in the shell. Our thread-local was
   chosen partly for this (§5) — P7 should prove it early rather than discover
   it.
5. **A negotiation matrix does not scale.** Theirs has ~60 hand-parsed
   `leptos_fluent!` parameters, including the full
   `initial_language_from_<source>_to_<target>` cross-product; the audit found
   a live copy-paste bug in that chain. `mf2-axum`'s strategy trait (§6) and
   `mf2.toml` (05 §3.1) should stay an ordered list of typed sources and
   sinks, never a boolean matrix. Their cheap insurance is worth stealing: a
   test asserting every configuration option has a section in the docs.
