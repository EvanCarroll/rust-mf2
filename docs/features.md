# Features of `mf2`

An application names `mf2` once, with the features it needs. **None is on by
default**, so `default-features = false` is never needed: there is nothing to
turn off. The starters `mf2 init` makes choose the features for each kind of
application; this page says what each one does and what it costs.

A feature exists only where the linker cannot decide for you: it changes
which crates are built, changes behaviour, or chooses who supplies locale
data. Everything else is linked because the generated module names it, and
that module names only what your messages use. So a generous feature list
costs build time rather than bytes, and the starters' lists stay simple.

The features answer four questions:

| Question | Features |
|---|---|
| Where does it run? | `leptos` or `leptos-0-8`, with one of `ssr`, `hydrate`, `csr`; `axum`; `native`; `ratatui`; `clap`; with no framework, `host-std` or `host-web` |
| What can messages do? | `fn-number`; `fn-datetime` |
| Who supplies locale data? | `number-intl`; `datetime-icu`; `datetime-intl`; `tzdb-bundled` |
| Behaviour and tools | `static-locale`; `mark-fallback-lang`; `compile` |

A feature decides which functions a message may use, so the build script
and `mf2 check` read the features cargo resolves for the crate, and a
message that calls a function whose feature is off is the
[`gated-function`](lints.md#gated-function) error.

`mf2 check` also prints the list for the corpus: the function features its
messages need, those that are on, those on and unused, and the features to
write on `mf2` with the modes kept as they are (`--format json` has the same
under `features`). When both date backends are on, it says which one
formats where: `datetime-intl` in the browser, `datetime-icu` everywhere
else.

## Where does it run?

### `leptos`

The Leptos layer renders with Leptos 0.9, the default line.

### `leptos-0-8`

The same layer with Leptos 0.8, for an application that stays on it.

A Leptos mode needs one line, and both at once is a compile error that says
what to write. The line goes on the `mf2` dependency
(`features = ["leptos"]`); the mode goes in the application's own feature
of the same name, beside Leptos's.

### `ssr`

A Leptos mode: rendered on the server. Exactly one mode is on, in the
application's feature of the same name (`ssr = ["leptos/ssr", "mf2/ssr"]`).
Each mode brings `mf2::leptos`: `tr!` in text, attributes and props, the
catalog of the request or the page, the live switch and the page's
components; and each implies its host. `ssr` implies `host-std` and
`tzdb-bundled`.

### `hydrate`

The server-rendered page, hydrated in the browser. Implies `host-web`.

### `csr`

Built and rendered in the browser alone. Implies `host-web`.

### `axum`

`mf2::axum`, with or without Leptos: the reader's language chosen from the
request, the catalogs served from the server binary under `/i18n/`
(precompressed), and the generated `Locale` as an extractor, with
`Locale::format`. Implies `host-std` and `tzdb-bundled`.

### `native`

`mf2::native`, for a command-line tool or a terminal UI: one corpus's
catalogs embedded in the executable or shipped beside it (checked against
the content hash in their names), installed once for the process, in the
system's language. A description's `Display`, `to_string()` and `to_cow()`
then show its text. Implies `host-std`. Time zones are not part of it: they
come with [`fn-datetime`](#fn-datetime), so a tool that shows no dates
carries no time-zone code.

### `ratatui`

`mf2::ratatui`: a message as Ratatui `Text` or `Line`, its markup
(`{#name}…{/name}`) as styles the application maps by name. It adds
`ratatui-core` alone, whose types `ratatui` re-exports. Implies `native`.

### `clap`

The generated `Locale` gets a `clap` value parser, so that `--lang` is
matched by the same rules as the system's language (`fr_CA.UTF-8` is
French), and `--help` lists the languages.

`native`, `ratatui` and `axum` are never in a browser build: beside
`hydrate` or `csr`, each is a compile error when compiling for `wasm32`. On
the host they compile together, as a workspace's `cargo check` unifies
features.

### `host-std`

Formatting on a native target: servers, tests, `wasm32-wasip1`. A mode
implies its host, so an application with a framework rarely names one.
With no framework (no Leptos, no Axum, no `native`), `host-std` is how you
use `mf2`: a library, a test, a service of your own.

### `host-web`

Formatting in the browser, for a browser application with no Leptos mode.

## What can messages do?

### `fn-number`

Numbers in the reader's language: its decimal and grouping separators,
digits and numbering system for `:number`, `:integer` and unannotated
numbers; and `:percent`, `:currency` and `:unit`. Without it, a number is
written with neutral symbols (`1234.5`), and the build says so
([`neutral-numbers`](lints.md#neutral-numbers)).

### `fn-datetime`

`:datetime`, `:date` and `:time`, and date and time values without a
function. On its own it formats with a neutral stand-in, ISO-style dates
with no locale data; a date backend (below) gives it the reader's language.
It is also what brings time zones ([below](#time-zones)). With a Leptos
mode, dates are shown in the reader's time zone: the browser reports its
zone, a page the server rendered in another zone is corrected after
hydrating, and the `mf2_tz` cookie lets the server render the next page in
it. Off, none of this is in the client.

## Who supplies locale data?

Each of these is a choice between **the same answer everywhere** (the data
in your catalogs, the time-zone database built into your binary) and **the
platform's data and a smaller build** (the browser's `Intl`, the machine's
time-zone database).

### `number-intl`

In a browser build, numbers are formatted and plurals chosen by the
browser's `Intl.NumberFormat` and `Intl.PluralRules` (which needs a browser
with `Intl.NumberFormat` v3), instead of Rust code in the wasm. Every other
build keeps the Rust code.

### `datetime-icu`

Dates formatted by ICU4X on the server and in the browser, with the data
each language needs in its catalog (`icu.blob`). The same text everywhere.
Implies `fn-datetime`. It needs `mf2-build`'s `icu-blob` feature in the
build dependency too; cargo cannot tie the two, and the build error says
so.

### `datetime-intl`

Dates formatted by the browser's `Intl.DateTimeFormat` in a browser build,
and by ICU4X with its compiled data everywhere else. Implies `fn-datetime`.
With both date backends on, the browser build formats with `datetime-intl`
and carries no ICU4X date code or data, while the server and native builds
format with `datetime-icu`, and `mf2 check` says so. A page rendered on the
server can then show a date one way and, once the browser hydrates it,
another: the server's text comes from your catalog's data, the browser's
from its own `Intl`.

### `tzdb-bundled`

A named time zone is looked up in the IANA database built into the binary,
not the one the machine has, so that every reply says the same thing
whatever its host holds. Servers want this, and `ssr` and `axum` turn it on
for you. It adds nothing without [`fn-datetime`](#fn-datetime), which is
what reads a zone at all.

## Behaviour and tools

### `static-locale`

No live switch: a language switch sets a cookie and loads the page again,
and rendered text registers nothing to update. It suits
[islands](delivery-modes.md#islands).

### `mark-fallback-lang`

Text borrowed from a fallback language (a message not translated yet) is
wrapped in a `<span lang>` of its own language, and `dir` when its
direction differs from the page's, identically on the server and in the
browser (WCAG 2.2's Language of Parts). Only a borrowed message in the page's
text is wrapped, so a page with no missing translation is unchanged; an
attribute or a string cannot carry a `lang` and stays unmarked.

### `compile`

`mf2::compile_str`: an ad-hoc message compiled into a one-message catalog,
for a server or a test. Never in a client.

## Time zones

A date in a named zone (`America/New_York`) needs that zone's rules. Where
they come from depends on the kind of application, and only with
`fn-datetime` on:

* **In the browser** (`hydrate`, `csr`): the browser's own data, with
  either date backend. The wasm carries no zone rules.
* **On a server** (`ssr`, `axum`): the database built into the binary,
  because both turn on `tzdb-bundled`. Every server gives the same answer,
  whatever its machine holds.
* **In a native application** (`native`, `ratatui`): the machine's
  database — `TZDIR`, else `/usr/share/zoneinfo`, else, on a platform that
  has none (Windows), a copy built in. A zone amended after the binary was
  built is then still right. Dates are shown in the system's zone, and
  `mf2::native::set_time_zone` changes it.
* **In a container with no time-zone data** (a `scratch` or distroless
  image, say): there is no `/usr/share/zoneinfo` to read, so turn on
  `tzdb-bundled`. A server already has it; a native tool shipped in such an
  image needs it written out.

## What each feature costs

Each figure is measured, not estimated, on the smallest application of its
kind, and re-measured every night:

{{#include feature-costs.md}}

Two figures are negative, because those features replace code rather than
add it:

* **`number-intl` in the browser.** It takes the Rust code that formats
  numbers and chooses plurals out of the wasm, and calls the browser's
  `Intl` in its place. The calls are smaller than the code they replace.
* **`ratatui` natively.** It is set against the same terminal UI making its
  Ratatui `Line` itself from a formatted `String`. Ratatui's conversion from
  a `String` links its tables of character display widths (about 9 KB) and
  its line splitting; `mf2::ratatui` builds the line's spans directly and
  needs neither. An application that lays out text by width links those
  tables anyway, so there expect `ratatui` to cost about nothing rather
  than to save.

## A smaller build

The levers, each with what it saves on the figures above and what it gives
up:

* **`number-intl` in the browser** saves about 3 KB of gzip. You give up the
  same text everywhere: the browser's `Intl` data, not your catalog's,
  writes numbers and picks plural forms, so it can differ slightly between
  browsers and from the server's rendering; and the reader needs a browser
  with `Intl.NumberFormat` v3.
* **`datetime-intl` rather than `datetime-icu`** saves about 100 KB of gzip
  in the browser. You give up the same dates everywhere: the browser's data
  writes them in the browser, ICU4X's compiled data on the server.
* **`fn-datetime` with no backend** costs about 6 KB of gzip in the browser
  (168 KB natively, with time zones), against 100 KB with ICU4X. You give
  up dates in the reader's language: they are written in a neutral,
  ISO-style form.
* **Leaving `tzdb-bundled` off** in a native application saves 248 KB. You
  give up the same zone rules on every machine, and a container with no
  time-zone data cannot resolve named zones. A server has it on through
  `ssr` or `axum`.
* **Leaving out a function feature no message uses** saves its whole cost:
  2.5 KB of gzip in the browser for `fn-number`, 8 KB natively; 168 KB
  natively for `fn-datetime`. You give up nothing.

You do not have to work out the last one yourself. `mf2 check` prints the
features the corpus needs, those on and unused, and the line to write on
`mf2`; and the build warns with
[`unused-feature`](lints.md#unused-feature) when a function feature is on
and no message can use it.

## What a browser build pays for text

A description (what `tr!` returns) turned into a `String` in a browser
build costs code in the wasm, and the ways differ: `.to_string()` is the
leanest; `format!("{}", …)` adds a few dozen bytes; `{:?}` adds about 1 KB,
and so does an `unwrap()` or an `assert_eq!` that involves a description,
since each reaches its `Debug`. In a view, `tr!` renders without any of them.
