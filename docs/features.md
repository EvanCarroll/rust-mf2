# Features of `mf2`

An application names `mf2` once, with the features it needs; everything
else is left out of the build. None is on by default. The applications
`mf2 init` makes choose them for each kind of application; this page says
what each one does.

A feature decides which functions a message may use, so the build script
and `mf2 check` read the features cargo resolves for the crate, and a
message that calls a function whose feature is off is the
[`gated-function`](lints.md#gated-function) error.

`mf2 check` also prints the list for the corpus: the function features its
messages need, those that are on, those on and unused, and the features to
write on `mf2` with the modes kept as they are (`--format json` has the same
under `features`). When both date backends are on, it says that
`datetime-icu` formats on every target.

## The Leptos line

### `leptos`

The Leptos layer renders with Leptos 0.9, the default line.

### `leptos-0-8`

The same layer with Leptos 0.8, for an application that stays on it.

A Leptos mode needs one line, and both at once is a compile error that says
what to write. The line goes on the `mf2` dependency
(`features = ["leptos"]`); the mode goes in the application's own feature
of the same name, beside Leptos's.

## A Leptos mode

Exactly one, in the application's feature of the same name
(`ssr = ["leptos/ssr", "mf2/ssr"]`). Each brings `mf2::leptos`: `tr!` in
text, attributes and props, the catalog of the request or the page, the live
switch and the page's components; and each implies its host.

### `ssr`

Rendered on the server. Implies `host-std`.

### `hydrate`

The server-rendered page, hydrated in the browser. Implies `host-web`.

### `csr`

Built and rendered in the browser alone. Implies `host-web`.

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

## A server

### `axum`

`mf2::axum`, with or without Leptos: the reader's language chosen from the
request, the catalogs served from the server binary under `/i18n/`
(precompressed), and the generated `Locale` as an extractor, with
`Locale::format`. Implies `host-std`.

## A native application

### `native`

`mf2::native`, for a command-line tool or a terminal UI: one corpus's
catalogs embedded in the executable or shipped beside it (checked against
the content hash in their names), installed once for the process, in the
system's language and time zone. A description's `Display`, `to_string()`
and `to_cow()` then show its text. Implies `host-std`.

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

## Functions

### `fn-number`

Numbers in the reader's language: its decimal and grouping separators,
digits and numbering system for `:number`, `:integer` and unannotated
numbers; and `:percent`, `:currency` and `:unit`. Without it, a number is
written with neutral symbols (`1234.5`), and the build says so
([`neutral-numbers`](lints.md#neutral-numbers)).

### `fn-datetime`

`:datetime`, `:date` and `:time`, and date and time values without a
function. On its own it formats with a neutral stand-in; a date backend
(below) gives it the reader's language. With a Leptos mode, dates are shown
in the reader's time zone: the browser reports its zone, a page the server
rendered in another zone is corrected after hydrating, and the `mf2_tz`
cookie lets the server render the next page in it. Off, none of this is in
the client.

### `datetime-icu`

Dates formatted by ICU4X on the server and in the browser, with the data
each language needs in its catalog (`icu.blob`). Implies `fn-datetime`.

### `datetime-intl`

Dates formatted by the browser's `Intl.DateTimeFormat` in a browser build,
and by ICU4X with its compiled data everywhere else. Implies `fn-datetime`.

### `number-intl`

In a browser build, numbers are formatted and plurals chosen by the
browser's `Intl.NumberFormat` and `Intl.PluralRules` (which needs a browser
with `Intl.NumberFormat` v3), instead of Rust code in the wasm. Every other
build keeps the Rust code.

## Hosts and tools

A mode implies its host; an application rarely names one.

### `host-std`

Formatting on a native target: servers, tests, `wasm32-wasip1`.

### `tzdb-bundled`

A named time zone is looked up in the IANA database jiff carries, not the
one the machine has. A server turns this on — `ssr` and `axum` do it for you
— so that every reply says the same thing whatever its host holds. Without
it a native application follows its machine (`TZDIR`, else
`/usr/share/zoneinfo`, else jiff's copy where the platform has none), so a
zone amended since the binary was built is right. It adds nothing without
[`fn-datetime`](#fn-datetime), which is what reads a zone at all.

### `host-web`

Formatting in the browser.

### `compile`

`mf2::compile_str`: an ad-hoc message compiled into a one-message catalog,
for a server or a test. Never in a client.

## What a browser build pays for text

A description (what `tr!` returns) turned into a `String` in a browser
build costs code in the wasm, and the ways differ: `.to_string()` is the
leanest; `format!("{}", …)` adds a few dozen bytes; `{:?}` adds about 1 KB,
and so does an `unwrap()` or an `assert_eq!` that involves a description,
since each reaches its `Debug`. In a view, `tr!` renders without any of them.
