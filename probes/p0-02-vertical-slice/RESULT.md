# P0.2 vertical slice + P0.10 hydration tolerance — RESULT

Probe for `plans/06-size-and-perf.md` §5 (P0.2, P0.10) and
`plans/07-phase-0-work-order.md`. The code is throwaway. The browser harness
it produced (`tools/e2e/`) is permanent.

## Verdict

| Threshold (06 §5) | Verdict |
|---|---|
| P0.2: zero hydration warnings | **met**: 0 console warnings and 0 errors across load, hydration, lazy-route navigation, switching locale and back, 4 streamed pages and a direct lazy load. Checked on debug and release builds, in Chromium 143 and Firefox 155 |
| P0.2: catalog request overlaps the wasm request | **met**: the catalog starts before the wasm, finishes first, and its preload is reused (one request) |
| P0.2: lazy route sees the i18n state | **met**: a `#[lazy_route]` chunk under `--split` renders `Tr` text, attributes and `current_locale()` in both locales, and switches live |
| P0.2: SSR context lookup works under streaming (D9) | **met with D9's fallback, narrowed** (see D9 outcome below). Every `Tr` rendered by tachys finds the request context in all 4 `SsrMode`s. A `TextProp` that leptos_meta evaluates outside the owner does not, so conversions capture the catalog under `ssr` |
| P0.10: a text difference gives no warning, and the server text stays until the next update | **confirmed**, in debug and release, in both browsers |
| P0.10: failure mode of a structural difference | **documented**: tachys logs an error, then panics; the wasm traps, hydration aborts and the page becomes inert. The probe's `Tr` glue logs one error and hydration continues |
| `forbid(unsafe_code)` in the client crate | **feasible**: the `tr` crate is `#![forbid(unsafe_code)]` and also denies `unwrap_used`/`expect_used`/`panic`/`indexing_slicing`. It builds for `ssr` (native) and `hydrate` (wasm32) with clippy clean on those lints |
| Minimum Leptos version verified | **leptos 0.8.20** (leptos_axum 0.8.10, leptos_router 0.8.15, leptos_meta 0.8.6, tachys 0.2.18, reactive_graph 0.2.14). This is the latest stable on 2026-09-20. Older 0.8.x versions were **not** tested |

**Go** for P0.2 as far as this probe can tell. The D9 decision text should
change as described under D9 outcome.

## What was built

`probes/p0-02-vertical-slice/` is a standalone workspace (its own
`[workspace]`, excluded from the root).

* **`tr/`** is the library crate that the real `leptos-mf2` will become:
  `#![forbid(unsafe_code)]`, panic-free on the client path.
  * `catalog.rs`: a hand-built binary catalog. It has magic `MF2B`, a format
    version, flags (rtl), `manifest_hash` u64, a locale tag, an offset index
    (2-bit kind ‖ 30-bit offset) and one UTF-8 pool. It validates once and
    `get()` never panics. There is a native writer and a unit test.
  * `glue.rs`: tachys 0.2 impls for the concrete `Tr { id: u32 }`, a 4-byte
    `Copy` value with a compile-time assert. It implements `Render`,
    `RenderHtml` (delegating to `<&str as RenderHtml>`), `AttributeValue`
    (delegating to `<&str as AttributeValue>::to_html`), `no_attrs!`, and
    `From<Tr>` for `TextProp`, `Signal<String>`, `String` and `Oco`.
    `Tr::to_string()` tracks the locale only when `Observer::get()` is `Some`.
  * `registry.rs`: **node-update strategy B**, a library-owned slab of
    `{Text | (Element, key), MsgId}`. Each view state holds a `SlotHandle`
    whose `Drop` frees the slot in O(1). There is no `RenderEffect` per node.
    Only derived values (`TextProp`, `Signal<String>`) subscribe to one
    `ArcTrigger`.
  * `client.rs` (feature `hydrate`): the boot gate. It reads the URL from
    `<link data-mf2>` and the locale from `<html lang>`, calls `fetch()` (which
    reuses the preload), validates the manifest hash and the lang, installs the
    thread-local catalog, and only then calls `leptos::mount::hydrate_lazy` (or
    `hydrate_body`). `set_locale(tag)` does fetch → validate → swap → registry
    walk → `<html lang dir>` + cookie → notify. The crate defines no
    `#[wasm_bindgen]` items.
  * `server.rs` (feature `ssr`): `RequestI18n { Arc<Catalog>, href }` in
    context, looked up **at render time**, with a default-locale fallback when
    no context is reachable. Hit and miss counters are exposed for the checks.
