# Changelog

Every Rust MF2 crate is released together, at one version, so this one file
covers them all. Newest first. What a version number promises is in
[`docs/versioning.md`](docs/versioning.md); a release that raises the
minimum Rust version says so here.

## 3.0.0

**Not yet published.** A major release, prepared by Phase 11: it carries
breaking changes that 2.x cannot, and each task that changes what an
application sees adds its line here.

* **`Host::nfc` is gone, and no host carries normalization tables.** The
  `Host` trait no longer has an `nfc` method: the runtime answers canonical
  equivalence itself, from the map its catalog carries. A hand-written host
  deletes its `nfc`; nothing else changes. `mf2-host-std` no longer depends
  on `unicode-normalization` and `mf2-host-web` no longer calls
  `String.prototype.normalize`, so a native binary links the normalization
  tables only with `compile` (a message compiled at run time), and the
  browser client has one glue function less.
* **A custom function can ask whether a value matches a key, accents written
  either way.** `FnContext::equivalent(value, key)` makes the comparison MF2
  asks for between a selector value and a variant key, from the catalog's own
  map: no allocation, no normalization tables. `:string` selection and the
  matching of argument names passed through the dynamic API now go through
  it, so they behave the same on every host.
* **A catalog carries its own canonical-equivalence map, and the binary
  format is version 2.** A compiled catalog now holds a small table of the
  code points that can reach its variant keys and argument names after
  decomposition, so the runtime can answer "is this value the same string as
  that key, allowing for accents written either way?" without any
  normalization tables. The format's major version moved with it: a 3.0
  reader refuses a catalog written by 1.x or 2.x (`not an .mf2b catalog` ·
  `unsupported .mf2b format version`), so rebuild your catalogs when you
  upgrade — a `mf2 build` does it. The table is catalog data: nothing of it
  reaches the client wasm. `mf2 stats` and `mf2 dump` report it.
* **The native host has a time-zone database only with dates.**
  `mf2-host-std` gained a feature `time-zones`, which `mf2`'s `fn-datetime`
  turns on, and a second static, `mf2::host_std::ZONES_HOST`: it resolves a
  named time zone, as `HOST` used to. Without dates `HOST` has no zone data,
  so a named zone is *Bad Option* and the IANA database is not linked. The
  generated `host::HOST` names the right one for the build, so an
  application that formats through it needs no change; one that names
  `mf2::host_std::HOST` itself and shows dates in a named zone writes
  `ZONES_HOST`. Native code reads the system's zone only with
  `fn-datetime`; without it the default zone is UTC.
