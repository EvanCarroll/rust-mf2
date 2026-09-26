# Versions and what 1.x promises

Every crate of mf2-two is released together, at one version: `mf2 1.2.0`
goes with `leptos-mf2 1.2.0` and `mf2-build 1.2.0`, and each asks for the
others at exactly that version. An application names the few crates it
uses (`mf2`, `mf2-build`, `mf2-cli`, `leptos-mf2`, `mf2-axum`) at the same
version, and `cargo update` moves them together.

The version numbers follow [Semantic Versioning](https://semver.org):
within 1.x, a patch release fixes things and a minor release adds them;
neither breaks a program that 1.0 built. Anything that would is 2.0.

## What 1.x promises

* **The public API of the published crates** — every item their
  documentation on docs.rs shows, with the feature flags that turn items
  on.
  A type marked `#[non_exhaustive]` may gain a variant or a field in a
  minor release: match it with a `_` arm, and build it with its
  constructor or from `Default`, not with a struct literal. Among them
  are the MF2 data model's alternatives (a later MF2 may add structures),
  error types, the functions' option values, `mf2.toml`'s `Config`, and
  what a build or a negotiation returns.
* **The forms of `tr!`**: in text, attributes, props and strings, with
  arguments, signals and markup, as [Call sites](call-sites.md) shows them.
  A call site that compiles under 1.0 compiles under every 1.x, and a
  message that is checked at compile time stays checked.
* **The `mf2` command line**: its commands (`init`, `check`, `compile`,
  `fmt`, `stats`, `dump`, `pseudo`, `export`, `import`, `watch`,
  `convert`) and their flags. A script that runs under 1.0 runs under 1.x.
* **The resource format as `mf2 fmt` writes it**: a `.mf2` file that 1.0
  accepts, 1.x accepts, with the same meaning.

## What it does not promise

* **Items hidden from the documentation** (`#[doc(hidden)]`). The code
  that `mf2-build` generates, the `tr!` macro, the runtime and the `mf2`
  command line share them; since the crates are released together and ask
  for one another at one exact version, they always agree. Do not call
  them yourself. They are:
  * what `tr!` and the generated module expand to (`tr`, `tr_args0` …
    `tr_args_n`, `tr_rich`, `tr_dyn`, `markup`, `ArgValue::str_static`,
    the proc-macros of `mf2-macros`), and the bits of a `MsgId`;
  * `mf2-build`'s pipeline — every module (`config`, `loader`, `corpus`,
    `manifest`, `check`, `slice`, `catalog`, `codegen`, `pseudo`, …) — and
    what the build hands the command line (`Outcome::catalogs`,
    `Outcome::manifest`, the loader's records); what a `build.rs` uses
    (`Build`, `Config`, `Features`, `Lint`, `Level`, `Report`, `Error`) is
    promised;
  * the compiled catalog's layout: in `mf2-catalog` everything but
    `Catalog` (loading it, its locale, direction, manifest hash, message
    count, `lookup`, its bytes), `CldrVersion` and the error types; the
    runtime's access to it (`FnContext::catalog`, `Formatter::simple_ref`,
    `StrRef`, `Sink::push_catalog_text`, `plural_category`), and the
    manifest (`Manifest`, `Compiled::manifest`);
  * the switches and helpers the function crates and the build share
    (`INTL_NUMBERS`, `Number::format_by_host`,
    `mf2_fn_datetime::icu::prime`, `literal_options`,
    `CldrVersion::to_u32` / `from_u32`, `mf2_build::Error::io`), and the
    WG test suite's error names (`ErrorKind::suite_name`);
  * `mf2-locale-data`'s tables and entry builders — all but its errors,
    `CLDR_VERSION` and `direction`;
  * `mf2-resource`'s Rust API, which mirrors the draft resource format
    (below) — the format itself, as `mf2 fmt` writes it, is promised;
  * in `leptos-mf2`, what `mf2-axum` and the library's own tests use: the
    rendering glue and its view states, the table of catalogs the server
    serves, the names the page and the server share (`links`), and
    `live_nodes`, `installed`.

  Each published crate commits the list of what it does promise as
  `api.txt` (`cargo xtask api`), and a change to it fails the project's CI
  until the list is updated with it — which is how a change to the promise
  is seen and reviewed.
* **The compiled catalog (`.mf2b`) and the manifest.** They are what a
  build produces and its server and browser read, not a format to keep. A
  server and a client built together agree; the manifest's hash is how
  each checks that. After an upgrade, rebuild both: a catalog from one
  version is not guaranteed to load in another.
* **The wording of reports** — `mf2 check`'s messages, compile errors,
  statistics. They get clearer; match on exit status, not text.
* **Exact figures**: sizes, timings and the like, which the project
  measures and budgets but which move with every dependency.

## Leptos versions

`leptos-mf2` and `mf2-axum` support two Leptos lines in 1.x: **Leptos 0.9
is the default** and **Leptos 0.8 is an opt-in** (`default-features =
false, features = ["leptos-0-8"]` on both; see
[Getting started](getting-started.md)).

| When | What changes | Release |
|---|---|---|
| a new Leptos 0.9 pre-release, or 0.9's release | taken as it comes (1.0 is published on `0.9.0-beta`) | patch |
| a new Leptos line (0.10) | added as an opt-in feature beside the others | minor |
| the default line changes, or a line is dropped | an application's build breaks | 2.0 |

## The W3C Message Resource format

The `.mf2` resource format follows a W3C draft that is not final. If the
draft changes, 1.x follows it in a way that keeps your files working:
`mf2 fmt` accepts the form 1.0 wrote and can rewrite it in the new one. A
change that would make 1.x reject a file that 1.0 accepted waits for 2.0.

## Minimum supported Rust version

**Rust 1.88.** Every crate states it as `rust-version`, and CI checks the
published crates on exactly that release, natively and for
`wasm32-unknown-unknown`, on both Leptos lines — and checks that 1.87 does
not build them, so the figure is measured rather than assumed. Leptos 0.9
itself needs 1.88.

Raising the minimum Rust version is a **minor** release, and the
changelog says so.
