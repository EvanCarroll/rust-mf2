# Rust MF2

Unicode MessageFormat 2 (MF2) for Rust applications, including
[Leptos](https://leptos.dev), native CLI tools and Ratatui TUIs: the full
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
* **Native CLI and Ratatui apps** use the same messages and the same
  `tr!`, through `mf2`'s `native` feature: the catalogs embedded in the
  executable or shipped beside it, the system's language and time zone.
  With `ratatui`, a message is Ratatui text, its MF2 markup drawn by a
  theme set once.
* **No locale data in the wasm**: no message text, ids, argument names or
  plural rules. A translation edit leaves the wasm byte-for-byte the same,
  so readers keep their cached copy. The library's client code costs
  25,875 bytes gzipped, plus 8.4 bytes a call site: 41,466 bytes for an
  application with 1,860 call sites (`cargo xtask size`, 2026-09-25).
* **The whole specification**: the MF2 working group's test suite passes at
  every layer, from the parser to the browser, and every normative
  statement has a test ([`conformance/REPORT.md`](conformance/REPORT.md),
  [`conformance/COVERAGE.md`](conformance/COVERAGE.md)).
* **WCAG 2.2 AA**: `<html lang dir>` follows the language, the switcher is
  a labelled form that applies a choice on a button, and bidi isolation is
  on where a person reads the text.

**Status:** 1.0.0 is on crates.io for all sixteen crates of the web
family. 1.1.0 was not published and will not be: its fixes ship in 2.0.0,
the next release, where the native and terminal support are `mf2`'s
`native` and `ratatui` features.
What each release contains, what it measures and its known limitations are
in [`CHANGELOG.md`](CHANGELOG.md); what 2.x promises is in
[`docs/versioning.md`](docs/versioning.md).

## Install

Rust 1.88 or later and the `mf2` command, plus the browser target for a
web application:

```sh
cargo install mf2-cli             # the `mf2` command
rustup target add wasm32-unknown-unknown
```

A Leptos application is one crate too: its messages sit in `locales/`, its
build script is `mf2_build::run()`, and it names `mf2` with `leptos` and
the functions its messages call, forwarding its `ssr` feature to `mf2/ssr`
and `mf2/axum`, and `hydrate` to `mf2/hydrate`:

```sh
cargo add mf2 --features leptos,leptos-client-number-intl,leptos-server-number-builtin   # leptos-0-8 on Leptos 0.8
cargo add --build mf2-build
```

`mf2 init --ssr` (or `--islands`, `--csr`) writes a complete one, and
[Getting started](docs/getting-started.md) builds one step by step.

A native application is the same shape: its messages sit in
`locales/`, its build script is `mf2_build::run()`, and it names `mf2`
with `native` — or `ratatui`, for a terminal UI — and the functions its
messages call:

```sh
cargo add mf2 --features native,native-number-builtin   # or ratatui,native-number-builtin
cargo add --build mf2-build
```

`mf2 init --cli` or `mf2 init --tui` writes a complete one;
[Native CLI and Ratatui apps](docs/native-apps.md) explains every file.
Both are on crates.io at 2.0.0.

## Documentation

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) explains the
ecosystem, crate roles, application workflows and integrations. Its source
and chapter links are in [`docs/`](docs/README.md). Start with
[getting started](docs/getting-started.md), then choose a path:
[how the crates fit together](docs/ecosystem.md),
[call sites](docs/call-sites.md), [delivery modes](docs/delivery-modes.md),
[switching language](docs/switching.md),
[accessibility](docs/accessibility.md),
[native CLI and TUI apps](docs/native-apps.md),
[migrating from `leptos-fluent`](docs/migrating-from-leptos-fluent.md),
[upgrading from 1.x](docs/upgrading.md) and
[versions](docs/versioning.md); what changed, [`CHANGELOG.md`](CHANGELOG.md).
`mdbook build` renders the book; `cargo xtask docs` compiles its application
examples for native and browser targets. Each crate's API reference remains
in rustdoc.

The web examples: [`examples/demo-ssr`](examples/demo-ssr)
(server-rendered, with a lazy route),
[`examples/demo-islands`](examples/demo-islands) and
[`examples/demo-csr`](examples/demo-csr). The native example — a CLI with
a Ratatui mode — is the application
[Native CLI and Ratatui apps](docs/native-apps.md) builds, compiled by
`cargo xtask docs`.

## Developing

* Current plans: [`plan/`](plan/). Start with
  [`plan/01-size-and-features.md`](plan/01-size-and-features.md).
* The plans and results of phases 0–10, kept as history:
  [`plan/archive/`](plan/archive/README.md).
* Vendored, pinned inputs (read-only): [`third_party/`](third_party/). The MF2
  specification text is not among them: its license does not allow public
  redistribution, so `cargo xtask spec-sync` fetches it, at the pinned commit
  and checked against recorded digests, into `target/xtask-cache/`. The
  conformance tests need it; run the command once after cloning. UTS #35
  Part 1, whose language-matching rules the locale matcher follows, is
  fetched the same way by `cargo xtask uts35-sync`; nothing builds from it.

```sh
cargo xtask spec-sync          # fetch the MF2 specification text (once)
cargo check --workspace        # the toolchain is pinned in rust-toolchain.toml
cargo xtask ci                 # everything the `ci` job runs, locally
cargo xtask docs               # compile every sample in docs/
```

## License

MIT — see [`LICENSE`](LICENSE). `mf2` and `mf2-locale-data` ship data from Unicode's
CLDR, so they are MIT AND Unicode-3.0 (each has its `LICENSE-UNICODE`). Vendored material
under `third_party/` keeps its own license (Unicode License v3), stated in each directory.