* **The app** (`src/`): cargo-leptos + Axum 0.8.9, catalogs `en` (ltr) and
  `ar` (rtl) with 32 messages (en 877 B, ar 1,221 B). It has:
  * the shell with `<html lang dir>` and
    `<link rel="preload" as="fetch" crossorigin="anonymous" href="/i18n/<tag>.<fnv64>.mf2b" data-mf2>`
    first in `<head>`;
  * `/i18n/{file}`, which serves immutable bytes, and `/i18n/{tag}`, which
    answers `307` to the hashed URL;
  * negotiation by cookie, then `Accept-Language`, then default, plus
    `Content-Language` and `Vary`;
  * the same `additional_context` passed to
    `generate_route_list_with_exclusions_and_ssg_and_context`,
    `leptos_routes_with_context` (which also registers server functions
    through `handle_server_fns_with_context`) and
    `file_and_error_handler_with_context`;
  * pages covering every call-site position that matters here: text child,
    attribute, `TextProp` prop, `Signal<String>`, `if`/`else` rebuild,
    `to_string()` in an event handler, a server function, `<Title>`, and a
    `<LocaleSwitcher>` (a labelled `<select>` whose autonyms are catalog
    messages);
  * `/lazy`, a `LazyRoute`;
  * `/stream/{ooo,inorder,blocked,async}` with `Suspense` + `Suspend` and
    `Suspense` + `move || res.get()`;
  * the P0.10 pages `/p010/{text,struct,struct-tr}`.

## Commands

Everything runs from the repository root, on toolchain 1.98.1 stable (via
`rust-toolchain.toml`), cargo-leptos 0.3.7, wasm-bindgen CLI 0.2.128 (the crate
is pinned `=0.2.128` to match; cargo-leptos uses the CLI on `PATH` before
downloading anything) and wasm-opt 120.

```sh
cd probes/p0-02-vertical-slice
# Every cargo-leptos build first empties target/site, so build the server,
# then the frontend.
CARGO_BUILD_JOBS=3 cargo leptos build --split --server-only    [--release]
CARGO_BUILD_JOBS=3 cargo leptos build --split --frontend-only  [--release]
./run-server.sh debug|release          # serves http://127.0.0.1:3702
# (or: CARGO_BUILD_JOBS=3 cargo leptos serve --split [--release], which builds both in parallel)

CARGO_BUILD_JOBS=3 cargo test -p tr --features ssr
CARGO_BUILD_JOBS=3 cargo clippy -p tr --features hydrate --target wasm32-unknown-unknown

cd ../../tools/e2e && npm install      # playwright 1.63.0
node run.mjs p002 --browser all --json ../../probes/p0-02-vertical-slice/results/p002-<build>.json
node run.mjs p002 --browser chromium --throttle --json ../../probes/p0-02-vertical-slice/results/p002-release-throttled.json
node run.mjs p010 --browser all --json ../../probes/p0-02-vertical-slice/results/p010-<build>.json
```

To reproduce the D9 failure, build a server with pure render-time lookup, then
the frontend, then run `p002`. Expect 4 `FAIL`s:

```sh
CARGO_BUILD_JOBS=3 cargo leptos build --split --server-only --bin-features ssr,d9-no-capture
CARGO_BUILD_JOBS=3 cargo leptos build --split --frontend-only
```

The raw JSON for every run is in `results/`.

## Observations and numbers

### e2e totals