* **A named time zone is read from the system's IANA database, unless the
  bundle is asked for.** 2.0 always answered from jiff's bundled copy. A
  native application now follows the machine it runs on (`TZDIR`, else
  `/usr/share/zoneinfo`, else jiff's copy on a platform that has none), so a
  zone amended since the binary was built is right. A server, where every
  reply must say the same thing whatever the host holds, keeps the bundle:
  the new feature `mf2/tzdb-bundled`, which `ssr` and `axum` turn on. Turn it
  on by hand to carry the bundle in any other build; a zone no database holds
  is *Bad Option*, as before.
* **`mf2` no longer depends on jiff.** The date library is the native host's
  alone: `mf2::host_std::jiff` re-exports it (feature `host-std` with
  `fn-datetime`), and `mf2::host_std::system_time_zone()` reads the zone the
  machine is set to. A program that hands `mf2` a jiff value and wants no
  jiff dependency of its own uses that re-export. The `IntoArg` impls for
  jiff's `Timestamp`, `Zoned`, `civil::Date` and `civil::DateTime` now come
  with `host-std` and `fn-datetime` rather than with `native`, so a server
  has them too.
* **A build links a time-zone database only where a date can reach a
  message.** The generated `host::HOST` names the zone-resolving host when
  `fn-datetime` is on *and* some message uses `:datetime`, `:date` or
  `:time` or has a placeholder with no function; otherwise it names the
  plain host, and `Catalogs` and `Locale::format` take the host from the
  corpus. An application that formats through the generated module needs no
  change.
* **`mf2-host-std` no longer depends on `ryu`.** Float text is `core`'s own
  shortest round-trip formatting; `ryu` is now only the test oracle. A native
  binary carries one dependency and about 13 KiB less, and nothing a program
  writes changes. Where the shortest text that round-trips a float is an
  exact tie between two decimals (about one `f64` in 4,000, such as
  `1731590483420272.25`), the last digit shown may differ from 2.x: both
  answers are the same length and the same distance from the value.

## 2.0.0

**Published on 30 September 2026.** 1.0.0 reached crates.io on
26 September 2026.
1.1.0, prepared as a minor release after it, was never published: its
items are part of this release. 2.0.0 is a major release: an application
names one crate, `mf2`, and turns on what it needs; it adds native
applications, gives every crate author, repository and book metadata, and
calls the project Rust MF2. What 2.x promises is in
[`docs/versioning.md`](docs/versioning.md); the steps from 1.x are in
[Upgrading from 1.x](docs/upgrading.md).

* **Changed: one crate.** 1.0.0's `leptos-mf2` and `mf2-axum`, and 1.1.0's
  unpublished `mf2-native` and `mf2-ratatui`, are gone: they are `mf2`'s
  features `leptos` (or `leptos-0-8`) with a mode, `axum`, `native` and
  `ratatui`. Sixteen crates make up 2.0.0 — `mf2`, `mf2-build`, `mf2-cli`,
  the eleven crates those three are built on, and the two
  helper crates of the Leptos components — and each depends on the others
  at exactly `=2.0.0`.
* **Native CLI and terminal apps.** `mf2::native` (new): `NativeI18n` holds
  one generated corpus's catalogs and an app-owned active locale, picks the
  first of the system's preferred languages the corpus supports (else the
  source locale), reports where the locale came from, refuses an
  unsupported explicit locale, and formats in the system's time zone with
  bidi isolation off (both settable).
* **`mf2::ratatui`** (new): a message as Ratatui `Text` or `Line`, its
  markup (`{#name}…{/name}`) as styles the application maps by name. It
  adds `ratatui-core` only.
* **`mf2-build`: `Emit::Native` and `Emit::NativeFiles`.** A module for a
  native application — the native host, no `ssr` feature to declare — with
  one `CORPUS` value; the catalogs embedded, or written beside the build
  for the application to ship.
* **`mf2`: `Corpus`, `CatalogFile` and `Message`.** `Message` formats any
  call-site description (`Tr`, `TrArgs`, `TrRich`, `TrDyn`) to text or to
  parts outside Leptos.
* **`mf2-catalog`: `Catalog::from_static`** (feature `static-bytes`), a
  catalog over bytes that live for the whole program (an embedded catalog,
  not copied). Off for the web client, which pays nothing for it: with
  the feature off, the reference application's wasm is 6 bytes smaller
  than before (`cargo xtask size`, raw after `wasm-opt`); an
  unconditional version cost it 392.
* **Fixed: a lazy route reached by a link was not interactive** when a
  message in it had a signal as an argument (`tr!("…", count = n)`). The
  message's first format read the signal in the route's own render, so
  every change re-ran the route's view with fresh state: a button's
  handler seemed to do nothing, and reactive text stayed at its first
  value. Loaded directly, the same route worked. The Leptos layer now formats
  every text, attribute and property untracked outside the node's own
  argument effect, which alone follows the signal.
* **Changed: a switch that meets another deploy's catalog reloads.** When
  the server has been redeployed since the page loaded, `set_locale` now
  remembers the new language (the cookie, or `localStorage` in a
  client-only application), takes `?lang=` out of the address and reloads
  into it, logging one `mf2:` line — as a page already did at boot. Before,
  the switch was refused and the page stayed in the old language. Every
  control that calls `set_locale` gets this, not only `LocaleSwitcher`;
  `set_locale` returns `Ok` with the reload under way.
