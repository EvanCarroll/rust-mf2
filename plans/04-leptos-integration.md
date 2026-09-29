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
| Marginal wasm per call site, weighted by the real mix, after `wasm-opt -Oz` (P0.1): **24.5 B gz** (35.7 against the harshest baseline) vs **97 B gz** for the closure-per-site control. The non-view `String` path (45 % of sites) and the prop/row shapes cost ≈ 0; **view positions cost 46–102 B gz** (text child 101.9, attribute 85.6, both about halved without arguments). The audit's "≈ 3 B gz" per view site holds only in raw bytes. | B5 is met. The view-position cost is tachys' per-block async hydration code compressing worse around a non-`&str` leaf, not our code; **P6 work item**: measure with `--cfg erase_components`, propose a tachys leaf hook that reuses `&str`'s state and async path, re-measure on tachys 0.3. *Closed by Phase 7 A8 without a proposal: with this library's glue the whole mix costs 11.9 B gz a site against a `&str` leaf and the case Phase 6 made for the hook was a benchmark bug ([15](15-phase-7-work-order.md) §"A8").* |
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

### 2.1 The call-site types in `mf2`, the Leptos layer as `mf2::leptos`, and the two extension points

**Where these types live** (2.0: owner, 2026-09-28, master plan D16 and
D20; built by Phase 10's B1, [18](18-phase-10-work-order.md)). `Tr`,
`TrArgs`, `TrRich`, `TrDyn`, `ArgValue`, `ArgList`, `ArgSource`,
`DateTimeValue`, `Text` and the markup traits are **defined once, in
`mf2`**, with only additive impls behind features — the one arrangement
that survives cargo's feature unification
([phase-6-results](phase-6-results.md), "A hazard found the hard way"). A
build with no mode compiles no Leptos code: the types format against a
catalog the caller supplies, so a server, a test, `mf2-cli` and a native
application use them as they are ([19](19-native-and-terminal.md) §4).

**Where their Leptos impls live.** The orphan rule decides it: `impl Render
for Tr` needs the trait or the type to belong to the implementing crate,
`Render` is tachys', so the impl is in the crate that defines `Tr`. The same
holds for `AttributeValue`, `IntoProperty`, `From<Tr> for TextProp` and
`From<Signal<T>> for ArgValue`. They are the Leptos layer, the module
`mf2::leptos` (`crates/mf2/src/leptos/`), compiled with a mode (`ssr`,
`hydrate`, `csr`) and a line (`leptos` for 0.9, `leptos-0-8`); §12.1 has the
features. Inside `mf2` each line is a dependency under a name of its own
(`leptos_0_9`, `tachys_0_3`, …, `leptos_0_8`, …), reached through a private
alias module (`crate::line`): the module `leptos` has that name at the
crate root, so no crate is bound to it there (Phase 10 A7).

**The six components** (the switcher, its options, the preload and catalog
links, `hreflang`, the islands gate) are written with `view!` and
`#[component]`, which write `::leptos` into the crate that uses them. They
live in one helper crate per line, `mf2-leptos-ui-0-9` and
`mf2-leptos-ui-0-8`, each compiling the same `src/ui.rs` (the 0.8 crate's is
a link to the 0.9 crate's) against its line under the name `leptos`. The
helpers cannot depend on `mf2` (Cargo forbids the cycle), so each component
is generic over the helper's `Layer` trait — what it reads from `mf2`: the
languages, the page's own, the preload and link URLs, the client's switch —
and `mf2` implements it for a type of its own (`mf2::leptos::components::Mf2`,
hidden) and wraps each component in a function that takes the helper's
props with that type chosen, so `view!` builds it as any component. The
calls resolve when the component is compiled: nothing is installed at
start-up and nothing goes through a function pointer (B1 measured this
against A7's function table: [18](18-phase-10-work-order.md), B1's record).

**`leptos-mf2`** is a shim: it depends on `mf2`, forwards its features
(`leptos-0-9`, its default, to `mf2`'s `leptos`; `leptos-0-8`; the modes;
`static-locale`, `mark-fallback-lang`, `fn-datetime`), and re-exports
every item under 1.x's path. `mf2::leptos_mf2` (hidden) is 1.x's path
through the facade, which the examples, the book and `mf2 init` name until
they move to `mf2::leptos`.

**1.x, for the record.** Phase 5b put the core in the facade; Phase 6 moved
its declaration into `leptos-mf2`, re-exported by `mf2`, because the orphan
rule kept the types with their `Render` impl and Leptos had to be an
optional part of that crate (a `leptos` feature, off in the core). 2.0
reverses the arrow: `mf2` owns the types and the layer, and `leptos-mf2`
depends on it.

The two extension points, as built:

* **`markup(h)`** is one name whose accepted forms the mode chooses, so
  that the macro's expansion never changes. Without a mode it takes
  `IntoMarkupHandler`'s core forms: `Handler(h)` over a
  `MarkupHandler + 'static` the caller wrote (Phase 5b's core took it
  bare), or an `Arc<dyn MarkupHandler>` already built. With one it also
  takes a **nesting** closure `Fn(AnyView) -> impl IntoAny` (the common
  case — `|c| view! { <kbd>{c}</kbd> }` infers with no annotation) and a
  `Flat` closure over `&MarkupPart` (§7; what L6 compares against
  `expParts`). Both erase to one of two concrete handler types, so the
  renderer downcasts to a type it knows rather than to the call site's
  closure.
* **`From<Signal<T>> for ArgValue`** for each of `reactive_graph`'s
  readable signals — `Signal`, `ReadSignal`, `RwSignal`, `Memo` and their
  `Arc` forms — through one `SignalArg<S>`: one instance per signal type,
  not per call site. A **disposed** signal reads as `Unset`, which the
  message reports as an Unresolved Variable; `Get::get` would panic, and
  the client path does not panic.

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

The Leptos layer's `impl From<Signal<T>> for ArgValue` wraps the signal in its own
`ArgSource`. One `ArgValue` whatever the mode, and the core resolves a
`Source` the same way wherever it formats — bounded, so a source that returns
a source cannot loop; it resolves to `Unset`, which is an Unresolved
Variable.

Markup handlers go through `markup(h)`, which the expansion names and which
therefore has to exist in both phases: Phase 5b's core takes anything that
already implements `MarkupHandler`, and Phase 6 supplies the one that takes a
view closure (a blanket impl for closures was impossible while `leptos-mf2` held the layer apart from the core — it could not
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
so the core cannot use it; the Leptos layer converts, `Counted` to `Shared`,
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

All tachys-facing code lives in one glue module (`glue/view.rs`). What
differs between tachys lines is switched inside it by feature: today only the
two `to_html_with_buf` impls, whose 0.3 form takes `RenderFlags`
(owner, 2026-09-24 — a second copy of the module would make every later glue
change twice). Since Phase 8 A0 tachys 0.3 (Leptos 0.9) is the default and the opt-in
feature `leptos-0-8` selects the 0.2 line (Leptos 0.8), owner 2026-09-24,
§10. *(Until A0 the switch was `tachys-0-3`, opting into 0.9.)* If a later 0.3 changes more than a few
methods, the line-specific part moves into a module of its own.

## 4. Reactivity to locale change (decision D7; probe P0.11)

Client-side state is one thread-local: the active `Arc<Catalog>` and a change
notifier. Candidate strategies for translated nodes:

| Strategy | Per node | Locale switch | Notes |
|---|---|---|---|
| A. `RenderEffect` tracking one global `ArcTrigger` | 10 allocs, 427 B on wasm32 (797 B native) | 12–17 ms script for 2,000 nodes at 4× throttle | simplest; **leaks dead subscribers under churn** (+72 B per churned node on wasm32) until the next switch |
| **B. Library-owned registry** (**decided**, P0.11) | one slab slot `{node, MsgId, args, optional arg effect}`, 44.8 B and ≈ 0 allocs on wasm32; the view state is the 4-byte slot index; `Drop` frees it in O(1) | walk the slab synchronously in `set_locale`, then notify conversions: 6.8 ms script for 2,000 nodes at 4× | no effect, no `Owner`, no task per node; flat heap under 100k churn; reactive args still work because an enclosing closure drives `rebuild`, or the node's own argument effect |
| C. No live update — switch = cookie + navigation | nothing — except a node with a reactive argument, which keeps its slot and argument effect (Phase 7; §8) | full reload | a feature (`static-locale`) for apps that prefer it; the documented default for islands |

Signal-valued arguments (`count = count`) are the one place a per-node
subscription is inherent; `TrArgs` creates a single effect only when at least one
argument is reactive, and that effect tracks **only the arguments** — the
registry handles the locale. P0.1: this costs one library effect and no per-site
code, whereas the user writing `move || tr!(…, count = count.get())` creates a
closure type per site and costs what the old stack did — the documentation
steers users to the signal-valued form. The conversions (`TextProp`,
`Signal<String>`, `to_string()` under an observer) still subscribe to the one
trigger through the consuming component's effect.

**Under churn (Phase 7 A5).** `reactive_graph` 0.2 removes an effect from a
source's subscriber set only when the effect re-runs. A dropped effect stays
there until the source next fires. Measured in `bench/churn`, both of the
remaining subscriptions leaked ≈ 70 B per churned row on wasm32:

* a conversion's consumer, until the next locale switch;
* a node's argument effect, until the application's signal changed, which
  may be never.

Both are closed:

* the conversions subscribe through `track_locale()`, which also registers
  an owner cleanup that unsubscribes. That cleanup runs before each re-run
  and at disposal, since every observer has an owner of its own. It costs
  48–64 B per *live* consumer.
* the argument effect clears its sources when its slot drops it.

Application code that wants to follow the locale calls `track_locale()`,
not `changed().track()`. `cargo xtask churn` holds every row shape flat
over 100,000 churned rows.

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

> **Changing in 2.0** (owner, 2026-09-28; master plan D21, D22). The default
> negotiation becomes `?lang=` → cookie → `Accept-Language`, and every locale
> match goes through one CLDR-based matcher. The rest of this section is
> unchanged; [18](18-phase-10-work-order.md) C3, D2 and D3. The 2.0 design is §12.5 and
> [19](19-native-and-terminal.md) §9.

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

**A failed boot does not hydrate** (Phase 7). If the catalog cannot be
fetched or is malformed, the boot logs one `mf2:` line and leaves the served
HTML as it is: readable, not interactive. Phase 6 hydrated against no catalog
instead, which blanks every message on its next update and, on a page with a
markup message, walks a structure the document does not have and traps the
wasm (P0.10) — so it bought an interactive page that says nothing, at best.
A catalog from another deploy still reloads (F6). `tools/e2e/checks/demo.mjs`
asserts the failure path (`failed-boot-*`).

**Switching**: `i18n.set_locale("fr")` → fetch → validate → swap the
thread-local → notify → update `<html lang dir>` and the cookie. *(Until
Phase 7 A13 the live switch wrote no cookie and left a `?lang=` in the
address, so a reload came back in the old locale; it now writes the
`mf2_locale` cookie `CookieLocale` reads by default and removes the query
with `history.replaceState`. `demo.mjs` asserts both, and the reload.)* On failure the
old catalog stays and the error is returned. `i18n.preload_locale()` lets a
language menu warm the cache on hover. A manifest-hash mismatch (deploy skew)
triggers a reload, never a misread. **How the client learns another locale's
hashed URL is an open owner decision.** P0.2 used `GET /i18n/<tag>` → `307` to
the immutable URL (response `Cache-Control: no-cache`): one extra round trip,
at switch time only, and nothing in the page. The alternative is a small
tag → URL map emitted by SSR into the DOM (no round trip, a few bytes per
locale in every page).

**Both are built (Phase 6 A4), and an application chooses by what its shell
emits.** `catalog_url(tag)` looks, in order, at the page's own
`<link rel=preload data-mf2>` (the locale the page was rendered in), then at a
`<link rel="mf2-catalog" data-mf2-locale="fr" href="…">` map, and falls back
to `GET /i18n/<tag>`. A shell that emits the map pays its bytes in every page
and never makes the extra request; one that does not pays the redirect at
switch time only. The measurement on the reference workload goes in
[phase-6-results](phase-6-results.md) for the owner to pick the default from;
nothing in the library prefers one.

**Time zone** for date functions: the precedence, the cookie, its
validation and the page's statement are [03-runtime](03-runtime.md) §6.1.
The client's side (Phase 8 A7, designed 2026-09-25):

* **Formatting zone on the client** is one `thread_local!` beside the
  active catalog, read where the `FormatContext` is built
  (`state::context_for`); on the server the request's zone is set around
  each format from `RequestI18n` (a format is synchronous, so a scoped
  thread-local is exact and costs no second context lookup).
* **`hydrate_body` / `hydrate_lazy`.** The boot reads P, the page's zone
  (`data-mf2-zone`, else `Setup`'s), and R, the reader's
  (`reader_time_zone` of the browser's name). Hydration runs in **P**, so
  everything formatted while hydrating — a markup message's structure, a
  `<Title>`, a `String` — agrees with the served HTML, and no `mf2:`
  mismatch can arise. If R is known and its name is not P's, then **after**
  the synchronous hydration: the zone becomes R; each text, attribute and
  property node hydration adopted is formatted in P and in R and written
  only where the two differ, and each markup node is rebuilt (tachys
  rewrites only the text that changed); the conversions' trigger fires, so a
  `TextProp` or `<Title>` re-reads; the cookie is written. A node that
  hydrates later — a lazy route's chunk, a `Suspense` — is compared the
  same way as it hydrates. After that, P is not used again. *(As built:
  the nodes come from a queue the hydration glue fills while the page
  hydrates, not from a walk of the registry, because under `static-locale`
  most nodes never register; `hydrate_lazy` is `hydrate_from_async`,
  awaited, so that "after" is after the lazy chunks too.)*
* **Which nodes change:** exactly those whose text depends on the zone,
  measured by the comparison; every other node is not written at all (the
  browser check observes it with a `MutationObserver`).
* **`mount_to_body`** (client-only): the zone is R before anything mounts.
  No cookie, no statement — there is no server.
* **`hydrate_islands`**: there is no moment after hydration (the island
  walk is Leptos'), so the zone is R before the walk and each island's node
  is compared as it hydrates — written in place after being adopted, which
  moves no hydration cursor. This needs no registry slot, so it holds under
  `static-locale` too. **Server-only components are never hydrated**: on a
  first visit their dates stay in P until the next page load, which the
  cookie then renders in R. The documentation says so.
* **A live switch** afterwards formats in R like everything else.

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
  inside islands they need the catalog before the island hydrates, which the
  islands gate (§8) provides. Without it, an island holding a markup message
  traps — the islands browser check keeps that control case.

## 8. Delivery modes

| Mode | Support | Notes |
|---|---|---|
| SSR + hydrate (cargo-leptos, Axum) | first-class, Phase 6 | everything above |
| …with `#[lazy]` routes / `--split` | Phase 6 | `hydrate_lazy`; state is shared with chunks |
| Islands | Phase 7 (A1) | server-only components cost **zero** code in the wasm (`cargo xtask islands-zero`); strategy C (`static-locale`) is the documented default; the **islands gate** below makes every island hydrate against the page's catalog |
| CSR only (trunk) | Phase 7 (A2) | no server: locale from storage → `navigator.languages` → default; catalog URLs from a tiny generated index (`i18n/index.json`, written by `mf2 compile --site`, preloaded from `index.html`); `leptos_mf2::mount_to_body` with the same gate |
| Non-Leptos hosts (CLI, workers, other servers) | `mf2-runtime` directly | catalogs from disk |

**What the documentation leads with (owner, 2026-09-23).** SSR +
hydrate first — the live switch with no reload is the headline, and it is
how most Leptos applications are built — then islands, as the smallest
download, with `cargo xtask islands-zero`'s measurement. In every mode a
translation edit leaves the wasm byte-identical, so a fix leaves every
reader's cached client alone (05 §4). An application with a server gets
**one i18n crate** (`Emit::Both`), which already achieves that — the
catalog names in its module are server-only (owner, 2026-09-23, Phase 7
question 6, which revised question 2's "catalogs apart by default" after
A6 measured it). A client-only application publishes its catalogs apart
(`Emit::Module` + `mf2 compile --site`), because it has no server to embed
them in; that publishing step must reject features that disagree with the
i18n crate's.

**Islands (Phase 7 A1).** Leptos' island script calls the entry point and
walks the document for islands in the same turn —
`mod.hydrate(); hydrateIslands(document.body, mod)` — without awaiting the
first, so `hydrate_islands` has no moment to await the catalog in, and
"cannot be gated" was right. But the walk **does** await an island whose
function returns a promise, and walks islands one at a time in document
order. So the gate is an island: `<IslandsGate/>`, empty and first in
`<body>`, whose export (`leptos_mf2::islands_gate!()`, written once in the
application because this crate forbids the `unsafe` a `#[wasm_bindgen]`
export expands to) resolves when the catalog is installed. Every island after
it hydrates against the page's catalog, markup included; it costs no page
bytes and no request (the fetch reuses the preload, which started before the
wasm did). A failed load never resolves the gate, so the islands stay as
served, as in §6. The alternative an earlier draft took — inlining the whole
catalog into every page as base64 — was dropped unmeasured: it puts a
catalog's worth of uncacheable bytes into every navigation.

**`static-locale` (strategy C), as built.** A switch writes the
`mf2_locale` cookie `mf2-axum` reads by default, removes a `?lang=` from the
address (the query source outranks the cookie), and navigates. Nothing
registers for the locale — except a node with a **reactive argument**, whose
argument effect needs an owner: before Phase 7, `static-locale` registered
nothing at all, so a signal-valued argument never re-formatted and a closure
that swapped one message for another never updated. Both work now, and the
islands check asserts the first.

**CSR (Phase 7 A2), as built.** `leptos_mf2::mount_to_body(App)` spawns
the boot and mounts only when it succeeds — the same gate as
`hydrate_body`, for the same two reasons (a `String` read at mount, and
markup structure), and here also so that the first frame is already in the
chosen locale.

* **The locale** is the first of: the tag remembered in `localStorage`
  (`mf2_locale`), each of `navigator.languages`, `navigator.language`, the
  source locale — each matched by `lookup_locale`, the RFC 4647 lookup
  `mf2-axum` negotiates `Accept-Language` with (moved into this crate so
  that the two sides cannot disagree). A remembered tag the build no longer
  has is ignored. `<html lang dir>` is set before mounting.
* **The catalog's URL** comes from `i18n/index.json`, `{"<tag>": "<file>"}`
  with each file relative to the index, which `index.html` preloads
  (`<link rel=preload as=fetch crossorigin data-mf2-index href=…>`; without
  the link the boot fetches `i18n/index.json` relative to the page). The
  browser parses it (`Response.json()`), so no JSON parser enters the wasm.
  Boot costs two serial requests — the index, then one catalog — against
  SSR's one; the index starts with the wasm, not after it.
* **Publishing.** `mf2 compile --site DIR` writes only what a static host
  serves: the catalogs (with `.br`/`.gz`) and the index, no manifest or
  module. The example runs it as a trunk `post_build` hook into the staged
  site, and its i18n crate emits `Emit::Module`, so the wasm names no
  catalog. The hook's `--features` must equal the i18n crate's.
* **A switch** is live (strategy B), and on success writes the tag to
  `localStorage`, so a reload comes back in it. Under `static-locale` a
  client-only switch writes the tag and reloads — there is no cookie
  reader. Where storage is unavailable, a choice lasts until the page
  closes.
* **A failed boot** (no index, no catalog, a malformed one) logs one `mf2:`
  line and mounts nothing; a manifest mismatch reloads, as in §6.
  `tools/e2e/checks/csr.mjs` asserts all of it against `examples/demo-csr`.

## 9. Accessibility and SEO (WCAG 2.2 AA is a requirement, not a nicety)

* `<html lang dir>` is always correct and updates on switch (WCAG 3.1.1).
* **Language of parts (3.1.2)**: text borrowed from a fallback locale is in a
  different language from the page. The catalog flags such messages
  ([02](02-catalog-format.md) F7); the `mark-fallback-lang` feature (opt-in) renders them
  inside `<span lang="…">`, identically on server and client. `mf2 stats`
  reports fallback counts so the gap is visible either way.

  **Built (Phase 7 A14, 2026-09-24; [15](15-phase-7-work-order.md)
  §"A14 — design", [phase-7-results](phase-7-results.md) §A14).** Only a *borrowed* message in a view position is
  wrapped (with `dir` when the lender's direction differs); an own message
  stays a bare text node. Hydration adopts the shape the server wrote
  rather than asking the catalog; on a switch the wrapper comes and goes
  around a text node that keeps its identity. Attributes and strings cannot
  carry a `lang` of their own and are documented as unmarked.
* A reference `<LocaleSwitcher>`: a labelled native control, each language named
  in its own language with its own `lang` attribute, flexbox layout, no inline
  SVG.

  **Decided (Phase 7 A11, owner, 2026-09-24): a choice applies on a button,
  never on `change`.** The keyboard fires a `<select>`'s `change` on every
  arrow key, so switching on it changed the page's language per keypress —
  and under `static-locale` reloaded it, dropping focus (WCAG 3.2.2, F37).
  The switcher is a `<form method="get">`: the `<select
  name=LOCALE_QUERY>` inside its `<label>` (no fixed `id`, so a page may
  have two), and a submit button whose text the application supplies. With
  no client code the form's `GET ?lang=…` is the switch (`QueryParam`
  negotiates it, and the cookie follows), so it works before the wasm
  loads, after a failed boot, and on an islands page without an island.
  Under `hydrate` and `csr` the submit is intercepted and becomes the live
  `set_locale`. A reader pays one extra action; an application supplies one
  more message. The option of the page's locale is `selected` in the markup, so
  the form submits the right language before any code runs. Built and
  audited in Phase 7 A11 ([phase-7-results](phase-7-results.md)).
* Head helpers: `<link rel="alternate" hreflang>` for path-prefix strategies;
  guidance for schema.org `inLanguage` on pages that emit structured data.
* Bidi: the spec's Default Bidi Strategy is on by default so interpolated names
  cannot scramble an RTL sentence. The isolating marks (U+2066–U+2069) belong in
  displayed text; in a value the user or another program consumes as plain text —
  `value=` / `prop:value`, text copied to the clipboard, a `String` handed to a
  server function — they are invisible junk. `BidiStrategy` is a `FormatContext`
  field, so a renderer can hold one formatter per strategy and pick by
  position at no per-call-site cost; §11 says why this is on the list.

  **Decided (Phase 6 A2, 2026-09-23; owner question 2).** The library holds
  two formatters, built once by `install`, and every position picks one:

  | Position | Strategy | Why |
  |---|---|---|
  | text child, `AttributeValue` (`title`, `placeholder`, `aria-label`, `alt`), markup parts, `TextProp`, `Signal<String>`, `Oco` | **Default** (isolated) | a person reads it, and an interpolated name must not scramble the sentence around it |
  | `IntoProperty` (`prop:value`), `to_string()`, `String::from` | **None** (plain) | a program consumes it: a server function, a comparison, the clipboard, `format!` — *`to_string()` and `String::from` superseded by the Phase 7 revision below* |

  The split follows *who reads the text*, not which Rust type carries it:
  `TextProp` and `Signal<String>` are display props, while a bare `String`
  is what a call site hands to code. A call site overrides per use, not
  globally — `to_display_string()` is the isolated form of `to_string()` —
  which is the thing the prior-art audit (§11, item 2) found missing: a
  global switch makes a user who wants clean `title` text lose isolation in
  the RTL sentences that need it. Nothing here costs a call site anything:
  the choice is which of two `&'static FormatContext`s a library function
  passes to `Formatter::new`.

  **Refined (Phase 7 A9, owner, 2026-09-24): an attribute is decided by its
  name.** The table above put every `AttributeValue` on the isolated side,
  but some attributes are read by a program, not a person: `value=` is
  submitted with a form, `download=` is a file name, `data-*` is read by
  scripts. An attribute's name reaches every place its text is made —
  `to_html(key)`, `build(el, key)`, `hydrate(key, el)` and the registry's
  `Target::Attribute(el, key)` on a switch — so the library picks the
  strategy from it, identically on server and client:

  | Attribute | Strategy |
  |---|---|
  | `value`, `href`, `src`, `srcset`, `action`, `formaction`, `poster`, `cite`, `download`, `id`, `name`, `for`, `form`, `list`, `class`, `type`, `data-*` | **None** (plain) |
  | every other name — `title`, `alt`, `aria-*`, `placeholder`, `label`, `content`, … | **Default** (isolated) |

  The rule is an ASCII case-insensitive match on the name, a fixed cost with
  nothing per call site. A developer gets plain text where a program reads it
  without knowing the marks exist. A **text child** has no name to decide by
  (a `<textarea>`'s starting text, a `<script>` body), so it stays isolated,
  and the documented way to get it plain is `move || tr!(…).to_plain_string()`
  (plain, and it follows a switch through `track_locale`; ≈ 546 B a live
  consumer against ≈ 113 B for a text node, Phase 7 A5; `to_string()` until
  the revision below). A per-site
  `plain(…)` wrapper for view positions was offered and not chosen.

  **Revised (Phase 7 A12, owner, 2026-09-24): a string is isolated by
  default, as the spec requires.** `formatting.md` makes the Default Bidi
  Strategy the default "when formatting a message as a single string", and
  the spec coverage matrix found `to_string()` / `String::from` plain by
  default — a departure from a MUST that Phase 6 had decided on usability
  grounds without it being put as a spec question. Now:

  | API | Strategy |
  |---|---|
  | `to_string()`, `String::from`, `to_display_string()` (kept, a synonym) | **Default** (isolated) — the spec's default for a single string |
  | `to_plain_string()` (new) | **None** (plain) — the call a developer makes when a program consumes the text: a server function, a comparison, the clipboard, `format!` |

  View positions are unchanged: a text child is isolated, an attribute
  follows its name (above), `IntoProperty` is plain — there the library picks
  among the strategies the spec allows (`MAY` supply others, including one
  that does nothing) by position, and a developer does not format a string.
  Users who hand a translated `String` to code switch to
  `to_plain_string()`, or get invisible U+2066–U+2069 in it; an RTL reader
  gets correct text wherever a `String` ends up displayed. The user
  documentation (A13) says so.

## 10. Version policy

> **Changing in 2.0** (owner, 2026-09-28; master plan D20, D23). The lines
> become features of `mf2`: `leptos` for 0.9, the default line, and
> `leptos-0-8` for 0.8. 0.8 stays supported, and changing the default line
> stays a major. `leptos-mf2` is folded into `mf2`. The policy below holds
> for 1.x; [18](18-phase-10-work-order.md) B1 and G1. The 2.0 design is §12.1.

**Leptos 0.9 is the default line, whether or not it is released** (owner,
2026-09-24: "that was supposed to be the default"; 0.9.0-beta / tachys
0.3.0-beta2 is the newest on crates.io at that date). The workspace, the
examples, the conformance layers and the user documentation build on it; a
later 0.9 pre-release or release is taken as it appears.
**Leptos 0.8 stays supported as an opt-in** (owner, 2026-09-24): a feature
selects the tachys 0.2 glue (§3), it is documented to users, and CI builds
and tests it beside 0.9 — not allowed-to-fail. Superseded: "target the
latest stable, track 0.9 betas nightly as allowed-to-fail, undocumented
until release" (Phase 7).

*As built (Phase 8 A0):* `leptos-mf2` and `mf2-axum` have features
`leptos-0-9` (default) and `leptos-0-8`; an application on 0.8 writes
`default-features = false, features = ["leptos-0-8"]` on both. The 0.8
crates are workspace dependencies under renamed keys (`leptos_0_8`,
`tachys_0_2`, `reactive_graph_0_2`, `leptos_axum_0_8`) and each crate root
renames them back (`extern crate leptos_0_8 as leptos`), so one source
serves both lines and both are in the one lock file. Both lines at once, or
none, is a `compile_error!` naming what to write. The `mf2` facade picks no
line: it depends on `leptos-mf2` without default features, so the line is
the application's own `leptos-mf2` dependency's. The requirement
`"0.9.0-beta"` admits every later pre-release and the release, so a new 0.9
arrives by `cargo update`. CI: the nightly job `leptos-0-8` (`cargo xtask
leptos-0-8`) lints and tests `leptos-mf2`, tests `mf2-axum` and runs layer
L6 on 0.8; `cargo xtask docs` compiles Getting started's application on
0.8 as well (`hello-0-8`). `leptos-fluent` 0.3.1 requires Leptos < 0.9, so
the `fluent-view` template and its converted twin `fluent-converted` stay
on 0.8. `leptos-mf2`'s major version follows its default
Leptos line. *The first release is 1.0.0 on the 0.9 beta (owner,
2026-09-25): a later 0.9 pre-release or the release is a patch, a new line
an opt-in in a minor, a change of default line or a dropped line 2.0 —
written for users in `docs/versioning.md` ([17](17-phase-9-work-order.md)
A3).*

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

## 12. 2.0: the web side's design (Phase 10 A8; approved by the owner, 2026-09-28)

Designed on 2026-09-28 by Phase 10's A8 ([18](18-phase-10-work-order.md)),
with the native side and the shared parts in
[19](19-native-and-terminal.md). The tasks that build it are B1 (the Leptos
layer into `mf2`), C3, C4 and D1–D6. As they land, each rewrites the 1.x
section it replaces: §2.1, §5, §6 and §10. `hello`'s exact code is 19 §1.4,
and its UX target is 19 §2.

### 12.1 `mf2::leptos`, and the two lines

- **Features** (19 §3):
  - `leptos` is the 0.9 line, the default line; it is not a default feature;
  - `leptos-0-8` is the 0.8 line;
  - exactly one of `ssr`, `hydrate` and `csr`.

  Two modes, both lines, or a mode with no line are each a `compile_error!`
  that says what to write.
- **An application writes the line once**, on its `mf2` dependency
  (`features = ["leptos"]`), and the mode where it writes Leptos's own
  (`hydrate = ["leptos/hydrate", "mf2/hydrate", …]`). A 0.8 application
  writes `leptos-0-8`, and turns off no default features.
- **The module**:
  - the six components, re-exported from the helper crate of the active line
    (question 13), with their props;
  - `html_lang`, `islands_gate!`, `Setup`, `LoadError`, `track_locale`;
  - the boots: `hydrate_body`, `hydrate_lazy` and `hydrate_islands` (`hydrate`),
    `mount_to_body` (`csr`);
  - `RequestI18n` (`ssr`);
  - the untyped `set_locale(&str)` and `preload_locale(&str)`, which the
    generated typed forms call (§12.2).
- **Inside `mf2`** (A7):
  - each line is reached through a private alias module;
  - `::axum` is written at the crate root;
  - there is no root rename.

  The helper crates reach `mf2` through static dispatch: each component is
  generic over the helper's `Layer` trait, and `mf2` implements it (§2.1).
  B1 measured it against the function table the components would install
  (A7's v3) in the three demos and kept it, the cheaper in each and the one
  within the gate ([18](18-phase-10-work-order.md), B1's record).
- **Applications** import items from `mf2::leptos`, and never `use
  mf2::leptos;` beside the `leptos` crate (A7, N5).

### 12.2 What the generated module gives a web application

From 19 §10. The items are the same names as a native application's, with the
same signatures, so code shared between the two sides needs no `#[cfg]`:
- **`Locale`**: typed, and parsed through the one matcher (19 §9);
- **`install()`, on each side**, returning nothing:
  - on the server it installs the setup and the embedded catalogs, each
    checked once;
  - in the browser it records the setup, which `mf2::leptos::hydrate_body`
    (and each boot) reads. A boot with nothing installed logs one `mf2:` line
    and leaves the page as served (§6's failed boot);
- **`setup()`**: still generated, for a boot of one's own;
- **`set_locale(Locale)` and `preload_locale(Locale)`**, on both sides:
  - in the browser, a spawned switch or fetch, whose failure is logged once
    (`mf2:`) and leaves the page as it was;
  - on the server, nothing: the next request's cookie decides;
- **`current_locale() -> Locale`**:
  - in the browser it is reactive: reading it in a view or an effect subscribes
    to the next switch, through `track_locale`;
  - on the server it is the request's language;
- **`Locale::name()`**: the `language.<tag>` message;
- **`Locale::format(&message)`**: on the server;
- **the prelude**.

There is no translation crate and no feature block: `links` carries `mf2`'s
features to the build (19 §11).

### 12.3 The switcher and its options

- **`<LocaleSwitcher label=… button=…/>` with no children lists every
  language**, in `Locale::ALL`'s order: one `<option value=tag lang=tag>` each,
  named by its `language.<tag>` message. The option of the page's language is
  `selected`, as in 1.x.
  - `setup()` carries each language's name message, and the helper renders it
    through the table.
  - A corpus that names some languages but not all gets a build warning. A
    language without a name shows its tag.
  - Adding a language needs no code.
- **Children make a list of one's own**: `<LocaleOption tag=Locale::Fr>…
  </LocaleOption>`. `tag` takes the generated `Locale`, which converts into the
  helper's tag type, and still a `&'static str`.
- **Each option carries its own language's `lang`** (§9). So E2's finding
  does not arise inside a switcher: under `mark-fallback-lang`, a
  `@do-not-translate` name borrowed from the source's catalog would otherwise
  be wrapped in `<span lang="en">`. An option's text takes no span; its `lang`
  is the language's own.
- **The query name comes from the installed query source** (D2), not a
  hard-coded `lang`.
- **The accessibility contract is unchanged** (§9): a `GET` form, the
  `<select>` inside its `<label>`, a submit button, no switch on `change`.

### 12.4 Switching, and the current language, without `#[cfg]`

Review finding #12. 1.x's `SwitchButton` (`docs/switching.md`) needed an
`#[cfg(feature = "hydrate")]` and an `#[cfg(feature = "ssr")]` form of each
helper. In 2.0:

```rust
#[component]
pub fn SwitchButton(lang: Locale, children: Children) -> impl IntoView {
    view! {
        <button
            type="button"
            lang=lang.tag()
            on:click=move |_| set_locale(lang)
            on:pointerenter=move |_| preload_locale(lang)
            on:focus=move |_| preload_locale(lang)
        >
            {children()}
        </button>
    }
}
```

1.x's `current_language()` helper becomes `move || current_locale().tag()`.

**Markup closures need no annotation** (review #10, D4):
`tr!("hotkey", kbd = |c| view! { <kbd>{c}</kbd> })` infers in every position.
D4 finds the positions where 1.x needed `|c: AnyView|`, and removes the need.

### 12.5 The server

- **`Negotiator::default()`** (D2) is `?lang=` (the switcher's query name),
  then the cookie, then `Accept-Language`. Each goes through the one matcher.
  The cookie sink is `Secure` except in a debug build, as 1.x's pages wrote by
  hand.
- **`Negotiator` is a tower `Layer`** (D3, a probe first). `.layer(
  Negotiator::default())`:
  - negotiates each request, and puts the `Negotiated` in the request's
    extensions;
  - writes `Content-Language`, `Vary` and the cookie on the response.

  The Leptos render finds the result through the request `Parts` that
  leptos_axum provides in the context. A render with no request (route
  listing, a bare owner) uses the source language, as in 1.x. So
  `leptos_routes` and `file_and_error_handler` are Leptos's plain forms: no
  `_with_context`, and no silent default-language page when one entry point
  misses the context. Fallback: keep the context wiring (19 §14).
- **`catalog_routes()`** is unchanged. The generated `install()` replaces
  `mf2_axum::install(setup(), CATALOGS)`.
- **Plain Axum**, with no Leptos, has three things:
  - the same layer;
  - `Locale` as an extractor;
  - `locale.format(&tr!(…))`.

  A per-request ambient language, so that `{}` works in a plain handler, is
  later (19 §13).

### 12.6 `Display` and `Debug` in a browser build

Question 14, as 19 §6 states it:
- `Display` pads the text `to_string()` builds (A9's S3): `{}` costs 25–70 B gz
  over `.to_string()`, and a wrapper's `.to_string()` 60–100 B gz;
- `Debug` goes through `write_str` (A9's S2): `{:?}` on a description with
  arguments costs about 1 KB gz, against 11.8–16.5 KB derived.

The book says `.to_string()` is the leanest. The demos keep a nightly check
that neither is linked (D6).

### 12.7 What becomes of §2–§10

- **§2.1:** the types return to `mf2`; rewritten by B1.
- **§3, §4, §7, §8 and §9:** unchanged in substance.
- **§5:** the storage is unchanged. The lookup order is 19 §5's: the request or
  the page first, then the native store where `native` is also on.
- **§6:** the defaults of §12.5, and the one matcher (C3).
- **§10:** the lines become features of `mf2` (§12.1; B1, G1).
