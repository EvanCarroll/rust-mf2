# Lints

The build script and `mf2 check` run the same checks over the messages.
Each has a name, which a report prints in brackets and `mf2.toml`'s
[`[lints]`](configuration.md#lints) uses to raise or lower it:

* `error`: reported, and the build (and `mf2 check`) fails;
* `warn`: reported, and the build goes on (`mf2 check --deny-warnings`
  fails on it);
* `allow`: not reported.

Any lint can be raised. Some state a rule the rest of the build relies on
and cannot be lowered: each section below gives the default and the lowest
level `mf2.toml` may set.

The examples show a message in the source language (`en`) and its
translation (`fr`), each as it stands in its `locales/<tag>/*.mf2` file.

## Errors by default

### `extra-id`

Default `error`; the lowest `mf2.toml` may set it is `error`.

A translation has an id the source language does not. Nothing in the
application can ask for it, and usually it is a message renamed or removed
in the source and left behind in a translation.

```text
en   (no message "mystery")
fr   mystery = Quoi ?
```

Fix: delete it from the translation, or add it to the source language if
the application needs it.

### `undeclared-variable`

Default `error`; the lowest `mf2.toml` may set it is `error`.

A translation uses a variable its source message does not have. A call site
passes the source's arguments, so the variable would never have a value.

```text
en   greeting = Hello, {$name}!
fr   greeting = Bonjour, {$user} !
```

Fix: use the source's variable (`{$name}`). A language that needs more
input than the source shows, a grammatical gender for one, gets it by the
source declaring it with `.input`, even where the source's own text does
not use it; the call site then passes it for every language.

### `undeclared-markup`

Default `error`; the lowest `mf2.toml` may set it is `error`.

A translation uses markup its source message does not. The application maps
only the source's markup names to elements or styles.

```text
en   close = Press {#kbd}Esc{/kbd} to close
fr   close = Appuyez sur {#kbd}{#b}Échap{/b}{/kbd} pour fermer
```

Fix: remove it from the translation, or add it to the source and the call
site.

### `dropped-markup`

Default `error`; the lowest `mf2.toml` may set it is `allow`.

A translation leaves out markup its source message has. A link that is gone
from the French sentence is gone for French readers, and nothing at run
time says so.

```text
en   terms = Accept our {#link}terms{/link}.
fr   terms = Acceptez nos conditions.
```

Fix: keep the markup around the words that carry it
(`Acceptez nos {#link}conditions{/link}.`). One variant of a `.match` may
leave it out as long as another keeps it. A corpus that drops emphasis on
purpose may lower it to `warn`.

### `dynamic-select`

Default `error`; the lowest `mf2.toml` may set it is `allow`.

A number's `select` option (plural, ordinal or exact) comes from a
variable, so the build cannot tell which rules the message selects by. The
catalog then has to carry both plural rule sets, and the message still
reports a bad option when it runs.

```text
en   place = .input {$n :integer select=$how} .match $n one {{…}} * {{…}}
```

Fix: write the value (`select=ordinal`), or make two messages, one for each
kind of selection.

### `bad-option-value`

Default `error`; the lowest `mf2.toml` may set it is `allow`.

An option that MF2 defines is given a literal value it cannot take. At run
time the function would report a bad option and format without it.

```text
en   price = It costs {$amount :currency currency=EUR minimumFractionDigits=lots}
```

Fix: a value the option takes (`minimumFractionDigits=2`).

### `gated-function`

Default `error`; the lowest `mf2.toml` may set it is `error`.

A message calls a function this build has no formatter for:

* `:number`, `:integer` and `:offset`, and choosing a plural form, need a
  number formatter for each side the build formats on (`number` alone is
  none);
* `:percent`, `:currency` and `:unit` need one that writes the language's
  own form: `builtin`, or the browser's `intl`. `plain` writes plain digits
  and does not;
* `:datetime`, `:date` and `:time` need a date formatter for each side
  (`datetime` alone is none).

A translation can never add formatting code to the application by itself.

The message names the sides that cannot format the function, the features
to write, the families of the frameworks that are on and what each
formatter costs. A server-rendered Leptos application that names only the
browser's formatter gets it, with the server's as the feature to add. The
recommended formatters are `intl` in the browser and, in native code,
`builtin` for numbers and `icu` for dates: for a server-rendered Leptos
application `leptos-client-number-intl` and `leptos-server-number-builtin`,
`leptos-client-datetime-intl` and `leptos-server-datetime-icu`; for a
command-line tool or a terminal UI `native-number-builtin` and
`native-datetime-icu`; for an Axum server `axum-number-builtin` and
`axum-datetime-icu`.

```text
en   price = It costs {$amount :currency currency=EUR}
```

with `mf2 = { …, features = ["native", "native-number-plain"] }`. Fix:
write the formatter the message names
(`features = ["native", "native-number-builtin"]`, see
[Features of `mf2`](features.md)), or leave the function out of the
message. `mf2 check` reads the features cargo resolves, or `--features`
when it is given.

### `do-not-translate`

Default `error`; the lowest `mf2.toml` may set it is `allow`.

A message the source marks `@do-not-translate`, a brand or a language's own
name, differs in a translation.

```text
en   @do-not-translate
     brand = Example
fr   brand = Exemple
```

Fix: delete it from the translation (the source's text is shown), or copy
it exactly.

### `duplicate-id`

Default `error`; the lowest `mf2.toml` may set it is `error`.

One language defines an id twice, in one file or two, so one of them would
be dropped without a word.

```text
en   save = Save
     save = Store
```

Fix: rename or delete one.

### `locale-mismatch`

Default `error`; the lowest `mf2.toml` may set it is `allow`.

A file's `@locale` header names another language than the directory it sits
in. It is nearly always a copy that was never finished.

```text
locales/fr/main.mf2   @locale de
```

Fix: make the header name the directory's language (`@locale fr`). A corpus
that keeps one language's files under another tag on purpose may lower it.

### `unknown-function`

Default `error`; the lowest `mf2.toml` may set it is `allow`.

A message calls a function that is neither one of MF2's nor listed under
[`[functions]`](configuration.md#functions).

```text
en   mood = Today: {$mood :app:emoji}
```

Fix: correct the name if it is a typo, or register the application's
function under `[functions]`. Lowered, the call stays in the catalog and the
message shows MF2's fallback for an unknown function when it runs.

### `date-mismatch`

Default `error`; the lowest `mf2.toml` may set it is `error`.

One language's message formats a variable with `:datetime`, `:date` or
`:time`, and another language's message shows the same variable bare. Only
a date function formats a date: a date in a bare placeholder is an error at
run time, shown as the placeholder's fallback. So one of the two languages
would show `{$when}` where the other shows a date. Each translation is
compared with the source message.

```text
en   posted = Posted {$when :datetime}
fr   posted = Publié {$when}
```

Fix: format the variable with the same date function in both
(`Publié {$when :datetime}`). A function of your own that formats dates is
not counted: keep its variable formatted the same way in every language.

## Warnings by default

### `missing-translation`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A language lacks an id the source language has. The report names the first
ten ids of each language, and what the catalog carries in their place
([`[catalog] missing`](configuration.md#missing)).

```text
en   save = Save
fr   (no message "save")
```

Fix: translate it. `mf2 stats` counts what each language lacks, and
`mf2 pseudo` shows untranslated text in the page. A message marked
`@do-not-translate` is never missing. Raise it to `error` to keep a release
from shipping with gaps.

### `plain-numbers`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A placeholder with no function can receive a number, and a side this build
formats on has no number formatter: the number would be written there in
plain digits (`1234.5`), not with the language's separators, grouping or
digits. The build cannot see what a call site passes, so a placeholder that
only ever receives text raises it too. A side whose number formatter is
`plain` chose plain digits, and raises nothing.

```text
en   files = {$count} files
```

with `mf2 = { …, features = ["native"] }`. Fix: write the number formatter
the message names (`native-number-builtin` here, see
[Numbers](features.md#numbers)); or, if these placeholders only receive
text, `plain-numbers = "allow"` under `[lints]`.

### `unused-feature`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A formatter is on for this build and no message can use it. It is raised
once per family, for the whole corpus:

* the number formatters (every `…-number-…` feature that is on, or
  `number` alone) when no message formats or selects on a number: no
  numeric function, no plural selection and no plain placeholder that could
  receive one;
* the date formatters (every `…-datetime-…` feature that is on, or
  `datetime` alone) when no message calls `:datetime`, `:date` or `:time`.
  Only a date function formats a date: a date handed to a plain
  placeholder is an error, so no message can use the formatter.

It is also raised for a formatter's feature of a framework that is off,
such as `axum-datetime-icu` in a crate without `axum`: it still turns its
side's formatter on, under a name that says nothing about this build, and
the message asks for the family of the framework that formats there.

The message says "on for this build": in a workspace another crate may have
turned the feature on, and cargo builds `mf2` once with the union.

```text
en   (native-datetime-icu on; no message names a date function)
```

Fix: drop the feature from `Cargo.toml` (or from the crate that turned it
on), or write the family of the framework in use; or
`unused-feature = "allow"` under `[lints]`.

### `several-formatters`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

More than one number formatter, or more than one date formatter, of one
side is on: two features of one family, or two frameworks' families that
disagree. A build formats with one of each, the strongest (for numbers
`builtin`, then `intl`, then `plain`; for dates `icu`, then `intl`, then
`iso`), and the message names it. The browser and native code are two
builds, so `leptos-client-datetime-intl` with `leptos-server-datetime-icu`
is one formatter on each side and raises nothing.

```text
en   (native-datetime-iso and native-datetime-icu on)
```

Fix: drop the formatters that do not format; or, if another crate of the
workspace turns one on, `several-formatters = "allow"` under `[lints]`.

### `unpaired-markup`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

Markup opened and not closed, or closed and not opened.

```text
en   close = Press {#kbd}Esc to close
```

Fix: close it (`{#kbd}Esc{/kbd}`), or remove the stray tag.

### `missing-plural-category`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A plural `.match` does not have a variant for every plural category of the
translation's own language. The catch-all `*` covers it, usually with the
wrong grammar.

```text
fr   visits = .input {$count :integer} .match $count one {{…}} * {{…}}
```

French also has `many` (for a million and more). Fix: add the variant
(`many {{…}}`). A language's categories are CLDR's, and they differ from
the source's: Polish has `one`, `few`, `many` and `other`.

### `non-nfc-source`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

Text that is not in Unicode Normalization Form C: an `é` typed as `e`
followed by a combining accent, say. It looks the same, but compares,
searches and sorts differently from the composed letter.

Fix: save the file normalized to NFC (most editors and translation tools
have the setting).

### `dropped-placeholder`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A translation never uses a variable its source message shows.

```text
en   greeting = Hello, {$name}!
fr   greeting = Bonjour !
```

Fix: put it back (`Bonjour, {$name} !`). A variable used anywhere in the
translation counts, so a plural's `one` variant may say "a message" without
`{$count}` while another variant shows it.

### `unknown-option`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A built-in function is given an option it does not define. MF2 ignores
unknown options, so the message formats as if it were not there.

```text
en   due = Due {$when :datetime dateStyle=long}
```

`dateStyle` is the browser's `Intl` name; MF2's is `dateLength`. Fix: the
function's own option (`dateLength=long`).

### `unused-id`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A message that no `tr!` in the application's sources names. It is raised
only by `mf2 check --src DIR`, which reads the sources; the build script
does not look.

```text
en   old-banner = Welcome to the beta!
```

The ids the generated code uses itself count as used: the languages' names,
`language.<tag>`, which the locale switcher and `Locale::name()` show. The
warning points at the line that defines the id.

Fix: for an id nothing uses, delete it from every language, once nothing
will use it again.

### `suspicious-bidi`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A bidirectional isolate character (U+2066 to U+2068) opened in literal text
and never closed with U+2069, or closed and never opened. The rest of the
line, and sometimes of the page, is laid out in the wrong direction.

Fix: close it, or remove it: a placeholder is isolated when it is
formatted, so a message rarely needs these characters by hand.

### `dynamic-currency`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A `:currency` whose `currency` option is a variable, and `[locale_data]
currencies` is not a list, so the catalog carries the data of every currency
CLDR has.

```text
en   price = It costs {$amount :currency currency=$code}
```

Fix: list the codes the variable can hold under
[`[locale_data] currencies`](configuration.md#currencies)
(`currencies = ["EUR", "USD"]`): the catalog carries only those, and the
warning stops. Or name the currency (`currency=EUR`), or accept the size
and lower it to `allow`.

### `dynamic-unit`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

The same for a `:unit` whose `unit` option is a variable, and
`[locale_data] units` is not a list: the catalog carries every unit.

```text
en   distance = {$value :unit unit=$how}
```

Fix: list the units the variable can hold under
[`[locale_data] units`](configuration.md#units), name the unit
(`unit=kilometer`), or accept the size and lower it.

### `nonstandard-name`

Default `warn`; the lowest `mf2.toml` may set it is `allow`.

A variable, option, function, markup or attribute name that is not an
ordinary identifier: it uses a character the Unicode security guidelines
advise against in identifiers, or mixes scripts. MF2 accepts it, but it
can look identical to another name.

```text
en   greeting = Hello, {$nаme}!
```

The `а` above is Cyrillic, so `$nаme` is not the `$name` a call site
passes. Fix: spell the name in one script, with letters and digits.

## The build's own faults

One check guards the build itself rather than the messages, and so has no
level in `mf2.toml`: `unread-data`. After the build has cut each language's
locale data, it checks that every piece went where one of its readers looks
(the browser, native code) and nowhere else: not into a catalog the browser
downloads when only the server reads it, not into the server's own table
when the server does not read it ([What ships where](features.md#what-ships-where)).
The build places the data itself, so this error is a fault of `mf2-build`,
never of your messages or features. Please report it, with the features of
`mf2` the build had.