* **Changed: `CookieLocale` writes the cookie only for an explicit
  choice.** As a sink it writes when the locale came from the query
  (`?lang=`) or the path, and not when it was guessed from
  `Accept-Language` or the default, so a guess is no longer remembered as
  if the reader had chosen it. A cookie that was read is not written back,
  so its one-year expiry no longer slides with every visit; the client
  still writes it on every switch.
* **The switcher on a site whose languages live in its URLs.**
  `LocaleSwitcher` takes an optional `href_of` (the shape `AlternateLinks`
  takes): each option carries its language's URL, and the button goes
  there instead of switching in place, which a `?lang=` could not do under
  `PathPrefix`. Without the wasm, the form's `?lang=` reaches the server,
  and the new `mf2_axum::path_prefix_redirect` middleware sends a request
  whose `?lang=` disagrees with its path to that language's URL.
* **`TrDyn::new`**, a public constructor for a message whose id is known
  only at run time (`msg_id!("…")`) and whose arguments arrive by name —
  for a tool or a server. `tr!` stays the form a page uses.
* **Fixed: no `<span lang>` inside an element that holds only text.**
  Under `mark-fallback-lang`, a borrowed message in a `<textarea>`,
  `<title>`, `<option>`, `<script>` or `<style>` showed the span's markup
  as characters. It is now written unmarked there, on the server and in
  the browser.
* **Fixed: both Leptos lines on gives one error.** `leptos-0-8` with the
  default `leptos-0-9` still on stopped the build with `leptos-mf2`'s
  `compile_error!` naming the fix, but also with eight unrelated errors
  from inside the view glue. Now the `compile_error!` is the only one.
* **Changed: `mf2 check` checks with the translation crate's features.**
  Without `--features` it checked with none, so it warned where the build
  does not (`neutral-numbers`) and failed where the build succeeds
  (`gated-function`). It now asks cargo for the crate's features, as
  `mf2 compile --site` does (offline), and reports what the build reports.
  Without cargo it checks with none and says so; `--features` still wins.
* **`missing-translation` names the missing ids**: the first ten of each
  locale, then how many more, where it gave only the count.
* **Fixed: a second `mf2 convert` run is not an error.** A `.mf2` file that
  already holds what the conversion would write is left alone, and a Rust
  file whose text would not change is not a rewrite, so a second `--write`
  writes nothing and exits as the first did, and a dry run lists no files
  that would not change. A file with other text still stops the command.
* **`mf2 convert --from leptos-fluent` reads `cookie_name:`**: the
  initializer's finding says to add `CookieLocale { name, ..Default::default() }`
  to the server's `Negotiator` as an extra source, since mf2's client
  always writes `mf2_locale`.
* **Fixed: a native catalog from another build loaded.** A rebuild that
  changes only a message's text keeps the manifest hash, so an old catalog
  file renamed to the new build's name loaded and printed the old text.
  `NativeI18n::from_directory` now checks each file's bytes against the
  content hash in its name, and a mismatch is the new
  `NativeError::ContentMismatch`. The hash is `mf2-catalog`'s
  `content_hash` (feature `content-hash`, which `mf2-build` shares); the
  web client, whose catalogs come from its own server or build, does not
  compute it.
* **Changed: `mf2 fmt` keeps a blank line after the frontmatter's `---`
  and on both sides of a message laid out on lines of its own** (a
  `.match`, or any value written under its `=`). Files keep their meaning;
  run `mf2 fmt` once, or `mf2 fmt --check` in CI reports the files 1.0
  formatted. `mf2 convert`, `mf2 import` and `mf2 pseudo` write the same
  layout.
