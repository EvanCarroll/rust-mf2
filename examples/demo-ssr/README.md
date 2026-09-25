# examples/demo-ssr — the Phase 6 example

One page that uses every position the integration supports, so that what
breaks is visible rather than theoretical, and a second route, `/lazy`,
whose code is a wasm chunk of its own (Phase 7 A3). It is what
`tools/e2e/checks/demo.mjs` and `tools/e2e/checks/lazy.mjs` drive.

## Build and run

From a clean checkout, with `cargo-leptos` installed
(`cargo install cargo-leptos`):

```sh
cd examples/demo-ssr
cargo leptos watch --split  # http://127.0.0.1:3702, rebuilding as you edit
```

or, without the watcher:

```sh
cargo leptos build --split  # add --release for the size-optimised client
LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
LEPTOS_SITE_ADDR=127.0.0.1:3702 ./target/debug/demo-ssr
```

The browser checks, against a running server:

```sh
cd ../../tools/e2e
node run.mjs demo --base-url http://127.0.0.1:3702 --browser chromium,firefox
node run.mjs lazy --base-url http://127.0.0.1:3702 --browser chromium,firefox
# dates in the reader's time zone (also needs demo-csr's dist/, below)
node run.mjs zone --base-url http://127.0.0.1:3702 --browser chromium,firefox
# the WCAG 2.2 AA audit: all three examples (demo-islands running on 3704)
node run.mjs a11y --base-url http://127.0.0.1:3702 --browser chromium,firefox
```

Without `--split` the app still works — `/lazy` is then an ordinary async
route in the one wasm — but `lazy.mjs` asserts the chunk, so it needs the
`--split` build.

This is a **workspace of its own**: an application turns on exactly one of
`ssr` and `hydrate`, and cargo unifies features across a workspace, so the
example cannot share one with libraries that are built both ways.

## What is on the page, and what it is there to show

| On the page | What it exercises |
|---|---|
| the `<title>` | D9 — leptos_meta reads it *after* rendering, outside the request owner, so the conversion captures the catalog inside it (04 §5) |
| the heading, the tagline | a `simple` message: tachys writes the catalog's borrowed `&str`, with no `String` in between |
| the search field's `placeholder` | a description as an attribute value |
| "N people are here" | a **signal-valued** argument: the call site writes `count = count`, and the effect that watches it is the library's, not one per site (04 §4) |
| the hotkey line | **markup as elements** — and in French the `<kbd>` lands at the *end* of the sentence, where French puts it, without the view knowing anything about word order (04 §7) |
| the published line | a date through `:datetime`, formatted by ICU4X from the catalog's own `icu.blob` — in the reader's time zone: a first visit is served in UTC and corrected after hydration, and the `mf2_tz` cookie makes every later page right from the server |
| the echo line | a plain `String` built in an event handler — no bidi isolation in it, because a program consumes it (04 §9) |
| the switcher | `<LocaleSwitcher>`: a labelled native control, each language named in its own language with its own `lang`, applied by its button — live once hydrated, the form's `GET ?lang=` before (WCAG 3.2.2) |
| the counter's line | `role="status"`: announced when a button changes it, focus left on the button (4.1.3) |
| the page's landmarks | banner, navigation, main and contentinfo as siblings, so "skip to main" lands on the content |

## The lazy route

`/lazy` is a `#[lazy_route]`, so under `--split` its view is
`pkg/split_…lazy_page_view….wasm` (11,255 B, 5,617 B gz in a release
build), fetched the first time the route is matched — on a client-side
navigation, or, when the page *is* `/lazy`, preloaded by the server's HTML
and awaited by `leptos_mf2::hydrate_lazy` before hydration walks it.
Nothing in `leptos-mf2` is aware of chunks: they share the main module's
linear memory and thread-locals, so the chunk's descriptions read the
catalog the boot installed, join the same node registry, and follow the
same switch. The route has its own `<title>`, a text, an attribute, a markup message and
the current locale, and leaving it frees every registry slot it took.

`ar` is right-to-left, so switching to it flips the whole page from `<html
dir>` alone — the CSS has no second set of rules.

## What the page carries, and what it does not

The served HTML contains, in `<head>`:

```html
<link rel="preload" as="fetch" crossorigin="anonymous" href="/i18n/en.52ab….mf2b" data-mf2>
<link rel="mf2-catalog" data-mf2-locale="fr" href="/i18n/fr.e276….mf2b">
<link rel="mf2-catalog" data-mf2-locale="ar" href="/i18n/ar.2b31….mf2b">
```

The first **is** the boot data: the client reads the catalog's URL from it and
the locale from `<html lang>`, so there is no inline script (no CSP nonce), no
JSON (no serde in the wasm), and the catalog downloads in parallel with the
wasm. The others are the in-page tag → URL map, which is the no-round-trip
half of owner question 1; a site that would rather keep its pages smaller
drops `<CatalogLinks/>` and the switch redirects through `/i18n/<tag>`
instead.

What the client bundle does **not** contain is any message text: the browser
check greps the `.js` and the `.wasm` for five strings from the corpus,
including the language names in the switcher, and finds none. The names are
catalog data (`language.fr` exists in every locale's catalog), not literals.

## Accessibility

- `<html lang dir>` is correct on the server and updated by `set_locale`
  (WCAG 3.1.1).
- The switcher is a real `<select>` with a visible `<label>`, and each
  `<option>` carries its own `lang` so that a screen reader pronounces
  "Français" with a French voice (3.1.2).
- Focus is visible everywhere (2.4.7, 2.4.11), the layout is flexbox and
  reflows to a phone (1.4.10), and the SVG is an external file.
- The page states its language as schema.org data as well as rendering it.
