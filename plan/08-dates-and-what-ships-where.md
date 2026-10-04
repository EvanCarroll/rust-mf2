# Dates, `Intl`, and what ships where

Status: **proposal, 2026-10-03, with the owner's decisions of that day (§2).**
It replaces what `plan/01-size-and-features.md` says about the date features
(its §3.3 and §3.4: `fn-datetime`, `datetime-icu`, `datetime-intl`, "both date
backends on at once"). The rest of `plan/01` stands. Phases 16 to 19 write
this, Phases 20 to 22 compile, test and measure it, and Phase 23 is the
release (§8).

## 1. What was found

Measured on 2026-10-03 at `ff5fb0c` unless a date is given. §1.3 has the
commands.

### 1.1 The three ways a date is formatted

| Way | Who does the work | Code | Data |
|---|---|---|---|
| ISO stand-in | our own Rust (`Neutral`) | browser +5,692 B gz; native +168 KB, mostly time zones | none: `2006-01-02 15:04` in every language |
| ICU4X | the ICU4X library, in Rust | browser +99,887 B gz; native +297,520 B | a per-language slice cut by the build (`icu.blob`), or ICU4X's compiled-in data for every language |
| the browser's `Intl` | `Intl.DateTimeFormat` | browser +239 B gz | none of ours |

Only a browser has `Intl`. A server, a command-line tool and a terminal UI
can use the first two only.

### 1.2 The figures

**Browser wasm, gzip.** §1.1's browser figures are `docs/feature-costs.md`'s,
on the reference workload. The size harness (`bench/b12/README.md`,
2026-09-22) has ICU4X by form: Gregorian without zone names 42,820 B;
Gregorian with zone names 69,641; any calendar without zone names 55,476;
any calendar with zone names 83,028.

**Native binary, stripped**, one language:

| Features | With a date message | No date, one plain placeholder |
|---|---:|---:|
| no date feature | 427,240 | 429,704 |
| date functions, ISO | 595,616 | 596,072 |
| ICU4X over the catalog's slice | 893,136 | 893,544 |
| ICU4X over its compiled-in data (`datetime-intl` off the browser) | 5,041,944 | — |
| ISO, plus one call to jiff's own formatter | — | 829,360 |

**The date slice, per language** (nine languages: ar de en fr hi ja ru th zh):

| Corpus | Slice, raw | Added to the catalog, brotli |
|---|---:|---:|
| one date shape | 332–889 B | 259–435 B |
| seven shapes | 582–1,695 B | 349–679 B |
| seven shapes and a zone name | 28,884–41,999 B | 15,607–17,781 B |

**Number data, per language, raw:** 12–135 B; 1,951–2,894 B when a currency
code is a variable.

**Speed, native, per date placeholder:** ICU4X 2.55–3.94 µs, 27.07 µs with a
zone name; ISO 0.32–0.47 µs.

**Cold debug build, one run each:** `mf2` with the native host and dates 5 s,
with ICU4X 15 s; `mf2-build` 11 s, with `icu-blob` 27 s.

**Numbers through `Intl`** (`bench/intl-probe/RESULTS.md`, September): 2–6×
slower per number in three engines; +2,205 B gz for plain numbers, −3,643 B
gz once currency and units are used; Firefox has no plural rules for 30 CLDR
languages, WebKit for 33, Chromium for 5, and each answers with English rules.

**Server and browser text differ** in known cases
(`tools/e2e/checks/datetime.mjs`, September): Polish short dates in three
engines; the Spanish long date-and-time joiner in Chromium and WebKit; the
Arabic short one in Chromium; narrow no-break space against space.

### 1.3 The commands

* Slices and number data: `mf2 stats --features …`, the command built with
  `--features icu-blob`, over scratch corpora.
* Native sizes: `tools/native-canary` copied, with `datetime-icu` and
  `datetime-intl` added to its features; `cargo build --release`, fat LTO,
  `CARGO_PROFILE_RELEASE_STRIP=symbols`.
* Speed: `cargo run --release -p runtime-bench --example date_cost`.
* Build time: `cargo build -p mf2 --features host-std,fn-datetime` against
  `host-std,datetime-icu`; `cargo build -p mf2-build` against
  `--features icu-blob`; each into an empty target directory.

### 1.4 What is wrong

* **F1.** With both date features on, every catalog carries the date slice,
  and a browser that formats with `Intl` downloads it and never reads it.
* **F2.** `datetime-intl` alone links ICU4X's compiled-in data into every
  build that is not a browser: 4,446,328 B against 297,520 B with slices.
  The guide tells a server-rendered application to use it.
* **F3.** In a command-line tool or a terminal UI `datetime-intl` has no
  `Intl` to use and means F2.
* **F4.** The generated module names the date statics, which are ICU4X's
  widest form. The narrower forms (`GregorianOnly`, `NoZones`) exist and
  nothing selects them.
* **F5.** Each date placeholder copies the slice into a provider and builds
  the formatter, twice. Nothing is cached.
* **F6.** With `number-intl`, the plural, number, currency and unit entries
  still go into the catalog the browser downloads.
* **F7.** For a corpus with dates `mf2 check` recommends `fn-datetime` alone:
  ISO dates in every language, with no warning.
* **F8.** Nothing reports which bundle ships to whom, no check fails when one
  ships unread, and the cost table has no native row for a date formatter.
* **F9.** Published 2.0.0 formats with ICU4X on every target when both are
  on (`git show v2.0.0:crates/mf2-fn-datetime/src/lib.rs`), so its browser
  build carries ICU4X. Fixed on `main` by 15.2a, unreleased.
* **F10.** With a date feature on, every plain placeholder (`{$name}`) links
  the date code, because a date may be handed to it: +166 KB with ISO,
  +464 KB with ICU4X, in an application that shows no date. The
  `unused-feature` warning says so. This is why no framework may bring a
  date default (§2.3).
* **F11.** A browser build with `datetime-intl` compiles the ICU4X crates,
  though it links none of them.

## 2. Owner decisions (2026-10-03)

1. **Every deviation is shown to the owner.** A regression, or a known
   departure from the smallest or fastest option, in a Leptos server or
   client, a command-line tool or a terminal UI, is reported to the owner in
   plain English with its measurement, when it is found. Every data bundle
   can be configured: whether it exists and where it ships.
2. **`Intl` formats what it can** in the browser whenever that is smaller or
   faster, **and server rendering is supported**: the server writes localized
   dates.
3. **Date features are named after the framework and the side, and no
   framework brings one.** An application names its date formatter; `mf2
   init`, `mf2 check` and the build's error say which. `fn-datetime`,
   `datetime-icu` and `datetime-intl` are removed. A framework that brought a
   default would make every translated application pay F10.
4. **No date feature, no date code and no date data.** A command-line tool
   that only translates text carries neither a slice nor the ISO formatter.
5. **Data goes only where it is read.** Nothing date-related is sent to a
   browser that formats with `Intl`.
6. **jiff is not a formatter here.** Its own formatting is not localized
   (its documentation says so), and `jiff-icu` only converts jiff values to
   ICU4X's types. There is no jiff feature. jiff values stay accepted as
   message arguments, and jiff stays the source of time-zone rules natively.
7. **Numbers: the split is built and measured first.** `Intl` for currency
   and unit names only, digits and plurals in Rust. It becomes the browser's
   default only if it is smaller and no slower, and the owner sees the
   figures before it does. `number-intl` stays an opt-in.
8. **In 3.0:** the narrowest ICU4X per corpus, the formatter cache, and the
   report of what ships where. **Not in 3.0:** a date formatter of our own in
   place of ICU4X.
9. **All the code first; then it is compiled, then tested, then measured**
   (§8). No test run and no measurement after each task or each phase.

## 3. The date features

### 3.1 Families

Two sides format dates: a browser build, and native code (a server, a
command-line tool, a terminal UI). Each framework has a family of features
for the side it runs on, and each family has one feature per formatter.

| Family | For | Formatters |
|---|---|---|
| `leptos-client-datetime-` | a browser build with a Leptos mode | `icu`, `intl`, `iso` |
| `leptos-server-datetime-` | a Leptos server | `icu`, `iso` |
| `axum-datetime-` | an Axum server | `icu`, `iso` |
| `native-datetime-` | a command-line tool, a terminal UI | `icu`, `iso` |
| `host-web-datetime-` | a browser build with no framework | `icu`, `intl`, `iso` |
| `host-std-datetime-` | native code with no framework | `icu`, `iso` |

| Formatter | What it does | Cost |
|---|---|---|
| `intl` | the browser's `Intl.DateTimeFormat` | +239 B gz; no date data downloaded |
| `icu`, in the browser | ICU4X, the same text as an ICU4X server | +43 to +100 KB gz, and the slice in each catalog |
| `icu`, native | ICU4X over the slices the build cuts | +298 KB, and the slices |
| `iso` | the ISO stand-in | no locale data, no ICU4X |

The features of one side are one switch under several names: the framework
families are written as the host families (§3.4). A feature acts only in
builds of its side. `leptos-client-datetime-intl` changes no code in the
server build; it tells the server's build script what the browser reads.

What the tools recommend: `intl` in a browser, `icu` in native code.

### 3.2 One formatter per build

The server binary and the client wasm are two builds. Each has its own
formatter, chosen from the features of its own side, and neither changes the
other's. `leptos-client-datetime-intl` with `leptos-server-datetime-icu` is
`Intl` in the browser and ICU4X on the server: nothing conflicts, and the
rule below does not apply.

The rule is about one build. `Locale::format` is one method, shared by
terminal code, Axum handlers and the Leptos server, so a binary formats
dates with one formatter. When more than one choice of its side is on, the
strongest formats: ICU4X, then `Intl`, then ISO. Two cases reach this rule:

* two features of one family;
* two frameworks in one build that disagree. An application whose optional
  web mode adds `axum-datetime-icu` to a command-line tool built with
  `native-datetime-iso` formats with ICU4X when the web mode is on, and its
  build without the web mode stays ISO and links no ICU4X.

A feature therefore only adds, which is what Cargo's feature unification
needs. The build warns (`several-date-formatters`) and names the one that
formats, and `mf2 check` shows it. This replaces 15.2a's rule, which was
about the old names.

One formatter per framework inside one binary would need `Locale::format`
split by framework. It is not planned.

### 3.3 A build with dates and no formatter

A date function in a message, in a build whose side has no date feature, is
the `gated-function` error. It names the families of the frameworks that are
on, the recommended formatter, and what each formatter costs. So a
server-rendered application that names only a client formatter does not
build, and the error says which server feature to add.

With no date feature a date cannot be passed to a message either: the
conversions of date types exist only with one, as they do under `fn-datetime`
today.

A build with a date feature links the date code for every plain placeholder
(F10). That is unchanged, and the `unused-feature` warning keeps saying it.

### 3.4 The manifests

`mf2` (the date lines only):

```toml
# The date functions. Every date formatter turns this on; it is not written
# by hand, and alone it is the error of §3.3.
datetime = ["dep:mf2-fn-datetime", "mf2-host-std?/time-zones"]

# The two sides, with no framework.
host-std-datetime-iso  = ["datetime"]
host-std-datetime-icu  = ["datetime", "mf2-fn-datetime/std-icu", "mf2-locale-data?/icu-blob"]
host-web-datetime-iso  = ["datetime"]
host-web-datetime-intl = ["datetime", "mf2-fn-datetime/web-intl", "mf2-host-web?/datetime-intl"]
host-web-datetime-icu  = ["datetime", "mf2-fn-datetime/web-icu", "mf2-host-web?/time-zones"]

# The frameworks' names for them.
leptos-client-datetime-iso  = ["host-web-datetime-iso"]
leptos-client-datetime-intl = ["host-web-datetime-intl"]
leptos-client-datetime-icu  = ["host-web-datetime-icu"]
leptos-server-datetime-iso  = ["host-std-datetime-iso"]
leptos-server-datetime-icu  = ["host-std-datetime-icu"]
axum-datetime-iso           = ["host-std-datetime-iso"]
axum-datetime-icu           = ["host-std-datetime-icu"]
native-datetime-iso         = ["host-std-datetime-iso"]
native-datetime-icu         = ["host-std-datetime-icu"]
```

`mf2-fn-datetime` has one feature per side and formatter: `std-icu`,
`web-icu`, `web-intl`. Its default backend is, in a browser build, the
strongest of `web-icu` and `web-intl`, else `Neutral`; elsewhere `std-icu`,
else `Neutral`.

**ICU4X is a dependency of one side.** A browser build compiles the `icu_*`
crates only with `web-icu`, and a native build only with `std-icu` (F11).
Cargo cannot scope one optional dependency by feature and target at once, so
the task chooses between a renamed dependency entry per target and a crate of
its own for the ICU4X backend, and says why.
16.1: Cargo refuses one package under two names, even in two target tables, and a
backend crate alone would still be one package on both sides; so the browser reaches
ICU4X through `mf2-fn-datetime-web-icu`, a published re-export crate (wasm table only).

**ICU4X's compiled-in data** (`Icu<_, _, Compiled>`) stays in
`mf2-fn-datetime` behind that crate's own feature, for a registry written by
hand. No feature of `mf2` turns it on (F2, F3).

No feature name holds `_`: `mf2`'s `build.rs` relies on that.

### 3.5 What the tools write

* **`mf2 init`:** the starters have no date in a message and name no date
  feature, so they link no date code.
* **The build's error** (§3.3) and **`mf2 check`** print the line to write.
  For a server-rendered Leptos application: `leptos-client-datetime-intl`
  and `leptos-server-datetime-icu`. For a command-line tool or a terminal
  UI: `native-datetime-icu`. For an Axum server: `axum-datetime-icu`.
* **`icu` in native code needs `mf2-build`'s `icu-blob`**, as `datetime-icu`
  does today; the error says so. It stays off by default: it adds about 16 s
  to a cold build.
* `mf2 check` warns when a family's feature is on without its framework.

### 3.6 From 2.0

| 2.0 | 3.0 |
|---|---|
| `fn-datetime` alone | the `iso` feature of each family in use |
| `datetime-icu` | the `icu` feature of each family in use |
| `datetime-intl` | `intl` in the browser's family and `icu` in the native one; the native side now needs `icu-blob` |
| both | the same as `datetime-intl` |

## 4. Data goes only where it is read

### 4.1 The rule

Each LOCALE entry has readers. The build writes it:

* **into the catalog**, when the browser build reads it, or when the build
  has no browser side (a native application: one reader, one file);
* **into the server-only table**, when a browser downloads the catalog and
  only native code reads the entry;
* **nowhere**, when no side reads it.

| Entry | A browser reads it when | Native code reads it when |
|---|---|---|
| `icu.blob` | its formatter is `icu` | its formatter is `icu` |
| `plural.*`, `number.*`, `currency.data`, `unit.data` | `number-intl` is off | always, under their own conditions |

The features that decide this are in the one list both builds see, so the
server's build script knows what the browser reads. `CATALOG_FEATURES`
(`crates/mf2-build/src/features.rs`) becomes the browser-side choices.

### 4.2 The server-only table

* **The build** writes, beside each catalog, the entries only native code
  reads, in the LOCALE section's own encoding. The catalog's name and
  content hash cover what the browser downloads.
* **The generated module** embeds the table for a server beside `CATALOGS`,
  under the gate that already keeps the catalogs out of a client.
  As built (17.1): each `CATALOGS` row is `(tag, file, bytes, table)`, and `CORPUS`
  gets the same table through `CatalogFile::with_server_data`.
* **The reader** gets a feature, `server-data`, that only the native host
  turns on: a catalog may carry a second source that `locale_entry` falls
  back to, and the plural spans are read from it too. Off, the field does
  not exist, as with `static-bytes`. The client's bytes must not move.
* **A native application** keeps every entry in its catalog, embedded or
  beside the executable.
* **`mf2 compile --site`** writes what a browser reads and nothing else.

The alternative, a second whole catalog for the server, was set aside: it
puts each language's messages into the server binary twice.

### 4.3 ISO on the server, another formatter in the browser

`leptos-server-datetime-iso` with `leptos-client-datetime-intl` is the
smallest server: no ICU4X, no slices. Two things follow, and the guide says
both:

* the page arrives with ISO dates, and a reader without JavaScript, or a
  crawler, sees only those;
* the client must rewrite the dates once it has hydrated. Today it rewrites
  a date only when the reader's zone differs from the page's
  (`crates/mf2/src/leptos/zone.rs`, `correction`), so the ISO text would
  stay.

The page states the server's formatter beside its zone (an attribute of the
preload link, as `data-mf2-zone` is). When it is not the client's, the
client rewrites every date it hydrated, through the queue the zone
correction already keeps. With ICU4X on the server and `Intl` in the
browser nothing is rewritten: the server's text stays until its node next
updates, as today.

