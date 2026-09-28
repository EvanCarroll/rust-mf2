# Changelog

Every Rust MF2 crate is released together, at one version, so this one file
covers them all. Newest first. What a version number promises is in
[`docs/versioning.md`](docs/versioning.md); a release that raises the
minimum Rust version says so here.

## 1.1.0

**Not published, and it will not be.** All sixteen crates reached
crates.io at 1.0.0 on 26 September 2026; the next release is 2.0.0,
which carries what follows. 1.1.0 was prepared as a minor release after
1.0.0: it adds native applications, gives every crate author, repository
and book metadata, and calls the project Rust MF2; the native support adds
public API to `mf2`, `mf2-build` and `mf2-catalog`.

* **Native CLI and terminal apps.** `mf2-native` (new): `NativeI18n` holds
  one generated corpus's catalogs and an app-owned active locale, picks the
  first of the system's preferred languages the corpus supports (else the
  source locale), reports where the locale came from, refuses an
  unsupported explicit locale, and formats in the system's time zone with
  bidi isolation off (both settable).
* **`mf2-ratatui`** (new): a message as Ratatui `Text` or `Line`, its
  markup (`{#name}…{/name}`) as styles the application maps by name. It
  depends on `ratatui-core` only.
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
  value. Loaded directly, the same route worked. `leptos-mf2` now formats
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