* **Changed: a translation that leaves out the source's markup is an
  error** (`dropped-markup`), in `mf2 check` and the build. French
  `Acceptez nos conditions.` for `Accept our {#link}terms{/link}.` used to
  pass every check and ship without its link. A variant may still leave
  markup out while another variant of the message keeps it; a corpus that
  drops markup on purpose sets `dropped-markup = "warn"` (or `"allow"`)
  under `[lints]`.
* **Changed: a message marked `@do-not-translate` is not missing.** It
  needs no translation, so `missing-translation` and `mf2 stats` leave it
  out of both the count and the total: a language with one of four
  messages, whose source marks one of them `@do-not-translate`, is missing
  two of three, not three of four, and a copy of it is not counted as
  translated. `mf2 stats --format json` adds `do_not_translate`. The mark
  on a `[section]` or at the top of a file now covers every message under
  it for every check, as it already did for XLIFF export's
  `translate="no"`.
* **Changed: `mf2 import` checks what it would write.** It used to write
  a translation with a variable its source does not declare (`{$nom}`)
  and exit 0, leaving the error for the next `mf2 check`. Now, JSON and
  XLIFF alike, it runs every check `mf2 check` makes on the files as they
  would be, with the same features (`--features` is new here), and writes
  nothing if that brings an error, which it reports as `check` does. An
  error the files already had does not stop it. **JSON import exits 1**
  when it leaves out ids the language does not have yet, and says to use
  XLIFF, which adds them where the source has them.
* **Changed: the call-site types and the Leptos layer are `mf2`'s.** `Tr`,
  `TrArgs`, `TrRich`, `TrDyn`, `ArgValue` and the rest are defined in
  `mf2`, and the Leptos layer is the module `mf2::leptos`, chosen by
  `mf2`'s own features: a Leptos line (`leptos` for Leptos 0.9, the
  default line, or `leptos-0-8`) and a mode (`ssr`, `hydrate` or `csr`,
  each now implying its host). Two modes, both lines, or a mode with no
  line is a compile error that says what to write. `mf2`'s `leptos`
  feature, which named the layer, now names the 0.9 line; a mode is what
  turns the layer on. The six components live in two new crates,
  `mf2-leptos-ui-0-9` and `mf2-leptos-ui-0-8`, one per Leptos line;
  `mf2::leptos` wraps each under its own name. Upgrading a 1.x translation crate means deleting its
  hand-written `setup()`: the generated module defines it now.
* **Descriptions print.** Under a Leptos mode `Tr`, `TrArgs`, `TrRich` and
  `TrDyn` implement `Display`: the text the fmt-free `to_string()` builds,
  padded as the format asks, so `{}` costs a browser build a few dozen
  bytes where `.to_string()` costs none. Every public type of `mf2`
  implements `Debug`; the call-site types write it without core's float
  and string-escape code (about 1 KB in a browser build rather than
  12–16 KB), so a float shows at most six fraction digits and a quote in a
  text is not escaped.
