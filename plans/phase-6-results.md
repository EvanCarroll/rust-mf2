# Phase 6 results — Leptos and Axum (layer L6)

What Phase 6 built and what it measured, against
[14-phase-6-work-order](14-phase-6-work-order.md). Every figure comes with
the command that produced it; where a figure moved a budget or a ledger
status, the commit that moved it says why (master plan §11).

## Status

**Phase 6 has not exited.** Six of the seven exit criteria are met; the
seventh — B5 on the full mix (A7) — is not, and §A7 says exactly what it
needs. `current_phase` therefore stays `P5b` in the ledger: the conformance
harness *is* green at P6 (nothing is overdue at any phase), but the exit
commit that moves the marker is the one that closes A7.

| Exit criterion | Verdict |
|---|---|
| L6 green in both configurations, SSR **and** hydrate, every L6d cell recorded, none `xfail` | **met** — L6 324/324, L6d 255/324 with the same 69 documented degradations as L5d; the browser half 20/20 in two engines |
| the browser run is in CI (or a nightly job CI gates on), in at least two engines | **met** — the nightly `l6-web` job, Chromium and Firefox |
| B5 met on the full mix, or restated with the owner | **not met** — §A7: measured on the `String` half (Phase 5b's 12.6 B gz), unmeasured on the view half |
| owner questions 1–3 answered and recorded in 04 | **met** — §6 (question 1), §9 (question 2), §10 (question 3) |
| the size gate runs in CI | **met** — `cargo xtask size`, the nightly `b5` job; the whole app measures **45,348 B gz** against the 105,120 the ambition allows (§A9) |
| `plans/phase-6-results.md` and the Phase 7 work order written | **met** — this file and [15](15-phase-7-work-order.md) |

The sixth column of the ledger is green either way: what A7 owes is a
**measurement**, not a behaviour. Nothing in the tree is known to be over
budget; what is missing is the number that would say so.

## A1 — `leptos-mf2`, and where the description types live

The crate exists, with the Leptos layer behind a default-off `leptos`
feature. One thing had to change from [04](04-leptos-integration.md) §2.1,
and the reason is not a preference:

> Rust's orphan rule keeps a type and its `Render` impl in one crate. `impl
> Render for Tr` needs either the trait or the type to belong to the
> implementing crate, and `Render` is tachys'. The same holds for
> `AttributeValue`, `IntoProperty`, `From<Tr> for TextProp` and
> `From<Signal<T>> for ArgValue`.

So the description types are **declared in `leptos-mf2` and re-exported by
`mf2`**. Everything the owner's Phase 5b decision asked for still holds:
`mf2::Tr` is this type, an application names one crate, and a build without
the feature compiles no `leptos`, no `tachys` and no `reactive_graph`.
Splitting a type from its rendering is not expressible in Rust; splitting
them by *feature* is, and that is what the crate does. 04 §2.1 records it.

**The two extension points.**

* `markup(h)` is one name and one signature, `IntoMarkupHandler`, in both
  builds. With `leptos` it takes a **nesting** closure `Fn(AnyView) -> impl
  IntoAny` — `|c| view! { <kbd>{c}</kbd> }` infers with no annotation — or a
  `Flat` closure over `&MarkupPart` (§7; what L6 compares against
  `expParts`); without it, a `Handler(h)` the caller wrote. The impls are
  coherent, which was not obvious and was checked before the design was
  fixed.
* `From<Signal<T>> for ArgValue` for each of `reactive_graph`'s readable
  signals through one `SignalArg<S>` — one instance per signal *type*, not
  per call site. A disposed signal reads as `Unset` (an Unresolved
  Variable): `Get::get` would panic, and the client path does not panic.

**A hazard found the hard way.** `markup` first had two signatures chosen by
the feature. The moment `mf2-axum` joined the workspace, cargo's feature
unification turned `leptos` on for every crate built with it, and
`crates/mf2/tests/call_site.rs` stopped compiling — a test broken by an
unrelated crate's existence. One signature in both builds is not tidiness;
it is the only arrangement that survives unification.

## A2 — rendering

`Render`, `RenderHtml`, `AddAnyAttr`, `ToTemplate`, `AttributeValue` and
`IntoProperty` for `Tr`, `TrArgs`, `TrRich` and `TrDyn`, plus `From` into
`TextProp`, `Signal<String>`, `Oco<'static, str>` and `String` — each
written **once** against a `Description` trait, never once per call site.
All of it is in `glue/tachys_0_2.rs`, so `tachys_0_3.rs` beside it is the
0.9 port.

Two things keep it off the per-site budget: a `simple` message hands tachys
the catalog's **borrowed** `&str` (no `String` in between, exactly as a
literal), and everything else formats into a scratch `String` the crate
reuses, so a locale switch over 2,000 nodes allocates once rather than
2,000 times. Writing is delegated to `<&str as RenderHtml>` throughout,
which keeps tachys' empty-text and `<!>` separator rules tachys'.