**Only the messages that format a date** (owner, 2026-10-04). The browser
rewrites a hydrated message, and the zone correction re-renders one, only
when that message formats a date. The catalog already says which, at no cost
in bytes: FUNCS lists the functions its messages call and each expression or
declaration marks a call, so a message that calls `:date`, `:time`,
`:datetime`, or a function the application registered as a date function,
is a date message, and a catalog whose FUNCS names none of them has none. A
marker stored per message, written only into catalogs that have dates, is
the fallback if Phase 22 finds this walk slow. The page's text is never
compared.

**A date is formatted only through a date function** (owner, 2026-10-04: the
framework is typed, so the message says it). The generated registry no
longer formats an unannotated date/time (`Registry::with_dates`): a date
value in a bare placeholder is a Bad Operand with its fallback, and the
message writes `{$when :datetime}`. The build fails when one language's
message formats a variable with a date function and another's leaves it
bare. Where the call-site macro can know which arguments a message formats
as dates, a date value passed to any other argument is a compile error.
Unannotated numbers still format by type. A server's number text that
differs from the browser's is not rewritten, and must not break hydration.
(17.5: `with_dates` left the runtime too, so a hand-written registry cannot format a bare date
either; `date-mismatch` counts only `:datetime`, `:date`, `:time`, as the build cannot see
`Function::formats_dates`; the call-site compile error is left — it needs a per-argument date
flag in the manifest that `mf2-macros` reads, and a dispatch for signals of dates.)

