# Translating

This page follows one round of translation with the application of
[Getting started](getting-started.md), which is in English and French. The
team adds German through a translation tool, which reads XLIFF 2; a French
reviewer, who works in a plain editor, gets JSON; pseudo-locales show
whether the layout has room; `mf2 stats` shows what is still missing; and
CI keeps it that way. Every command below runs in `cargo xtask docs`,
which holds each output to what this page shows.
[The command line](command-line.md) lists every option of these commands.

## A new language

German needs two things. The source language names it, in the `[language]`
section, so that the switcher can offer it:

```mf2 file=translate/locales/en/main.mf2 before
@locale en
---

app-title = Hello, MessageFormat 2
greeting = Hello, {$name}!

# A plural: the number picks the variant, by this language's rules.
visits =
  .input {$count :integer}
  .match $count
  one {{You have been here once.}}
  *   {{You have been here {$count} times.}}

visit-again = Visit again
not-found = There is nothing here.

[language]
label = Language
apply = Apply

# Each language's name, in that language, the same in every catalog.
@do-not-translate
en = English
@do-not-translate
fr = Français
@do-not-translate
de = Deutsch
```

A language's own name is marked `@do-not-translate`: it reads the same in
every catalog, so no translator is asked for it, a language that leaves it
out shows the source's, and none counts as missing it. A language that
copies it has to copy it exactly, as French does
([`do-not-translate`](lints.md#do-not-translate)).

And German gets a file, which starts with no messages:

```mf2 file=translate/locales/de/main.mf2 before
@locale de
---
```

## Export for a translation tool

```sh run=translate output=export-de.txt
mf2 export de --format xliff -o de.xlf
```

```text file=translate/export-de.txt generated
mf2 export: 10 messages of en with de's translations to de.xlf
```

`de.xlf` is an XLIFF 2 document, which translation tools read: each
message of the source language is a unit, with the German translation as
its target where German has one (none yet). A placeholder is a `<ph>`
element, which a tool keeps intact and lets the translator move; a
`.match` is a group of its variants, one unit each; a comment above a
message is a note for the translator; and a message marked
`@do-not-translate` is a unit the tool locks.

## The translation comes back

The translator sends the document back with targets filled in, all but
one: `not-found` is still in English.

```xml file=translate/from-translator/de.xlf before
<?xml version="1.0" encoding="UTF-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.1" srcLang="en" trgLang="de" xml:space="preserve">
  <file id="f1" original="main.mf2" canResegment="no">
    <unit id="app-title" name="app-title">
      <segment>
        <source>Hello, MessageFormat 2</source>
        <target>Hallo, MessageFormat 2</target>
      </segment>
    </unit>
    <unit id="greeting" name="greeting">
      <originalData>
        <data id="d1">{$name}</data>
      </originalData>
      <segment>
        <source>Hello, <ph id="1" dataRef="d1" disp="{$name}"/>!</source>
        <target>Hallo, <ph id="1" dataRef="d1" disp="{$name}"/>!</target>
      </segment>
    </unit>
    <group id="visits" name="visits" type="mf2:select">
      <notes>
        <note category="comment">A plural: the number picks the variant, by this language's rules.</note>
      </notes>
      <unit id="visits:1" name="one">
        <segment>
          <source>You have been here once.</source>
          <target>Du warst einmal hier.</target>
        </segment>
      </unit>
      <unit id="visits:2" name="*">
        <originalData>
          <data id="d1">{$count}</data>
        </originalData>
        <segment>
          <source>You have been here <ph id="1" dataRef="d1" disp="{$count}"/> times.</source>
          <target>Du warst <ph id="1" dataRef="d1" disp="{$count}"/>-mal hier.</target>
        </segment>
      </unit>
    </group>
    <unit id="visit-again" name="visit-again">
      <segment>
        <source>Visit again</source>
        <target>Wieder besuchen</target>
      </segment>
    </unit>
    <unit id="not-found" name="not-found">
      <segment>
        <source>There is nothing here.</source>
      </segment>
    </unit>
    <group id="s:language" name="language" type="mf2:section">
      <unit id="language.label" name="language.label">
        <segment>
          <source>Language</source>
          <target>Sprache</target>
        </segment>
      </unit>
      <unit id="language.apply" name="language.apply">
        <segment>
          <source>Apply</source>
          <target>Übernehmen</target>
        </segment>
      </unit>
      <unit id="language.en" name="language.en" translate="no">
        <notes>
          <note category="comment">Each language's name, in that language, the same in every catalog.</note>
        </notes>
        <segment>
          <source>English</source>
        </segment>
      </unit>
      <unit id="language.fr" name="language.fr" translate="no">
        <segment>
          <source>Français</source>
        </segment>
      </unit>
      <unit id="language.de" name="language.de" translate="no">
        <segment>
          <source>Deutsch</source>
        </segment>
      </unit>
    </group>
  </file>
</xliff>
```

`mf2 import` reads it into German's files:

```sh run=translate output=import-de.txt
mf2 import de from-translator/de.xlf
```

```text file=translate/import-de.txt generated
./locales/de/main.mf2:1:1: warn: 1 of 7 messages are missing here and fall back to en: not-found (locale de) [missing-translation]
mf2 import: 0 message(s) changed, 6 added, in de
```

```mf2 file=translate/locales/de/main.mf2 generated
@locale de
---

app-title = Hallo, MessageFormat 2
greeting = Hallo, {$name}!

visits =
  .input {$count :integer}
  .match $count
  one {{Du warst einmal hier.}}
  * {{Du warst {$count}-mal hier.}}

visit-again = Wieder besuchen

[language]
label = Sprache
apply = Übernehmen
```

An XLIFF document adds the messages the language does not have yet, in the
file and section where the source has them, and it changes the ones it
has; the files keep their comments and properties. The names are not
there: German shows the source's. The warning is the message the
translator left: until it is translated, a German reader sees it in
English ([`missing-translation`](lints.md#missing-translation)).

## A review, as JSON

A reviewer who works in a plain editor gets French as JSON, one entry per
id:

```sh run=translate output=export-fr.txt
mf2 export fr -o fr.json
```

```text file=translate/export-fr.txt generated
mf2 export: 9 messages of fr to fr.json
```

The reviewer sends back the two entries they changed, and one has a
mistake: the placeholder `{$name}` became `{$nom}`, which the code never
passes.

```json file=translate/review/fr.json before
{
  "greeting": "Salut, {$nom} !",
  "not-found": "Cette page n’existe pas."
}
```

```sh run=translate status=1 output=import-fr.txt
mf2 import fr review/fr.json
```

```text file=translate/import-fr.txt generated
./locales/fr/main.mf2:5:21: error: $nom is not an input of the source message; if this language needs it, the source has to declare it with .input (in greeting, locale fr) [undeclared-variable]
./locales/fr/main.mf2:5:12: warn: the source message shows $name, which this translation does not (in greeting, locale fr) [dropped-placeholder]
mf2 import: 1 error(s) that fr does not have now; nothing was written
```

Before it writes anything, `import` runs every check `mf2 check` makes on
the files as they would be. A translation that would bring an error is
reported with its file, line and code, nothing is written, and the command
exits with 1; an error the files had already does not stop it. The same
holds for markup: a translation that leaves out a link or emphasis its
source has is an error ([`dropped-markup`](lints.md#dropped-markup)),
because the reader would lose it.

The reviewer corrects the placeholder:

```json file=translate/review/fr-fixed.json before
{
  "greeting": "Salut, {$name} !",
  "not-found": "Cette page n’existe pas."
}
```

```sh run=translate output=import-fr-fixed.txt
mf2 import fr review/fr-fixed.json
```

```text file=translate/import-fr-fixed.txt generated
mf2 import: 2 message(s) changed in fr
```

JSON changes only the messages the language has, which is what a review
needs. An id the language does not have yet is left out, named, and the
command exits with 1 and points to XLIFF, which knows where in the files a
new message goes.

## Pseudo-locales: room for longer text

Pseudo-locales show what a translation will do to the layout, before a
translator starts:

```sh run=translate-pseudo output=pseudo.txt
mf2 pseudo
```

```text file=translate-pseudo/pseudo.txt generated
mf2 pseudo: en-XA — 7 message(s) in 1 file(s)
mf2 pseudo: ar-XB — 7 message(s) in 1 file(s)
```

```mf2 file=translate-pseudo/locales/en-XA/main.mf2 generated
@locale en-XA
---

app-title = [Ĥéļļö, ṀéššåĝéƑöŕɱåţ 2 one two]
greeting = [Ĥéļļö, {$name}! one]

# A plural: the number picks the variant, by this language's rules.
visits =
  .input {$count :integer}
  .match $count
  one {{[Ýöû ĥåṽé ƀééñ ĥéŕé öñçé. one two]}}
  * {{[Ýöû ĥåṽé ƀééñ ĥéŕé {$count} ţîɱéš. one two]}}

visit-again = [Ṽîšîţ åĝåîñ one]
not-found = [Ţĥéŕé îš ñöţĥîñĝ ĥéŕé. one two]

[language]
label = [Ļåñĝûåĝé one]
apply = [Åþþļý one]

# Each language's name, in that language, the same in every catalog.
@do-not-translate
en = English
@do-not-translate
fr = Français
@do-not-translate
de = Deutsch
```

`en-XA` is the source text accented, in brackets and about 30 % longer.
Text that shows without brackets did not come from a catalog, and a label
that overflows or is cut short has no room for a longer language. `ar-XB`
forces each run of text right to left, so a page that does not apply the
language's direction shows it reversed. Placeholders, markup and the
messages marked `@do-not-translate` are left as they are. Run the
application (`cargo leptos watch`) and open
<http://127.0.0.1:3000/?lang=en-XA>, then `?lang=ar-XB`.

The pseudo-locales are for a development build, and a release does not
ship them. They are generated, so keep them out of the repository:

```text
# .gitignore
/locales/en-XA/
/locales/ar-XB/
```

While they are there, `mf2 check` reports a warning about `ar-XB` (the
other is German's gap):

```sh run=translate-pseudo output=check.txt
mf2 check --features native,native-number-builtin
```

```text file=translate-pseudo/check.txt generated
./locales/ar-XB/main.mf2:9:3: warn: ar-XB has the plural categories zero, two, few, many, which no variant names; they all fall to the catch-all (in visits, locale ar-XB) [missing-plural-category]
./locales/de/main.mf2:1:1: warn: 1 of 7 messages are missing here and fall back to en: not-found (locale de) [missing-translation]
mf2 features (as --features names them):
  the corpus needs: numbers
  on:               native-number-builtin
  on and unused:    none
  native code:      `builtin` formats numbers (native-number-builtin; +9.7 KB over `plain`, and the number data)
  write:            mf2 = { ..., features = ["native", "native-number-builtin"] }
mf2 check: 0 error(s), 2 warning(s)
```

`ar-XB` has the variants of the English source, which is enough to test
the layout. The pseudo-locales need no name messages: the build names each
by its tag, so the switcher and `Locale::name()` show `en-XA` and `ar-XB`.

## What is missing: `mf2 stats`

```sh run=translate output=stats.txt
mf2 stats --features native,native-number-builtin
```

```text file=translate/stats.txt generated
corpus . — 10 messages (3 marked @do-not-translate), source locale en, manifest 0x6e06fdf7f6d6e715
CLDR 48.2.1 · MF2 spec 5c4ddb27 · catalog format v2

locale    coverage  missing       raw        gz        br  catalog
de           85.7%        1       425       328       271  de.bf22f132db3ecb6e.mf2b
en          100.0%        0       394       307       225  en.672c914838662fd5.mf2b
fr          100.0%        0       448       350       307  fr.06bc11d3b946c25a.mf2b

what ships where (raw bytes, who reads it, where it ships):
  de
    messages               387 B  native code alone            catalog
    fallback                21 B  native code alone            catalog
    plural.cardinal          5 B  native code alone            catalog
    number.symbols          12 B  native code alone            catalog
  en
    messages               377 B  native code alone            catalog
    plural.cardinal          5 B  native code alone            catalog
    number.symbols          12 B  native code alone            catalog
  fr
    messages               409 B  native code alone            catalog
    fallback                 9 B  native code alone            catalog
    plural.cardinal         16 B  native code alone            catalog
    number.symbols          14 B  native code alone            catalog
bytes a browser downloads and never reads: 0 B (no browser side: native code alone reads these catalogs)

canonical equivalence, the keys a decomposed value can reach:
  de            0 B  (nothing: only an identical string matches a key)
  en            0 B  (nothing: only an identical string matches a key)
  fr            0 B  (nothing: only an identical string matches a key)

0 error(s), 1 warning(s) — run `mf2 check`
```

For each language, `coverage` and `missing` count the messages that need
translating: the three names marked `@do-not-translate` count in neither,
here or in `check`. German lacks one of seven. The next columns are the
language's catalog, raw, gzipped and brotli-compressed. The next section
says what ships where: each part of each catalog, its bytes, who reads it
and where it ships. Here native code alone reads the catalogs, so every
part ships in them and the closing line says so; in a hydrated application
it names what the browser reads, and what stays in the server's own table
([`stats`](command-line.md#stats-dump-what-is-in-a-catalog)).
`--format json` prints the same for a dashboard. Like `check`, `stats`
counts with the features cargo resolves for the application's `mf2`;
`--features` names them instead, as here.

## The checks in CI

Two commands keep the messages in shape. `mf2 fmt --check` fails when a
file is not in the one layout (`import` and `pseudo` write it; a hand edit
may not):

```sh run=translate output=fmt.txt
mf2 fmt --check
```

```text file=translate/fmt.txt generated
mf2 fmt: 0 of 3 file(s) would change
```

`mf2 check` runs every check the build makes. Errors fail it always;
`--deny-warnings` fails it on warnings too, and here the German gap does:

```sh run=translate status=1 output=ci-check.txt
mf2 check --features native,native-number-builtin --deny-warnings
```

```text file=translate/ci-check.txt generated
./locales/de/main.mf2:1:1: warn: 1 of 7 messages are missing here and fall back to en: not-found (locale de) [missing-translation]
mf2 features (as --features names them):
  the corpus needs: numbers
  on:               native-number-builtin
  on and unused:    none
  native code:      `builtin` formats numbers (native-number-builtin; +9.7 KB over `plain`, and the number data)
  write:            mf2 = { ..., features = ["native", "native-number-builtin"] }
mf2 check: 0 error(s), 1 warning(s)
```

Which gaps fail a build is the team's choice. Without `--deny-warnings`, a
missing translation is reported and the fallback ships; `mf2.toml` can
raise `missing-translation` alone to an error, or lower another code
([Lints](lints.md), [`mf2.toml`](configuration.md)).

A Forgejo workflow that runs them on every push and pull request, in the
directory that holds `locales/` (with `-C DIR` if the messages live in a
crate of their own). The runner needs git and a Rust toolchain; `check`
asks cargo which features the application turns on.

```yaml
# .forgejo/workflows/messages.yml
on:
  push:
  pull_request:

jobs:
  messages:
    runs-on: rust # one of your runner's labels
    steps:
      - name: Fetch the sources
        run: |
          git init --quiet .
          git fetch --quiet --depth=1 "${{ github.server_url }}/${{ github.repository }}.git" "${{ github.sha }}"
          git checkout --quiet --detach FETCH_HEAD
      - name: Install mf2
        run: cargo install mf2-cli --locked
      - name: One layout
        run: mf2 fmt --check
      - name: Every check the build makes
        run: mf2 check --deny-warnings
      - name: What each language lacks
        run: mf2 stats
```

The job fetches the sources with git; a private repository adds its token
to the fetch. `mf2 check --format json` prints the findings as data, for a
job that comments on a pull request.
