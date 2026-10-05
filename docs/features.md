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
| What can messages do? | a number formatter and a date formatter for each side ([Numbers](#numbers), [Dates](#dates)) |
| Who formats, and from whose data? | the number formatter (`builtin`, `intl` or `plain`); the date formatter (`icu`, `intl` or `iso`); `tzdb-bundled` |
| Behaviour and tools | `static-locale`; `mark-fallback-lang`; `compile` |

A feature decides which functions a message may use, so the build script
and `mf2 check` read the features cargo resolves for the crate, and a
message that calls a function no side has a formatter for is the
[`gated-function`](lints.md#gated-function) error.

`mf2 check` also prints the list for the corpus: whether its messages need
numbers or dates, the features of each that are on, those on and unused, and
the features to write on `mf2` with the modes kept as they are
(`--format json` has the same under `features`). For numbers and for dates it
names the formatter of each side and the features to write when a side has
none; for dates, also the form of ICU4X the build chose and why.

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

### Number functions

`:number`, `:integer` and `:offset`, `:percent`, `:currency` and `:unit`,
and choosing a plural form, need a number formatter on each side the
application formats on. [Numbers](#numbers) below says which to write; with
none, a message that calls a number function is the
[`gated-function`](lints.md#gated-function) error, and a number passed to a
bare placeholder (`{$count}`) prints in plain digits (`1234.5`), which the
build warns of ([`plain-numbers`](lints.md#plain-numbers)).

### Date functions

`:datetime`, `:date` and `:time` need a date formatter on each side the
application formats on. [Dates](#dates) below says which to write; with
none, a message that calls a date function is the
[`gated-function`](lints.md#gated-function) error, and a date cannot be
passed to a message at all.

## Who formats, and from whose data?

Each of these is a choice between **the same answer everywhere** (the data
in your catalogs, the time-zone database built into your binary) and **the
platform's data and a smaller build** (the browser's `Intl`, the machine's
time-zone database). A side's number formatter and its date formatter make
that choice for numbers and for dates ([Numbers](#numbers),
[Dates](#dates)); `tzdb-bundled` makes it for time zones.

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

## Numbers

### Two sides, one number formatter each

Numbers are formatted on two sides: in a browser build, and in native code
(a server, a command-line tool, a terminal UI). Each framework has a family
of features for the side it runs on, and each family has one feature per
formatter:

| Family | For | Formatters |
|---|---|---|
| `leptos-client-number-` | the browser build of a Leptos application (`hydrate`, `csr`) | `builtin`, `intl`, `plain` |
| `leptos-server-number-` | a Leptos server (`ssr`) | `builtin`, `plain` |
| `axum-number-` | an Axum server | `builtin`, `plain` |
| `native-number-` | a command-line tool, a terminal UI | `builtin`, `plain` |
| `host-web-number-` | a browser build with no framework | `builtin`, `intl`, `plain` |
| `host-std-number-` | native code with no framework | `builtin`, `plain` |

The formatters:

* **`builtin`**: `mf2`'s own code. The build cuts, for each language, the
  CLDR number data its messages can ask for (symbols and patterns, plural
  rules, the currencies and units they name) and puts it in that language's
  catalog. Every side that uses it writes the same text.
* **`intl`**: the browser's own `Intl.NumberFormat` and `Intl.PluralRules`.
  Only a browser has it, and it needs one with `Intl.NumberFormat` v3. The
  catalogs the browser downloads carry no number data and no plural rules.
* **`plain`**: plain digits (`1234.5`) in every language, with the
  catalog's plural rules to choose a plural form and no other number data.
  A percentage, an amount of money and a measure cannot be written that way,
  so with `plain` the functions `:percent`, `:currency` and `:unit` are the
  `gated-function` error.

What each one does:

| Formatter | `:number`, `:integer`, `:offset`, a number in a bare placeholder | `:percent`, `:currency`, `:unit` | Plural selection | Catalog data its side reads |
|---|---|---|---|---|
| `builtin` | `mf2`'s code, in the language's own form | `mf2`'s code | `mf2`'s code | plural rules, symbols, patterns, currency and unit data |
| `intl` | `Intl.NumberFormat` | `Intl.NumberFormat` | `Intl.PluralRules` | none |
| `plain` | plain digits (`1234.5`) | build error | `mf2`'s code | plural rules |
| none | a number function is a build error; a bare number prints in plain digits, with a warning | build error | build error | none |

No framework turns a number formatter on for you, and no feature of `mf2`
is on by default: an application whose messages show no number names none.

### What to write for numbers

| Application | Features of `mf2` |
|---|---|
| server-rendered Leptos (`ssr` and `hydrate`) | `leptos-client-number-intl`, `leptos-server-number-builtin` |
| client-only Leptos (`csr`) | `leptos-client-number-intl` |
| command-line tool (`native`) | `native-number-builtin` |
| terminal UI (`ratatui`) | `native-number-builtin` |
| Axum server, no Leptos | `axum-number-builtin` |

These are what `mf2 init` writes. Write them on the `mf2` dependency line,
where both builds of a Leptos application see them, not under the
application's `ssr` or `hydrate` feature. A feature acts only in the builds
of its own side: `leptos-client-number-intl` changes no code in the server,
and tells the server's build what the browser reads, so that the server
keeps out of the browser's catalogs the data the browser never reads.

For a server-rendered Leptos application, in `Cargo.toml`:

```toml
[dependencies]
mf2 = { version = "3", features = ["leptos", "leptos-client-number-intl", "leptos-server-number-builtin"] }
```

When a side has a number message and no formatter, the build fails with
[`gated-function`](lints.md#gated-function), which names the features to
write for the frameworks that are on and what each formatter costs.
`mf2 check` prints the same line.

### What each number formatter costs

From the [cost table](#what-each-feature-costs):

| Formatter | Browser wasm | Native binary | Data |
|---|---|---|---|
| `plain` | the smallest that formats a number | the smallest that formats a number | the plural rules, when a message chooses a plural form |
| `intl` | about 0.5 KB of gzip less than `plain` | — | none of yours: the browser's |
| `builtin` | about 2.5 KB of gzip more than `plain` | about 10 KB more than `plain` | each language's number data, in its catalog |

`intl` is smaller than `plain` because the calls to the browser replace the
code that rounds, writes digits and applies plural rules.

### The number features, one by one

#### `leptos-client-number-intl`, `host-web-number-intl`

Numbers formatted and plural forms chosen by the browser's
`Intl.NumberFormat` and `Intl.PluralRules` in the browser build. The
catalogs the browser downloads carry no number data and no plural rules.

#### `leptos-client-number-builtin`, `host-web-number-builtin`

Numbers formatted by `mf2`'s own code in the browser build, from the number
data in each language's catalog: the same text as a server on `builtin`.

#### `leptos-client-number-plain`, `host-web-number-plain`

Plain digits in the browser build, in every language, and plural forms
chosen by the catalog's rules. No `:percent`, `:currency` or `:unit`.

#### `leptos-server-number-builtin`, `axum-number-builtin`, `native-number-builtin`, `host-std-number-builtin`

Numbers formatted by `mf2`'s own code in native code, from the number data
in each language's catalog (for a server, in its own table beside the
catalog when the browser does not read it).

#### `leptos-server-number-plain`, `axum-number-plain`, `native-number-plain`, `host-std-number-plain`

Plain digits in native code, in every language, and plural forms chosen by
the catalog's rules. No `:percent`, `:currency` or `:unit`.

#### `number`

The number functions themselves, which every feature above turns on. It is
not written by hand: alone it gives no side a formatter, so a number message
is still the `gated-function` error.

### One number formatter per build

The server binary and the browser's wasm are two builds, and each formats
with the formatter of its own side. `leptos-client-number-intl` with
`leptos-server-number-builtin` is `Intl` in the browser and `mf2`'s own code
on the server: nothing conflicts.

When more than one number formatter of a side is on, the strongest formats:
`builtin`, then `intl`, then `plain`. The build warns with
[`several-formatters`](lints.md#several-formatters) and names the one that
formats.

### Numbers on the server and in the browser

A server-rendered page is formatted by the server, and again by the browser
whenever its text changes.

* **`builtin` on the server, `intl` in the browser** (what the tools
  recommend). The server writes numbers and picks plural forms from your
  catalogs' data, the browser from its own `Intl` data, so the text can
  differ slightly between browsers and from the server's rendering. The
  browser downloads no number data, and the reader needs a browser with
  `Intl.NumberFormat` v3.
* **`builtin` on both sides.** The same text on both sides, for about 3 KB
  of gzip more wasm than `intl` and the number data in each catalog the
  browser downloads.
* **`plain` on a side** writes plain digits there, whatever the other side
  writes.

## Dates

### Two sides, one formatter each

Dates are formatted on two sides: in a browser build, and in native code (a
server, a command-line tool, a terminal UI). Each framework has a family of
features for the side it runs on, and each family has one feature per
formatter:

| Family | For | Formatters |
|---|---|---|
| `leptos-client-datetime-` | the browser build of a Leptos application (`hydrate`, `csr`) | `icu-cached`, `icu`, `intl`, `iso` |
| `leptos-server-datetime-` | a Leptos server (`ssr`) | `icu`, `iso` |
| `axum-datetime-` | an Axum server | `icu`, `iso` |
| `native-datetime-` | a command-line tool, a terminal UI | `icu`, `iso` |
| `host-web-datetime-` | a browser build with no framework | `icu-cached`, `icu`, `intl`, `iso` |
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
mf2 = { version = "3", features = ["leptos", "leptos-client-number-intl", "leptos-server-number-builtin", "leptos-client-datetime-intl", "leptos-server-datetime-icu"] }

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

#### `leptos-client-datetime-icu-cached`, `host-web-datetime-icu-cached`

The same ICU4X formatter with its cache: ICU4X's data is set up once for
each catalog and a formatter is kept for each language and date shape,
where `icu` builds both again for every placeholder. The text is the same.
It adds 1,667 B of gzip to the client, measured on the module
`tools/e2e/datetime/speed.sh` builds, and formats a date 3.5 to 8.9 times
faster than `icu` does. In native code `icu` always has the cache, which
costs no download there.

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
[`several-formatters`](lints.md#several-formatters) and names the one that
formats.

### A date needs a date function

Only `:date`, `:time` and `:datetime` format a date. A date passed to a
bare placeholder (`{$when}`) is an error at run time, which the message
shows as its fallback, `{$when}`; write `{$when :datetime}`. Numbers are
different: a number in a bare placeholder is still formatted as a number,
by its side's number formatter.

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
| the plural rules | with `builtin` or `plain` in the browser | with `builtin` or `plain` on the server |
| number, currency and unit data | only with `builtin` in the browser | with `builtin` on the server |

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

* **`leptos-client-number-intl` in the browser.** It takes the code that
  rounds a number, writes its digits and applies plural rules out of the
  wasm, and calls the browser's `Intl` in its place. The calls are smaller
  than the code they replace, so it weighs less than `plain`.
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

* **`intl` rather than `builtin` for numbers in the browser** saves about
  3 KB of gzip, and the number data in each catalog. You give up the same
  text everywhere: the browser's `Intl` data, not your catalog's, writes
  numbers and picks plural forms, so it can differ slightly between
  browsers and from the server's rendering; and the reader needs a browser
  with `Intl.NumberFormat` v3.
* **`plain` rather than `builtin` for numbers** saves about 2.5 KB of gzip
  in the browser and 10 KB natively, and the number data. You give up
  numbers in the reader's language: they are written in plain digits, and
  `:percent`, `:currency` and `:unit` do not build.
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
* **A formatter no message uses** costs nothing, in the browser or
  natively: a date formatter's code is linked only when a message calls a
  date function, and a number formatter's only when a message calls a
  number function, chooses a plural form or has a bare placeholder, which
  may be handed a number.

You do not have to work that out yourself. `mf2 check` prints what the
corpus needs, the features on and unused, and the line to write on `mf2`;
and the build warns with [`unused-feature`](lints.md#unused-feature) when
a formatter is on and no message can use it.

## What a browser build pays for text

A description (what `tr!` returns) turned into a `String` in a browser
build costs code in the wasm, and the ways differ: `.to_string()` is the
leanest; `format!("{}", …)` adds a few dozen bytes; `{:?}` adds about 1 KB,
and so does an `unwrap()` or an `assert_eq!` that involves a description,
since each reaches its `Debug`. In a view, `tr!` renders without any of them.