* **Changed: the native support is `mf2`'s.** `NativeI18n` and
  `NativeError` are the module `mf2::native`, behind `mf2`'s new `native`
  feature, which implies `host-std`. Beside `hydrate` or `csr` it is a
  compile error when compiling for the browser (`wasm32`): a native
  application's module has no place in a browser build. On the host the
  two compile together, so a workspace that holds a browser client and a
  native application checks as one (`cargo check --workspace`, and
  rust-analyzer's check), as in 1.x.
  `LocaleSource`, where the active locale came from, keeps its name.
  `NativeI18n` implements `Debug`: the active locale, where it came from,
  the catalogs and the settings.
* **Changed: the Ratatui support is `mf2`'s.** It is the module
  `mf2::ratatui`, behind `mf2`'s new `ratatui` feature, which implies `native` and adds `ratatui-core` alone. Like
  `native`, beside `hydrate` or `csr` it is a compile error when compiling
  for the browser (`wasm32`), in a sentence that names `ratatui`; on the
  host the two compile together, so a workspace that holds a browser
  client and a terminal UI checks as one. A call site converts into
  Ratatui's `Span`, `Line` and `Text` itself, and a `Theme`, set once,
  styles its markup. **Removed: 1.x's `MarkupStyles`, `line` and `text`**,
  which took the handle and a map of styles on every call.
* **`tr!` takes any number, text, date or path, and any type with
  `Display`.** An argument converts through the new `mf2::IntoArg`, which an
  application may implement for its own types: every integer type exactly
  (`u64`, `i128` and `u128` past `i64` as their exact decimal, and `usize`
  without saturating; `ArgValue::from(usize)` still saturates, as in 1.x),
  the `NonZero` integers, `bool` as the string `true` or `false`, which
  `.match` selects on, `Cow<'static, str>` (borrowed text stays borrowed),
  `&Path`, `PathBuf`, `&OsStr` and `OsString` (their text, lossy where it
  is not UTF-8), `SystemTime` (an instant), with `native` jiff's
  `Timestamp`, `Zoned`, `civil::Date` and `civil::DateTime`, a signal of
  any of these, and `&T` for any of them that is `Copy`. Any other type with
  a `Display` is an argument as its text, made when the description is
  built: an `io::Error`, an address, a key binding. That text is not
  translated; a word that needs a translation is a string the message
  selects on. A type that is none of these is a compile error at the
  argument, in `IntoArg`'s words, where 1.x's named `ArgValue`. Every type
  1.x's `tr!` took still converts as it did, an application's own
  `From<T> for ArgValue` included.
* **A native application formats without a handle.**
  `mf2::native::install(&CORPUS)` installs a generated corpus's catalogs
  once, for the whole process, and chooses the first of the system's
  languages the corpus has, else its source language. A description then
  shows its text wherever text is wanted: `println!("{}", tr!("welcome"))`,
  `.to_string()`, `.to_plain_string()` (never isolated), and `.to_cow()`,
  which borrows a simple message's text from the executable instead of
  copying it. `set_locale` changes the language of every thread's next
  format; `with_locale` gives one thread another for a scope, restored when
  the scope ends or unwinds, and needs no `install`, so tests pinned to
  different languages run in parallel; `locale()` and `locale_source()` say
  which is in force; `set_bidi` and `set_time_zone` apply to every thread.
  `install_from_directory` loads catalogs shipped beside the executable and
  accepts a partial set: only the source language's file is required.
  `mf2::native::Catalogs` is the explicit form, with no globals:
  `catalogs.format("fr", &message)`. Before `install`, a build whose only
  mode is `native` panics, naming `install()`; beside a Leptos mode, the
  request's or the page's catalog comes first, and there is never a panic.
  `NativeError` is named `mf2::native::Error`; `NativeI18n` keeps its
  name.
* **Changed: dates follow the system's daylight-saving rules.** A native
  application whose system zone has no IANA name (`TZ` holding a POSIX
  rule, or a copied `/etc/localtime`) formatted dates at the offset in
  force when it started, so a date on the other side of a change of offset
  was an hour off. The zone is now one that follows the system's rules:
  `TimeZone::rules`, a POSIX TZ rule, which `mf2-host-std` evaluates.
  `NativeI18n` gets the same zone.
* **`Debug` on every public type of `mf2-runtime` and `mf2-catalog`**,
  derived, or written by hand where a field has none (a handler, a host, an
  application's value, a byte table: each shows what identifies it). A
  browser build links none of it unless it formats one with `{:?}`.
* **Changed: one locale matcher, by CLDR's data.** Everything that chooses
  a language now chooses the same way: a native application's `install`,
  `set_locale`, `with_locale`, `Catalogs::format` and `NativeI18n`;
  `mf2::axum`'s negotiation and `lookup_locale`; a client-only
  application's boot. Each tag is filled in by CLDR's likely subtags
  (`zh-TW` is Traditional Chinese of Taiwan), the distance CLDR's
  language-matching data gives is added for each part that differs, a
  reader's later languages count for less, and a language is served only
  when it is close enough — the algorithm of UTS #35 Part 1. Case, `_`
  and POSIX names read as before (`fr_CA.UTF-8` finds `fr`); `C`, `POSIX`,
  `*` and an empty value still match nothing. What a 1.x application sees
  change:
  * `zh-Hant-TW` and `zh-Hant` find the application's `zh-TW`, where 1.x
    truncated them to `zh` first;
  * Traditional Chinese (`zh-TW`, `zh-Hant`, `zh-HK`) no longer gets the
    application's Simplified `zh` or `zh-CN`, nor Simplified the
    Traditional: CLDR has no rule between the two scripts, so the reader
    gets their next language, else the source language (a list that also
    names plain `zh` gets Simplified through it). Likewise the other script
    of a language CLDR does not serve across: `pa-PK` does not find `pa`,
    nor `uz-AF` `uz`, where 1.x's web negotiation took any locale of the
    same language;
  * Serbian's Latin and Cyrillic are served for each other (`sr-Cyrl` and
    `sr-RS` find `sr-Latn`), which 1.x's native matcher refused;
  * a language CLDR says a reader understands is served when theirs is
    missing — a reader of Breton gets French, of Catalan Spanish, of Swiss
    German German — where 1.x gave the source language;
  * of several regions, the closest: an Australian reader gets `en-GB`
    before `en-US`, a Mexican one `es-419` before `es`;
  * a reader's list is weighed as one — `Accept-Language` in quality order,
    `navigator.languages`, the system's languages: a regional variant of
    the first language (`de-AT` finds `de`) beats an exact match of the
    second, and nothing past the tenth language is served.

  Natively, choosing a language no longer copies the tag. A server matches
  with CLDR's whole table. **`mf2-build` generates `LANGUAGE_MATCHING`**,
  the part of the table the corpus's languages need, which gives them the
  same answers; a native application's `CORPUS` carries it (`mf2::native`
  matches with it), and a client-only application's generated `setup()`
  carries it. A client whose `Setup` does not (one built by hand with
  `Setup::new`) matches with no data: it finds a locale of
  the reader's own language (`fr-CA` still finds `fr`), the first in the
  build's order among several, and no other script or language. Its type
  is `mf2::LanguageMatching`.