| Build | Chromium 143.0.7499.4 | Firefox 155.0 |
|---|---|---|
| debug, `--split` | 67/67 | 67/67 |
| release (`wasm-release` + `wasm-opt -Oz`), `--split` | 67/67 | 67/67 |
| release, throttled (150 ms RTT, 1.6 Mbps) | 67/67 | — |
| debug, `d9-no-capture` | 63/67: the 4 `SsrMode::{PartiallyBlocked, Async}` title and miss assertions fail | — |
| P0.10, debug / release | 8/8 / 8/8 | 8/8 / 8/8 |

Playwright 1.63.0 could not download its own browser builds (the CDN timed
out), so the harness drove the cached builds 1200 (Chromium) and 1542
(Firefox) through `executablePath`.

### Catalog vs. wasm timing

Resource Timing gives ms from navigation start; Playwright network events are
consistent with it (`results/*.json`, `networkEvents`).

| Run | catalog start → end | main wasm start → end | wasm size | hydrated after `goto` |
|---|---|---|---|---|
| release, Chromium | 12.5 → 20.4 | 13.0 → 25.1 | 559,101 B | 563 ms |
| release, Firefox | 118 → 340 | 119 → 344 | 559,101 B | 1,080 ms |
| release, Chromium throttled | **168.8 → 329.0** | **168.9 → 3,199.5** | 559,101 B | 3,534 ms |
| debug, Chromium | 14.2 → 19.3 | 14.8 → 177.6 | 7,002,774 B | 630 ms |
| debug, Firefox | 294 → 645 | 296 → 770 | 7,002,774 B | 1,542 ms |

* In every run the catalog starts first, or 0.1 ms before the wasm, and ends
  first.
* Resource Timing records exactly one catalog request, with
  `initiatorType: "link"`, so the gate's `fetch()` reused the preload and
  Chromium did not warn "preloaded but not used".
* Throttled, the catalog was complete about 2.9 s before the wasm finished.
  The boot gate therefore adds no round trip (B11).
* A direct `/lazy` load also preloads the lazy chunk and `__wasm_split.js`
  from `<head>`, so the catalog, main module and chunk download in parallel.

### Streaming and D9 (render-time lookup)

`GET /__probe/ctx` counts render-time lookups that found, or did not find,
the request context. The assertion is the per-page delta, with the request in
`ar` (the default is `en`, so a miss is visible as English text).

| `SsrMode` | Streamed `Tr` text | SSR `<title>` (a `TextProp` read by leptos_meta) | misses, render-time only | misses, with capture |
|---|---|---|---|---|
| OutOfOrder | ar ✔ (templates streamed after 300 / 600 ms) | ar ✔ | 0 | 0 |
| InOrder | ar ✔ | ar ✔ | 0 | 0 |
| PartiallyBlocked (blocking resource) | ar ✔ | **en ✘** | **1** | 0 |
| Async | ar ✔ | **en ✘** | **1** | 0 |

Other context checks:

* Route-list generation at startup: 12 hits, 0 misses. It runs with mock
  parts, and the provider falls back to the default locale.
* The 404 page rendered by `file_and_error_handler_with_context` comes out in
  the negotiated locale.
* A server function formats from the cookie-negotiated context: `en`, then
  `ar` after `set_locale`.
* A lookup made with no owner at all (`/__probe/no-ctx`) falls back to the
  default locale and does not panic.

The cause of the two misses: leptos_meta 0.8.6 reads `<Title text>` in
`ServerMetaContextOutput::inject_meta_context` (`title.as_string()` →
`TextProp::get()`). That happens after the app has rendered and outside the
request owner. In OutOfOrder and InOrder mode the lookup happened to find the
owner, but that relies on leptos_meta internals. After hydration the client
corrects the title, so only the server HTML (what crawlers and pre-hydration
users see) is wrong.

### Locale switch, lazy route, registry

After a switch to `ar` and back, the client-side text of the lazy page and
the home page (header + main, `textContent`) is **byte-identical** to the
server-rendered page in that locale. This covers:

* `<html lang dir>`;
* `document.title`, through `TextProp`;
* the `placeholder` and `aria-label` attributes;
* the `TextProp` prop and `Signal<String>`;
* the `if`/`else` rebuild.

