# workload-gen — the reference workload

Deterministic generator of the synthetic reference workload of
[`plans/06-size-and-perf.md`](../../plans/06-size-and-perf.md) §2 (Phase 0
task A7). **Same knobs and seed ⇒ byte-identical output**: the PRNG
(`SplitMix64`, `src/rng.rs`), the length distributions (integer quantile
tables, `src/shape.rs`), the word lists (`src/vocab.rs`) and the JSON writer
(`src/json.rs`) are all in this crate, so no dependency bump can change the
output. Every consumer draws from its own `(seed, label, index)` stream.

## Command line

```sh
cargo xtask gen-workload <COMMAND> [OPTIONS]        # = cargo run --release -p workload-gen -- …
```

| Command | Does |
|---|---|
| `all [-t TEMPLATE]… [--out DIR]` | locales, flat JSON, `sites.json`, one app crate per template (default `-t literal -t closure`; default `--out target/workload`) |
| `locales [--out DIR]` | locales and flat JSON only |
| `corpora [--check] [--out DIR] [--suite DIR]` | writes `bench/corpora/workload-<N>.json` and `bench/corpora/suite.json`; `--check` compares instead and exits 1 if stale |
| `stats [--json FILE]` | prints the shape table; exits 1 if a checked row is outside plans/06 §2's tolerances. `--json` measures an existing flat corpus |
| `canaries [--grep]` | prints the B6 canaries (`--grep`: just the three patterns) |
| `templates [--dump NAME --out DIR]` | lists built-in templates, or copies one as a starting point |

Knobs (every command except `templates`):

| Flag | Meaning | Default |
|---|---|---|
| `--seed` | seed | `1` (the committed corpus) |
| `-n, --messages` | N, messages per locale | 1600 |
| `-m, --sites` | M, call sites | 1860 |
| `-l, --locales` | L, real locales **including** `en` (`pl`, `de`, `fr`, `ar`, `ru`, `ja`, `cy` are added in that order) | 2 |
| `--no-pseudo` | leave out `en-XA` and `ar-XB` | pseudo on |
| `-r, --routes` | R, `#[lazy_route]` routes besides the eager home route (≤ 24) | 3 |
| `-k, --components` | K, components | 60 |
| `--files` | source files per locale (≤ 24) | 18 |
| `--number K` / `--datetime K` | K variable-carrying messages format a variable with `:number` / `:datetime` (options vary); the variable-count shape is unchanged | 0 |

10× scale (`-n 16000 -m 18600`) is covered by a test, for locales and the app.

## Output layout (`all --out DIR`)

```
DIR/.workload-gen              marker (knobs); DIR is refused if non-empty without it
DIR/locales/<tag>/<ns>.mf2     18 files per locale: en, pl, en-XA, ar-XB
DIR/json/<tag>.json            the same messages, flat {id: source}, sorted by id (= MsgId order)
DIR/sites.json                 one object per call site: site, component, route, shape, mode, index, id, args
DIR/app-<template>/            a standalone cargo-leptos crate (own [workspace])
```

Rewriting a directory replaces `locales/`, `json/` and each app's `src/` and
`style/`, and keeps each app's `target/`.

## Locales

* **`en`** (source): English words to an exact byte length per message.
* **`pl`** (and further real locales): synthetic text from a per-locale
  syllable inventory, ≈ 1.2× longer, with the locale's **own plural
  categories** (`pl`: `one few many *`). Same variables and markup as `en`.
* **`en-XA`**: `[` + accented text + padding words (≈ +30 %) + `]`, per pattern.
* **`ar-XB`**: every text run wrapped in U+202E RLO … U+202C PDF.

All files follow the working grammar of plans/05-tooling.md §2: comments and
`@locale <tag>` before `---`; `[section]` heads giving dotted ids (the first
block of `common.mf2` has no head, so its ids are bare); `# comments` directly
above the section head or entry they attach to; one `@param $name - …`
property per variable, between an entry's comment and the entry. Comments are
sized to **60 % of the `en` bytes** (other locales carry the same comments, so
their share is lower). Values use two continuation forms:

