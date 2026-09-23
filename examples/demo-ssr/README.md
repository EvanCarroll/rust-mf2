# examples/demo-ssr — the Phase 6 example

One page that uses every position the integration supports, so that what
breaks is visible rather than theoretical. It is also what
`tools/e2e/checks/demo.mjs` drives.

## Build and run

From a clean checkout, with `cargo-leptos` installed
(`cargo install cargo-leptos`):

```sh
cd examples/demo-ssr
cargo leptos watch          # http://127.0.0.1:3702, rebuilding as you edit
```

or, without the watcher:

```sh
cargo leptos build
LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
LEPTOS_SITE_ADDR=127.0.0.1:3702 ./target/debug/demo-ssr
```

The browser checks, against a running server:

```sh
cd ../../tools/e2e
node run.mjs demo --base-url http://127.0.0.1:3702 --browser chromium,firefox
```

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
| the published line | a date through `:datetime`, formatted by ICU4X from the catalog's own `icu.blob` |
| the echo line | a plain `String` built in an event handler — no bidi isolation in it, because a program consumes it (04 §9) |
| the switcher | `<LocaleSwitcher>`: a labelled native control, each language named in its own language with its own `lang` |

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
