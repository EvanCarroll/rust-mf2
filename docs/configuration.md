# `mf2.toml`

`mf2.toml` sits beside the `Cargo.toml` of the crate that holds the
messages, next to `locales/`. It is optional: without it, every key has the
default this page gives. The build script and every `mf2` command read the
same file, so a build and `mf2 check` always agree. A key the file does not
know is refused, with its line and column, rather than ignored.

A file that sets every key:

```toml
source_locale = "en"

[fallback]
"es-MX" = ["es", "en"]

[catalog]
strip = ["cold", "ids"]
missing = "fallback"

[locale_data]
currencies = "used"
units = ["kilometer", "mile"]

[lints]
neutral-numbers = "allow"
dropped-markup = "warn"

[functions]
"app:emoji" = "my_app_i18n::functions::EMOJI"
```

Which functions a message may use is not configured here: it follows the
features the application turns on for `mf2` ([Features of `mf2`](features.md)).

## `source_locale`

The language the messages are written in first, as a BCP 47 tag. Default
`"en"`. Every other language is checked against it: its ids are the
application's ids, its variables are the arguments a call site passes, and
every lint compares a translation with it. It is also the last language
every fallback chain ends at. An empty tag is refused.

## `[fallback]`

For a language, the languages a missing message is taken from, in order:

```toml
[fallback]
"es-MX" = ["es-419", "es"]
"pt-BR" = ["pt"]
```

A language with no entry falls back to its parent tags, longest first
(`es-MX`, then `es`), and every chain ends at the source language whether
it names it or not. A language may not name itself. The chains are resolved
when the catalogs are built, so the browser follows no chain at run time.

The parents are found by cutting the tag, never by relating scripts:
Traditional and Simplified Chinese do not fall back to each other, as
CLDR's data says. A reader of Traditional Chinese whom the application
serves in the source language needs a Traditional catalog (`zh-Hant`). On
the web, the server says so once for each first language it could not
match, naming the reader's languages.

## `[catalog]`

What each language's catalog carries.

### `strip`

The parts of a catalog left out, a list of:

* `"cold"`: the messages' attributes and comments, which nothing formats;
* `"ids"`: the table of message ids. The generated code formats by number,
  so a catalog needs no ids to be used; `mf2 dump --manifest` reads them
  from the manifest instead.

Default `["cold", "ids"]`, which is what a browser should download. `[]`
keeps both, for a catalog a tool reads.

### `missing`

What a language that lacks a message gets in its catalog:

* `"fallback"` (the default): the text of the first language in its
  [fallback chain](#fallback) that has it. The message is marked as borrowed,
  and with `mf2`'s `mark-fallback-lang` feature the page wraps it in its own
  `lang` ([Accessibility](accessibility.md)).
* `"id"`: the message's id, so that a gap shows in the page while it is
  being translated.
* `"empty"`: nothing.

Each choice is reported by the `missing-translation` lint with the ids it
applies to.

## `[locale_data]`

How much of CLDR's currency and unit data a catalog carries, when `mf2`'s
`fn-number` feature is on (without it no number data is carried at all).

### `currencies`

* `"used"` (the default): the currencies the messages name, as in
  `{$price :currency currency=EUR}`.
* `"all"`: every currency CLDR has.
* a list of ISO 4217 codes, `["USD", "EUR"]`: those, as well as the ones the
  messages name.

A `:currency` whose `currency` option is a variable makes the catalog carry
every currency under `"used"` or `"all"`, and `"used"` raises
[`dynamic-currency`](lints.md#dynamic-currency). A list says which codes the
variable can hold: the catalog carries only those (and the ones the
messages name), with no warning. A code outside the list formats with the
code itself as its symbol and name, and two fraction digits.

### `units`

The same for `:unit` and CLDR's unit identifiers (`"kilometer"`,
`"liter-per-100-kilometer"`): `"used"`, `"all"`, or a list. A `:unit` whose
`unit` option is a variable carries every unit under `"used"` (raising
[`dynamic-unit`](lints.md#dynamic-unit)) or `"all"`, and only the listed
ones under a list. A unit outside the list is an Unsupported Operation
error when formatted, unless it is `X-per-Y` of two units the catalog has.

## `[lints]`

A lint's level, by its name: `"allow"` (say nothing), `"warn"` (report,
and the build goes on) or `"error"` (report, and the build and `mf2 check`
fail).

```toml
[lints]
missing-translation = "error"
unknown-option = "error"
dropped-markup = "warn"
```

Any lint can be raised. A lint that states a rule the build relies on
cannot be lowered below `"error"`, and one set lower is refused with the
reason; [Lints](lints.md) gives each lint's default and the lowest level
it takes.

## `[functions]`

The application's own MF2 functions: the name a message calls it by, and
the Rust path of a `&'static dyn mf2::Function` that implements it.

```toml
[functions]
"app:emoji" = "my_app_i18n::functions::EMOJI"
```

A message may then write `{$mood :app:emoji}`, and the generated module
registers the function. A name that is neither built in nor listed here is
the [`unknown-function`](lints.md#unknown-function) error. A function is
linked only if some message uses it.