## 5. A smaller and faster ICU4X

### 5.1 The narrowest form per corpus

The build knows what the messages can ask for (`DateNeeds`,
`crates/mf2-locale-data/src/icu_blob.rs`):

* **zone names** are needed only if some message has `timeZoneStyle`, which
  must be a literal;
* **calendars** other than Gregorian are needed only if some language's
  preferred calendar is not Gregorian, or a message has a `calendar` option
  that is a variable or names another calendar.

The generated module states the two facts, and a macro of `mf2` turns them
into the date statics of the right type, as `__use_host!` picks the host. The
slice is then cut for that form alone, not for every form
(`IcuBlobSpec::every_variant`).

`mf2.toml` overrides it:

```toml
[dates]
calendars  = "auto"   # or "gregorian", or "all"
zone-names = "auto"   # or true, or false
```

A date argument that carries a calendar the build left out is an
*Unsupported Operation*, reported, with a fallback value. `mf2 check` prints
the form chosen and why.

### 5.2 The formatter cache

Per date placeholder today: the slice is copied into a `BlobDataProvider`
and the formatter is built, once for `supports` and once for `format`
(`crates/mf2-fn-datetime/src/icu.rs`). The cache keeps the provider per
catalog and the formatter per language and shape. It needs `std`, so it is a
feature of `mf2-fn-datetime` that the native host turns on. Whether a browser
build with ICU4X gets it is decided by its measured size. The baseline is
§1.2's speed.

