# mf2-two

Unicode MessageFormat 2 (MF2) for [Leptos](https://leptos.dev): the full
specification, one small binary catalog per language loaded when it is
needed, and a wasm that contains none of the text.

```rust file=calls/src/lib.rs
#[component]
pub fn Overview(name: String, count: RwSignal<u32>) -> impl IntoView {
    view! {
        <h1>{tr!("signature.text", name = name)}</h1>
        <input type="search" placeholder=tr!("search.placeholder") />
        // A signal as an argument: the text follows the count and the language.
        <p role="status">{tr!("cart.items", count = count)}</p>
        // Markup in the message becomes elements, wherever the translation puts them.
        <p>
            {tr!(
                "terms.accept",
                link = |c: AnyView| view! { <a href="/terms">{c}</a> },
                strong = |c: AnyView| view! { <strong>{c}</strong> },
            )}
        </p>
    }
}
```

One macro works in every position: text, attributes, props, strings and
`const` tables. Each call is checked against the messages when it compiles.
(This sample is compiled too: it belongs to the library that
[call sites](docs/call-sites.md) builds.)

* **Server-rendered and hydrated** is the default. The server renders each
  page in the reader's language, and the browser hydrates it and switches
  language **live, without a reload**. The page stays correct and readable
  before the wasm arrives, and if it never does.
* **Islands** give the smallest download. A server-only component costs
  the wasm **nothing**, however many messages it uses: the code section was
  185,925 bytes with and without one full of call sites (`cargo xtask
  islands-zero`, 2026-09-23).
* **Client-only** applications and **lazy routes** are supported too.
* **No locale data in the wasm**: no message text, ids, argument names or
  plural rules. A translation edit leaves the wasm byte-for-byte the same,
  so readers keep their cached copy. The library's client code costs
  22,102 bytes gzipped, plus 12.6 bytes a call site: 45,517 bytes for an
  application with 1,860 call sites (`cargo xtask size`, 2026-09-24).
* **The whole specification**: the MF2 working group's test suite passes at
  every layer, from the parser to the browser, and every normative
  statement has a test ([`conformance/REPORT.md`](conformance/REPORT.md),
  [`conformance/COVERAGE.md`](conformance/COVERAGE.md)).
* **WCAG 2.2 AA**: `<html lang dir>` follows the language, the switcher is
  a labelled form that applies a choice on a button, and bidi isolation is
  on where a person reads the text.

**Status:** not released yet, and not on crates.io. Phase 7 of the plan is
under way (see below).

## Documentation

[`docs/`](docs/README.md): [getting started](docs/getting-started.md),
[call sites](docs/call-sites.md), [delivery modes](docs/delivery-modes.md),
[switching language](docs/switching.md) and
[accessibility](docs/accessibility.md). CI compiles every code sample in
these pages (`cargo xtask docs`).

The examples: [`examples/demo-ssr`](examples/demo-ssr) (server-rendered,
with a lazy route), [`examples/demo-islands`](examples/demo-islands) and
[`examples/demo-csr`](examples/demo-csr).

## Developing

* Plans and decisions: [`plans/`](plans/README.md). Start with
  [`plans/00-master-plan.md`](plans/00-master-plan.md).
* Current work order: [`plans/15-phase-7-work-order.md`](plans/15-phase-7-work-order.md).
* Vendored, pinned inputs (read-only): [`third_party/`](third_party/).

```sh
cargo check --workspace        # the toolchain is pinned in rust-toolchain.toml
cargo xtask ci                 # everything the `ci` job runs, locally
cargo xtask docs               # compile every sample in docs/
```

## License

MIT — see [`LICENSE`](LICENSE). Vendored material under `third_party/` keeps its
own license (Unicode License v3), stated in each directory.