```
cargo test -p leptos-mf2 --features ssr --test render
  → 12 passed
```

The twelve cover: a simple message, escaping, arguments, the bidi
difference between a text child and a `String`, an attribute value, markup
as elements, the two pairing rules, a rich message with no handler, the D9
`TextProp` capture, and a lookup with no catalog at all.

## A3 — hydration that cannot break the page

The cursor is walked here rather than by tachys, because tachys'
`failed_to_cast_text_node` is `pub(crate)` and traps the wasm on a node that
is not a `Text` — P0.10's finding, where the page goes inert. A mismatch now
logs one message and binds a detached node.

**It was demonstrated by accident, in our own code.** `<LocaleSwitcher>`
first rendered its `<option>`s on the server only, on the theory that a
client view with no children would leave them alone. It does not: tachys
walks the cursor for the element's own end marker, finds an `<option>`, and
traps — exactly the structural mismatch P0.10 documented. The browser check
caught it as *one registered node instead of fifteen*, with the rest of the
page still showing served HTML. Our own text hydration, on the same page,
did not trap.

## A4 — the catalog, the registry, the switch

D7's strategy **B**, as P0.11 measured it: one slab slot per rendered
description — the target, the description, and an argument effect **only**
when an argument is reactive — with the four-byte slot index as the whole
view state and `Drop` freeing it in O(1). `set_locale` walks the slab
synchronously, then notifies the derived conversions.

A rich message is the one node that rebuilds rather than rewriting a string,
because its *structure* comes from the catalog; its fragment is shared with
the slab through one `Rc`, and rich messages are rare (§7).

The client boot is §6's: the locale from `<html lang>`, the catalog's URL
from the preload link, the fetch reusing that preload. The demo check
measured **exactly one** catalog request, `initiatorType: "link"`.

**Two behaviours the browser checks corrected.**

1. A `String` did not follow a locale switch. About half of real call sites
   produce a `String` (04 §2), and `move || label(x.get())` in a view is how
   it is ordinarily done. `to_string()` now subscribes to the locale change
   **when there is an observer**, so such a closure re-runs after
   `set_locale`; with no observer — an event handler, `format!` — it
   subscribes to nothing, which is what §4 requires.
2. A signal argument warned when read outside an observer. `Get::try_get`
   warns there, and a description is formatted outside one whenever it is
   built, hydrated or stringified. `SignalArg` reads untracked when nothing
   is watching and tracks when something is — the same rule from the other
   side.

## A5 — `mf2-axum`

Negotiation is an **ordered list of typed sources and sinks**, never a
boolean matrix (§11 item 5). `LocaleSource` and `LocaleSink` are traits;
`CookieLocale`, `AcceptLanguage`, `PathPrefix` and `QueryParam` implement
them; configuration is the order they are listed in. The first source that
offers a locale this build has wins. Matching is RFC 4647 lookup: the tag,
then the tag with its last subtag removed, and finally any locale whose
language matches, so `fr` finds `fr-CA` when that is all the build has.

`Vary` is the union of the headers the sources read. A response that depends
on `Cookie` and is cached without saying so is how one user's language
reaches another, and the browser check asserts the header rather than
trusting it.