The cache knows a catalog by a number the catalog is given when it is loaded
(a process-wide counter, never reused), not by its bytes or its address: a
lookup is one integer comparison, the cache keeps no copy of a blob, and a
catalog loaded after another is dropped never gets the other's formatters
(owner, 2026-10-04, after 18.2 compared the whole blob on every lookup and
kept about two copies of it per thread).

## 6. Numbers: the split, measured first

`Intl.NumberFormat` writes currency and unit names in the browser; digits,
rounding and plural selection stay in Rust. The September probe put the
byte saving there and the slowdown and the missing plural rules elsewhere.
It was never built.

The task builds it behind features of the sub-crates, not of `mf2`, and
measures: the client's gzip bytes, the catalog's bytes per language, the
time per placeholder in three engines, and agreement with the Rust path on
the language panel. The owner then chooses: the browser's default, an opt-in,
or neither. Until then `fn-number` formats in Rust everywhere and
`number-intl` is the opt-in it is.

As built (18.4): `intl-names` of `mf2-fn-number` and `mf2-host-web` (and the name `mf2-build`
reads); `Intl` also gives the layout around the name and a currency's own digits, and without it
the client shows digits and the code, so the Rust currency and unit code is not linked.

`number-intl` itself is fixed by §4.1: its number data stops going to the
browser.