* **The generated module names the languages.** `mf2-build` generates
  `enum Locale`, one variant per locale in tag order (`pt-BR` is `PtBr`;
  the build refuses two tags that give one name), with `ALL`, `SOURCE`,
  `tag()`, `dir()` and `best_match()`. `FromStr` goes through the one
  matcher (`"fr_CA.UTF-8".parse()` is French; a hydrated page, which never
  matches, takes an exact tag), and its error, `mf2::UnknownLocale`, lists
  the languages there are; `Display` writes the tag; `name()` is the
  `language.<tag>` message when every locale has one. With `mf2`'s new
  `clap` feature, `--lang` parses through the matcher and `--help` lists
  the tags. Beside it, where the build has what they need: `setup()`,
  `install()`, `install_from_directory()`, `set_locale(Locale)`,
  `current_locale()`, `with_locale(Locale, body)`, `preload_locale(Locale)`
  and `Locale::format(&message)`; with `ratatui`, `markup::*`, a constant per
  markup name holding the name and its hash (`mf2::ratatui::Markup`); and a
  `prelude`. Each choice in the module follows how `mf2` was built,
  whichever crate turned its features on: `CATALOGS` is in every build
  whose `mf2` has `host-std` (1.x: the translation crate's `ssr`, which
  every 1.x translation crate forwards to `mf2/host-std`), and the host
  follows `mf2`'s date features. A module that embeds catalogs embeds each
  once, shared by `CATALOGS` and `CORPUS`. Under a Leptos mode, `setup()`
  is the `Setup` value a 1.x translation crate wrote by hand, with a
  client-only application's language-matching data, and `install()`
  installs it; upgrading a 1.x translation crate means deleting its
  hand-written `setup()`, which would otherwise clash with the generated
  one.
