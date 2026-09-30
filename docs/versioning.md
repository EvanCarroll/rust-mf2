# Versions and what 2.x promises

Every crate of Rust MF2 is released together, at one version: `mf2 2.0.0`
goes with `mf2-build 2.0.0`, and each crate asks for the others at exactly
that version. An application names two: `mf2`, with the features it needs
(a Leptos line and a mode, `axum`, `native`, `ratatui`, …), and
`mf2-build` in its translation crate's build; `cargo update` moves them
together. `mf2-cli` is not a dependency: it is the `mf2` command, installed
with `cargo install mf2-cli`, at the same version as the rest. The other
crates are what these are built on, and an application does not name them.

The version numbers follow [Semantic Versioning](https://semver.org):
within 2.x, a patch release fixes things and a minor release adds them;
neither breaks a program that 2.0 built. Anything that would is 3.0.

## What 2.x promises

* **The public API of the published crates** — every item their
  documentation on docs.rs shows, with the feature flags that turn items
  on.
  `mf2`'s promise is **per mode**: what it offers depends on the mode an
  application turns on, and the Leptos modes exclude one another, so its
  API is listed once for each — the core (no mode), `ssr`, `hydrate`,
  `csr`, `native`, `ratatui` and `axum`. An item a mode has, that mode
  keeps within 2.x; a mode may gain items in a minor release.
  A type marked `#[non_exhaustive]` may gain a variant or a field in a
  minor release: match it with a `_` arm, and build it with its
  constructor or from `Default`, not with a struct literal. Among them
  are the MF2 data model's alternatives (a later MF2 may add structures),
  error types, the functions' option values, `mf2.toml`'s `Config`, and
  what a build or a negotiation returns.
* **The forms of `tr!`**: in text, attributes, props and strings, with
  arguments, signals and markup, as [Call sites](call-sites.md) shows them.
  A call site that compiles under 2.0 compiles under every 2.x, and a
  message that is checked at compile time stays checked.
* **The generated module's items** — what `mf2-build` writes into the
  translation crate, as the book shows them: `tr!` and `msg_id!`,
  `Locale` (its variants, `ALL`, `SOURCE`, `tag`, `dir`, `best_match`,
  its parsing and `Display`, `format` with `native`, `name` when every
  language names itself), `LOCALES`, `SOURCE_LOCALE`, `LANGUAGE_MATCHING`,
  `CATALOGS`, `CORPUS`, `registry()`, the functions that install the
  catalogs and choose the language (`setup`, `install`,
  `install_from_directory`, `set_locale`, `current_locale`,
  `preload_locale`, `with_locale`), `markup` with `ratatui`, and the
  `prelude`. Each is there when the build has what it needs, as the
  modes decide; a name that begins with `__` is not promised.
* **The `mf2` command line**: its commands (`init`, `check`, `compile`,
  `fmt`, `stats`, `dump`, `pseudo`, `export`, `import`, `watch`,
  `convert`) and their flags. A script that runs under 2.0 runs under 2.x.
* **The resource format as `mf2 fmt` writes it**: a `.mf2` file that 2.0
  accepts, 2.x accepts, with the same meaning. The layout `fmt` gives it —
  where it leaves blank lines — may change in a minor release, so a
  project that runs `mf2 fmt --check` in CI runs `mf2 fmt` once after such
  an upgrade.

## What it does not promise

* **Items hidden from the documentation** (`#[doc(hidden)]`). The code
  that `mf2-build` generates, the `tr!` macro, the runtime and the `mf2`
  command line share them; since the crates are released together and ask
  for one another at one exact version, they always agree. Do not call
  them yourself. They are:
  * what `tr!` and the generated module expand to (`tr`, `tr_args0` …
    `tr_args_n`, `tr_rich`, `tr_dyn`, `markup`, `ArgValue::str_static`,
    the argument dispatch `mf2::__arg`, the proc-macros of `mf2-macros`),
    and the bits of a `MsgId`;
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
  * the locale matcher's table and its entry points, which the generated
    module, the native module and the web server call
    (`LanguageMatching::new`, `LanguageMatching::EMPTY`,
    `LanguageMatching::best_match`, `LanguageMatching::distance_of`,
    `LanguageMatching::cldr`, `Corpus::with_language_matching`,
    `mf2::leptos::best_locale`); `LanguageMatching` itself, the generated
    `LANGUAGE_MATCHING` and `Setup::with_language_matching` are promised;
  * the switches and helpers the function crates and the build share
    (`INTL_NUMBERS`, `Number::format_by_host`,
    `mf2_fn_datetime::icu::prime`, `literal_options`,
    `CldrVersion::to_u32` / `from_u32`, `mf2_build::Error::io`), and the
    WG test suite's error names (`ErrorKind::suite_name`);
  * `mf2-locale-data`'s tables and entry builders — all but its errors,
    `CLDR_VERSION` and `direction`;
  * `mf2-resource`'s Rust API, which mirrors the draft resource format
    (below) — the format itself, as `mf2 fmt` writes it, is promised;
  * in `mf2::leptos`, what `mf2::axum` and the library's own tests use:
    the rendering glue and its view states, the table of catalogs the
    server serves, the names the page and the server share (`links`), and
    `live_nodes`, `installed`.

  Each published crate commits the list of what it does promise as
  `api.txt`, and `mf2` one list per mode in `api/` (`cargo xtask api`);
  each list's first line says it is what 2.x promises. A change to it fails the project's CI
  until the list is updated with it — which is how a change to the promise
  is seen and reviewed. From the second release on, each release is also
  compared with the version before it on crates.io by
  [cargo-semver-checks](https://crates.io/crates/cargo-semver-checks), and
  one that breaks it is refused (`cargo xtask release`). The check skips
  `mf2-macros`: a procedural-macro crate has no Rust API for the tool to
  read, and its promise is the macros' names and the `tr!` forms, which
  the project's tests hold.
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

`mf2` supports two Leptos lines in 2.x, each a feature an application
names beside its mode: **`leptos` is Leptos 0.9**, the default line, and
**`leptos-0-8` is Leptos 0.8** (see [Getting started](getting-started.md)).
Each line has its helper crate of components, `mf2-leptos-ui-0-9` and
`mf2-leptos-ui-0-8`, which `mf2` depends on; an application does not name
them. Both lines, or a mode with neither, is a compile error that says
what to write.

| When | What changes | Release |
|---|---|---|
| a new Leptos 0.9 pre-release, or 0.9's release | taken as it comes (2.0.0 is built on `0.9.0-beta`) | patch |
| a new Leptos line (0.10) | added as a feature beside the others, with its helper crate | minor |
| the `leptos` feature moves to another line, or a line is dropped | an application's build breaks | 3.0 |

## The W3C Message Resource format

The `.mf2` resource format follows a W3C draft that is not final. If the
draft changes, 2.x follows it in a way that keeps your files working:
`mf2 fmt` accepts the form 2.0 wrote and can rewrite it in the new one. A
change that would make 2.x reject a file that 2.0 accepted waits for 3.0.

## Minimum supported Rust version

**Rust 1.88.** Every crate states it as `rust-version`, and CI checks the
published crates on exactly that release, natively and for
`wasm32-unknown-unknown`, on both Leptos lines — and checks that 1.87 does
not build them, so the figure is measured rather than assumed. Leptos 0.9
itself needs 1.88.

Raising the minimum Rust version is a **minor** release, and the
changelog says so.

## Where the releases stand

**1.0.0 is on crates.io**: all sixteen crates of the 1.0 family, on
26 September 2026. **1.1.0 was never published**, and will not be: it was
prepared as a minor release after 1.0.0, and its fixes and its native
support are part of 2.0.0. **2.0.0 is the next release**, not yet
published: one crate, `mf2`, where 1.x had `leptos-mf2` and `mf2-axum`
beside it; how to move a 1.x application is in
[Upgrading from 1.x](upgrading.md). See the
[changelog](https://github.com/EvanCarroll/rust-mf2/blob/main/CHANGELOG.md).
