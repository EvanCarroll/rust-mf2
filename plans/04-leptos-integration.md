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
argument names equal the message's variables exactly; each markup name has a
handler; argument types convert to `Arg`.

Expansion, by shape:

| Message | Expands to | Type |
|---|---|---|
| no variables, no markup | `$crate::__mf2::tr(MsgId(123))` — a `const fn` | `Tr` — `Copy`, 4 bytes, identical on every target |
| variables | `$crate::__mf2::tr_args(MsgId(7), [ArgValue::from(a), ArgValue::from(b)])` | `TrArgs` — **one** concrete type: `MsgId` + an `ArgList` holding up to 4 values inline (the reference workload's maximum) and spilling to a boxed slice beyond that; never a const generic. (P0.1's `tr_args` constructor was generic over the array length — 3 instances in the workload; arity-specific constructors avoid even that.) |
| markup | `$crate::__mf2::tr_rich(MsgId(9), args, [handler…])` | `TrRich` — type-erased handlers, renders a fragment |

`$crate::__mf2` is a re-export in the application's generated i18n module, so the
expansion never hard-codes a crate name and downstream crates need no direct
dependency on ours.

`ArgValue` is the **owned, `'static`** call-site value —
`Str(Oco<'static, str>)`, `Int`, `Float`, `Decimal`, `DateTime`, `Custom`, and
`Reactive(Signal<ArgValue>)` for signal-valued arguments. It is borrowed into the
runtime's `Arg<'a>` ([03-runtime](03-runtime.md) §2) at format time; the runtime
never sees a signal.

**Where these types live** (owner, 2026-09-23; Phase 5b's owner question 1,
[13](13-phase-5b-work-order.md)). `Tr`, `TrArgs`, `TrRich` and `ArgValue` are
a **Leptos-free core in the facade** — `mf2::Tr`, `mf2::TrArgs`,
`mf2::ArgValue` — formatting against a catalog the caller supplies, so a
server, a test and `mf2-cli` can use them with no Leptos in the tree. L5
needs them in Phase 5b, before `leptos-mf2` exists.

`leptos-mf2` (Phase 6) then *adds* rendering, the catalog context and the
reactive argument; it does **not** re-declare `ArgValue`. `Reactive` is
therefore not a variant of the core enum: the core carries `Custom`, and
`leptos-mf2` supplies the signal through it (`impl From<Signal<T>> for
ArgValue`), which is what keeps one type across both crates. Phase 5b's A1
fixes whether that extension point is the `Custom` variant or a trait, and
records it here.

No closure, no `String`, no `Signal`, no `HashMap`, no id string and no argument
names are emitted at the call site.

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
  cannot scramble an RTL sentence.

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