Live registry slots were 20 on home, 15 on `/lazy`, and 20 again back on
home. Slots are freed on `Drop`, including for nodes created by code in the
lazy chunk. Chunks share linear memory and thread-locals, as 04 §1 predicted.

### P0.10: hydration tolerance

| Page | Console | DOM | Hydration |
|---|---|---|---|
| `/p010/text`: static `&str`, reactive text and `title` attribute differ | nothing, in debug or release | server text kept after hydration. The reactive node shows client text only after its next update (`SERVER-0` → `CLIENT-1`). Static text and attributes never update, so they keep the server text for good | completes |
| `/p010/struct`: server `<b>`, client text (tachys `&str`) | debug: `A hydration error occurred while trying to hydrate an element defined at src/p010.rs:40:10. The framework expected a text node, but found this instead: <b>…`, then `panicked at tachys-0.2.18/src/hydration.rs:248:9: Unrecoverable hydration error`, then `RuntimeError: unreachable`. Release: only `panicked at …hydration.rs:227:9: internal error: entered unreachable code`, then `RuntimeError: unreachable` | server `<b>` stays | **aborts**: no handler after the mismatch works, and clicking a nav link before it does nothing (no client-side navigation, no reload). The page is inert |
| `/p010/struct-tr`: server `<b>`, client `Tr` | one `mf2: hydration expected a text node …` error from the probe glue | server `<b>` stays; the `Tr` binds to a detached text node and will not update | **continues**: later handlers and client navigation work |

Results were identical in Chromium and Firefox. Consequences:

* Text differences, such as `datetime-intl` versus server ICU output, are
  harmless and silent.
* A structural difference in markup (a future `TrRich` whose structure
  differs between the server's catalog and the client's) kills the whole app
  unless the glue handles it. The hydration gate (same catalog, same manifest
  hash) is what prevents it.

### Sizes (reference only)

This is the whole demo app: Leptos + router + meta + server function (serde)
+ `console_error_panic_hook`. It is **not** an i18n cost, which P0.1 and P0.3
measure.

| Release `--split` artifact | raw | gzip -9 |
|---|---|---|
| main module after split + wasm-bindgen (before wasm-opt) | 653,107 | 214,789 |
| main module after `wasm-opt -Oz` (= the served `pkg/p002.wasm`, `cmp` identical) | 559,101 | 220,318 |
| lazy chunk (`-Oz`) | 6,975 | 3,590 |
| `p002.js` / `__wasm_split.js` | 24,247 / 2,467 | 6,975 / 893 |

Commands used:

```sh
wasm-bindgen --target web --keep-lld-exports --no-demangle --out-name p002 --out-dir $T \
  target/front/wasm32-unknown-unknown/wasm-release/p002_split.wasm
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int $T/p002_bg.wasm -o $T/oz.wasm
gzip -9 -c < FILE | wc -c
```

The pre-split cargo output (4.2 MB) carries relocations for wasm-split, which
wasm-opt cannot handle, so it is not a meaningful baseline.

Canary grep (B6): `cat target/site/pkg/*.wasm target/site/pkg/*.js | rg -a -c -F …`
finds **0** hits for the canary text `CANARY-QX7`, the canary id
`canary-msg-id-qx7`, `Welcome! Every`, `مرحب`, the autonym `العربية`, and the
ids `welcome-text` and `nav-stream-inorder`. The manifest hash is computed by
a `const fn` over the id list, so the ids never reach the wasm. Present: the
locale tags `en`/`ar` (option values), the cookie name and the diagnostic
strings.

## D9 outcome

**Render-time lookup is verified** for every `Tr` that tachys renders (text
child and attribute) in OutOfOrder, InOrder, PartiallyBlocked and Async
streaming, including `Suspend` chunks streamed later.

It **fails** for derived values that a third party evaluates outside the
request owner: leptos_meta's `<Title text>` in PartiallyBlocked and Async.

**Fallback implemented and verified, narrowed:** under `ssr`,
`From<Tr> for TextProp` and `From<Tr> for Signal<String>` capture the request's
`Arc<Catalog>` at conversion time, which is inside the owner. `Tr` itself stays
a 4-byte `Copy` value, looked up at render time. The larger non-`Copy` `Tr`
under `ssr` that D9 anticipated is **not** needed.