* **Ratatui text from the descriptions.** With `ratatui`, a description —
  or a reference to one — converts into a `Span`, `Line` or `Text` in the
  language in force, so it goes straight into a `Paragraph`, a title, a
  `Cell` or a `List`; it is `Styled` as a `Line` (`tr!("quit").bold()`)
  and a `Widget`. Markup takes its style from `mf2::ratatui::Theme`
  (`set_theme`, `with_theme`, `theme()`), whose default styles `b`,
  `strong`, `i`, `em`, `u`, `s`, `del`, `code` and `kbd`. Catalog text is
  borrowed, not copied: only placeholders allocate, beside Ratatui's own
  `Vec`s. A `Text` starts a new line at a line break; a `Line` and a `Span`
  join the lines with a space, and a `Span` keeps no markup style.

## 1.0.0

The first release. Unicode MessageFormat 2 for Leptos: the whole
specification, one small binary catalog per language loaded when it is
needed, and a wasm that contains none of the text. 1.0 is a promise: within
1.x nothing an application uses breaks ([what is promised, and what is
not](docs/versioning.md)).

### What is in it

* **The whole of MF2**, as the Unicode working group's repository stood at
  commit `5c4ddb27` (2026-08-31, after the LDML 48.2 tag): the syntax, the
  data model, formatting, selection, fallback, bidi isolation, markup, the
  error kinds, and the default functions — `:string`, `:number`,
  `:integer`, `:offset`, `:percent`, `:currency`, `:unit`, `:datetime`,
  `:date`, `:time`. The working group's test suite passes at every layer,
  from the parser to hydrated pages in a browser, and every normative
  statement of the specification has a test
  ([`conformance/REPORT.md`](conformance/REPORT.md),
  [`conformance/COVERAGE.md`](conformance/COVERAGE.md)). Without the
  number and date features a build formats the rest and reports those
  functions as unsupported, as MF2 allows; each such case is recorded.
* **`tr!`**, one macro for text, attributes, props, strings and `const`
  tables, checked against the messages when the application compiles:
  arguments, signals, dates, and markup rendered as the elements the call
  site gives ([call sites](docs/call-sites.md)).
* **Catalogs, not code.** A build step (`mf2-build`) checks the messages
  and writes one binary catalog per language and a manifest; the browser
  downloads one language's catalog when it needs it. No message text, id,
  argument name or plural rule is in the wasm, so a translation edit leaves
  it byte-for-byte the same.
* **Every Leptos delivery mode** ([delivery modes](docs/delivery-modes.md)):
  server-rendered and hydrated (the default), islands, client-only, and lazy
  routes. The language switches live, without a reload, or — with
  `static-locale` — by a cookie and a navigation.
* **Leptos 0.9** by default and **Leptos 0.8** as an opt-in
  (`leptos-0-8`), on `leptos-mf2` and `mf2-axum`.
* **`mf2-axum`**: the reader's language chosen from an ordered list of
  sources (a cookie, `Accept-Language`, a path prefix), `Content-Language` and
  `Vary`, and the catalogs served immutable from the server binary
  ([switching language](docs/switching.md)).
* **Dates in the reader's time zone**, with no code in the application:
  the first page re-renders only its dates after hydration and remembers
  the zone in a cookie, and every later page is rendered in it on the
  server ([call sites](docs/call-sites.md)).
