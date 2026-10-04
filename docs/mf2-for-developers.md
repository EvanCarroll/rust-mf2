# MF2 for developers

This page is about the message language: how to write a message, and why
it looks the way it does. Unicode MessageFormat 2 (MF2) is part of
Unicode's CLDR specification (UTS #35). It defines **one message**: its
text, the values it takes, how it formats them, and how it chooses
between wordings. The files that hold many messages follow a W3C draft,
the *Message Resource* format, which is still being written; this
library reads it as `.mf2` files.

Every example here belongs to one small command-line application,
`guide`, which prints each message in English and in French. `cargo
xtask docs` builds it, so every message on this page is one the build
accepts. [Native applications](native-apps.md) explains the parts of such
an application that this page only shows.

```toml file=guide/Cargo.toml
[package]
name = "guide"
version = "0.1.0"
edition = "2024"

[dependencies]
mf2 = { version = "2", features = ["native", "fn-number"] }

[build-dependencies]
mf2-build = "2"
```

```rust file=guide/build.rs
fn main() {
    mf2_build::run();
}
```

## The `.mf2` file

Messages live in `locales/<language>/<name>.mf2`, one directory per
language. A file has two parts, split by a line of three dashes:

* **Before `---`**, the file's properties. `@locale` names the file's
  language, and is the one property every file needs.
* **After `---`**, the messages: one `id = message` each. A message may
  continue on the following lines if they are indented; the indentation is
  removed.

A line that starts with `#` is a comment, and it belongs to the message
(or section) right below it, unless an empty line comes between them. A
line that starts with `@` is a property of what follows. A `[section]`
line puts its name, and a dot, in front of every id after it, up to the
next section: below, `total` in section `[order]` is the message
`order.total`. Sections do not nest, so a section names its full path
(`[settings.sound]`).

```mf2 file=guide/locales/en/main.mf2
@locale en
---

# The first line the application prints.
@param $name - the reader's name, as they wrote it
welcome = Welcome, {$name}!
braces = Write \{ and \} for a brace in a message.
```

`tr!` names a message by its id and passes its arguments by name:

```rust file=guide/src/main.rs
//! Prints every message of the MF2 chapter, in each of its languages.

mf2::include_generated!();

fn main() {
    install();
    for locale in Locale::ALL {
        set_locale(locale);
        println!("{}", tr!("welcome", name = "Ada"));
        println!("{}", tr!("braces"));
```

## Text and placeholders

Everything in a message is text, except what is in braces:

* `{$name}` is a **placeholder**: the value of the argument `name`.
* `{|some text|}` is a literal, and prints as it is. A literal rarely
  helps on its own, but it can be formatted, like any value.
* `\{`, `\}`, `\|` and `\\` write a brace, a bar or a backslash.

The spaces after `=` are not part of the message; every other space is,
even at the end of a line.

## Functions and options

A placeholder can name a **function**, which formats its value: `{$amount
:number}`. Options follow the function's name, as `name=value`:

```mf2 file=guide/locales/en/main.mf2
[order]

@param $amount - the order's total, in the shop's currency
total =
  .local $sum = {$amount :number minimumFractionDigits=2}
  {{Total: {$sum}}}

@param $share - the discount, as a fraction: 0.15 is 15%
discount = {$share :percent} off
```

The functions this library provides:

