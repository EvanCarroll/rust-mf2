# examples/demo-csr — the Phase 7 client-only example

An application with **no server**: trunk builds the wasm, `mf2 compile
--site` publishes the catalogs beside it, and any static host serves the
result. The browser chooses the locale, and remembers it. This is what
`tools/e2e/checks/csr.mjs` drives (`plans/15-phase-7-work-order.md` A2).

## Build and run

From a clean checkout, with `trunk` installed (it runs offline, using the
`wasm-bindgen` CLI 0.2.128 already on the `PATH`):

```sh
cd examples/demo-csr
trunk serve                 # http://127.0.0.1:8080
```

or `trunk build`, which writes the site to `dist/`. The browser checks serve
`dist/` themselves:

```sh
trunk build
cd ../../tools/e2e
node run.mjs csr --browser chromium,firefox
```

Like the other examples, this is a **workspace of its own**.

## What is on the page

| On the page | What it exercises |
|---|---|
| the `<title>` | a description through `TextProp`, following a switch |
| heading, tagline | plain text |
| the search field's `placeholder` | a description as an attribute |
| the hotkey line | markup as elements, whose structure comes from the catalog |
| the counter | a signal-valued argument |
| the published line | a date through the browser's `Intl.DateTimeFormat` (`leptos-client-datetime-intl`), in the reader's time zone from the first frame |
| the switcher | a live switch on the button (never on the select's `change`), remembered in `localStorage` |
| `inLanguage` | schema.org's language of the page, kept up to date across a switch |

## Three things a client-only application does

```rust
// main.rs: install the generated setup, then mount through the gate.
fn main() {
    my_i18n::install();
    mf2::leptos::mount_to_body(App);
}
```

```html
<!-- index.html: the index downloads in parallel with the wasm. -->
<link rel="preload" as="fetch" crossorigin="anonymous" href="i18n/index.json" data-mf2-index />
```

```toml
# Trunk.toml: publish the catalogs and the index into the site.
[[hooks]]
stage = "post_build"
command = "sh"
command_arguments = ["-c", "mf2 -C i18n compile --site \"$TRUNK_STAGING_DIR/i18n\""]
```

The i18n crate's build script emits `Emit::Module`: the wasm never names a
catalog, so a translation edit changes the catalogs and the index but not
the wasm. `mf2 compile --site` builds the catalogs for the i18n crate's
features as cargo resolves them, so the functions the wasm has and the ones
the catalogs use cannot drift apart; a `--features` that disagrees with
cargo's fails the build, naming both lists.

**Which locale.** The one the reader chose last time (`localStorage`,
`mf2_locale`), else the first of `navigator.languages` the build has — `fr-CA`
finds `fr`, as the server's `Accept-Language` matching does — else the source
locale. Nothing renders before the catalog is installed, so the first frame
is already in that locale, with `<html lang dir>` set to match.

**The index.** `i18n/index.json` maps each locale to its content-hashed
catalog, `{"fr": "fr.87bf7771890c4b05.mf2b", …}`. Serve the catalogs
`immutable` and the index `no-cache`: it keeps its name while what it says
changes from one deploy to the next. The browser parses it, so the wasm carries no
JSON parser.

**A failed boot** — no index, no catalog — logs one `mf2:` line and mounts
nothing, leaving what `index.html` holds. A catalog from another deploy
reloads the page.
