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
| What can messages do? | `fn-number`; a date formatter for each side ([Dates](#dates)) |
| Who supplies locale data? | `number-intl`; `<family>-number-names-intl` for the currency and unit names; the date formatter (`icu`, `intl` or `iso`); `tzdb-bundled` |
| Behaviour and tools | `static-locale`; `mark-fallback-lang`; `compile` |

A feature decides which functions a message may use, so the build script
and `mf2 check` read the features cargo resolves for the crate, and a
message that calls a function whose feature is off is the
[`gated-function`](lints.md#gated-function) error.

`mf2 check` also prints the list for the corpus: the function features its
messages need, those that are on, those on and unused, and the features to
write on `mf2` with the modes kept as they are (`--format json` has the same
under `features`). For dates it names the formatter of each side, the
features to write when a side has none, and the form of ICU4X the build
chose and why.

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
come with a [date formatter](#dates), so a tool that shows no dates
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

### Date functions

`:datetime`, `:date` and `:time` need a date formatter on each side the
application formats on. [Dates](#dates) below says which to write; with
none, a message that calls a date function is the
[`gated-function`](lints.md#gated-function) error, and a date cannot be
passed to a message at all.

## Who supplies locale data?

Each of these is a choice between **the same answer everywhere** (the data
in your catalogs, the time-zone database built into your binary) and **the
platform's data and a smaller build** (the browser's `Intl`, the machine's
time-zone database).

### `number-intl`

In a browser build, numbers are formatted and plurals chosen by the
browser's `Intl.NumberFormat` and `Intl.PluralRules` (which needs a browser
with `Intl.NumberFormat` v3), instead of Rust code in the wasm. Every other
build keeps the Rust code, and the number and plural data then stay on the
server rather than in the catalogs the browser downloads.

Write it on the `mf2` dependency line, where both builds see it, never
under the application's `hydrate` feature:

```toml
mf2 = { version = "3", features = ["leptos", "fn-number", "number-intl"] }
```

The server's build decides what goes into the catalogs the browser
downloads, so it must know that the browser formats numbers itself. With
the feature on one build only, the two write different catalogs, and the
browser asks for a catalog file the server does not serve.

### `host-web-number-names-intl`, `leptos-client-number-names-intl`

The **number split**: in a browser build, `:currency` and `:unit` take the
currency symbol, the currency or unit name and the layout around them from
the browser's `Intl.NumberFormat`, while the digits, the rounding and the
plural form stay in Rust, written with your catalog's number symbols. That
browser then downloads no currency or unit names at all. Every other build
keeps the Rust path, and those names stay on the server.

It is a family named after the side and the framework, as the date
formatters are, with one source so far — only a browser can ask `Intl`.
With no feature the names come from the catalog, as with no date feature a
date is written in the ISO stand-in. Write the Leptos name in a Leptos
application and `host-web-number-names-intl` with no framework, on the
`mf2` dependency where both builds see it:

```toml
mf2 = { version = "3", features = ["leptos", "fn-number", "leptos-client-number-names-intl"] }
```

Each turns on `fn-number`, which is what formats a currency or a unit at
all. `number-intl`, which moves the whole of number formatting into the
browser, takes precedence where both are on.

**It is off by default, and it is a trade.** Measured over nine languages,
it adds 118 B gzipped to the client and takes 227 to 387 B brotli out of
each language's catalog, so a visitor, who downloads one language, saves
roughly 100 to 270 B. It costs 4.0 to 7.4 times the time per `:currency`
placeholder and 2.5 to 4.5 times per `:unit` one — microseconds either way,
but the slower path. And the text is the browser's, which in three cases
is not ours:

* Arabic and Hebrew come back with doubled direction marks around the
  number. The text reads the same; the bytes differ.
* Arabic writes a long litre as `لتر1` — the name after the digits, where
  our catalog puts it before.
* Chromium has no unit or currency names for some languages (Welsh among
  them), and writes the unit code instead.

Beyond those, a browser that has no name for a currency writes the bare
currency code (`XTS`), where the catalog would have the name. Choose it
when a few hundred bytes per visitor is worth more to you than the same
text everywhere.

### `tzdb-bundled`

A named time zone is looked up in the IANA database built into the binary,
not the one the machine has, so that every reply says the same thing
whatever its host holds. Servers want this, and `ssr` and `axum` turn it on
for you. It adds nothing without a [date formatter](#dates), which is
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

## Dates

### Two sides, one formatter each

Dates are formatted on two sides: in a browser build, and in native code (a
server, a command-line tool, a terminal UI). Each framework has a family of
features for the side it runs on, and each family has one feature per
formatter:

| Family | For | Formatters |
|---|---|---|
| `leptos-client-datetime-` | the browser build of a Leptos application (`hydrate`, `csr`) | `icu`, `intl`, `iso` |
| `leptos-server-datetime-` | a Leptos server (`ssr`) | `icu`, `iso` |
| `axum-datetime-` | an Axum server | `icu`, `iso` |
| `native-datetime-` | a command-line tool, a terminal UI | `icu`, `iso` |
| `host-web-datetime-` | a browser build with no framework | `icu`, `intl`, `iso` |
| `host-std-datetime-` | native code with no framework | `icu`, `iso` |

The formatters:

* **`intl`**: the browser's own `Intl.DateTimeFormat`. Only a browser has
  it. The wasm carries a few hundred bytes of glue, and the catalogs carry
  no date data.
* **`icu`**: ICU4X, in Rust. The build cuts, for each language, the date
  data its messages can ask for and puts it in that language's catalog, so a
  server carries no date data for the languages it does not serve. It needs
  `mf2-build`'s `icu-blob` feature (below).
* **`iso`**: a neutral stand-in that writes ISO-style dates
  (`2006-01-02 15:04`) in every language, with no locale data and no ICU4X.

No framework turns a date formatter on for you, and no feature of `mf2` is
on by default: an application with no date in its messages names none, and
carries no date code and no date data.

### What to write

| Application | Features of `mf2` | `mf2-build` |
|---|---|---|
| server-rendered Leptos (`ssr` and `hydrate`) | `leptos-client-datetime-intl`, `leptos-server-datetime-icu` | `icu-blob` |
| client-only Leptos (`csr`) | `leptos-client-datetime-intl` | — |
| command-line tool (`native`) | `native-datetime-icu` | `icu-blob` |
| terminal UI (`ratatui`) | `native-datetime-icu` | `icu-blob` |
| Axum server, no Leptos | `axum-datetime-icu` | `icu-blob` |

Write them on the `mf2` dependency line, where both builds of a Leptos
application see them, not under the application's `ssr` or `hydrate`
feature. A feature acts only in the builds of its own side:
`leptos-client-datetime-intl` changes no code in the server, and tells the
server's build what the browser reads, so that the server keeps out of the
browser's catalogs the data the browser never reads.

For a server-rendered Leptos application, in `Cargo.toml`:

```toml
[dependencies]
mf2 = { version = "3", features = ["leptos", "fn-number", "leptos-client-datetime-intl", "leptos-server-datetime-icu"] }

[build-dependencies]
mf2-build = { version = "3", features = ["icu-blob"] }
```

`icu-blob` is a feature of the build dependency, and cargo cannot turn it on
from `mf2`'s features; the build's error names the line when it is missing,
and `mf2 check` reports the same error, from your `Cargo.toml`.
It is off by default because it adds about 16 seconds to a cold build.

When a side has a date message and no formatter, the build fails with
[`gated-function`](lints.md#gated-function), which names the features to
write for the frameworks that are on and what each formatter costs. A
server-rendered application that names only the browser's formatter does
not build until it names the server's too. `mf2 check` prints the same
line.

### What each formatter costs

From the [cost table](#what-each-feature-costs):

| Formatter | Browser wasm | Native binary | Data |
|---|---|---|---|
| `intl` | about 250 B of gzip more than `iso` | — | none of yours: the browser's |
| `icu` | about 59 to 101 KB of gzip more than `iso`, by the form below | about 329 KB | each language's date slice, in its catalog |
| `iso` | about 5 KB of gzip | about 167 KB, mostly time zones | none |

Each figure is against the same application with no date formatter, except
where it says otherwise. A formatter on in an application whose messages
show no date costs nothing: the table measures 2 B less in the browser and
40 B less natively.

The date slice of one language, added to a catalog and compressed with
brotli, is a few hundred bytes for a corpus of a handful of date shapes
(298 to 466 B for the reference workload),
and about 16 to 18 KB once a message shows a time-zone name. A browser that
formats with `intl` downloads none of it.

Only a date function formats a date: a plain placeholder never does (a
date handed to one is an error). A formatter on in a corpus with no date
function therefore has nothing to format, and the build warns with
[`unused-feature`](lints.md#unused-feature).

### The features, one by one

#### `leptos-client-datetime-intl`, `host-web-datetime-intl`

Dates formatted by the browser's `Intl.DateTimeFormat` in the browser
build. The catalogs the browser downloads carry no date data, and the wasm
links no ICU4X.

#### `leptos-client-datetime-icu`, `host-web-datetime-icu`

Dates formatted by ICU4X in the browser build, over the date slice in each
language's catalog: the same text as an ICU4X server. Needs `mf2-build`'s
`icu-blob`.

#### `leptos-client-datetime-iso`, `host-web-datetime-iso`

ISO-style dates in the browser build, in every language.

#### `leptos-server-datetime-icu`, `axum-datetime-icu`, `native-datetime-icu`, `host-std-datetime-icu`

Dates formatted by ICU4X in native code, over the date slice in each
language's catalog (for a server, in its own table beside the catalog when
the browser does not read the slice). Needs `mf2-build`'s `icu-blob`.

#### `leptos-server-datetime-iso`, `axum-datetime-iso`, `native-datetime-iso`, `host-std-datetime-iso`

ISO-style dates in native code, in every language, with no ICU4X and no
date data.

#### `datetime`

The date functions themselves, which every feature above turns on. It is
not written by hand: alone it gives no side a formatter, so a date message
is still the `gated-function` error. With a Leptos mode it also brings
dates in the reader's time zone: the browser reports its zone, a page the
server rendered in another zone is corrected after hydrating, and the
`mf2_tz` cookie lets the server render the next page in it. Without a date
formatter none of this is in the client.

### One formatter per build

The server binary and the browser's wasm are two builds, and each formats
with the formatter of its own side. `leptos-client-datetime-intl` with
`leptos-server-datetime-icu` is `Intl` in the browser and ICU4X on the
server: nothing conflicts.

Inside one build, dates are formatted by one formatter, since
`Locale::format` is shared by terminal code, Axum handlers and the Leptos
server. When more than one formatter of a side is on, the strongest
formats: `icu`, then `intl`, then `iso`. That happens with two features of
one family, or with two frameworks in one build that disagree: a
command-line tool built with `native-datetime-iso` whose optional web mode
adds `axum-datetime-icu` formats with ICU4X when the web mode is on, and
without it stays ISO and links no ICU4X. The build warns with
[`several-date-formatters`](lints.md#several-date-formatters) and names
the one that formats.

### A date needs a date function

Only `:date`, `:time` and `:datetime` format a date. A date passed to a
bare placeholder (`{$when}`) is an error at run time, which the message
shows as its fallback, `{$when}`; write `{$when :datetime}`. Numbers are
different: a number in a bare placeholder is still formatted as a number.

The build fails with [`date-mismatch`](lints.md#date-mismatch) when one
language's message formats a variable with a date function and another
language's message shows the same variable bare, since one of them would
show `{$when}` where the other shows a date.

A function of your own that formats dates says so: its
`mf2::Function` implementation returns `"datetime"` from `part_kind`, and
it is registered under [`[functions]`](configuration.md#functions) in
`mf2.toml`. A message that calls it is then a date message, as one that
calls `:datetime` is: a Leptos client rewrites it after hydrating when the
server's formatter or time zone was not its own (below). `date-mismatch`
counts only the three built-in date functions.

### Server rendering and the browser

A server-rendered page is formatted twice: by the server, and by the
browser once it hydrates.

* **ICU4X on the server, `Intl` in the browser** (what the tools
  recommend). The server writes dates in the reader's language, and the
  browser leaves them as they are until the text next changes. The two can
  differ, since the server's text comes from your catalogs' data and the
  browser's from its own. Known cases: Polish short dates in all three
  browser engines; the joiner between a Spanish long date and its time in
  Chromium and WebKit; the Arabic short one in Chromium; and a narrow
  no-break space where the other writes a space.
* **ISO on the server, `Intl` in the browser**
  (`leptos-server-datetime-iso`). The smallest server: no ICU4X and no date
  data. The page arrives with ISO-style dates, and once it hydrates the
  browser rewrites every message that formats a date. A reader without
  JavaScript, and a crawler, see the ISO dates only.
* **ICU4X on both sides** (`leptos-client-datetime-icu`). The same text on
  both sides, for the price of ICU4X in the wasm and the date slice in each
  catalog the browser downloads.

The page states the server's formatter and time zone; a message is
rewritten after hydration only when it formats a date and either differs
from the browser's.

### What ships where

Data goes only to a side that reads it:

| Data | In the catalogs a browser downloads | In the server binary |
|---|---|---|
| the messages | yes | yes |
| ICU4X's date slice | only with `icu` in the browser | with `icu` on the server |
| number and plural data | unless `number-intl` | always, when a message needs it |
| currency and unit names | unless `number-intl` or a `-number-names-intl` feature | always, when a message needs them |

A server keeps what only it reads in a table of its own beside each
catalog, which no browser downloads. A command-line tool or a terminal UI
has one reader, so every piece of data it reads stays in its catalogs.
`mf2 stats` lists, for each language, every piece with its size, who reads
it and where it ships, and the bytes a browser downloads and never reads.

### The form of ICU4X

With `icu`, the build links the narrowest form of ICU4X the messages need:
time-zone names only if some message has `timeZoneStyle`, and calendars
other than Gregorian only if some language prefers another calendar or a
message asks for one. The form is a large part of what `icu` costs in the
browser: about 59 KB of gzip more than `iso` for Gregorian dates without
zone names, and about 42 KB more again for every calendar with zone names.
Natively the widest form adds about 152 KB. `mf2 check` prints the form chosen and why,
and [`[dates]`](configuration.md#dates) in `mf2.toml` overrides it.

## Time zones

A date in a named zone (`America/New_York`) needs that zone's rules. Where
they come from depends on the kind of application, and only with a date
formatter on:

* **In the browser** (`hydrate`, `csr`): the browser's own data, with
  any formatter. The wasm carries no zone rules.
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

Two figures are negative because those features replace code rather than
add it (the two no-date rows, a few bytes either side of nothing, are a
date formatter that links nothing when no message shows a date):

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
* **The number split in the browser** (`leptos-client-number-names-intl`)
  saves 227 to 387 B brotli in each language's catalog for 118 B gzipped in
  the client: roughly 100 to 270 B for a visitor. You give up the same
  currency and unit names everywhere — Arabic and Hebrew gain direction
  marks, Arabic writes `لتر1` for a long litre, Chromium has no names for
  some languages (Welsh), and a browser with no name for a currency writes
  its code — and pay 4.0 to 7.4 times the time for a currency and 2.5 to
  4.5 for a unit.
* **`intl` rather than `icu` in the browser** saves about 59 to 101 KB of gzip,
  and the date slice in each catalog. You give up the same dates
  everywhere: the browser's data writes them in the browser, your
  catalogs' on the server ([known differences](#server-rendering-and-the-browser)).
* **`iso` on the server** with `intl` in the browser saves about 161 KB of
  the server binary and every date slice. You give up localized dates in
  the page as the server sends it: a reader without JavaScript, and a
  crawler, see ISO-style dates.
* **`iso` in a native application** costs about 167 KB, with time zones;
  ICU4X costs about 161 KB more, and the slices. You give up dates in the
  reader's language: they are written in a neutral, ISO-style form.
* **Leaving `tzdb-bundled` off** in a native application saves 248 KB. You
  give up the same zone rules on every machine, and a container with no
  time-zone data cannot resolve named zones. A server has it on through
  `ssr` or `axum`.
* **Leaving out a function feature no message uses** saves its whole cost:
  2.5 KB of gzip in the browser for `fn-number`, 10 KB natively. You give
  up nothing. A date formatter no message uses already costs nothing, in
  the browser or natively: its code is linked only when a message calls a
  date function.

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