| Function | Formats |
|---|---|
| `:string` | a value as text; `.match` compares the text |
| `:number` | a number, in the reader's language; `.match` uses exact values, then the plural category |
| `:integer` | a number, as a whole number; `.match` as `:number` |
| `:percent` | a fraction, as a percentage (with `fn-number`) |
| `:currency`, `:unit` | an amount of money, a measure (with `fn-number`) |
| `:datetime`, `:date`, `:time` | a date and time (with a [date formatter](features.md#dates)) |

Common `:number` options are `minimumFractionDigits`,
`maximumFractionDigits` and `useGrouping`. `select=ordinal` makes a number
select on its ordinal category ("1st", "2nd") instead of its plural one.

A message that starts with a **declaration** writes its text between
double braces, `{{…}}`:

* `.local $sum = {…}` gives a formatted value a name, used by the text
  below it (`total` above).
* `.input {$count :integer}` gives an argument a function, used everywhere
  the message names the argument. It is how a message says what a
  selection is made on.

A translation may choose other options, or other functions, from the
source. It is the translator's message too.

## Plurals

`.match` chooses among **variants**, one per wording. Each variant starts
with its key; `*` matches any value, and every `.match` needs a `*`
variant. A number matches an exact key first (`0`), then its **plural
category** in the language: `zero`, `one`, `two`, `few`, `many` or
`other`.

```mf2 file=guide/locales/en/main.mf2
# Still in [order], so its id is order.items.
@param $count - how many items the basket holds; a whole number
items =
  .input {$count :integer}
  .match $count
  0   {{Your basket is empty.}}
  one {{One item in your basket.}}
  *   {{{$count} items in your basket.}}
```

The categories are the language's own. English has `one` and `other`;
French adds `many` (for a million and more), and its `one` also covers 0;
Polish, Arabic and others have more. Each language's file names the
categories its grammar needs, and the build warns about a category the
language has that no variant names.

### Ordinals

`select=ordinal` chooses by position: "1st", "2nd", "3rd", "4th". English
uses four ordinal categories for this, French two.

```mf2 file=guide/locales/en/main.mf2
[race]

@param $place - the reader's place in the race: 1 is first
place =
  .input {$place :integer select=ordinal}
  .match $place
  one {{You finished {$place}st.}}
  two {{You finished {$place}nd.}}
  few {{You finished {$place}rd.}}
  *   {{You finished {$place}th.}}
```

```rust file=guide/src/main.rs
        for count in [0, 1, 3] {
            println!("{}", tr!("order.items", count = count));
        }
        println!("{}", tr!("order.total", amount = 1234.5));
        println!("{}", tr!("order.discount", share = 0.15));
        for place in [1, 2, 3, 4, 11, 22] {
            println!("{}", tr!("race.place", place = place));
        }
```

## Selecting on several values

`.match` can take several values, and each variant then has one key per
value, in order. The usual case is a person's grammatical gender and a
count:

```mf2 file=guide/locales/en/main.mf2
[social]

@param $name - who shared the photos
@param $gender - their gender, for the grammar: female, male or other
@param $count - how many photos they shared
shared =
  .input {$gender :string}
  .input {$count :integer}
  .match $gender $count
  female one {{{$name} added a photo to her album.}}
  female *   {{{$name} added {$count} photos to her album.}}
  male   one {{{$name} added a photo to his album.}}
  male   *   {{{$name} added {$count} photos to his album.}}
  *      one {{{$name} added a photo to their album.}}
  *      *   {{{$name} added {$count} photos to their album.}}
```

The order of the variants does not matter: the best match wins, and a
key that names a value beats `*`. Every combination needs a variant that
matches it, which is why the last one is `* *`.

`:string` selects on text, and that is the MF2 way to choose on a Rust
enum: pass a **key**, and let each language's message say what the key
reads as.

```mf2 file=guide/locales/en/main.mf2
[presence]

# The code passes one of: online, away, offline.
status =
  .input {$status :string}
  .match $status
  online {{Online}}
  away   {{Away}}
  *      {{Offline}}
```

Passing the enum itself, through `Display`, would put `Display`'s text in
the message. That text comes from Rust code and is **not translated**: a
French reader would see English. The argument is a key, so the words stay
in the messages:

```rust file=guide/src/main.rs
        for (name, gender) in [("Ada", "female"), ("Alan", "male"), ("Sam", "other")] {
            println!("{}", tr!("social.shared", name = name, gender = gender, count = 2));
        }
        for presence in [Presence::Online, Presence::Away, Presence::Offline] {
            println!("{}", tr!("presence.status", status = presence.key()));
        }
```

## Markup

`{#name}…{/name}` marks a stretch of a message, and `{#name/}` stands on
its own. The message says **what** a stretch is (a key to press, a link,
something to stress); the code decides **how** it looks: an element on
the web ([Call sites](call-sites.md#markup-as-elements)), a style in a terminal
([Native applications](native-apps.md)). As plain text, as here, the
markup is left out.

```mf2 file=guide/locales/en/main.mf2
[help]
save = Press {#key}Ctrl+S{/key} to save, or {#key}Esc{/key} to close.
```

Keep a sentence with styled words in **one message**. Splitting it into
"Press", "Ctrl+S" and "to save" fixes the English word order, and another
language may need the key at the end or in the middle. With markup, the
translator moves it. A translation that drops markup the source has is an
error (`dropped-markup`, [The command line](command-line.md)).

## Notes for translators

Translators read the files, not the code, so the files carry what they
need:

* a `#` comment above a message says where it appears or what it means;
* `@param $name - …` says what an argument holds, so a translator can
  choose the right grammar;
* `@do-not-translate` marks a message that stays as written: a brand, or
  a language's name in that language. On a section, it covers every
  message in it; at the top of a file, the whole file.

```mf2 file=guide/locales/en/main.mf2
# The product's name: the same in every language.
@do-not-translate
[brand]
name = Photon
```

`mf2 export` passes comments and `@param` notes to a translation tool,
and `mf2 check` holds a translation to `@do-not-translate`. A language
that leaves such a message out shows the source's, as French does here.

```rust file=guide/src/main.rs
        println!("{}", tr!("help.save"));
        println!("{}", tr!("brand.name"));
    }
}

/// Whether a reader is available. Its words are in the messages; the code
/// passes a key.
#[derive(Clone, Copy)]
enum Presence {
    Online,
    Away,
    Offline,
}

impl Presence {
    fn key(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Away => "away",
            Self::Offline => "offline",
        }
    }
}
```

## The French file

Each language writes its own variants. French names `many`; its ordinals
have two forms; and its album is *son album* whatever the owner's gender,
so every variant has `*` for the gender: it matches on the count alone,
but still names both values, as the source does.

```mf2 file=guide/locales/fr/main.mf2
@locale fr
---

welcome = Bienvenue, {$name} !
braces = Écrivez \{ et \} pour une accolade dans un message.

[order]

total =
  .local $sum = {$amount :number minimumFractionDigits=2}
  {{Total : {$sum}}}

discount = {$share :percent} de remise

items =
  .input {$count :integer}
  .match $count
  0    {{Votre panier est vide.}}
  one  {{Un article dans votre panier.}}
  many {{{$count} d’articles dans votre panier.}}
  *    {{{$count} articles dans votre panier.}}

[race]

place =
  .input {$place :integer select=ordinal}
  .match $place
  one {{Vous finissez {$place}er.}}
  *   {{Vous finissez {$place}e.}}

[social]

shared =
  .input {$gender :string}
  .input {$count :integer}
  .match $gender $count
  * one  {{{$name} a ajouté une photo à son album.}}
  * many {{{$name} a ajouté {$count} de photos à son album.}}
  * *    {{{$name} a ajouté {$count} photos à son album.}}

[presence]

status =
  .input {$status :string}
  .match $status
  online {{En ligne}}
  away   {{Absent}}
  *      {{Hors ligne}}

[help]
save = Appuyez sur {#key}Ctrl+S{/key} pour enregistrer, ou sur {#key}Échap{/key} pour fermer.
```

`cargo run` prints both languages.

## Side by side with other formats

`order.items`, from [Plurals](#plurals), in each:

**Fluent** (`.ftl`):

```text
order-items = { $count ->
    [0] Your basket is empty.
    [one] One item in your basket.
   *[other] { $count } items in your basket.
}
```

**ICU MessageFormat 1** (Java, ICU4C, FormatJS):

```text
{count, plural,
  =0 {Your basket is empty.}
  one {One item in your basket.}
  other {# items in your basket.}}
```

**i18next** (JSON, one key per plural form):

```json
{
  "items_zero": "Your basket is empty.",
  "items_one": "One item in your basket.",
  "items_other": "{{count}} items in your basket."
}
```

| | MF2 | Fluent | ICU MessageFormat 1 | i18next |
|---|---|---|---|---|
| Defined by | Unicode (CLDR) | Mozilla's Project Fluent | ICU | the i18next library |
| Several values | one `.match`, a key per value | selectors nested inside variants | `select` and `plural` nested | key suffixes: a context, then the plural form |
| Formatting | functions with options (`:number`) | `NUMBER()`, `DATETIME()` | `{n, number, …}` styles and skeletons | formatters named in the placeholder |
| Markup | part of the syntax, checked | none: text, or HTML the application parses | none (some libraries add tags) | through a component, in React |
| Reuse | none: each message stands alone | terms (`-brand`) and message references | none | nesting (`$t(key)`) |
| Files | `.mf2`, sections and properties | `.ftl` | whatever holds the strings | JSON |

Fluent users will miss terms. MF2 has no references between messages,
so each message is complete and can be translated on its own. A brand
is a `@do-not-translate` message the code shows, or text written in
each message that needs it. `mf2 convert --from fluent` copies each term
into the messages that use it ([The command line](command-line.md),
[Migrating from `leptos-fluent`](migrating-from-leptos-fluent.md)).

## Why messages have ids

Some libraries extract messages from the code: the English text is the
key, and a tool collects every string it finds. This library does the
opposite. The messages are written in files, each under an id, and the
code names the id. That choice buys several things:

* **The source language is a translation like the others.** Fixing a
  typo in the English does not change a key, so no translation is lost.
* **One English word can be two messages.** "Open" the verb and "open"
  the state get two ids, and languages that use two words can.
* **A message is more than a string.** A plural, a gender, markup and
  options live in the message, where the translator can change them,
  instead of in code around it.
* **Mistakes are compile errors.** `tr!` checks that the id exists (a
  misspelt one gets a suggestion) and that the arguments are the
  message's; `mf2 check` finds missing translations and ids nothing uses.
* **The code carries no text.** On the web, the browser downloads a
  language's messages only when it needs them, and the wasm has none of
  them.
* **Translators never read Rust.** They get the files, or an export,
  with every comment and note.