## 7. What ships where: the report and the guards

* **`mf2 stats`** prints every bundle per language: its bytes, who reads it,
  and where it ships. One line gives the bytes a browser downloads and never
  reads, which is 0.
* **`unread-data`**, an error: the build never writes an entry where none of
  its readers looks. Checked after slicing, over every feature set of
  `xtask`'s table.
* **`cargo xtask feature-costs`:** the date families by name; native rows
  for `iso` and `icu`; a row for a date feature that is on in an application
  with no date and a plain placeholder (F10); the slice's bytes per language
  in a browser catalog.
* **`cargo xtask native-canaries`:** no date feature means no symbol of
  `jiff`, `icu_*` or `mf2_fn_datetime`, with a plain placeholder in the
  corpus; `iso` means no `icu_*`; an `icu` binary stays under a ceiling that
  compiled-in data would break.
* **The client:** a browser build with `intl` has no ICU4X symbol, and its
  dependency graph no `icu_*` crate.
* **`CLAUDE.md`** carries decision 1 as a rule.

## 8. The phases

**All the code first; then it is compiled, then tested, then measured**
(owner, 2026-10-03). A full run of the check suite takes over an hour on the
owner's machine (73 minutes at Phase 14's exit), and the first order of
these phases had one `cargo xtask ci` after every task and one full suite
after every phase: seventeen of the one and six of the other. Now the long
runs happen once, after all the code is in.