`/i18n/*` serves the catalogs embedded in the server binary: the
content-hashed name `immutable` for a year, a bare tag a `307` to it with
`no-cache`. Brotli (q11, window 22 — P0.7's B7 settings) and gzip variants
are made once per catalog, on the first request that wants one, so a cold
start does not compress locales nobody asked for.

One function does the per-request work — `mf2_axum::provide_locale` —
because leptos_axum already puts the request's `Parts` and a
`ResponseOptions` in context. It must be passed to **every** `_with_context`
entry point; the demo's 404 assertion is there because the file/error
handler is the one that is easiest to forget.

## A6 — layer L6

### The server half: L6 324/324, L6d 255/324

```
cargo xtask conformance-report
  → L6 324/324, L6d 255/324 (+69 documented degradations)
```

The same call sites L5 formats, rendered the way a page does:
`RenderHtml::to_html`, the catalog and the registry from the request
context, the errors discarded (the release client's policy, and therefore
what a user sees). Two assertions — the text, unescaped, must be `exp`; and
a message with markup, rendered again through `Flat` handlers, must produce
one marker element per markup part whose order, kind, name and options equal
the markup entries of `expParts`.

L6 applies to the 324 tests the spec accepts, which is L3's set. L6d defers
to L5d for what degrades — a gated function is a *build* rejection, and none
of the 69 is about rendering — and adds only that a test L5d passes must
also render correctly with the default registry.

Two tests keep the layer from passing vacuously, which is the failure mode a
comparison has (`conformance/tests/l6.rs`): one asserts the markup half runs
on real tests, one asserts a deliberately wrong expectation is rejected.

**What the library needed for it.** A request can now override the
formatter's registry and bidi strategy, not just its catalog
(`RequestI18n`): the four L5 crates are four closed worlds, L6d needs the
default registry as well, and the suite's per-test `bidiIsolation` has to
reach the renderer. All three resolve in **one** context lookup, because it
happens per rendered node.

### The browser half: 20/20 in two engines

```
cargo xtask l6-web --browser chromium,firefox
  → l6: 20/20 assertions passed
```

`conformance/l6-web` renders all **297** runtime-valid `en-US` suite
messages through the view type and writes the page and its catalogs; the
*same crate*, built for `wasm32-unknown-unknown`, hydrates it.

| Assertion | Chromium | Firefox |
|---|---|---|
| every case rendered (297/297) | pass | pass |
| hydration changed no text, per case | pass | pass |
| the console was silent through hydration | pass | pass |
| the registry holds a slot per case (297) | pass | pass |
| the switch to the twin locale succeeded | pass | pass |
| switching back restored the server's text, per case | pass | pass |
| no slot leaked | pass | pass |

`exp` is not re-checked in the browser: the server-rendered text **is**
`exp`, which the Rust half asserts over the same cases, so what an engine
can answer is whether the client agrees with the server.

The **twin** is `en-GB` — a real locale, not a relabelling: its `:unit`
names are the British ones (`42 metres` where `en-US` says `42 meters`), so
the switch genuinely rewrites text and coming back genuinely has to restore
it. What makes it a twin is that the messages are the same, so one manifest
has two catalogs and they are switchable at all.

**It found a design hole on its first green run.** `<CatalogLinks/>` emitted
a link per *other* locale, reasoning that the preload link already names the
page's own. But the preload names the locale the page was **rendered** in,
and after one switch that is no longer the locale the user may want back —
so the run switched to the twin and could not come home. Every locale now
gets a link. A site that omits the map entirely still works: the switch
redirects through `/i18n/<tag>`.

## A7 — B5 on the whole mix

**Not measured with the real leaf.** Phase 5b measured the description and
the `String` path (12.6 B gz against `idlit`, 34.0 against the `dummy`
bound) with every template formatting to a `String`, because `leptos-mf2`
did not exist. Phase 6 makes a description render itself, so the
measurement P0.1 made — the whole mix, view positions included — is now
possible, and it is **not done**: it needs two more workload templates
(`tr` with the description in the view positions, and an `idlit` whose view
positions are a `&'static str` leaf, which is what P0.1's baseline was) and
four more fat-LTO wasm builds.

**What is in the tree for it.** The two templates the measurement needs are
written and verified to *generate and compile*: `tr-view` (a description in
the text-child, attribute and prop positions; `.to_string()` in the `String`
positions) and `idlit-view` (P0.1's baseline — a `&'static str` leaf in the
view positions, a `String` elsewhere), selected by `cargo xtask b5 --view`.
A generated `tr-view` application `cargo check`s clean for
`wasm32-unknown-unknown` with `hydrate`. What is owed is the *run*: four
fat-LTO wasm builds, and P0.1's open item with them — the
`--cfg erase_components` figure, and the tachys leaf hook that would let a
description reuse `&str`'s state and async path.

Writing the templates found a generator bug worth naming: every existing
template forwards exactly **one** feature to its own crates, and the
emitter wrote a separator *and* a trailing comma per entry, so one entry
produced valid TOML by luck and two produced `,,`. `tr-view` forwards two.

**One thing the unfinished run already says.** `cargo xtask b5 --view` was
started and, after six hours on this machine, had completed three of its
six builds — where the `String`-path run of the same six took about half an
hour. The difference is concentrated in one place: compiling the
1,860-site `tr-view` application crate took **over two hours** on its own,
against minutes for `tr` at the same scale. The two differ in exactly one
thing, which is whether a call site hands tachys a `String` or a
description, so what the compiler is doing with those hours is
instantiating tachys' view machinery per site.

That is a **compile-time** observation on a loaded machine, not a
benchmark, and it is not a byte of wasm. But it is the first direct
evidence for the hook P0.1 asked Phase 6 to propose — a tachys leaf that
lets a description reuse `&str`'s state and async path would remove exactly
this instantiation, and the case for it can now be made from something
measured rather than from a size delta alone. Whoever finishes A7 should
record the wall-clock beside the bytes.

**This is the one exit criterion Phase 6 does not meet as written**, and it
is stated here rather than quietly restated: B5 is met on the half of the
mix Phase 5b measured, and unmeasured on the other half. [15](15-phase-7-work-order.md)
does not carry it; it belongs at the head of whatever runs next, because the
budget it belongs to is the one the project's size claim rests on.

## A8 — accessibility and SEO

* `<html lang dir>` is correct on the server and updated by `set_locale`
  (WCAG 3.1.1). The RTL locale flips the example's whole page from `<html
  dir>` alone; the CSS has no second set of rules.
* `<LocaleSwitcher>` is a real `<select>` with a visible `<label>`, and each
  `<LocaleOption>` carries its own `lang`, so a screen reader pronounces
  "Français" with a French voice (3.1.2). The option text comes from the
  application's catalog — `language.fr` is a message in *every* locale's
  catalog — so the names are catalog data, not literals, and the browser
  check greps the client bundle for five of them and finds none (B6).
* `<AlternateLinks/>` emits `hreflang` plus `x-default` for a path-prefix
  site; the example carries schema.org `inLanguage`.
* `mark-fallback-lang` (WCAG 3.1.2 for borrowed text) is declared as a
  feature and **not implemented**: it changes the rendered *structure* of a
  message (a `<span lang>` around it), which is the one thing hydration
  cannot disagree about, and it deserves its own design rather than a flag
  that half works.

**Owner question 2 is answered** in 04 §9: isolated where a person reads the
text (a text child, an attribute, markup, `TextProp`, `Signal<String>`,
`Oco`), plain where a program consumes it (`prop:value`, `to_string()`,
`String`), with `to_display_string()` as the per-use override rather than a
global switch, and a provider-level override for a whole request or subtree.
The split follows *who reads the text*, not which Rust type carries it.

## A9 — the size gate

`cargo xtask size` reads one measurement three ways and fails on any of
them: **B1** (what does not grow with the call sites), **B5** (what does,
per site), and their sum at the reference scale against 06 §3's whole-app
ambition of `30 KB gz + sites × 40 B gz`. It reuses `cargo xtask b5`'s six
builds rather than making its own, and it has replaced the b5 step in the
nightly workflow.

```
cargo xtask size
```

| workload | template | bindgen gz | opt raw | opt gz |
|---|---|---:|---:|---:|
| 1,860 sites | **tr** | 703,909 | 2,711,962 | 730,770 |
| 1,860 sites | idlit | 661,117 | 2,711,750 | 685,422 |
| 1,860 sites | dummy | 627,889 | 2,443,553 | 643,781 |
| 3,720 sites | **tr** | 1,216,163 | 5,054,309 | 1,280,443 |
| 3,720 sites | idlit | 1,151,813 | 5,099,586 | 1,211,659 |
| 3,720 sites | dummy | 1,090,423 | 4,553,491 | 1,130,174 |

| Gate | Measured | Limit | Verdict |
|---|---:|---:|---|
| **B1**, fixed | 21,912 B gz | 30,720 | **met** |
| **B5**, per call site | 12.6 B gz | 40 | **met** |
| **whole app**, 1,860 sites | 45,348 B gz | 105,120 | **met** |

Against the `dummy` bound: 34.0 B gz per site, 23,709 B gz fixed.

**The whole app is 43 % of the ambition** — 45,348 B gz where 06 §3 budgeted
105,120, against 525 KB gz for the reference application it is the i18n of.
That is the claim the project has been making since Phase 0, measured end to
end for the first time rather than added up from parts.

Both marginals are Phase 5b's to the decimal (12.6 and 34.0), and the fixed
part moved by **+15 B gz** (21,897 → 21,912) — the call-site core changing
crates and `markup` gaining its conversion trait. Inside the budget by 8.8 KB,
and recorded here rather than raised.

## A10 — the example

`examples/demo-ssr`, driven by `tools/e2e/checks/demo.mjs`:

```
node run.mjs demo --base-url http://127.0.0.1:3702 --browser chromium
node run.mjs demo --base-url http://127.0.0.1:3702 --browser firefox
  → 46/46 assertions passed, both
```

WebKit is not installed in this working tree and was not run here.

One page using every position the integration supports — the `<title>`
(D9), a `simple` message, an attribute, a signal-valued argument, markup as
elements, a date through ICU4X, a plain `String` from an event handler, and
the switcher — so that what breaks is visible rather than theoretical. The
French translation puts the `<kbd>` at the *end* of the sentence where
English puts it in the middle, which is the whole argument for markup in one
line of a page.

The demo's catalogs, as served:

| Locale | raw | brotli |
|---|---:|---:|
| `en` | 1,172 B | 697 B |
| `fr` | 1,208 B | 807 B |
| `ar` | 1,341 B | 794 B |

**Two of the check's own mistakes are worth naming**, because both looked
like product bugs: Playwright's request contexts share a cookie jar, so the
cookie source — listed first on purpose — decided every request after the
first; and a 404 for a mistyped wasm path returns the *application's* 404
page, whose Arabic text then matched a B6 canary.

## Owner questions

1. **How the client learns another locale's hashed URL** — 04 §6. **Both are
   built**, and an application chooses by what its shell emits.
   `catalog_url(tag)` looks at the page's own preload link, then at a
   `<link rel="mf2-catalog" data-mf2-locale>` map, and falls back to
   `GET /i18n/<tag>` → `307`. A shell that emits the map pays a link per
   locale in every page and never makes the extra request; one that does not
   pays the redirect at switch time only. The demo emits the map; the
   measurement that would settle a *default* is the one A7 owes, since it is
   the same page bytes against the same workload.
2. **Which bidi strategy applies in which position** — answered, §A8 above
   and 04 §9.
3. **Which Leptos 0.8.x releases are supported** — 0.8.20 and its siblings,
   which are still the latest of the line (checked against the index on
   2026-09-23: leptos 0.8.20, leptos_axum 0.8.10, leptos_router 0.8.15,
   leptos_meta 0.8.6, tachys 0.2.18; 0.9.0-beta is pre-release). Older
   0.8.x remain untested, and 04 §10 says so. Nothing in this phase used an
   API newer than 0.8.20.

## What Phase 6 leaves

* **B5 on the view half of the mix** (A7), with P0.1's `erase_components`
  item and the tachys leaf-hook proposal. The one exit criterion not met.
* **`mark-fallback-lang`**: declared, not implemented (A8).
* **The conversions under churn**: P0.11 left it to Phase 6 and Phase 6
  leaves it to Phase 7 (15 A5) — the registry is flat under churn, but a
  derived `TextProp` subscribes to the locale trigger and is dropped with
  its component, which is the shape strategy A leaked through.
* **WebKit**: the browser checks run in it wherever a build is installed;
  this tree has none.
