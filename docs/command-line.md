# The command line

`mf2` is the command for an application's messages: it makes a starter,
checks the messages, formats their files, compiles its catalogs, and moves translations in and
out. Install it once:

```sh
cargo install mf2-cli
```

Every command works on the crate that holds the messages (the directory
with `locales/`, and `mf2.toml` if it has one), which is the current
directory or the one `-C DIR` names. It needs no cargo and no build: a
translator's machine, a CI job or an editor can run it. On this page it
runs in the application of [Getting started](getting-started.md),
and `cargo xtask docs` runs each command and checks that it prints and
writes what the page shows.

A command exits with **0** when it has nothing to report, and **1** when
it reports an error (or a warning, under `--deny-warnings`) or could not
run; a flag it does not know is refused before it starts. What it prints
is for a person and may be worded differently in a later release: a
script should read the exit status, or ask for `--format json` where a
command offers it.

## `init`: a starter

```sh run=count
mf2 init --cli
```

makes a complete command-line application in the current directory, which
must be empty (or name a new one: `mf2 init --cli count`). It counts the
files in a directory and says so in English or French; `cargo run -- --lang
fr` tries it. [Native applications](native-apps.md#a-command-line-tool)
shows and explains every file it writes.

```sh run=hops
mf2 init --tui
```

makes a Ratatui terminal UI instead: a bordered table, a language menu, a
key-hint bar and a status line, with `1` and `2` switching the language
while it runs. Both are the shapes the [native applications](native-apps.md)
page explains.

Three more make a Leptos application, each in one crate, with its messages
in `locales/` beside its code:

```sh run=hello-ssr
mf2 init --ssr
```

makes a server-rendered application that hydrates in the browser, built
with [cargo-leptos](https://github.com/leptos-rs/cargo-leptos): `cargo
leptos watch` serves it on <http://127.0.0.1:3000>. Its page shows a
greeting, a counter whose text is a plural, and a language switcher that
changes the language live.

```sh run=hello-islands
mf2 init --islands
```

makes the same page as an [islands](delivery-modes.md#islands) application:
only the counter runs in the browser, and a switch loads the page again in
the new language.

```sh run=hello-csr
mf2 init --csr
```

makes a [client-only](delivery-modes.md#client-only) application, built
with Trunk: `trunk serve` builds it, and its `Trunk.toml` runs `mf2 compile
--site` after each build, to publish the catalogs beside the wasm.

Every new application `init` makes has a `.gitignore`, as `cargo new`'s
do, that keeps what the build writes out of version control: `/target`,
and for the client-only one Trunk's `/dist` too.

In a directory that holds a crate already, each mode adds translations to
it instead: a build script, `locales/en/main.mf2`, and `mf2` (with the
mode's features) and `mf2-build` through `cargo add`. It prints what is
left to write, which it cannot write into an existing manifest or code: for
a Leptos application, the lines that forward its `ssr` and `hydrate`
features to `mf2`, the include, `install()` on each side and a first
`tr!`; and the build-override above, for the workspace's root manifest.
`--locale` adds a language, `--source-locale` changes the source language
from `en`, `--no-messages` leaves the messages out, and `--force` writes
over files that are there.

Without a mode, `init` lists the five and changes nothing.

## `check`: every check the build makes

```sh run=cli output=check.txt
mf2 check --features fn-number
```

```text file=cli/check.txt generated
mf2 check: 9 messages in 2 locales, nothing to report
```

`check` parses every file, checks every message against the source
language's, and runs the lints `mf2.toml` configures, exactly as the build
script does. Which functions a message may use depends on `mf2`'s features
in the crate, so `check` uses the features cargo resolves for the
application's build, from what cargo has already fetched for this
machine (it never downloads). `--features` names them instead (as above).
Where cargo cannot answer — no `Cargo.toml`, or dependencies not fetched
yet — `check` says so in one line and checks as if every function were on,
so that it reports none as missing that the build may have.

* `--deny-warnings` makes a warning fail the command, as it fails a build
  whose lints are raised to errors.
* `--format json` prints the findings as JSON, for an editor or CI.
* `--src DIR` (repeatable) also reads the Rust sources in `DIR`, and
  reports a message no `tr!` uses (`unused-id`).

A finding names its file, line and column, the problem, and its code in
brackets: `missing-translation`, for one, names the first ten missing ids
of each language. `mf2.toml`'s `[lints]` raises or lowers a code;
[Lints](lints.md) explains each one, with an example and its fix.

A message the source marks `@do-not-translate` — a brand, or a language's
own name — needs no translation. A language that does not have it shows the
source's, and it is not counted as missing, here or in `stats`; one that
copies it has to copy it exactly (`do-not-translate`). The mark on a
`[section]`, or at the top of a file, covers every message under it.

A translation that leaves out markup its source message has is an error,
`dropped-markup`: `Accept our {#link}terms{/link}.` translated as
`Acceptez nos conditions.` would take the link away from French readers.
One variant of a message may leave the markup out as long as another keeps
it, and a corpus that drops emphasis on purpose lowers the code with
`dropped-markup = "warn"` under `[lints]`.

## `fmt`: one layout for every file

```sh run=cli output=fmt.txt
mf2 fmt --check
```

```text file=cli/fmt.txt generated
mf2 fmt: 0 of 2 file(s) would change
```

`fmt` writes every `.mf2` file under `locales/` (or the paths it is given)
in one layout, so that a diff shows what changed and not how it was typed:
a blank line after the `---` that ends the file's header, one around a
message that starts on its own line (a `.match`), and none anywhere else;
an entry's value as the message's own canonical form. `--check` changes
nothing and exits with 1 if a file would change. The other commands that
write `.mf2` files (`convert`, `import`, `pseudo`) write this layout too.

## `compile`: the catalogs

```sh run=cli output=compile.txt
mf2 compile --features fn-number --out catalogs
```

```text file=cli/compile.txt generated
mf2 compile: 9 messages, 2 locales to catalogs (8 file(s) changed)
```

writes what the build script writes — each language's catalog (with `.br`
and `.gz` versions), the manifest, and the generated module — into a
directory of your choosing. It is what a client-only site publishes (with
`--site DIR`, which also writes `index.json` and takes its features from
cargo; see [Delivery modes](delivery-modes.md#client-only)) and what a
native application ships ([Native apps](native-apps.md)). Give it the
features the application builds the crate with: they decide what a
catalog holds, and its content-hashed name. `-v` lists the files.

## `stats`, `dump`: what is in a catalog

`mf2 stats` prints, for each language, how many of the messages
that need translating it has and lacks (not those marked
`@do-not-translate`), and its catalog's size raw, gzipped and
brotli-compressed; then
the locale data each catalog carries, entry by entry. `--format json` for
a dashboard. Like `check`, it uses the features cargo resolves for the
application's `mf2`, or those `--features` names.

`mf2 dump <file.mf2b>` prints a compiled catalog back as MF2 (or, with
`--format json`, as data), all of it or one message (`--id`), reading ids
from the manifest (`--manifest`) when the catalog was built without them.

## `export`, `import`: translations in and out

[Translating](translating.md) follows a team through a round of this, from
export to the checks in CI.

A translation tool or a translator who does not work in the repository gets
one language as a file, and gives it back. As JSON, one message per id:

```sh run=cli
mf2 export fr -o fr.json
```

```json file=cli/fr.json generated
{
  "app-title": "Bonjour, MessageFormat 2",
  "greeting": "Bonjour, {$name} !",
  "language.apply": "Appliquer",
  "language.en": "English",
  "language.fr": "Français",
  "language.label": "Langue",
  "not-found": "Il n’y a rien ici.",
  "visit-again": "Revenir",
  "visits": ".input {$count :integer}\n.match $count\none  {{Vous êtes venu une fois.}}\nmany {{Vous êtes venu {$count} fois.}}\n*    {{Vous êtes venu {$count} fois.}}"
}
```

or as XLIFF 2, which translation tools read, with the source text beside
each translation, placeholders as `<ph>` elements the tool keeps intact,
and a `.match` as a group of its variants:

```sh run=cli
mf2 export fr --format xliff -o fr.xlf
```

`import` reads either back (it tells them apart by their content) into the
language's `.mf2` files as they stand: only the messages' values change,
and the files keep their sections, comments and properties. `--dry-run`
says what would change and writes nothing:

```sh run=cli output=import.txt
mf2 import fr fr.json --dry-run
```

```text file=cli/import.txt generated
mf2 import: 0 message(s) would change in fr
```

Before it writes anything, `import` runs every check `check` makes on the
files as they would be, with the same features (`--features` names them).
A translation that would bring an error — a variable its source does not
declare, a link left out — is reported as `check` reports it, and nothing
is written; an error the files already had does not stop it. An XLIFF
document also adds the messages the language does not have yet, in the
file and section where the source has them. JSON changes only the messages
the language has: it names the ones it leaves out, and exits with 1.

## `pseudo`: find what is not translated

```sh run=cli output=pseudo.txt
mf2 pseudo --dry-run
```

```text file=cli/pseudo.txt generated
would write ./locales/en-XA/main.mf2
mf2 pseudo: en-XA — 9 message(s) in 1 file(s) (dry run)
would write ./locales/ar-XB/main.mf2
mf2 pseudo: ar-XB — 9 message(s) in 1 file(s) (dry run)
```

writes two pseudo-languages made from the source messages. `en-XA` is the
text in brackets, accented letter for letter and about 30 % longer, so a
string left untranslated, or a layout with no room for a longer language,
stands out. `ar-XB` forces each run of text right to left, so a page that
does not apply the language's direction shows its text reversed.
Placeholders and markup are left alone. `--locale` writes one of the two.

## `watch`: rebuild on every edit

`mf2 watch --out DIR` compiles like `compile` (into `dist` if no
`--out` is given), then again each time a file under `locales/` changes, for a development server that serves the
catalogs from `DIR`. A `cargo leptos watch` does not need it: its
`watch-additional-files` rebuilds the application instead
([Getting started](getting-started.md)).

## `convert`: from Fluent

`mf2 convert --from leptos-fluent` converts a `leptos-fluent` application —
its `.ftl` files and its call sites — and has
[its own page](migrating-from-leptos-fluent.md). `--from fluent` converts
`.ftl` files only, for a project that does not use `leptos-fluent`:

```sh
mf2 convert --from fluent locales-ftl
```

`locales-ftl` holds one directory per language (`locales-ftl/fr/*.ftl`),
and each `.ftl` file becomes a `.mf2` file named after it, in the
crate's `locales/`. A second run over the same files writes
nothing and says the files are unchanged; a file it would write that
already holds other text stops it, so nothing is overwritten.

A message converts to the MF2 message that formats as `fluent-bundle`
formatted it, for the same arguments. What cannot be converted that way is
reported with the file, line and column, and a code; the conversion exits
with 1 until none is an error:

| Code | Level | What |
|---|---|---|
| `fluent-junk` | error | text Fluent itself could not parse |
| `fluent-missing-reference` | error | a reference to a message, attribute or term that does not exist |
| `fluent-cyclic-reference` | error | messages or terms that refer to each other in a loop |
| `fluent-unknown-function` | error | a function other than `NUMBER` and `DATETIME` |
| `fluent-number-operand` | error | `NUMBER` of something that is not a variable or a number |
| `fluent-currency-missing` | error | `NUMBER(…, style: "currency")` with no `currency` |
| `fluent-datetime-option` | error | a `DATETIME` option MF2 has no counterpart for |
| `fluent-date-selector` | error | a date used to choose a variant: MF2 dates do not select |
| `fluent-mixed-keys` | error | a select whose keys are both numbers or plural categories and plain words, so whether it selects on a number or a string is decided only at run time |
| `fluent-variant-limit` | error | a message that would need more than 256 variants |
| `fluent-duplicate-id` | error | an id defined twice in one language |
| `fluent-file-collision` | error | two files that would write one `.mf2` file |
| `fluent-locale` | error | a directory whose name is not a language tag |
| `fluent-datetime-approximate` | warning | every converted `DATETIME`: MF2's nearest form, which may not be identical |
| `fluent-number-option` | warning | a `NUMBER` option Fluent ignored, dropped |
| `fluent-unreachable-variant` | warning | a variant Fluent could never choose, kept |
| `fluent-unbound-term-variable` | warning | a variable in a term that nothing sets: kept as the text Fluent printed |
| `fluent-term-positional` | warning | a positional argument to a term, which Fluent ignores |

Fluent terms (`-brand`) have no MF2 counterpart: each is copied into every
message that uses it, and the report counts where.

## Every option

Every command takes `-C DIR` (`--dir`), the crate that holds the messages
(the current directory by default), and `-h` (`--help`). `--features LIST`
names the features of `mf2` the application builds with, separated by
commas (`fn-number,fn-datetime`). `--format json` reports as JSON, for CI and
editors.

| Command | Arguments and options |
|---|---|
| `init [DIR]` | `--cli`, `--tui`, `--ssr`, `--islands`, `--csr`: the kind of application; `--name NAME`: the crate's name (a new application's is its directory's); `--source-locale TAG` (default `en`); `--locale TAG`: another language, repeatable; `--force`: write over files that are there; `--no-messages`: no starter `locales/<tag>/main.mf2`, for messages that come from `convert` or `import` |
| `check` | `--features LIST`; `--format text\|json`; `--deny-warnings`: fail on a warning too; `--src DIR`: the Rust sources, for [`unused-id`](lints.md#unused-id), repeatable |
| `fmt [PATH]…` | the files or directories (default `locales/`); `--check`: write nothing, name the files that would change, exit 1 if any would |
| `compile` | `--features LIST`; `-o`, `--out DIR` (default `dist`): the manifest, the catalogs and the generated module; `--site DIR`: instead, only what a static host serves, the catalogs and `index.json`, with the features cargo resolves; `--facade PATH`: the crate the generated module re-exports as `__mf2` (default `::mf2`); `-v`, `--verbose`: list what was written |
| `stats` | `--features LIST`; `--format text\|json` |
| `dump FILE.mf2b` | `--format mf2\|json`: MF2 source, one message per line (the default), or the data model as JSON; `--manifest FILE.mf2m`: the ids of a catalog built without them; `--id ID`: one message |
| `pseudo` | `--locale en-XA\|ar-XB`: one of the two (both by default); `--dry-run`: write nothing |
| `export LOCALE` | `-o`, `--out FILE` (default: standard output); `--format json\|xliff` (default `json`) |
| `import LOCALE FILE` | `FILE` is flat JSON or XLIFF 2, told apart by content; `--dry-run`: write nothing; `--features LIST` |
| `watch` | `--features LIST`; `-o`, `--out DIR` (default `dist`); `--interval MS`: how often to look (default 300); `--max-rebuilds N`: stop after N rebuilds (0, the default, never stops) |
| `convert DIR` | `--from fluent\|leptos-fluent`; `--format text\|json`; `--write` (`leptos-fluent`): write the files, where without it the diff is shown; `--locales DIR` (`leptos-fluent`): the `.ftl` directory, if not the initializer's `locales:` or `APP_DIR/locales`; `--i18n-crate NAME` (`leptos-fluent`): the crate the rewritten `use` names |

The lints `check` reports are listed, each with an example and its fix, in
[Lints](lints.md), and `mf2.toml`'s keys in [`mf2.toml`](configuration.md).