| Phase | File | What | What runs |
|---|---|---|---|
| 16 | `plan/09-phase-16-date-features.md` | The tools of this order; the families, one formatter per build, the errors, the tools | nothing: code only |
| 17 | `plan/10-phase-17-data-where-read.md` | The server-only table, `number-intl`'s data, ISO on the server | nothing: code only |
| 18 | `plan/11-phase-18-icu-and-numbers.md` | The narrowest ICU4X, the cache, the number split, the harness for date speed in a browser | nothing: code only |
| 19 | `plan/12-phase-19-report-and-guide.md` | The report, the cost table's rows, the canaries, the guide, the samples, the plan pointers in the code | nothing: code only |
| 20 | `plan/13-phase-20-compile.md` | Everything compiles | builds; no test |
| 21 | `plan/14-phase-21-test.md` | Everything passes; a cold start | `ci` and the suite, once |
| 22 | `plan/15-phase-22-measure.md` | Every figure; the owner's report; the number split is asked | the measurements, once |
| 23 | `plan/16-phase-23-release.md` | Release 3.0.0 | the pre-flight, then the owner's word |

They run in that order. Phase 15's task 15.4 is 19.5, and its 15.5, the cold
start, is 21.6. Phase 15's exit took no run of its own.

**What this order changes.**

* **Phases 16 to 19:** a task is done when its code and its tests are
  written and committed. It compiles nothing, runs nothing and measures
  nothing. `cargo fmt`, `cargo metadata` and `cargo tree` are allowed: they
  compile nothing, and they catch a syntax error or a manifest that does not
  resolve. One coordinating session can run all four files in a row.
* **Phase 20** gets every compile error of the four phases at once, and
  fixes them crate by crate.
* **Phase 21** runs `ci` and the suite once each, finds every failure in
  that run, and confirms each fix with the narrowest command that shows it.
* **Phase 22** takes every figure once. There is **no figure per task**: a
  change is measured against a switch that turns it off in the same tree,
  or against a figure recorded before the code went in (§1.2, the suite's
  table `p14`, the cost table). When a figure moves and nothing explains it,
  finding the change that moved it is extra work there, and may fail; the
  figure is then reported as not attributed.
* **Decision 1 stands:** every figure that moved the wrong way reaches the
  owner, in Phase 22's one report, not task by task.
* **The owner is stopped for three things only:** a choice the design does
  not settle, Phase 22's report and its question, and the word to publish.
* Between Phase 16's first commit and Phase 21's exit, `main` holds commits
  that have not been through `ci` (`CLAUDE.md`, Conventions).

## 9. Not verified

* **The speed of `Intl` against ICU4X for a date in a browser.** No
  measurement exists. Task 18.3 writes the harness and Phase 22 runs it
  (22.8).
* **That a browser build under `number-intl` reads no number entry.** The
  September probe says so; task 17.2 reads the code before it moves them.
* **What the narrow ICU4X forms save natively.** Only the browser's figures
  exist.
* **What a date feature costs a browser client that shows no date** and has
  a plain placeholder (F10 in the browser). The native figure exists; task
  19.2 writes the row and Phase 22 measures this one (22.3).
* **How a date argument carries a calendar**, and whether the build can see
  it. Task 18.1 finds out; until then §5.1's fallback stands.
* **Cargo accepting one package under two dependency names, one per
  target.** Task 16.1 tries it with `cargo tree`, which compiles nothing,
  and falls back to a crate of its own.
* **What `[profile.dev] opt-level = 1` does to the whole workspace's test
  time.** Measured on the conformance crate only (task 16.0). Phase 21
  measures the rest (21.1).