## Departures, and points the plans should absorb (C1/C2)

1. **D9 / 04 §5 wording.** Replace "capture at construction under `ssr`" with
   "`Tr` renders with a render-time lookup (verified); conversions to reactive
   derived types capture the request catalog under `ssr`". Also document that
   the `additional_context` provider must tolerate missing `Parts`:
   `file_and_error_handler_with_context` calls it in a bare `Owner` for static
   files. Server functions get the context through `leptos_routes_with_context`
   itself, so in practice the four entry points are three calls.
2. **`set_locale` URL discovery is unspecified** (04 §6 says only "the client
   never guesses a hash"). The probe uses `GET /i18n/<tag>` → `307` to the
   hashed, immutable URL (`Cache-Control: no-cache`). That costs one extra
   round trip at switch time only. The alternative is an SSR-emitted map in the
   DOM. **Owner/C2 decision.**
3. **`02` §2 inconsistency.** STRINGS is described as "varint length + UTF-8",
   while F4 requires one `str::from_utf8` pass over the pool. Length bytes of
   0x80 and above are not UTF-8, so both cannot hold. The probe derives lengths
   from the next index offset instead. This is for P0.7 to settle.
4. **04 §3 hydrate rule.** Add "never panic on a non-Text node". tachys'
   `failed_to_cast_text_node` is `pub(crate)` and panics, so `leptos-mf2` must
   walk the cursor itself, as the probe does, and degrade.
5. **LocaleSwitcher.** Autonyms are catalog messages, identical in every
   catalog, so no language names are compiled into the wasm. Locale *tags* do
   appear in the wasm (option values). B6's canary list does not cover tags,
   and that seems right.

## What the real `leptos-mf2` should do differently from `probes/audit/tr-prototype`

* **Strategy B registry instead of a `RenderEffect` per node**, for both text
  and attributes. The probe shows it works across lazy chunks and frees slots
  on `Drop`; P0.11 still sizes it.
* **Track only under an observer.** The prototype's unconditional `track()`
  would log reactive_graph's "outside a reactive tracking context" warning
  (`traits.rs` 117–140, debug builds) from every event handler. The probe's
  event-handler `to_string()` runs with 0 warnings.
* **No `.expect` on the client.** The prototype panics in `hydrate` (cast) and
  in `rebuild` (`prev.expect`). The probe replaces the first with log-and-degrade
  and the second with a slot-id update.
* **SSR.** Use one `with_context` borrow and no `Arc` clone per lookup (the
  prototype did two lookups), fall back to the default locale rather than
  `""`, and capture in derived conversions.
* **Boot.** Provide `hydrate_body` / `hydrate_lazy` wrappers that gate on the
  preloaded catalog and also check `<html lang>`. Keep every `#[wasm_bindgen]`
  export in the application, which is what keeps `forbid(unsafe_code)`
  feasible. On a manifest mismatch the probe logs
  `mf2: catalog rejected (manifest hash mismatch)` and stays inert (e2e S1).
  The real library should reload once, as 04 §6 says.

## Caveats

* One app, two locales, 32 simple messages. There are no patterns,
  selectors, arguments or markup, and islands and CSR were not tried.
* The catalog is not precompressed (`.br`/`.gz` siblings were not produced),
  and the probe-only `/__probe/*` endpoints exist.
* The fmt-free and panic-free-reachability budget (B12) was **not** assessed:
  `console_error` has a native `eprintln!` branch, and the app links
  `console_error_panic_hook`. That is P0.3's question.
* Timings are from localhost, plus one CDP-throttled run. They show ordering
  and overlap, not real-network latency.
* Server and client must be the same build mode. A debug server with a release
  client failed to hydrate (debug SSR emits extra marker comments).
  cargo-leptos always pairs them.
* Environment side effect: the failed `npx playwright install` attempt
  garbage-collected a stale `firefox-1544` build from the shared
  `~/.cache/ms-playwright`. Playwright deletes builds whose referencing install
  no longer exists.