* a `.match` message is written as `key =` followed by indented lines; each line
  break is one LF. **Interpretation:** an empty value on the `key =` line
  contributes nothing — the value starts with the first continuation line, with
  no leading LF (MF2 allows leading whitespace in complex messages, so either
  reading gives the same message; the JSON has no leading LF);
* a single-line value longer than 100 bytes is wrapped at spaces outside
  placeholders with an **escaped line break** (`… word \` + LF + indentation,
  both removed), keeping the space before the backslash.

No other resource escapes are produced; MF2 text never contains `{ } \`.

## Shape (plans/06 §2) and how it is measured

`stats` measures the generated sources, not the plan. Definitions:

* **text length** = UTF-8 bytes of the MF2 source value (what the flat JSON
  holds), placeholders and `.match` syntax included;
* **id length** = characters of the **full dotted id** (`chat.input.send`),
  i.e. exactly what a call site writes;
* **simple** = no variable. Inline markup (`{#kbd}Esc{/kbd}`, 0.5 %) sits in this
  bucket, as the old stack counted its private-use stand-ins as text;
* **`.match`** = 0.9 %, plural on `$count` (declared `.input {$count :integer}`),
  inside the 1-variable bucket;
* variable names are snake-case (valid Rust identifiers, so call sites can name
  them).

Quotas are exact (largest-remainder apportionment), lengths are quantiles of a
piecewise-linear distribution (mean 27, median 19, p90 58, max 259), ids are
built to lengths with mean 23.5.

**Placeholder-free subset** (the parser gate's third corpus row): the messages
of `workload-1600.json` whose source contains no `{` and does not start with
`.` — i.e. no variable, no markup, not complex (1,256 of 1,600).

## Canaries (B6)

| | value |
|---|---|
| message id | `app.canary.zq7-canary-msg` |
| variable | `zq7_canary_var` |
| text | `ZQ7-CANARY-TEXT-<TAG>` — `ZQ7-CANARY-TEXT-EN`, `-PL`, `-EN-XA`, `-AR-XB` (never pseudo-localised) |

Grep the final client wasm for `app.canary.zq7-canary-msg`, `zq7_canary_var` and
`ZQ7-CANARY-TEXT` (`workload-gen canaries --grep` prints them); any hit fails.
Site 0 of every app is a text child referencing the canary with its argument.
By design the `closure` control ships ids and names (hits on the first two) and
the `literal` baseline ships source text (hit on the third): B6 applies to the
implementation under test.

## The app

`app-<template>/`, package `workload-app-<template>` (distinct per template, so
several apps can share one `CARGO_TARGET_DIR` and reuse compiled
dependencies — with equal names cargo would take one app's artifacts for the
other's): Leptos 0.8 SSR + hydrate, Axum server (`src/main.rs`),
`leptos::mount::hydrate_lazy`, R routes via `#[lazy_route] impl LazyRoute`
(`Lazy::<RouteN>::new()`) so their components land in separate chunks under
`cargo leptos build --split`; component k belongs to route `k mod (R+1)`
(route 0 = eager home). `[profile.wasm-release]` (opt-level z, fat LTO, cgu 1,
panic abort, strip) is used for the lib through
`[package.metadata.leptos] lib-profile-release`.

Call sites — shares of M, exact:

| Shape (`[site.<key>]`) | Share | Generated context |
|---|---|---|
| `string` | 45 % | match arms in helper fns, fn returns, `set_error.set(…)` in `on:click` handlers |
| `child` | 20 % | `<p>{…}</p>`, button labels |
| `attr` | 8 % | `<input aria-label={…} placeholder={…}/>` |
| `text_prop` / `signal_prop` | 4 % + 4 % | `<TextLabel text={…}/>` (`TextProp`), `<SignalHint text={…}/>` (`Signal<String>`) |
| `string_prop` | 4 % | `<StringBadge text={…}/>` (`String`) |
| `deferred` | 8 % | `Row { label: … }` in `pub static T<k>: &[Row]` (`src/tables.rs`), rendered by a list |
| `if_else` | 7 % | `{move \|\| if flag.get() { A } else { B }}` — each branch is one site; both branches share a mode |

21 % of sites pass arguments (never `deferred`), always every variable of
their message (1–3; the three 4-variable messages have no site). Modes:
`plain` (values `n: i64`, `who: &'static str`, `when: &'static str`, in scope
everywhere), and for `child`/`attr`/props alternately `signal` (the signal —
`count`, `name`, `stamp` — is the argument) and `get` (an enclosing
`move ||` reads `.get()`). `string`, `string_prop`, `if_else` args are plain.
Child sites whose message has markup use mode `rich`.

Build checks (what A7 verified; see the task report for sizes):

```sh
cd target/workload/app-closure
CARGO_BUILD_JOBS=3 cargo check --features ssr
CARGO_BUILD_JOBS=3 cargo build --lib --no-default-features --features hydrate \
    --target wasm32-unknown-unknown --profile wasm-release
```

## Template format

A template is a directory with `template.toml` and an optional support file;
`-t <dir>` uses it, `-t literal` / `-t closure` the built-ins in
`templates/`. Placeholders are `{{name}}`; `{{` always starts one, and an
unknown name is an error when the template loads (every shape × mode is
rendered once with dummy data).

```toml
name = "tr"                        # app directory becomes app-tr/
description = "…"
dependencies = ['mf2-probe = { path = "{{template_dir}}/crate" }']  # verbatim [dependencies] lines
prelude = "use mf2_probe::{tr, tr_args, Arg};"   # top of every component module and src/tables.rs
support = "support.rs"             # copied to src/support.rs (module `crate::support`)
boot = "crate::support::boot();"   # first statements of `hydrate()`

[args]                             # how one argument renders inside {{args}}
sep = ", "
plain = "Arg::from({{value}})"     # required if any snippet uses {{args}}
get = "…"                          # optional, falls back to plain
signal = "Arg::reactive({{signal}})"  # optional, falls back to get (with .get() values)

[deferred]
type = "mf2_probe::Tr"             # type of the table's label field (in a `static`)
view = "{{label}}"                 # a row's label as a view child
string = "{{label}}.to_string()"   # a row's label as a String

[site.child]                       # one table per shape key; keys: none rich plain signal get
none = "tr({{index}})"
plain = "tr_args({{index}}, [{{args}}])"
get = "move || tr_args({{index}}, [{{args}}])"
# … [site.string] [site.attr] [site.text_prop] [site.signal_prop]
#   ([site.reactive_prop] serves both) [site.string_prop] [site.deferred] [site.if_else]
```

Snippet fallbacks per table: `signal → get → plain → none`, `get → plain →
none`, `plain → none`, `rich → none`. The snippet must be an expression of the
type its context needs: `String` for `string`; anything renderable for
`child` and the `if_else` branches (both branches must have one type); an
attribute value for `attr`; `Into<TextProp>`, `Into<Signal<String>>`,
`Into<String>` for the props; a const-context value of `[deferred] type` for
`deferred`.

| Placeholder | In | Value |
|---|---|---|
| `{{id}}` | site | message id (`chat.input.send`) |
| `{{index}}` | site | `MsgId`: rank of the id in bytewise sorted order |
| `{{text}}` | site | source text escaped for a `"…"` literal (a `.match`'s catch-all variant) |
| `{{args}}` | site | the `[args]` items, joined by `sep`, in slot order |
| `{{nargs}}` `{{site}}` `{{markup}}` | site | argument count, global site number, markup names |
| `{{name}}` `{{slot}}` `{{kind}}` | arg | variable name, positional slot (ascending bytewise name order, plans/05 §3), `num`/`str`/`date` |
| `{{value}}` | arg | `plain`: `n`/`who`/`when`; `get`: `count.get()`/`name.get()`/`stamp.get()`; `signal`: `count`/`name`/`stamp` |
| `{{signal}}` | arg | the signal in scope for the variable's kind |
| `{{label}}` | deferred | the label expression (`row.label`) |
| `{{template_dir}}` | dependencies | absolute path of the template directory |

The built-ins: **`literal`** — every site is the source text as a string
literal and arguments are dropped (the "no i18n" baseline of plans/06 §3);
**`closure`** — `move || lookup("id")` per child/attribute,
`Signal::derive(move || …)` per reactive prop, `|| lookup("id")` entries in a
`fn() -> String` registry, `lookup_args("id", &[("name", v.to_string())])` for
arguments, with one shared `lookup` in the support module (opaque to the
optimiser: `boot()` fills its map from `<html data-catalog>`).
