# examples/demo-islands — the Phase 7 islands example

A page whose interactive parts are **islands**: most of it renders on the
server and ships no code, and two parts hydrate. It is what
`tools/e2e/checks/islands.mjs` drives and what `cargo xtask islands-zero`
measures (`plans/15-phase-7-work-order.md` A1).

## Build and run

From a clean checkout, with `cargo-leptos` installed:

```sh
cd examples/demo-islands
cargo leptos watch          # http://127.0.0.1:3704
```

or, without the watcher:

```sh
cargo leptos build
LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_PKG_DIR=pkg \
LEPTOS_SITE_ADDR=127.0.0.1:3704 ./target/debug/demo-islands
```

The browser checks, against a running server:

```sh
cd ../../tools/e2e
node run.mjs islands --base-url http://127.0.0.1:3704 --browser chromium,firefox
```

and the accessibility audit, which drives all three examples (demo-ssr on
port 3702, this one on 3704 — `MF2_ISLANDS_URL` to change it):

```sh
node run.mjs a11y --base-url http://127.0.0.1:3702 --browser chromium,firefox
```

and the size claim, from the repository root:

```sh
cargo xtask islands-zero
```

Like `examples/demo-ssr`, this is a **workspace of its own**.

## What is on the page

| On the page | Where it runs | What it exercises |
|---|---|---|
| the `<title>`, heading, tagline, note | server | plain text |
| the search field's `placeholder` | server | a description as an attribute |
| the hotkey line | server | markup as elements, with no client code |
| the counter | **island** | a signal-valued argument under `static-locale` |
| the note above the counter | **island** | a markup message inside an island — the case the gate exists for |
| the switcher | server | `static-locale`'s switch: the form's `GET ?lang=`, which the server negotiates and remembers in the cookie — no code, so it works before the wasm loads |

## Three things an islands application does

```rust
// The shell: the gate is the first thing in <body>, outside every island.
view! {
    <body>
        <IslandsGate />
        <App />
    </body>
}

// The client entry point: set the owner, start the catalog load.
#[wasm_bindgen]
pub fn hydrate() {
    leptos_mf2::install(my_i18n::setup());
    leptos_mf2::hydrate_islands();
}

// Once, in the application crate: the gate's export.
leptos_mf2::islands_gate!();
```

**Why a gate.** Leptos' island script calls `hydrate()` and walks the
document for islands straight away, without waiting for it, so `hydrate()`
cannot wait for the catalog. The walk *does* wait for an island that returns
a promise, and it visits islands in document order. `<IslandsGate/>` is an
empty island that resolves once the catalog is installed, so every island
after it hydrates against the page's catalog. That matters for a markup
message: its elements come from the catalog, and without the gate its island
fails to hydrate. The browser check keeps that control case. The gate adds no
page bytes and no request, because the catalog fetch reuses the preload.

**Why `static-locale`.** Server-only components are static HTML: a live switch
could update the islands but not them. So a switch is a navigation: the switcher's
form submits `?lang=`, the server renders the whole page in the new locale and
writes the `mf2_locale` cookie. The switcher is therefore not an island — a
choice applies on its button, never on the select's `change` (WCAG 3.2.2). On the
client, only a node with a reactive argument registers — here, the counter's
line.

## The size claim

`cargo xtask islands-zero` builds the client twice, with and without the
`more-server` feature, which adds a server-only component with a call site in
every position. First run (2026-09-23): the same 1,019 functions and the same
185,925-byte code section in both, the data section 1 byte apart from
reordered constants, and 94,904 B gz shipped either way.
