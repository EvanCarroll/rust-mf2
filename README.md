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
  `tr!` through `mf2-native`: the catalogs embedded in the executable or
  shipped beside it, the system's language and time zone, and the locale
  in state the application owns. `mf2-ratatui` turns messages into
  Ratatui text, with MF2 markup as styles.
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
family. 1.1.0 was not published and will not be: its fixes, and the native
crates `mf2-native` and `mf2-ratatui`, ship in 2.0.0, the next release.
What each release contains, what it measures and its known limitations are
in [`CHANGELOG.md`](CHANGELOG.md); what 1.x promises is in
[`docs/versioning.md`](docs/versioning.md).

## Install

Rust 1.88 or later and the `mf2` command, plus the browser target for a
web application:

```sh
cargo install mf2-cli             # the `mf2` command
rustup target add wasm32-unknown-unknown
```

In a Leptos application, `mf2 init` makes the translation crate (it
depends on `mf2`, and builds with `mf2-build`); the application adds the
Leptos layer and, for its server, the Axum one:

```sh
mf2 -C i18n init --name my-app-i18n --locale fr
cargo add my-app-i18n --path i18n
cargo add leptos-mf2@1
cargo add mf2-axum@1 --optional   # turned on by the application's `ssr` feature
```

Leptos 0.9 is the default; on Leptos 0.8, add both with
`--no-default-features --features leptos-0-8`.
[Getting started](docs/getting-started.md) builds a complete application
step by step.

In a native application, the translation crate's build script emits
`mf2_build::Emit::Native`, and the application adds the native layer and,
for a terminal UI, the Ratatui one:

```sh
cargo add mf2-native@1
cargo add mf2-ratatui@1           # only for a Ratatui application
```

[Native CLI and Ratatui apps](docs/native-apps.md) builds one, from the
translation crate to the draw loop.

`mf2-native` and `mf2-ratatui` are not on crates.io yet (they ship in
2.0.0): until then, name them by path into a checkout of this repository
instead.

## Documentation

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) explains the
ecosystem, crate roles, application workflows and integrations. Its source
and chapter links are in [`docs/`](docs/README.md). Start with the
[ecosystem overview](docs/ecosystem.md), then choose a path:
[getting started](docs/getting-started.md),
[call sites](docs/call-sites.md), [delivery modes](docs/delivery-modes.md),
[switching language](docs/switching.md),
[accessibility](docs/accessibility.md),
[native CLI and TUI apps](docs/native-apps.md),
[migrating from `leptos-fluent`](docs/migrating-from-leptos-fluent.md) and
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

* Plans and decisions: [`plans/`](plans/README.md). Start with
  [`plans/00-master-plan.md`](plans/00-master-plan.md).
* Current work order: [`plans/17-phase-9-work-order.md`](plans/17-phase-9-work-order.md).
* Vendored, pinned inputs (read-only): [`third_party/`](third_party/). The MF2
  specification text is not among them: its license does not allow public
  redistribution, so `cargo xtask spec-sync` fetches it, at the pinned commit
  and checked against recorded digests, into `target/xtask-cache/`. The
  conformance tests need it; run the command once after cloning.

```sh
cargo xtask spec-sync          # fetch the MF2 specification text (once)
cargo check --workspace        # the toolchain is pinned in rust-toolchain.toml
cargo xtask ci                 # everything the `ci` job runs, locally
cargo xtask docs               # compile every sample in docs/
```

## License

MIT — see [`LICENSE`](LICENSE). Vendored material under `third_party/` keeps its
own license (Unicode License v3), stated in each directory.
