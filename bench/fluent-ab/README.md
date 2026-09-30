# bench/fluent-ab — the `leptos-fluent` A/B

Phase 8 A5 (`plans/16-phase-8-work-order.md`; `plans/06-size-and-perf.md`
§6; master plan §9 P8): the question a Fluent user asks first — how does
this compare with `leptos-fluent`? — answered **once**, at migration, on the
reference application, and committed as a snapshot: "at this commit, this is
what we had". It is not in CI and is not re-run per commit; a later commit is
compared with the snapshot only when the owner asks, by running the same
command there.

```sh
cargo xtask fluent-ab                          # build, check, measure: target/fluent-ab/SNAPSHOT.md
cargo xtask fluent-ab --snapshot               # the same, and write it here (clean tree only)
cargo xtask fluent-ab --no-build --runs 20     # measure again what target/fluent-ab/ holds
```

| File | What it is |
|---|---|
| [`SNAPSHOT.md`](SNAPSHOT.md), [`snapshot.json`](snapshot.json) | the snapshot: the commit measured, the versions, the date, the machine's load, the command, every figure |
| `mf2/` | the migrated application's hand-finishing, as [the migration guide](../../docs/migrating-from-leptos-fluent.md) describes it: `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/support.rs`. Since Phase 10 B4 it names `mf2` (`leptos-0-8`) and `mf2::leptos` where the guide names the 1.x shim `leptos-mf2`; the guide follows with the web book (D6) |
| `mf2/src/ab.rs`, `fluent/ab.rs` | the timing hooks, one per side, compiled only into the timed build (`ab-bench`); the message ids are filled in from the workload |

Neither application is committed: both are generated, and `leptos-fluent`
never enters this repository's own resolve — each application is a cargo
workspace of its own under `target/fluent-ab/wl/`, as every generated
application is.

## The two applications

* **`leptos-fluent`**: the reference workload (seed 1: 1,600 messages, 1,860
  call sites, `en`, `pl`, `en-XA`, `ar-XB`) generated with the template
  `fluent-view` — each call-site shape in `leptos-fluent`'s own idiom — and
  its messages as `.ftl` (`workload-gen --format ftl`). `leptos-fluent`
  0.3.1, its newest release, requires Leptos < 0.9, so **both sides are on
  Leptos 0.8** (this library's `leptos-0-8` opt-in).
* **mf2**: *that application migrated*, not a twin written by hand — a copy,
  a translation crate, `mf2 convert --from leptos-fluent --write` (whose
  report must be exactly the hand-finishing the guide describes, as in
  `cargo xtask fluent-migrate`), then finished as the guide says: the
  manifest, the entry points and the server from `mf2/`, and the shell's
  `<html lang dir>`, catalog preload and catalog links edited into the
  generated `src/app.rs`. The converted catalogs are the ones Phase 8 A3
  checked against `fluent-bundle`.

**Like with like.** Both render the same text at every site: a sentence with
an inline element, which `leptos-fluent` cannot express as one message, is
the sentence split around the element on both sides (8 messages; the
snapshot counts the sites). Before anything is timed, the browser check
compares the whole hydrated `<main>` of every route in `en` and `pl` on the
two sides, bidi isolation marks aside, so that neither side is measured with
its translations optimized away.

## What is measured

**Size** — the client as it ships, built as `cargo xtask size` builds:
`wasm32-unknown-unknown`, profile `wasm-release`, `wasm-bindgen --target
web`, `wasm-opt -Oz`; raw, `gzip -9` and brotli (quality 11, window 22 —
what `mf2-axum` serves a catalog with). What a first visit downloads in one
locale: the wasm, the JS and that locale's text (on `leptos-fluent` the text
is in the wasm — every locale's). What each added locale costs: on
`leptos-fluent`, the client with all four locales less the client with `en`
alone (a third build, `fluent-view` pointed at a copy of `ftl/en` only), over
three — a cost every visitor pays; on mf2, that locale's catalog, which only
its readers download.

**Speed** — in Chromium and Firefox, the two applications' servers up at
once and run **alternately** (the development machine's clock drifts, so
back-to-back runs of one then the other would compare two clock speeds), the
order swapped every run, each run a fresh first visit:

* the first translated frame — from the wasm's load (and from the
  navigation's start) to the first animation frame after the application
  hydrated. Both pages are fully translated from the server; this is when
  the client's translations are live;
* a format to a `String` — a simple message, a one-argument message, a
  plural select — in µs, 100,000 at a time;
* mounting 2,000 live translated nodes, and a switch `en` → `pl` (and back)
  with them live, until every one of the 2,000 shows the new text.

The timed build is the shipped one plus the `ab-bench` hooks: a
`performance.mark` in an `Effect` after hydration, and four exported
functions (`ab_mount`, `ab_format`, `ab_switch`, `ab_preload`) that the size
build does not contain. The switch and format hooks do the same thing on both
sides through each library's own API: `leptos-fluent`'s `move_tr!` / `tr!`
under its provider and `I18n::language`; mf2's `tr!` and
`mf2::leptos::set_locale`. mf2's switch fetches the catalog, so it is
preloaded, and both sides are warmed by a switch there and back before the
timed one.

`tools/e2e/checks/fluent-ab.mjs` is the browser half; `xtask/src/fluent_ab.rs`
the rest.