* **Localized numbers and dates** from CLDR: symbols, grouping and
  numbering systems, currencies and units; dates through ICU4X on client
  and server (`datetime-icu`) or through the browser's
  `Intl.DateTimeFormat` (`datetime-intl`). The `intl` option formats
  numbers through the browser's `Intl` too.
* **The `mf2` command** (`mf2-cli`): `init`, `check` (lints, among them an
  option a function does not define), `compile`, `fmt`, `stats`, `dump`,
  `pseudo`, `watch`.
* **Migration from Fluent**: `mf2 convert --from fluent` for `.ftl` files,
  and `--from leptos-fluent` for an application — its messages and its call
  sites in one command
  ([migrating from `leptos-fluent`](docs/migrating-from-leptos-fluent.md)).
* **XLIFF 2** for translators: `mf2 export --format xliff` and
  `mf2 import`, each file valid against the XLIFF 2.0 core schema.
* **Accessibility**: `<html lang dir>` follows the language, the switcher
  is a labelled form, bidi isolation where a person reads the text, and
  text borrowed from a fallback language can carry its own `lang`
  (`mark-fallback-lang`) ([accessibility](docs/accessibility.md)).
* **Minimum Rust version 1.88**, checked in CI on exactly that release.

### Budgets, as measured

The project holds its size and speed to written budgets, gated in CI.
Exact figures are not part of the 1.x promise: they move with every
dependency. Each was measured with the command beside it, on 2026-09-25
unless it says otherwise.

| What | Budget | Measured | Command |
|---|---|---|---|
| the library's fixed client cost | ≤ 30 KB gz | 25,875 B gz | `cargo xtask size` |
| each call site, at the margin | ≤ 40 B gz | 8.4 B gz | `cargo xtask size` |
| the reference application's i18n, 1,860 call sites | ≤ 30 KB + 40 B a site (105,120 B) | 41,466 B gz | `cargo xtask size` |
| locale data in the wasm | none | none | `cargo xtask codegen-matrix` |
| the 1,600-message `en` catalog on the wire | ≤ 23,296 B brotli | 18,072 B | `cargo xtask catalog-size` |
| `core::fmt` and panic formatting in the client runtime | absent | absent | `bench/b12/check.sh` |
| an unused function's code in the wasm | absent | absent | `bench/b12/check.sh`, `cargo xtask b12-generated` |
| a simple message / a one-argument message, native | ≤ 100 / 500 ns | 20–35 / 118–137 ns, `en` (2026-09-21) | `cargo run --release -p runtime-bench -- b10` |

Against `leptos-fluent`, on the same application of 1,600 messages and
1,860 call sites, measured once
([`bench/fluent-ab/SNAPSHOT.md`](bench/fluent-ab/SNAPSHOT.md), 2026-09-25):
a first visit in English downloads 642,980 B gz against 983,963; each
added language is a 21–27 KB gz catalog for its own readers against
66,042 B gz more in every visitor's wasm; switching language with 2,000
translated nodes on the page takes 10.3 ms against 82.4 (Chromium,
medians).

### Known limitations

* **Leptos 0.9 is a pre-release** (`0.9.0-beta`). 1.0 is published on it;
  Leptos 0.9's release, and any pre-release before it, is taken in a patch
  release. Leptos 0.8 is supported through `leptos-0-8`.
* **The W3C Message Resource format** that `.mf2` files follow is a
  draft. If it changes, 1.x follows it with `mf2 fmt` able to rewrite
  files, and does not reject a file 1.0 accepted
  ([versioning](docs/versioning.md#the-w3c-message-resource-format)).
* **Browsers tested: Chromium and Firefox.** Every browser check ran in
  those two; WebKit (Safari) has not been run.
* **No screen reader has been run** on the examples. The WCAG 2.2 AA audit
  covered the rest.
* **A date inside an island** is built and checked when it compiles, but
  not yet asserted in a browser: no example has one.
* **Pushing an edited catalog to open pages during development** is not
  built. A translation edit is picked up by the normal rebuild.
