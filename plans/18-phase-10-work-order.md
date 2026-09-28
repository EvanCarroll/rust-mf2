# 18 — Phase 10 work order: 2.0, the user experience

Part of the [master plan](00-master-plan.md) (§9, P10). RFC 2119 keywords
apply. Written on 2026-09-28, at the owner's request, from four sources:
- a UX review of the library, written 2026-09-27. It is kept outside the tree (`comparison.md`,
  untracked), so its findings are transcribed below;
- a report on why `mf2` always depends on `leptos-mf2`;
- the owner's staged MF2 port of a real Ratatui application (`vendor/trippy`, also untracked);
- the thirteen owner answers below.

Phases 1–9 built the library and released it as 1.0.0. Phase 10 rebuilds
the way people **use** it, for a 2.0.0 release:
- **Ratatui first:** best in class, unless that costs performance or executable size;
- **the web too:** its setup cut down;
- **what failed silently fixed:** the tooling gaps the review found;
- **a book that teaches:** MF2 itself, not only this library.

What comes after (hot reload, editor tooling, per-route catalogs, …) is
kept open by each decision, not built.

## State at the start (commit `7c8d1c0`)

| In the tree or on crates.io | Where |
|---|---|
| All 16 original crates published at **1.0.0** (2026-09-26; 11–37 downloads each on 2026-09-28, likely mirrors). The workspace says `1.1.0`. `mf2-native` and `mf2-ratatui` were never published. 1.1.0 is not published, and won't be (owner question 1) | crates.io API; `Cargo.toml` `[workspace.package]` |
| `docs/versioning.md` "Where the releases stand", `README.md` "Status", `docs/getting-started.md` and the 1.1.0 changelog entry still say that only five crates reached 1.0.0 | those files |
| The call-site types (`Tr`, `TrArgs`, `TrRich`, `TrDyn`, `ArgValue`, `DateTimeValue`, `Text`, the markup traits) are declared in `leptos-mf2` (`tr.rs`, `arg.rs`, `dynamic.rs`, `markup.rs`). They sit there because of the orphan rule, and `mf2` re-exports them | [04](04-leptos-integration.md) §2.1; `crates/mf2/src/lib.rs` |
| `mf2` depends on `leptos-mf2` always (without its default features). Its `ssr`/`hydrate`/`csr` pick no Leptos line; the line comes from an application's own `leptos-mf2` dependency | `crates/mf2/Cargo.toml`, root `Cargo.toml` |
| The Leptos 0.8 line is renamed back to `leptos` at `leptos-mf2`'s and `mf2-axum`'s roots (`extern crate leptos_0_8 as leptos;`). This is so that the paths `view!` and `#[component]` write into the crate reach the active line. Only `components.rs` uses those macros: 14 uses, six components | `crates/leptos-mf2/src/lib.rs:95-104`, `components.rs` |
| `to_string()` / `to_plain_string()` / `From<Tr> for String` exist only under Leptos, and there is no `Display` (B12's rule for the client). Only `Tr` derives `Debug` | `crates/leptos-mf2/src/glue/view.rs:651-699`, `tr.rs:35` |
| `ArgValue` converts from `&str` (copied into `Arc<str>`), `String`, `char`, `i8`…`i64`, `u8`…`u32`, `usize`, `f32`/`f64` and date values. Nothing for `u64`, `i128`, `bool`, `Cow<str>`, `Path`, `SystemTime` or jiff types. A wrong type is an error about `ArgValue`, which the user never wrote | `crates/leptos-mf2/src/arg.rs:230-322`; `crates/mf2-macros/src/expand.rs:219` |
| Native: `mf2-native` holds an app-owned `NativeI18n`, with `set_locale(&mut self, &str)` and `format(&self, &impl Message) -> String`. Every locale's catalog file is required, and an unnamed system zone becomes a frozen offset. `mf2-ratatui`: `line`/`text(&NativeI18n, &impl Message, &MarkupStyles)`; `MarkupStyles` is a `Vec<(String, Style)>`, and each call allocates per span and per markup name | `crates/mf2-native/src/native.rs`, `crates/mf2-ratatui/src/lib.rs` |
| The book's native page: a separate translation crate (17-line `Cargo.toml` with feature forwarding, 8-line `build.rs`, a `mf2.toml` that is all defaults, 1-line `lib.rs`) in a two-member workspace, and `&i18n` passed to every call. Its `main` never reaches the TUI | `docs/native-apps.md` |
| `mf2 init` scaffolds a web translation crate only. On a native build its module would not compile (`host_web`) | `crates/mf2-cli/src/init.rs` |
| The generated module gates `CATALOGS` and the host on the including crate's own `ssr` / `datetime-*` features, which is why each translation crate declares and forwards them. The build reads that crate's `CARGO_FEATURE_*` | `crates/mf2-build/src/codegen.rs`, `features.rs:167-188` |
| `tr!` is a `#[macro_export]` in an `include!`d file. Inside its own crate it can be called only unqualified, before any `mod` item (rustc #52234) | [05](05-tooling.md) §4 |
| Two locale matchers. Both truncate `zh-Hant-TW` to `zh-Hant` and then to `zh`, before `zh-TW` is ever tried. The web's (`lookup_locale`) then allows any locale of the same language; the native one refuses that for 14 multi-script languages | `crates/leptos-mf2/src/state.rs:209-246`, `crates/mf2-native/src/locale.rs:18-80` |
| `Negotiator::default()` is cookie, then `Accept-Language`. The switcher hard-codes the query name `lang` | `crates/mf2-axum/src/negotiate.rs`, `crates/leptos-mf2/src/components.rs` |
| `mf2-axum` requires Leptos (default `leptos-0-9`). Direct Leptos use is confined to `context.rs`; the rest uses `leptos-mf2` items that are gated on `leptos` but whose content is neutral | `crates/mf2-axum/` |
| `api.txt` and the release's semver check see one feature set per crate (docs.rs's, i.e. `ssr`). The client-only API (`set_locale`, `hydrate_body`, …) is neither listed nor checked | `xtask/src/api.rs`, `release.rs` |
| Budgets as of Phase 9: B1 25,875 B gz; B5 8.4 B gz a site; the whole app 41,466 B gz at 1,860 sites; B7 `en` 18,072 B br; MSRV 1.88; ledger at `current_phase = "P9"`, and `Phase` ends at P9 | [phase-9-results](phase-9-results.md); `conformance/src/matrix.rs` |

**The owner's trippy port** (`vendor/trippy`: Apache-2.0, untracked, staged and uncommitted on upstream `12aca14`):
- **Scope:** `crates/trippy-tui` moved from an in-tree `t!` over a TOML catalog to MF2. 162 call sites, 109 keys, 12 locales.
- **API:** it pins an older `mf2-native` API (`from_embedded` with six arguments, `=1.0.1`).
- **The last build failed with 34 errors** (2026-09-27, before `Emit::Native` existed). Still true of today's design:
  - `$crate::tr!` used inside the crate that generates it (rustc #52234);
  - no `ArgValue` from a `Display` type (`KeyBinding`) or from `Cow`;
  - it needed its own `thread_local! RefCell<NativeI18n>` and a `t!` wrapper;
  - every message became an owned `String`, where upstream borrowed constant text.
- **Tests:** it also deleted upstream's 22 locale tests.
- **Upstream trippy's bugs,** each a class MF2 prevents:
  - a key typo (`t!("chart")` against the catalog's `title_chart`);
  - a placeholder typo (`%{plural_flow}` in two locales);
  - an English-only plural (`> 1`);
  - word order assembled with `format!` (one of them renders "AS awaited: <awaited>");
  - key hints bolded by slicing the translated word (`[h]aide`).
- **Per-frame cost upstream:** every `t!` clones the locale `String`.

## Where the work stands (handoff, 2026-09-28)

Kept current so that any task can be picked up cold, from this file and the
commits. The first session (2026-09-28) ran Part A, C3's data half and E1–E3 as
parallel agents; three were stopped by the account's session limit and were
finished by new agents. **Part A is done except A8, and A9, which the owner
added afterwards.** The owner holds everything else: the next tasks start
from fresh sessions, as listed under "Next".

**Done** — records below, in this file:
- **A1** (`8f6569e`, `3a296a9`, `cc2416c`): the release statements; the 1.x baselines; `examples/tui`
  and `cargo xtask tui-gate`; the 1.x binaries in `target/p10-baseline/`; the UX table's 1.x column.
- **A2, A3, A6** (`5a660d1`): `links` + cfg macros adopted; the in-crate `tr!` chosen (a hidden
  exported wrapper re-exported as `tr`, a crate `prelude`); one crate works for the web.
- **A4** (`678bd6a`): the ambient store meets its gate; text borrowed through a hidden runtime
  seam (`probes/p10-ambient/seam.patch`).
- **C3's data half** (`acdbc01`): CLDR's `languageMatching` and `territoryContainment` vendored;
  the evidence for question 11.
- **E1–E3** (`a72d057`, `3f1eed9`, `894c0c4`): `dropped-markup`, an error by default (floor
  `allow`); `@do-not-translate` messages neither missing nor covered; `mf2 import` checks what it
  would write and writes nothing on an error it brings, and JSON import names XLIFF for new ids.
- **A5** (probe branch `p10-a5-display`; the variant kept as `probes/p10-display/display.patch`):
  `Display` on the four descriptions, the always-on inherent `to_string` / `to_plain_string` and
  `Debug` everywhere are **adopted** — no `Display`, `Debug` or `core::fmt` reaches a client
  wasm; B1 26,400 → 26,317 B gz (gzip noise on 79 fewer raw bytes), B5 +0.03 B, B12 clean.
- **A7** (probe branch `p10-a7-names`; `probes/p10-names/`, with the crate changes as
  `helper-crates.patch`): `mf2::leptos` with no root rename, the components in per-line helper
  crates, hydration green on both lines (the six browser checks on 0.9, and on 0.8 through
  copies of the demos); the coherence rules confirmed with their errors. The table's cost: see B1
  below.

**In flight:** nothing (2026-09-28). Part A's probes are all recorded; their branches
(`p10-a4-ambient`, `p10-a5-display`, `p10-a7-names`, `p10-e-silent-failures`) and worktrees are
kept until B1 has taken what it reuses from `p10-a7-names`.

**Next, each from a fresh session** (the owner holds everything until then):
- **A9**, what `Display` / `Debug` cost the browser's wasm (added by the owner, 2026-09-28). A5's
  positive control measured +16.6 KB gz for one `{}` and one `{:?}` together, in the fixture
  client only. A9 splits that figure, measures it in the size workloads and the demos, measures
  the ways to shrink or remove it, and lists the ways a description reaches `Display` with no
  `format!` in the application. It runs before A8 states the `Display` trade-off.
- **A8** (after A9), the design for the owner's review: `plans/19-native-and-terminal.md`, the
  four samples' exact code, the UX targets, the gate table; the web side into 04 and 05. It starts
  by asking owner questions 1 and 2 below, since the matcher's rules go into the design. Part C's
  API work waits for the owner's review of it.
- **B1**, the merge (after A1 and A7, both done; it may start before A8's review). From A7's
  record: reuse the branch's helper crates, line aliases and component wrappers; **the function
  table costs +459 B gz in demo-ssr and +130 B gz in demo-csr** (apps that render the switcher
  on the client; the size workloads render no component, so B1 and `b5 --view` did not see it).
  B1 therefore measures the demos too (`probes/p10-names/measure-demo.mjs`), tries the untested
  static dispatch A7 describes (a trait of the table's entries, implemented by the Leptos layer
  for a zero-sized type, the components generic over it), and keeps the cheaper one. If neither
  holds ±64 B gz in the demos, question 13's fallback applies: back to the owner with its other
  two options.

**Owner questions, to ask when their task starts** (found in the work, not yet asked; A8 asks both first):
1. *C3, before A8 states the matching rules:* CLDR's data refuses Traditional ↔ Simplified
   Chinese (no rule; the default script distance, 50), where question 11's answer expected
   Traditional → Simplified to be served. Follow the data, or add a documented project rule?
2. *C3:* the matching algorithm's normative text (UTS #35 Part 1 §4.4 — the threshold,
   `oneway`, demotion, match-variable groupings) is not in the tree, and the network rule does
   not reach it. Work from the C3 data record and ICU's documented behaviour, or allow vendoring
   that section?

**Found along the way, routed to later tasks** (details in the records):
- C6: a missing `mf2.toml` reruns the build script on every build (A3); `mf2 check` turns a
  failed `cargo metadata` into false `gated-function` errors (A1); `mf2 check` must see the
  function features the builds use (A6); `neutral-numbers` fires on a corpus whose only
  placeholder is a string (A3).
- A8 (a design trade-off to state): with `Display` always on, `format!("{}", tr!(…))` in a
  browser build compiles and pulls in `core::fmt`, ≈ 16.6 KB gz; 1.x refused it at compile time,
  and so would A5's fallback (`Display` only with the std modes) (A5). A9 investigates the figure
  first.
- C1: a `&str` argument from a variable is copied into an `Arc<str>` (A4).
- C2: time the ambient lookup's first step when `ssr` and `native` are unified (A4); the B10
  times need a quiet machine (A1).
- C7/C8: Ratatui without its default features needs `layout-cache` (A1).
- D5: the one-crate web starter writes `watch-additional-files = ["locales"]` (A6).
- Every size gate: an A/B is valid only within one tree and one `Cargo.lock` (A4's record).
- Not scheduled: each `.match` message allocates 4 times inside the runtime (A4);
  `mf2-catalog`'s timing test `linear.rs` failed once under load 10–13 (C3 data).

## The UX review's findings, and where each goes

Transcribed from the review (2026-09-27), since its file is not in the tree.

| # | Finding | Goes to |
|---|---|---|
| 1 | A translation can drop markup (`{#link}…{/link}`) and neither `check` nor XLIFF `import` complains | E1 |
| 2 | `mf2 check` ignored the app's features | done (Phase 9 B8) |
| 3 | Missing translations counted but not named | done (Phase 9 B8) |
| 4 | `@do-not-translate` messages count as missing, which adds a warning per language for a switcher's language names | E2 |
| 5 | `mf2 import` accepts translations that fail the checks (an undeclared `$nom`), and JSON import silently skips new ids | E3 |
| 6 | A page rendered without the request's language falls back to the source language silently; with no catalogs installed, every text renders empty | E4 |
| 7 | `Negotiator::default()` ignores `?lang=`, which the switcher submits without the wasm, and the switcher hard-codes `lang` | D2 |
| 8 | Path-prefix sites and the switcher | done (Phase 9 B4) |
| 9 | Missing argument types; `DateTimeValue::instant` returns `Option`; errors name `ArgValue` | C1 |
| 10 | Markup closures need `\|c: AnyView\|` | D4 |
| 11 | The message types can't be printed (`Display`) or derive `Debug` | A5, C2 |
| 12 | `#[cfg(feature = …)]` pairs in app code to switch language, and to read it reactively | D4 |
| 13 | Language codes are strings | C4, D4 |
| 14 | Unclear which crate an app depends on: five from the family; the facade's docs open with low-level types | B1–B4, D1, F |
| 15 | No server use without Leptos; no per-call language on a server | C2, D1 |
| 16 | About 250 lines to copy before anything runs; no starter templates | C7, D5 |
| 17 | `mf2 init` only scaffolds the web; `build.rs` boilerplate; `mf2 --help`'s summary omits `init` and `convert` | C6, C7 |
| 18 | About 1.8 s wasted per translation edit: unoptimized build scripts (fixed by `[profile.dev.build-override] opt-level = 2`, `plans/phase-5a-results.md`); maximum-quality brotli in debug builds | C6, C7 |
| 19 | Missing chapters: an MF2 guide, reference pages (`mf2.toml`, lints, features), the translator workflow, custom functions, testing, troubleshooting, deployment | F |
| 20 | First impressions: the crate map before Getting started; mdBook's playground on by default; mechanism mixed into tutorials. The landing title and the broken crate-doc sentence are already fixed | F |

## Owner questions (all answered, 2026-09-28)

1. **Release** — **answered: skip 1.1.0 and ship 2.0.0.** The redesign is
   the next release, and 1.1.0's queued fixes ship with it. No crate name
   is registered only to be retired. *As put:*
   > All 16 original crates reached crates.io as 1.0.0 on 26 September, with 11–37 downloads each
   > (probably bots and mirrors). The native and Ratatui crates were never published, so 1.1.0 isn't
   > out. Should we skip 1.1.0 and make the redesign the next release, 2.0.0?
2. **Where the call-site types and integrations live** — **answered: one
   crate, `mf2`, with features.**
   - The types move back to `mf2`.
   - `leptos-mf2`, `mf2-native` and `mf2-ratatui` become feature-gated modules.
   - An application names `mf2` plus `mf2-build`.
   - `tr!` goes straight into Ratatui widgets (`Block::bordered().title(tr!("title"))`, and
     `tr!("quit").bold()` through `Stylize`), and into Leptos views as today.

   *As put:*
   > Only the crate that defines what `tr!` returns may make it go straight into a Leptos view or a
   > Ratatui widget (Rust's orphan rule). That's why those types live in `leptos-mf2` today, and why
   > `mf2-ratatui` can only offer helper calls. Where should they live in 2.0?
3. **Native language state** — **answered: an app-wide current language
   with a per-thread override.**
   - `install()` once;
   - `set_locale(Locale::Fr)` switches the app, and the next frame draws in French;
   - `with_locale(Locale::Fr, || …)` pins one thread, so tests run in parallel;
   - `Locale::Fr.format(&tr!(…))` formats with no global at all;
   - `println!("{}", tr!(…))` works;
   - the cost is about one atomic read per message.

   *As put:*
   > A native app today creates an i18n handle and passes it to everything that makes text. The
   > trippy port had to wrap it in its own thread-local and `t!` macro. Web apps never pass one,
   > because the page or request supplies the language. Should native apps get an app-wide current
   > language?
4. **Web scope** — **answered: native and web together**, native and
   Ratatui first. *As put:*
   > Besides Ratatui and CLI apps, should 2.0 also cut the web setup's boilerplate? Candidates: no
   > feature forwarding into the translation crate, a generated setup function, `?lang=` → cookie →
   > browser as the default negotiation, typed language values, a prelude, and switching language
   > without `#[cfg]` blocks.
5. **Ratatui styles** — **answered: one app-wide theme.**
   - It is set once.
   - Common markup names (`b`, `i`, `u`, `em`, `strong` …) are styled by default.
   - The build generates a constant for every markup name the corpus uses, so a typo is a compile
     error.
   - A draw can switch themes with a scoped `with_theme`.
   - There are no per-call style arguments.
   - Messages say what a stretch is (a key, a host), and the theme says how it looks.

   *As put:*
   > In a Ratatui app, markup such as `{#key}q{/key}` or `{#ok}Connected{/ok}` has to become a style.
   > Where should an app say what each markup name looks like?
6. **Axum** — **answered: fold `mf2-axum` into `mf2`** as an `axum`
   feature. A Leptos app then names only `mf2` and `mf2-build`, and the same
   feature later serves a plain Axum application. *As put:*
   > With one crate, should the Axum server support also move into `mf2` (an `axum` feature), so a
   > Leptos app names only `mf2` and `mf2-build`?
7. **The trippy port** — **answered: finish it as 2.0's acceptance test and
   record the results.**
   - Its figures and findings go in the plans, as the leptos-fluent audit did
     ([04](04-leptos-integration.md) §11).
   - The port itself stays untracked in `vendor/`.
   - A trippy-shaped sample committed to the repository carries the repeatable gates.

   *As put:*
   > `vendor/trippy` holds your staged MF2 port of trippy's TUI (162 call sites, 12 languages). It
   > stopped at 34 build errors, several of which 2.0 removes. Should 2.0 be proven by finishing that
   > port and measuring it against upstream trippy?
8. **The Leptos module's name, and Leptos 0.8** — **answered: `mf2::leptos`,
   with 0.8 kept.** The owner, verbatim: "mf2::web is a horrible name because we may support other
   web frameworks, but I don't want to drop support of 0.8. Why can't we have a feature gate change
   the behavior of mf2::leptos for 0.8 and 0.9 (the default)?"
   - **Features:** `leptos` is the 0.9 line and the default; `leptos-0-8` is the opt-in. Changing
     the default line stays a major.
   - **The obstacle, as found (question 13 settles it):** Leptos's `view!` and `#[component]` are
     procedural macros that write `::leptos` into the crate using them. The 0.8 line, listed under
     another name, is therefore renamed back at the crate root, and that clashes with a public
     `leptos` module.

   *As put:*
   > In one crate, the Leptos layer can't be a module named `leptos` while Leptos 0.8 stays
   > supported. Supporting 0.8 means binding the name `leptos` to the 0.8 crate inside mf2, and Rust
   > rejects a module and a crate with the same name there. Which do you prefer for 2.0? — Keep 0.8,
   > name it `mf2::web`; or drop 0.8, name it `mf2::leptos`.
9. **Matching, first form** — the owner, verbatim: "I'm not sure how to answer this, if the user
   requests Traditional, and we have Traditional we should obviously serve Traditional, but I think
   if we don't we should fall back to Simplified, the same is true about es-MX vs es. If we have a
   Mexican variant we should serve it if requested, or we should fall back to spanish. If this isn't
   right please push back and give me more context about the problem." Pushed back: for some
   languages the two scripts are not mutually readable (Punjabi's Gurmukhi and Shahmukhi). That led
   to question 11. *As put:*
   > A reader whose Mac asks for Traditional Chinese for Taiwan (zh-Hant-TW) gets Simplified Chinese
   > (zh) today, on the web and natively, even when the app has zh-TW. 2.0 fixes that with one
   > matcher everywhere. The web and native also disagree when an app only has the other script: the
   > web shows it, native shows the source language. Which should 2.0 do everywhere?
10. **The silent failures** — **answered: required for 2.0** (Part E). *As put:*
    > The UX review also found tooling that fails silently. A translation can drop a link's markup
    > unnoticed; do-not-translate messages count as missing; `mf2 import` accepts broken
    > translations; and a page rendered without the request's language gives no warning. None of
    > these needs a major version. When should they be fixed?
11. **Matching** — **answered: follow CLDR's language-matching data.**
    - A requested script the app has is served. The CLDR likely subtags we already vendor fix
      `zh-Hant-TW` → `zh-TW`.
    - A missing script falls back only where CLDR says readers accept it. Expected, and confirmed
      once the file is vendored: Traditional↔Simplified and Serbian Latin↔Cyrillic yes; Punjabi's
      two scripts no.
    - Spanish regions fall back as the owner described.
    - One matcher everywhere (C3).

    *As put:*
    > 2.0 will serve Traditional to a reader who asks for it whenever the app has it. That's a bug
    > today: zh-Hant-TW falls to zh before zh-TW is tried. The fix uses the CLDR likely-subtags data
    > we already vendor. When the requested script is missing, should the fallback follow CLDR's
    > language-matching data, or your rule for every language?
12. **The book** — **answered: the chapters, and starter templates.**
    - The 2.0 book gains an MF2 guide for developers, reference pages, the translator workflow,
      testing and troubleshooting (Part F).
    - `mf2 init` grows into a starter that makes a complete, runnable CLI, TUI or Leptos app,
      compiled by the docs check, so it cannot go stale (C7, D5).

    *As put:*
    > The review's remaining findings concern learning the library. The book is missing chapters: an
    > MF2 syntax guide for developers; reference pages for `mf2.toml`, every lint and the features;
    > the translator workflow; testing and troubleshooting. There are also no starter templates.
    > Should 2.0 include these too?
13. **The six built-in Leptos components** — **answered: one helper crate
    per Leptos line.** Rejected first: rewriting them without the macros. The owner asked why the
    Leptos macros couldn't be used and called the rewrite a bad idea. The explanation given:
    - the macros are the problem only *inside `mf2`*, only because of 0.8. Applications always use
      them, through their own `leptos` dependency;
    - a macro-free rewrite would change nothing for applications. But it would put about 300 lines
      of builder markup in the accessibility- and hydration-critical switcher, and hand-build the
      plumbing `#[component]` generates for `view!`. That plumbing is public but made for the macro,
      so a Leptos release could break it or split it per line.

    **As decided:**
    - The six components move to `mf2-leptos-ui-0-9` and `mf2-leptos-ui-0-8`: the same source, each
      crate depending on its own Leptos under the real name `leptos`, so `view!` and `#[component]`
      are used as normal.
    - `mf2::leptos` re-exports them, so applications still name only `mf2`.
    - The helpers must not depend on `mf2` (Cargo forbids the cycle). `mf2` installs a small table
      of functions at start-up for them: the languages, the current one, switching, preloading, link
      URLs.
    - The rest of `mf2`'s Leptos layer reaches each line through internal aliases, with no root
      rename.
    - A future Leptos line adds one helper.

    *As put:*
    > Leptos's `view!`/`#[component]` can't be used inside mf2 itself while one crate supports both
    > 0.8 and 0.9 under the module `mf2::leptos`. Which way should the six built-in components (the
    > switcher, its options, preload and alternate links, the islands gate) go? — Helper crate per
    > line; generated into the app; without the macros, inside mf2.

**Decided without asking, and the owner may overturn any of them:**
- **`NativeI18n` stays** as the explicit, no-globals `mf2::native::Catalogs`. The ambient store is
  built on it; it keeps servers, tools and several message sets possible later.
- **A native-only build that formats with nothing installed panics**, and the message names
  `install()`. A build with a web mode keeps the web's rule: empty text, never a panic
  ([04](04-leptos-integration.md) §5), plus E4's warnings.
- **Two questions wait for release time (G2):** final 2.0.0 stub releases of `leptos-mf2` and
  `mf2-axum` whose `compile_error!` points to `mf2`; and whether to reserve the never-published
  `mf2-native` / `mf2-ratatui` names.
- **`vendor/` and `comparison.md` stay untracked;** every commit stages files by name.

## What 2.0 looks like

A native Ratatui application, in one crate:

```toml
[dependencies]
mf2 = { version = "2", features = ["ratatui"] }      # ratatui implies native
ratatui = "0.30"

[build-dependencies]
mf2-build = "2"
```

```rust
// build.rs
fn main() { mf2_build::run() }                        // what to emit, from mf2's features

// src/main.rs
mf2::include_generated!();                             // tr!, Locale, markup::*, install(), prelude
mod ui;

fn main() -> Result<(), mf2::native::Error> {
    let args = Args::parse();                          // lang: Option<Locale>, parsed by FromStr
    install()?;                                        // embedded catalogs + the system's language
    mf2::ratatui::set_theme(Theme::default().style(markup::KEY, Style::new().bold().yellow()));
    if let Some(l) = args.lang { set_locale(l) }
    println!("{}", tr!("welcome"));
    ratatui::run(ui::run)
}

// src/ui.rs — no handle and no styles passed around
f.render_widget(
    Paragraph::new(tr!("status", host = h, sent = n)).block(Block::bordered().title(tr!("title"))),
    area,
);
let header = Row::new([tr!("col.host"), tr!("col.loss")]);
let hint = tr!("help");                                // `{#key}h{/key}elp`: the theme styles the key
```

**The shape** (A8 writes the exact code of every sample for the owner's review):

- **Crates: 18 become 16.**
  - `leptos-mf2`, `mf2-native`, `mf2-ratatui` and `mf2-axum` fold into `mf2`.
  - Two supporting crates carry the Leptos components, one per line (question 13).
  - Applications name `mf2`, and `mf2-build` in their build script; `mf2-cli` is the tool.
- **`mf2`'s features:**
  - `leptos` (the 0.9 line) or `leptos-0-8`;
  - exactly one of `ssr`, `hydrate`, `csr`;
  - `static-locale`, `mark-fallback-lang`;
  - `axum`;
  - `native`, and `ratatui` (which implies `native`);
  - the function features, `intl` and `compile`.
- **The call-site types** are defined once, with only additive impls behind features. That is the
  only arrangement that survives cargo's feature unification (`plans/phase-6-results.md`, "A hazard
  found the hard way"). The impls:
  - the Leptos glue;
  - `From` into Ratatui's `Span` / `Line` / `Text`, `Widget`, and `Styled` (so `Stylize` works);
  - `Display`, beside the fmt-free inherent `to_string()` the web client keeps;
  - `Debug` on every type;
  - argument conversions with a readable error.
- **The ambient store** (native):
  - an atomic active-locale index plus a thread-local override;
  - `&'static` catalogs (embedded, or files leaked once), so constant text is borrowed, not copied.
- **One ambient lookup,** used by `Display` and every conversion: the request context (ssr) or the
  client's catalog (hydrate/csr), then the native thread override, then the native global.
- **The generated module:**
  - `enum Locale` (`ALL`, `SOURCE`, `tag`, `dir`, `FromStr` through the one matcher, `Display`,
    `format`);
  - `install()` and friends; `markup::*`;
  - the web `setup()`, and a prelude.
- **The build:**
  - `links` metadata carries `mf2`'s features to the build script, so no translation crate declares
    or forwards features;
  - `mf2_build::run()` is the whole build script;
  - native applications default to one crate.
  - All three are gated by probes (A2, A3).

## Method and gates

1. **Write the applications first.** A8 writes the exact code of four
   samples, and the owner reviews it before any API work (Part C):
   - a one-file CLI (`--lang`, a plural, an error message, `println!`);
   - a trippy-shaped TUI: bordered blocks, table headers, a key-hint bar with styled keys, a status
     line with a plural and numbers, a language menu, a live switch;
   - a two-crate workspace (a library and a TUI sharing messages);
   - the Leptos `hello` application.

   They become compiled book pages as the work lands (`cargo xtask docs`).
2. **Count what users pay.** A1 fills the UX table below for 1.x, by
   written rules:
   - **setup lines:** lines that exist only for translation, plus lines naming our crates or
     forwarding features to them;
   - **crates named:** from the family, plus the application's own translation crate;
   - **concepts:** distinct API names used before the first translated output;
   - **commands:** from an empty directory to the first translated output.

   A8 sets 2.0's targets. **Every row must fall** (C8, D6).

   | Sample | 1.x (A1, by the rules; the detail is in A1's record) | 2.0 target (A8) |
   |---|---|---|
   | one-file CLI | **35 setup lines** (a translation crate of 24, a two-member workspace); **3 + 1 crates** (`mf2`, `mf2-build`, `mf2-native`; the translation crate); **18 concepts**; **1 command**, the translation crate written by hand | — |
   | trippy-shaped TUI | **48 setup lines**: as the CLI, plus `mf2-ratatui` and a `MarkupStyles` map built for each draw; the handle in all 118 calls (48 of them `line(i18n, &tr!(…), styles)`); **4 + 1 crates**; **22 concepts**; **1 command**. The real port added **76** (a 59-line `locale.rs`) | — |
   | two-crate workspace | **48 setup lines** (a third, shared translation crate; the handle as a parameter in the library); **4 + 1 crates**; **22 concepts**; **1 command** | — |
   | Leptos `hello` | **96 setup lines**: 53 in the translation crate `mf2 init` writes (a 9-feature `Cargo.toml`, a hand-shaped `setup()`), 43 in the application (20 of them server wiring), and 3 lines changed to `_with_context` forms; **4 + 1 crates** (`mf2`, `mf2-build`, `leptos-mf2`, `mf2-axum`; the translation crate) and the `mf2` tool; **28 concepts**; **5 commands** | — |

3. **Measure against what exists** (D1's rule: a baseline, a gate, a
   fallback).

   | Change | Baseline (A1) | Gate | Fallback |
   |---|---|---|---|
   | The merge (B1–B4, D1) | B1, B5, whole app, B7, B12 | B1 within ±64 B gz, B5 within ±0.2 B a site, B7 catalogs byte-identical, B12 clean | revert the offending impl (e.g. `Display` only with the std modes) |
   | Helper crates and the function table (B1) | A7 | the above, **and the demos' shipped wasm (demo-ssr, demo-csr, demo-islands) within ±64 B gz** — the size workloads render no component, and A7 measured the table at +459 / +130 / −8 B gz there; e2e green on both Leptos lines | static dispatch in place of the table (A7's record); then back to the owner with the other two options from question 13 |
   | The ambient store (C2) | 1.x `NativeI18n`; the port's `RefCell` | time per frame ≤ 1.x (alternating binaries); stripped size ≤ 1.x | the explicit `Catalogs` path; drop the thread override |
   | Ratatui conversions (C5) | 1.x `mf2-ratatui`; an **in-house re-implementation** of upstream trippy's `t!` (a TOML map, the locale `String` cloned per call, `%{x}` replace; not copied code) | allocations per frame ≤ both; time ≤ 1.x | a reusable-buffer API |
   | The matcher (C3) | both matchers' current tests | every current case still passes, except the changes decided in question 11 (each listed); the client table measured against B1 | the client uses the server's choice |
   | `links` (C6) | today's forwarding | every A2 scenario; the wasm byte-identical across a translation edit (P0.9's scenario) | function features stay on the translation crate |
   | In-crate `tr!` (C6) | textual scope | A3 passes under cargo and rust-analyzer | textual scope, documented, with a clear error |
   | Server layer (D3) | the `_with_context` e2e results | e2e green without the context | keep the context wiring |

4. **Cold start** (G3). A fresh agent with only the book and `mf2 init` builds the CLI, the TUI and
   the Leptos application. Every stumble is fixed, and the run repeated until clean.
5. **The trippy port** (C9) is the real-world acceptance test.
6. **Keep later work open.** Each design task states what it does to:
   - hot reload: catalogs replaceable in the ambient store, the old ones leaked in development;
   - editor tooling: the manifest stays discoverable;
   - servers without Leptos: `Locale::format`, the `axum` feature;
   - per-route catalogs: the format is untouched;
   - custom functions;
   - several message sets in one process: the store keyed by corpus later.

## Part A — the plan, baselines and probes (A0 first, then A1; A2–A7 in any order; A9 after A5; A8 after A2–A7 and A9)

Probe crates live under `probes/p10-*`, which the workspace already
excludes. They are deleted at the exit; each probe's result stays in its
task record.

| Task | Deliverable | Done when |
|---|---|---|
| **A0** The plan in the tree | This work order; the master plan (D16–D24, P10, "Later", the doc table and §11); "superseded for 2.0" notes in 04 and 05; `plans/README.md`; `CLAUDE.md` "Start here" | committed (2026-09-28) |
| **A1** Stale release statements; baselines | **First commit:** `docs/versioning.md`, `README.md`, `docs/getting-started.md` and the 1.1.0 changelog intro say what is true: 16 crates at 1.0.0, 1.1.0 not published, 2.0.0 next. **Then:** web `size`, `catalog-size`, `bench/b12/check.sh`, `b12-generated`, B10. A new standalone `examples/tui` (excluded like the demos) **in its 1.x form**: the trippy-shaped frame, about 110 messages, drawn into a ratatui-core `Buffer`, plus `upstream.rs`, the in-house baseline renderer. `cargo xtask tui-gate` (`xtask/src/tui_gate.rs`): allocations per frame (counting allocator), median ns per frame over 31 runs, and stripped release sizes. The 1.x binaries kept under `target/p10-baseline/` for A/B alternation. The native book project's stripped sizes (CLI, and `--features tui`). The UX table at 1.x | two runs agree exactly on allocation counts; every figure recorded with its command |
| **A2** Probe: `links` metadata | A stand-in `mf2` with `links = "mf2-v2"` whose build script prints `cargo::metadata=features=…`. Each row is recorded pass or fail, with the output observed: <br>• a single-crate application, and a two-crate one whose translation crate sees what the *application* turned on; <br>• cargo-leptos: two builds, two values, and a second `cargo leptos build` that does nothing; <br>• rust-analyzer; <br>• a feature toggle reruns the dependent's script; <br>• `cargo metadata` carries the features (for `mf2 check`); <br>• the error a duplicate `links` gives; <br>• `cargo package` / `publish --dry-run`. <br>Also **cfg-forwarding macros** (`#[cfg(feature = "ssr")] #[macro_export] macro_rules! __if_ssr { … }` and an empty twin), so the generated module's compile-time choices live in `mf2` | D19 adopted as "`links` + cfg macros", or the fallback recorded |
| **A3** Probe: `tr!` inside its own crate | Five variants, each recorded: <br>1. today's form (the exact error and lint); <br>2. the future-incompatibility `allow` (recorded, not adopted); <br>3. a generated non-exported macro plus `pub(crate) use … as tr`, used from modules before and after the include and through a crate prelude; <br>4. an env-driven proc macro (`cargo::rustc-env=MF2_MANIFEST=…`): invalidation, a relocated target directory, rust-analyzer; <br>5. a generated `tr` against a glob-imported prelude `tr` | the in-crate mechanism chosen, and the rule for a translation crate and its consumers written for 05 §4 |
| **A4** Probe: the ambient store's cost | Four variants against the path crates: <br>(a) `NativeI18n::format`; <br>(b) the design (`OnceLock` store, an atomic index, a thread-local override, settings behind a lock with a generation counter and a per-thread copy); <br>(c) (b) reading the lock every time; <br>(d) the port's `RefCell`. <br>Time and allocations measured for a simple message, a one-argument message, markup to `Line`, and a 110-message frame, plus the stripped CLI size of (a) against (b). **Zero copy:** a simple message's `&'static str` from a `&'static Catalog`, and pattern text parts recovered as `'static` by a catalog-range check (safe code) through a hidden seam, measured against B1 because the parts sink is on the client's rich path. Parallel `with_locale` tests | D17's figures against the gates; the text-borrowing method chosen |
| **A5** Probe: `Display` / `Debug` against B12 | On a branch: `Display` for the four descriptions; an always-on inherent `to_string` / `to_plain_string`; `Debug` everywhere (hand-written for `Custom` / `Source`). Then `cargo xtask size`, `b5 --view`, `b12-generated`, `bench/b12/check.sh`, and a twiggy look for `Display` / `Debug` symbols. `clippy::inherent_to_string_shadow_display` needs an `allow` with B12 as the reason | adopted if the bytes hold (B1 ±64 B gz, B5 ±0.2 B); otherwise `Display` only with the std modes (an added impl, which survives unification) |
| **A6** Probe: a single-crate web application | Getting started's `hello` with `build.rs` and `locales/` in the application crate, checked for: <br>• both builds, and `watch` (is `watch-additional-files` still needed?); <br>• a translation-only edit leaves the wasm byte-identical (as `cargo xtask scenarios` does); <br>• `--split`; <br>• rust-analyzer; <br>• A3's in-crate `tr!` | one crate for the web too (then the web starter offers it), or why not recorded |
| **A7** Probe: names and coherence | 1. `mf2::leptos` with no root rename. The components come from per-line helper crates through a function table: on both lines, under SSR, hydrate and islands; B1 measured for the table's indirection. <br>2. No Leptos procedural macro outside the six components. <br>3. `pub mod axum` against `use axum::…` inside `mf2`. <br>4. With `leptos` and `ratatui` both on, every intended impl compiles, and the rules are recorded: <br>• never `From<Tr> for Cow<str>` (it collides through `Span`'s blanket impl); <br>• no `FromIterator<Tr> for Line`; <br>• `Cell` and `ListItem` come through their blankets over `Into<Text>` | the module and crate names confirmed; the coherence rules for 19 |
| **A8** The design, for the owner's review | A new companion, `plans/19-native-and-terminal.md`: the store and the lookup order, the Ratatui conversions and theme, the generated module, the build, the in-crate `tr!`, the argument conversion, and the per-mode API. **The exact target code of the four samples**, the UX targets and the gate table. The web side's design goes to 04 and 05 | the owner has reviewed it. This gates Part C's API work; Part B can start before |
| **A9** Probe: what `Display` / `Debug` cost the browser's wasm (added by the owner, 2026-09-28) | **What A5 left open.** In a client that formats no description, A5 found none of our `Display` / `Debug` code and no new `core::fmt`. Its positive control, one `format!("{save} {items:?}")` in the fixture client (a `Tr` through `{}`, a `TrArgs` through `{:?}`), cost **+16,604 B gz** after `wasm-opt`: both at once, and in the fixture only. The `tr` workload already holds 72 items matching B12's fmt pattern (10,354 B with names kept, from `core`, `alloc`, `std` and Leptos's stack), so what an application pays is not known. On A5's variant (the branch `p10-a5-display`, or `probes/p10-display/display.patch` at `2fb7f54`), in one tree and one lock, with A5's scripts: <br>• **the split:** nothing formatted (the base), `{}` alone, `{:?}` alone, both, and the inherent `to_string()` (the fmt-free control), over `Tr`, `TrArgs`, `TrRich` and `TrDyn`; <br>• **in applications:** the fixture client, `tr` and `tr-view` at 1,860 sites, and the three demos (`probes/p10-names/measure-demo.mjs`); <br>• **what the bytes are:** twiggy's dominators, `core::fmt`'s own code apart from our impls; <br>• **the ways to shrink it,** each measured: `Display` through `write_str` instead of `Formatter::pad` (then `{:<12}` no longer pads), and whatever the dominators point to (e.g. `Debug` written through `write_str`, without `core::fmt`'s number and string-escape code); <br>• **the ways to remove it:** A5's fallback (`Display` only with the std modes) and a target `cfg` (no `Display` on `wasm32-unknown-unknown`, whatever the features). Each brings back 1.x's compile error for `{}` in a browser build, whose text is recorded. The same choice for `Debug`, which 1.x had on `Tr` only; <br>• **the silent paths:** where a description reaches `Display` with no `format!` in the application. Generic code bounded on `ToString` or `Display` takes the blanket `ToString`, not the inherent `to_string()`. Searched in Leptos 0.8 and 0.9 and in our own crates; each path found is shown compiling, or ruled out; <br>• **a check:** whether CI can catch our types' `Display` / `Debug` in the demos' client builds (the shipped wasm has no names; e.g. A5's `twiggy.sh` over a build that keeps them) | every figure recorded here with its command; the silent paths listed; a recommendation for A8: A5's design with a rule in the book (and the check), a way to shrink it, or a way to remove it. If it changes what A5 adopted, A8 puts it to the owner |

## A1 — stale statements and 1.x baselines: what was built

* **The release statements** (commit `8f6569e`). `docs/versioning.md`
  ("Where the releases stand", the intro's example versions, the Leptos
  table), `README.md` ("Status", the install note),
  `docs/getting-started.md` (the install note, the version callout) and the
  1.1.0 changelog intro now say what is true: all 16 original crates are on
  crates.io at 1.0.0 (2026-09-26); 1.1.0 was not published and will not be,
  and its items ship in 2.0.0, the next release; `mf2-native` and
  `mf2-ratatui` were never published and are named by path until then. The
  `## 1.1.0` heading stays: the workspace is versioned 1.1.0 until G1, and
  `xtask`'s changelog test holds the tree to that entry. Full
  `cargo xtask docs` and `cargo xtask ci` green.
* **`examples/tui`** (commit `3a296a9`), excluded from the root workspace
  like the demos: a workspace of its own with its translation crate. One
  trippy-shaped frame — a header with a key-hint bar, a table of 12 hops
  under 14 column headers, the selected hop's details, chart titles,
  settings (7 tabs, 7 values), help, a language menu, flows, an event log
  and a status line — drawn into a Ratatui `Buffer` at 160 × 47 cells,
  with no terminal. **118 messages**
  in `en`, `de`, `es` and `fr`, **126 formatted per frame**. They use
  plurals (two of them on two selectors, with `many` variants where Spanish
  and French have the category), `:number`, `:integer` and `:percent` with
  `fn-number`, and markup (`key`, `host`, `ok`, `warn`, `alert`). The French
  failures message selects on another value than the English (the verb
  agrees with the failed probes); the German frozen status moves the styled
  word to the end; the language names are `@do-not-translate`.
  `mf2 check --features fn-number`: nothing to report (without
  `--features`, see the second finding below); the files are in `mf2 fmt`'s
  form. The same frame is drawn twice:
  * `src/ui.rs` — MF2 on **the 1.x API, as the user guide's native page
    has an application written**: the translation crate beside the
    application, a `NativeI18n` passed to all 10 draw functions,
    `MarkupStyles` built for each draw and passed to the 6 that draw
    markup; 48 calls `mf2_ratatui::line(i18n, &tr!(…), styles)` and 70
    `i18n.format(&tr!(…))`.
  * `src/upstream.rs` — **the baseline**: upstream trippy's approach,
    re-implemented from this work order's description, not from its code.
    A TOML table per message (122 keys) parsed once into a `HashMap`; the
    locale a thread-local `String`, cloned by every lookup; `%{name}`
    replaced one `str::replace` at a time; English plurals (`n > 1`); word
    order assembled with `format!` and spans; key hints bolded by slicing
    the translated word. Its bugs show in the frame, e.g. in French
    `[h]aide`, `Cible:` and "2411 sur 14448 (16.7%) sondes ont échoué".
  * `tests/frame.rs`: both renderers in every language; MF2's grouping,
    percent signs, plurals and the French agreement checked; no MF2
    fallback (`{…}`) in any frame. `cargo run -- --lang fr [--upstream]`
    prints a frame.
  * **Found while building it:** with Ratatui's default features off (no
    terminal backend), its layout cache is off too. Every frame then
    re-solved every layout: about 6,700 allocations and 1.2 MB a frame for
    either renderer, with counts that moved between runs (the solver's hash
    maps). The example turns on `std`, `layout-cache` and
    `underline-color` — Ratatui's defaults without the backend — and the
    counts became exact.
  * **Found while checking it:** `mf2 -C examples/tui/i18n check` reads the
    crate's features with `cargo metadata --offline`, which fails here —
    the lock holds Ratatui's optional backend, and its Windows-only
    `crossterm_winapi` has never been downloaded on this machine. The
    command then says it checks "with no function features" and reports 12
    `gated-function` errors that a build does not have (every `:percent`).
    With `--features fn-number` it passes. C6 rewrites how `check` reads
    the features; it should not turn a metadata failure into errors.
* **`cargo xtask tui-gate`** (`xtask/src/tui_gate.rs`). It builds the
  example's `tui-mf2` and `tui-upstream` in release, stripped
  (`CARGO_PROFILE_RELEASE_STRIP=symbols`), and runs them alternately,
  31 runs each by default. Each run draws two warm-up frames per language,
  counts one frame's allocations and bytes per language (a counting global
  allocator in each binary), and times 50 frames per language, switching
  language between them. It reports allocations and bytes per frame per
  language, the median of the runs' mean frame time with its range, and the
  stripped sizes, to standard output and to `target/tui-gate/report.{md,json}`.
  **A run whose counts differ from an earlier run's fails the command**
  (unit-tested, with the negative control). `--save-baseline DIR` keeps the
  binaries, with the commit in `BUILT-AT`; `--baseline DIR` puts kept
  binaries back into the rotation, so that C2 and C8 compare 2.0 with 1.x by
  alternating them; `--book` adds the user guide's native project's
  stripped sizes. No gate yet: C8 adds it.
* **The 1.x binaries are kept** in `target/p10-baseline/` (`tui-mf2`,
  `tui-upstream`, and `BUILT-AT`: built at `3a296a9`, a clean tree).
* **Not in CI yet:** nothing in `cargo xtask ci` builds `examples/tui`;
  `tui-gate` does. It stays on the `mf2-native` / `mf2-ratatui` shims until
  C8 rewrites it.

### Figures at 1.x (the crates as at `2fb7f54`; measured 2026-09-28)

| What | Figure | Command |
|---|---|---|
| B1, fixed | **26,676 B gz** (limit 30,720); the `dummy` bound 27,659 | `cargo xtask size` |
| B5, per call site | **8.2 B gz** (limit 40); the `dummy` bound 25.1 | same |
| the whole app at 1,860 sites | **41,889 B gz** (the ambition 105,120) | same |
| B7, `en` | **18,072 B br** (limit 23,296); `pl` 24,137, `en-XA` 21,537, `ar-XB` 18,423 — every locale passes | `cargo xtask catalog-size` |
| B12 | clean: no panic path, no `core::fmt` in the reader, the runtime, the numeric and the date functions; B13 shown | `bash bench/b12/check.sh` |
| B1's runtime part | 18,888 B gz (the reader 6,781; the core numbers 5,407; B2 2,022; B3 5,486) | same |
| B1′ and B13 on the generated module | +0 B; 13,573 B avoided | `cargo xtask b12-generated` |
| B10, `en` allocations | 0 (simple), 0 (1-argument, reused `String`), 1.018 (new `String`), 4.000 and 1,024 B (select) | `cargo run --release -p runtime-bench -- b10 --gate --md target/p10-a1/B10-P10-1x.md --json target/p10-a1/b10-p10-1x.json` |
| B10, `en` time, **under load** (12.0–12.3, CPU at 1,500 MHz) | 142.7 ns simple, 802.5 ns 1-argument, 2,587.8 ns select: the gate **fails** | same |
| `tui-mf2`, allocations per frame (`en` / `de` / `es` / `fr`) | **1,816 / 1,815 / 1,816 / 1,817**; bytes 171,425 / 176,838 / 175,402 / 178,333 | `cargo xtask tui-gate --save-baseline target/p10-baseline --book` |
| `tui-upstream`, allocations per frame | **1,517 / 1,519 / 1,518 / 1,526**; bytes 135,767 / 143,799 / 139,764 / 141,463 | same |
| `tui-mf2` / `tui-upstream`, median time per frame, **under load** (14.4–16.9) | 2,484.7 / 2,136.1 µs; the second run 1,908.1 / 1,721.5; the third 1,864.5 / 1,803.9 | same, then `cargo xtask tui-gate`, then `cargo xtask tui-gate --save-baseline target/p10-baseline` at `3a296a9` |
| `tui-mf2` / `tui-upstream`, stripped | **1,965,320 B** / **1,390,784 B** | same |
| the user guide's native project, stripped | `native-demo` 1,620,208 B; with `--features tui` 1,620,160 B | same (`--book`, after `cargo xtask docs`) |

* **Three runs agree exactly** on every allocation and byte count (31 runs
  of each binary in each), and a fourth, alternating with the kept
  binaries, agrees with them (`cargo xtask tui-gate --baseline
  target/p10-baseline --runs 5`).
* **B1 against Phase 9's record.** Phase 9 recorded 25,875 / 8.4 / 41,466
  (A6, 2026-09-25, before its Part B); today's tree gives 26,676 / 8.2 /
  41,889. Phase 10's gates (B1 within ±64 B gz, B5 within ±0.2 B) are
  against today's figures.
* **B10's time is not a baseline.** Every `en` time in the report is
  3.9–5.9 × Phase 3's committed `bench/runtime-bench/B10-P3.md` (load 0.84,
  1,823 MHz), including the load-time function table lookup, whose code
  has not changed (3.2 → 15.9 ns). The allocation counts equal Phase 3's;
  select's bytes per call are 1,024 against Phase 3's 928. C2's B10
  criterion needs the run repeated on a quiet machine; the reports stay
  under `target/p10-a1/`.
* **The user guide's native project** is the same size with and without
  its `tui` feature: its `main` never calls the TUI, so the linker drops it.

### The UX table's 1.x column, by the rules

Counted by method §2's rules, read this way so that C8 and D6 count 2.0
alike:

* **setup lines** — non-blank lines that are not comments, and that exist
  only for translation, or name one of our crates, or forward a feature to
  one; a line the application has anyway, in another form (Leptos's
  `_with_context` calls), is listed as *changed*, not counted;
* **crates named** — the family's crates in the manifests, plus the
  application's own translation crate; the `mf2` tool apart;
* **concepts** — distinct names of our API, generated items, features and
  `mf2.toml` keys the application's author writes before the first
  translated output; generated code the author does not write is counted
  apart;
* **commands** — what the book's page has the reader run, from an empty
  directory to the first translated output, installs included; files
  written by hand are listed beside them.

The native samples are written the way `docs/native-apps.md` has an
application written (the translation crate by hand: that page has no
`mf2 init`); the TUI is `examples/tui`'s MF2 side; the Leptos sample is
`docs/getting-started.md`'s `hello`.

| Sample | Setup lines | Crates named | Concepts | Commands |
|---|---|---|---|---|
| one-file CLI | **35**: the translation crate 24 (`Cargo.toml` 12, `build.rs` 8, `mf2.toml` 3, `lib.rs` 1); the application's `Cargo.toml` 5 (a `[workspace]` of 3 lines, `mf2-native`, the translation crate); `main.rs` 6 (the `NativeI18n` and its `set_locale`, 4; `--lang`, 2) | **3 + 1**: `mf2`, `mf2-build`, `mf2-native`; the translation crate | **18**: `Build`, `Build::new`, `emit`, `Emit::Native`, `emit_cargo`, `run`, `into_result`; features `host-std`, `fn-number`; `source_locale`, `missing`; `include_generated!`; `NativeI18n`, `embedded`, `CORPUS`, `set_locale`, `format`, `tr!` | **1** (`cargo run`); the translation crate's 4 files by hand |
| trippy-shaped TUI | **48**: as the CLI, plus `mf2-ratatui` in `Cargo.toml` (1), three `use` lines, and an 8-line `MarkupStyles` map built for each draw (+1). Every text carries the handle: 118 calls, 48 `line(i18n, &tr!(…), styles)` and 70 `i18n.format(&tr!(…))`; the 10 draw functions take `i18n`, and 6 of them `styles` too | **4 + 1**: the CLI's, and `mf2-ratatui` | **22**: the CLI's, and `MarkupStyles`, `MarkupStyles::new`, `with`, `mf2_ratatui::line` | **1**; as the CLI |
| two-crate workspace (a library and a TUI sharing messages) | **48**: the shared translation crate 24 (a third crate, as the book has it); the library 3 (`mf2-native` and the translation crate in its `Cargo.toml`, a `use`), with an `i18n: &NativeI18n` parameter on each function that makes text; the TUI 21 (`Cargo.toml` 3, the handle and `--lang` 6, `use` 3, the styles 9) | **4 + 1** | **22** | **1**; as the CLI |
| Leptos `hello` | **96**: the translation crate `mf2 init` writes, 53 (`Cargo.toml` 18 with a 9-feature block, `build.rs` 13, `mf2.toml` 10, `lib.rs` 12 with the hand-shaped `setup()`); the application 43 — `Cargo.toml` 12 (a `[workspace]` of 3, 3 dependencies, 5 forwarded features, `watch-additional-files`), `lib.rs` 11 (2 `use`, `html_lang`, the two head components, a 4-line switcher, `install`, `hydrate_body`), the server 20 (2 `use`, `install`, a 10-line negotiator, a 6-line context closure, `catalog_routes`); and 3 lines changed to their `_with_context` forms | **4 + 1**: `mf2`, `mf2-build`, `leptos-mf2`, `mf2-axum`; the translation crate; and the `mf2` tool | **28**: `tr!`, `html_lang`, `CatalogPreload`, `CatalogLinks`, `LocaleSwitcher`, `LocaleOption`, `leptos_mf2::install`, `setup`, `hydrate_body`, `mf2_axum::install`, `CATALOGS`, `Negotiator`, `empty`, `source`, `sink`, `QueryParam`, `CookieLocale`, `secure`, `AcceptLanguage`, `provide_locale`, `catalog_routes`, `leptos_routes_with_context`, `file_and_error_handler_with_context`; features `fn-number`, `fn-datetime`, `datetime-icu`; the mode forwarded to two crates; `watch-additional-files` (and 7 more in the generated `lib.rs`) | **5**: `rustup target add`, `cargo install cargo-leptos`, `cargo install mf2-cli`, `mf2 init`, `cargo leptos watch` — 2 of them this library's |

**What the owner's trippy port added** (`vendor/trippy`, counted only,
nothing copied): **76 setup lines** against the 1.0.1 API it pins — a
59-line `locale.rs` (71 with comments and blank lines) holding its own
`thread_local! RefCell<NativeI18n>`, the system-locale choice and a `t!`
wrapper; `build.rs` +9; `Cargo.toml` +4; `mf2.toml` 3; `lib.rs` +1.

**Verdict against "Done when":** met — the runs agree exactly on every
allocation count, and every figure above is recorded with its command.
B10's time, taken under load, is recorded but is not a baseline (above).

**For A8 and later** (interpretation, kept brief):
* The 1.x TUI costs +291 to +299 allocations (+19–20 %) and +33 to +37 KB
  allocated per frame, and +574,536 B of stripped executable, against the
  trippy-style baseline. C5's gate ("allocations per frame ≤ both") has
  that gap to close: 1.x allocates a `String` for each `format`, and a
  builder with owned spans for each `line`. The size gate is against 1.x,
  not against the baseline.
* A Ratatui application built without the default features needs
  `layout-cache` (and `std`) on; the 2.0 starter and book should keep
  Ratatui's defaults or say so.
* The book's native project measures nothing of the TUI until its `main`
  reaches it (C8).

## A2 — `links` metadata: what was built

Probe: `probes/p10-links/` (its README lists how to re-run each row), run on
2026-09-28 with cargo 1.98.1, cargo-leptos 0.3.9 and rust-analyzer
1.98.0-nightly (b30f3df 2026-06-11).

* **The stand-in `mf2`** (`p10-links-mf2`, named `mf2` by its dependents)
  has `links = "mf2-v2"` and the real crate's features (`ssr`, `hydrate`,
  `csr`, `leptos`, `axum`, `native`, `ratatui`, `fn-number`, `fn-datetime`,
  `datetime-icu`, `datetime-intl`, `intl`, `static-locale`,
  `mark-fallback-lang`, `compile`). Its `build.rs` prints
  `cargo::metadata=features=<sorted, comma-separated, without default>` and
  `cargo::metadata=target=$TARGET`.
* **The cfg-forwarding macros** in it: `__if_ssr!`, `__if_not_ssr!`,
  `__if_hydrate!` (each defined twice, under a `cfg` and its negation: the
  tokens pass through or vanish), and `__use_host!(HOST)` (four
  definitions: the three-way browser-host choice `codegen.rs` makes today
  with `#[cfg(all(not(feature = "ssr"), …))]` on the *including* crate's
  features).
* **The stand-in `mf2-build`**: `run()` is the whole build script. It
  reads `DEP_MF2_V2_FEATURES`, else the crate's own `CARGO_FEATURE_*`, and
  writes a module whose text is the same in every build except for what
  the script decides (`BUILD_DECIDED_EMIT`; `EMITTED_FOR_SSR` /
  `EMITTED_FOR_NATIVE` only when it saw them). Compile-time choices go
  through the macros (`CATALOGS` only under `ssr`; `host::HOST`).
  `report()` prints the build script's view next to `mf2`'s `cfg!` truth.
  With `P10_RUN_LOG` set, each script run appends a line.
* **Scenarios**, each its own workspace: a one-crate application
  (`single/`); an application with a translation crate that has no
  features and a second dependent using the macros (`two-crate/`); `mf2`
  as normal plus build dependency, and as build dependency only
  (`host-target/`); cargo-leptos's two builds (`leptos-app/`, no Leptos
  crate — see "Scope"); rust-analyzer (`ra/`, `ra-control/`); duplicate
  `links` (`dup/`); `mf2`'s own test through a translation-crate
  dev-dependency (`fixture/`).

### Rows

| # | Row | Result | Observed | Command (in `probes/p10-links/`) |
|---|---|---|---|---|
| 1 | The variable's name | **confirmed** | `DEP_MF2_V2_FEATURES` and `DEP_MF2_V2_TARGET`: the `links` value upper-cased with `-` → `_`, then the key. The dependent's warning: `p10-links-single [x86_64-unknown-linux-gnu] via links: features=[fn-number,native] … vars=DEP_MF2_V2_FEATURES=fn-number,native DEP_MF2_V2_TARGET=x86_64-unknown-linux-gnu`. A present but empty value means `mf2` with no features (`DEP_MF2_V2_FEATURES=`); absent means no `links` at all | `cd single && cargo build -v --features native` |
| 2 | One-crate application | **PASS** | `build saw [fn-number,native] via links …; mf2 compiled with [native,fn-number]; build decided … emit=native`; with `--features ssr`: `emit=web server; macros chose ssr=true … catalogs=1 host=host_std::HOST`; with `hydrate,datetime-intl`: `emit=web client; macros chose … host=host_web::INTL_HOST` | `cd single && cargo build -v --features <f> && ./target/debug/p10-links-single` |
| 3 | Two crates: the translation crate sees what the application turned on | **PASS** | The application names `mf2` with `fn-number` and turns on `mf2/ssr`; the translation crate names `mf2` with no features and has no `[features]`: `i18n: build saw [fn-number,ssr] via links …; macros chose ssr=true … catalogs=1 host=host_std::HOST`; the second dependent (no features, no build script): `widgets: server widgets; mf2 compiled with [ssr,fn-number]`. `--workspace --features p10-links-app/ssr`: the same. **But** `cargo build -p p10-links-i18n` alone: `features=[]` — that build compiles `mf2` with no features, so the script's view still equals the compiled one | `cd two-crate && cargo build -v -p p10-links-app --features ssr`; `… -p p10-links-i18n` |
| 4 | Host vs target: `mf2` as a normal dependency (`hydrate`) and a build-dependency (`native`, `compile`) | **PASS** — the target instance's | Two instances built (`--cfg feature="compile" … "native"` and `--cfg feature="hydrate"`); the script got `DEP_MF2_V2_FEATURES=hydrate DEP_MF2_V2_TARGET=x86_64-unknown-linux-gnu`; with `--target wasm32-unknown-unknown`: `hydrate` and `wasm32-unknown-unknown`. **`mf2` as a build-dependency only: no `DEP_MF2_V2_*` at all** (`via own CARGO_FEATURE_*: features=[] mf2-target=(unset) vars=`) | `cd host-target && cargo build -v -p p10-links-both [--target wasm32-unknown-unknown]`; `… -p p10-links-build-only` |
| 5 | cargo-leptos: two builds, two values | **PASS** | `cargo build --package=p10-links-leptos-app --bin=p10-links-leptos-app --no-default-features --features=ssr` → the translation crate's script saw `[fn-number,ssr]` on `x86_64-unknown-linux-gnu`; `cargo build … --lib --target-dir=…/target/front --target=wasm32-unknown-unknown --no-default-features --features=hydrate` → `[fn-number,hydrate]` on `wasm32-unknown-unknown` (cargo-leptos 0.3.9 gives the client build a target directory of its own) | `cd leptos-app && cargo leptos build` |
| 6 | A second `cargo leptos build` does nothing | **PASS** | Builds 2 and 3: no `Compiling` line, no script run (the run log stays at the first build's two lines), and `target/site/pkg/p10_links.wasm` and the server binary byte-identical (sha256 `931f868bc3fdfd25…`, `f06558561d23336c…` before and after). cargo-leptos re-runs its own wasm-bindgen step each time (≈ 160 ms), as for any application | `cd leptos-app && cargo leptos build` × 3; `sha256sum` |
| 7 | The macros' choices in cargo-leptos's builds | **PASS** | The wasm holds no `catalog bytes` (the `ssr`-only `CATALOGS`) and names only `host_web::HOST`; the server binary holds `catalog bytes` and `host_std::HOST`. The generated files differ only in the script's decisions (`BUILD_DECIDED_EMIT: "web server"` / `"web client"`; `EMITTED_FOR_SSR` in the server's) | `grep -a -c 'catalog bytes'` on both artefacts |
| 8 | rust-analyzer | **PASS** | From a deleted `target/`, `rust-analyzer analysis-stats .` ran the scripts through cargo (the file it left says `BUILD_SAW: "fn-number,ssr"` and has `EMITTED_FOR_SSR`), collected the generated items (`EMITTED_FOR_SSR`, `CATALOGS`, `MACRO_SAW_SSR`, `report`) and inferred everything: `exprs: 656, ??ty: 0`, `pats: 89, ??ty: 0`. **Negative control** — two references to items this build does not have (`MACRO_SAW_HYDRATE`, `EMITTED_FOR_NATIVE`): `exprs: 662, ??ty: 4`, `pats: 92, ??ty: 2`. `rust-analyzer diagnostics .`: only `inactive-code` hints in `mf2`, e.g. "code is inactive due to #[cfg] directives: feature = "ssr" is enabled" and "… feature = "hydrate" is disabled" — its `cfg`s for `mf2` match the build. **Two limits of the RA command line, not of `links`:** `rust-analyzer unresolved-references` panics on any crate, the three-line `ra-control/` included ("Try to use attached db, but not db is attached"); `diagnostics` does not report an unresolved value path (the negative control printed nothing where cargo gives E0425) — hence `analysis-stats` | `cd ra && rm -rf target && rust-analyzer analysis-stats .`; `rust-analyzer diagnostics .` |
| 9 | A feature toggle reruns the dependent's script, and only what it must | **PASS** | `single/`, `native` → `ssr`: `mf2` recompiled as a new unit, its script and the application's run, the application compiles; back to `native`: every unit `Fresh`, no script runs; the same for `hydrate,datetime-intl` → `ssr`. `two-crate/`, `ssr` → `hydrate`: `mf2` recompiled, `mf2`'s and the translation crate's scripts run (the latter is **not recompiled**: the same `build/p10-links-i18n-6a6f157356afc973/build-script-build` every time), the translation crate, the second dependent and the application recompile, `mf2-build` `Fresh`; back: all `Fresh`. No `rerun-if-env-changed` is printed or needed: a new feature set is a new unit of `mf2`, and the dependent's script-run unit (and its `OUT_DIR`) is new with it | `cargo build -v --features …`, in turn |
| 10 | Also: a translation-only edit; `check` and `build` alternating | **PASS** | Editing `i18n/locales/en.mf2`: `Dirty p10-links-i18n …: the file i18n/locales has changed`, only that script runs, with the same metadata (`[fn-number,ssr]`); `mf2` `Fresh`. `cargo check`, `build`, `check`: no script runs (0 in the log), only the edited crate rebuilds in each mode | `cd two-crate && cargo build -v …`; `cargo check -v …` |
| 11 | `cargo metadata` carries the features (for `mf2 check`) | **PASS, with two caveats** | `resolve.nodes[]`, `mf2`'s node: `["default","fn-number"]`; with `--features p10-links-app/ssr`: `["default","fn-number","ssr"]`. **(a)** the set is the union over every workspace member plus the given `--features` (with both `ssr` and `hydrate` passed, both listed) — it is not per build side. **(b)** host and target instances are merged: `host-target/` shows `["compile","default","hydrate","native"]` where the builds used `[hydrate]` and `[compile,native]`, also with `--filter-platform wasm32-unknown-unknown` | `cargo metadata --format-version 1 [--features …] \| jq '.resolve.nodes[] \| select(.id \| contains("p10-links-mf2@")) \| .features'` |
| 12 | A duplicate `links` | **the error, recorded** | Two packages with `links = "mf2-v2"`: the error in block (a) below. The same for two semver-incompatible versions of one package (0.1.0 and 0.2.0). The next major with `links = "mf2-v3"` resolves beside it, and a script depending on both gets `DEP_MF2_V2_*` and `DEP_MF2_V3_*` | `cd dup/app-copy && cargo build`; `dup/app-next`, `dup/app-v3` |
| 13 | `cargo package` / `cargo publish --dry-run` | **PASS** | `Packaged 6 files, 8.0KiB (3.0KiB compressed)`, verified by building; `publish --dry-run`: `Uploading p10-links-mf2 v0.1.0 … warning: aborting upload due to dry run`, exit 0; the normalized manifest keeps `build = "build.rs"` and `links = "mf2-v2"`. Still passes with the path-only dev-dependency of row 14. **`links` without a build script is a manifest error** (block (b) below) | `cargo package -p p10-links-mf2 --allow-dirty`; `cargo publish --dry-run -p p10-links-mf2 --allow-dirty`; `cd dup/mf2-nobuild && cargo build` |
| 14 | Also: `mf2`'s own test through a translation crate that depends on `mf2` (the dev-dependency cycle) | **PASS** | `cargo test -p p10-links-mf2 --test generated`: the fixture's script saw `[]`; with `--features ssr`: `[ssr]`, and `MACRO_SAW_SSR == cfg!(feature = "ssr")` holds | `cargo test -p p10-links-mf2 --test generated [--features ssr]` |
| 15 | cfg-forwarding macros under unification | **PASS** | Rows 3 and 7: the application turns `ssr` on; the translation crate's generated module (same text in every build) and the second dependent's own code follow `mf2`'s compiled `cfg`, with no `cfg(feature)` of their own. In every build observed, the script's decisions and the macros' agreed. **One finding:** a choice made *inside a function* through a macro lints differently per mode (`unused_mut` in one build, `unused_assignments` in the other); choices made at item level (a whole `static`, `fn` or `use`) do not | rows 2, 3, 5, 7 |

(a) Row 12, `dup/app-copy` (paths shortened to `<p10-links>`):

```text
error: failed to select a version for `p10-links-mf2-copy`.
    ... required by package `p10-links-dup-copy v0.0.0 (<p10-links>/dup/app-copy)`
versions that meet the requirements `*` are: 0.1.0
package `p10-links-mf2-copy` links to the native library `mf2-v2`, but it conflicts with a previous package which links to `mf2-v2` as well:
package `p10-links-mf2 v0.1.0 (<p10-links>/mf2)`
    ... which satisfies path dependency `mf2` of package `p10-links-dup-copy v0.0.0 (<p10-links>/dup/app-copy)`
note: only one package in the dependency graph may specify the same links value to ensure that only one copy of a native library is linked in the final binary
for more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links
help: try to adjust your dependencies so that only one package uses the `links = "mf2-v2"` value
failed to select a version for `p10-links-mf2-copy` which could resolve this conflict
```

(b) Row 13, `dup/mf2-nobuild`:

```text
error: failed to parse manifest at `<p10-links>/dup/mf2-nobuild/Cargo.toml`

Caused by:
  package specifies that it links to `mf2-v2` but does not have a custom build script
```

**Scope.** The cargo-leptos application has no Leptos crate in it: the
question is how the features reach the build script, and cargo-leptos runs
the same two cargo builds (features, targets, its own target directory for
the client) with or without it. Leptos's own no-op rebuild under
cargo-leptos is P0.9's and the demos'. C6 re-runs these scenarios on the
real crates, with Leptos, per its "Done when".

### Verdict

**D19's first half is adopted as "`links` + cfg macros".** Every row passes.
The fallback (the translation crate keeps its function features) is not
needed. Nothing needs the owner.

### For C6, C4 and D3 (interpretation)

- `mf2`'s `build.rs` prints `cargo::metadata=features=…`. The `cargo::` form
  needs Rust 1.77; the MSRV is 1.88. `mf2_build::run()` reads
  `DEP_MF2_V2_FEATURES`: an empty value means no features, an absent one a
  crate that does not name `mf2` directly. It then errors, or falls back to
  `CARGO_FEATURE_*` for 1.x-shaped crates.
- The crate that includes the module must name `mf2` as a **normal**
  dependency (row 4). `mf2-build` must never depend on `mf2`: `cargo
  metadata`, and so an editor's `cfg`s, would see the union of both
  instances (row 11b).
- Split the choices this way. The build script decides what to *emit*
  (native or web, server catalogs, compression) from the metadata. The
  generated text makes its *compile-time* choices at item level through
  `mf2`'s cfg macros (rows 7, 15). That puts the choices in `mf2` and keeps
  the generated file lint-clean in every mode.
- `mf2 check` reads `mf2`'s node from `cargo metadata`, with the
  application's `--features`. For a cargo-leptos application that means
  the function features, which are the same on both sides; the mode
  features differ per side, and `metadata` gives their union (row 11a).
- A build-dependency's features cannot travel this way. If `mf2`'s
  `datetime-icu` is on while `mf2-build` lacks `icu-blob`, `run()` must say
  so; it can test `cfg!(feature = "icu-blob")` in itself. This is C6's
  "clear error".
- More metadata keys cost nothing (row 1's `target`). A `version` key would
  let `run()` refuse a mismatch between `mf2` and `mf2-build`.
- A translation crate built on its own (`-p i18n`) sees only what that
  build turns on. That is correct for that build, but it is not the
  application's set (row 3).
- Cargo replays a fresh script's `cargo::warning` lines on every build (rows
  6, 9). The real build's lint warnings already work this way.

## A3 — `tr!` inside its own crate: what was built

**The probe.** `probes/p10-tr-in-crate/`, a standalone workspace with path
dependencies on the real `mf2` and `mf2-build` (the tree at `2fb7f54`), a
two-message corpus (`hello`, `greet = Hello, {$name}!`, in `en` and `fr`),
and `Emit::Native` so that no Leptos is compiled. Every variant uses the real
`mf2-build` module and the real `__tr_impl!` proc macro; only the *wrapper*
that reaches it is varied — for variant 3 the build script cuts
`mf2-build`'s generated `tr!` wrapper off the module and appends the
variant's shape, for variant 4 two stand-in proc macros forward to
`__tr_impl!`. `./run.sh` compiles every case (102, one feature set each) and
writes each compiler output to `results/<case>.txt`; the scenarios run by
hand (invalidation, relocation, rust-analyzer, a git dependency's
future-compatibility report: `gitdep.sh`) are in `results/` with their
commands. rustc 1.98.1, rust-analyzer 1.98.0-nightly (2026-06-11).

### The five variants

| # | Shape | Result | Observed |
|---|---|---|---|
| 1 | today's: `#[macro_export] macro_rules! tr` inside `include_generated!` | **unqualified only, and only after the include** | unqualified after the include (a module declared after it, or the root): PASS. Unqualified in a module declared *before* it: `error: cannot find macro `tr` in this scope` — and rustc's own help is `consider importing this macro through its public re-export: use crate::tr;`, which is the next error. `crate::tr!`, `use crate::tr;`, `super::tr!`, `self::tr!` and `$crate::tr!` in a macro of the crate, before or after the include: `error: macro-expanded `macro_export` macros from the current crate cannot be referred to by absolute paths` … `= warning: this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!` … `= note: #[deny(macro_expanded_macro_exports_accessed_by_absolute_paths)] (part of #[deny(future_incompatible)]) on by default` (rustc #52234). Another crate: `v1_today::tr!` and `use v1_today::tr;` PASS. Also tried: `#[path = concat!(env!("OUT_DIR"), "/mf2_generated.rs")] mod generated;` → `error: malformed `path` attribute input … must be of the form #[path = "file"]` |
| 2 | 1 + `#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]` | **compiles, not adoptable** | Every path spelling compiles (unqualified before the include still fails). Every build of the crate then ends `warning: the following packages contain code that will be rejected by a future version of Rust: v1-today …`; `cargo report future-incompatibilities` shows the lint at each call site. **For a dependency that is not local** (the allowing crate committed to a git repository, `gitdep.sh`), the same warning is printed in every *consumer's* build, and the report tells the consumer to "ensure the maintainers know of this problem" or `[patch]` the dependency |
| 3a | `macro_rules! tr` (not exported) + `pub(crate) use tr;` | in-crate yes, other crates **no** | In the crate: every spelling PASS, before and after the include, except unqualified before it. An import after the include is redundant (textual scope wins): `warning: unused import: crate::tr`. A crate that never calls it: `warning: unused macro definition: tr`. Another crate: `error[E0603]: macro `tr` is private` |
| 3b | exported `tr` + `pub(crate) use tr;` at the root | **fails to compile** | `error[E0255]: the name `tr` is defined multiple times` … `tr must be defined only once in the macro namespace of this module`; and a prelude's `pub use super::tr;` is #52234 again |
| **3c** | `#[doc(hidden)] #[macro_export] macro_rules! __mf2_tr` + `pub use __mf2_tr as tr;` + `pub mod prelude { pub use super::tr; }` | **everything but one spelling, one name everywhere** | In the crate, before and after the include: `crate::tr!`, `use crate::tr;`, `use crate::prelude::*;` PASS, no lint, no warning; unqualified at the root after the include PASS. Unqualified in a module without an import: `cannot find macro `tr`` with rustc's help `use crate::tr;` — which now compiles. Another crate: `my::tr!`, `use my::tr;`, `use my::prelude::*;` PASS. A **binary** crate (a module before the include with the prelude, one after with `use crate::tr`, the root): PASS, no warning |
| 3d | 3a + `#[macro_export] macro_rules! __mf2_tr_export` re-exported as `exports::tr` | in-crate as 3a; other crates only as `my::exports::tr` | `my::tr!` from another crate: `error[E0603]: macro `tr` is private` |
| 3e | 3c + a textual `macro_rules! tr` for the code after the include | **ambiguous where both are in scope** | Unqualified after the include PASS; but `use crate::tr;` or `use crate::prelude::*;` in a module after the include: `error[E0659]: tr is ambiguous` … `ambiguous because of a conflict between a macro_rules name and a non-macro_rules name from another module` |
| 4 | an env-driven proc macro, by path (`use v4_env_macro::tr_env;`) | **works; not chosen** | `tr_env!` reads `MF2_MANIFEST` / `MF2_MANIFEST_HASH` from the build script's `cargo::rustc-env`; `tr_outdir!` reads `$OUT_DIR/manifest.mf2m`. Before, after and at the root: PASS. **Invalidation:** `$other` added to the source's `greet` → both call sites report `message greet needs argument other (its variables: $name, $other); write tr!("greet", name = …, other = …)` at the id; reverted → PASS; a translation-only edit reruns the build script and recompiles the crate, as today. **Relocated target directory** (`mv target target-moved`, a source touched, `CARGO_TARGET_DIR=target-moved`): the build script stays fresh, and both macros saw the *moved* path — the stored output still says `target/…`, so cargo rewrote the old `OUT_DIR` prefix in the replayed `rustc-env`. **rust-analyzer:** expands both; the seeded negative controls are reported at the call site (`unknown message id helo; did you mean hello?`, `message greet has no variable $nme …`), no `unresolved-macro-call` / `unresolved-proc-macro` |
| 5 | a library prelude's `tr` (glob-imported) against a generated `tr` | **conflict** | Root glob + generated `tr`, call at the root: PASS (the generated one). A module *after* the include with the glob: `error[E0659]: tr is ambiguous` (today's shape and 3a's alike). A module *before* the include with the glob: the prelude's `tr` is chosen **silently** (the stand-in's `compile_error!("the PRELUDE's tr! was chosen")` fired). Glob + explicit `use crate::tr;` after the include: today's shape → #52234; 3a → PASS with `unused import` |

**rust-analyzer** (`rust-analyzer diagnostics .` over the probe with 3c's call
sites on and seeded controls, 49 s): 3c and 4 both expand; each seeded error
is reported at its id or argument; the only other diagnostics are
`inactive-code` hints and one clippy hint in `mf2-catalog`
(`results/ra-diagnostics.txt`).

### Verdict

**The in-crate mechanism: 3c.** The generated module exports the wrapper
under a hidden name and re-exports it as `tr` (and `msg_id` alike), and adds
a `prelude`. Because `tr` is then a `use` of the exported macro — not the
macro itself — it can be named by path in the crate that includes it, which
the exported macro cannot. It is today's wrapper, proc macro, manifest path,
hash and relocation fallback, unchanged; only two lines of the module move.
Variant 4 works too (and survives relocation without a fallback), but it
gives the crate a second spelling (`mf2::tr!` inside, `my::tr!` outside), a
new public proc macro in `mf2`, and variant 5 shows that a `tr` in `mf2`'s
own prelude collides with a generated one. **Done when:** met.

**Proposed text for 05 §4** (replacing "and **inside the i18n crate itself
only unqualified** … fixture's tests are written that way"):

> **`tr!` in its own crate and in others** (2.0; Phase 10 A3). The module
> exports the wrapper under a hidden name and names it `tr` with a `use`,
> and does the same for `msg_id`:
>
> ```rust
> #[doc(hidden)] #[macro_export] macro_rules! __mf2_tr { … }   // path and hash baked in, as before
> pub use __mf2_tr as tr;
> pub mod prelude { pub use super::tr; /* and the other generated names */ }
> ```
>
> A `macro_export` macro that arrives through `include!` cannot be named by
> a path in its own crate (rustc #52234); a `use` of it can. So:
> * **in the crate that includes the module** — a one-crate application, or
>   a library with messages of its own — any module, declared before or
>   after the include, writes `use crate::tr;` or `use crate::prelude::*;`
>   (or `crate::tr!(…)`); at the root, after the include, `tr!` needs no
>   import. A module that forgets gets rustc's own suggestion, `use
>   crate::tr;`, which compiles;
> * **in a crate that depends on a translation crate** — `my_i18n::tr!(…)`,
>   `use my_i18n::tr;` or `use my_i18n::prelude::*;`, as in 1.x;
> * **three rules keep the name unambiguous:** the module defines no
>   textual `macro_rules! tr` beside the re-export (the two are ambiguous,
>   E0659, wherever both are in scope); `mf2`'s own prelude has no `tr` (a
>   glob-imported `tr` is ambiguous with the generated one after the
>   include, and silently chosen before it); and
>   `macro_expanded_macro_exports_accessed_by_absolute_paths` is never
>   allowed — a crate that allows it prints a future-incompatibility warning
>   in every build that depends on it.
>
> Upgrading from 1.x: a module declared after the include that called `tr!`
> unqualified now imports it (`use crate::tr;` or the prelude).

### Found along the way

- **A missing `mf2.toml` rebuilds the crate on every build.** `mf2-build`
  prints `cargo::rerun-if-changed=<crate>/mf2.toml` whether or not the file
  exists; cargo reports `Dirty v4-env …: the file v4-env/mf2.toml is missing`
  and reruns the build script and recompiles the crate each time, with
  nothing edited (`results/v4.no-mf2-toml-rebuilds.txt`). D19 makes
  `mf2.toml` optional, so C6 has to print the line only for a file that
  exists (or watch the directory for its creation). Observed on 1.x.
- **`neutral-numbers` fires on a corpus with no number in it:** a single
  `{$name}` placeholder draws "this locale formats numbers but fn-number is
  off" once per locale, because an unannotated placeholder may receive a
  number. Recorded; not in A3's scope.

### What it means for the design (A8, C6, C7)

- C4/C6 generate 3c's shape (`__mf2_tr` + `pub use … as tr`, `__mf2_msg_id`
  + `msg_id`) and the crate `prelude`; the 2.0 samples write `use
  crate::prelude::*;` (or `use crate::tr;`) in each module that calls
  `tr!` — the work order's `src/ui.rs` sample needs that line.
- `mf2`'s own prelude (if 2.0 has one) must not contain `tr`.
- The native one-crate default (D19) needs nothing more from the compiler;
  the in-crate part of D19 is settled without a fallback.
- C7's `mf2 init` scaffold note and the fixture's comment about
  "unqualified only" go; "Upgrading from 1.x" gains the import line.

## A4 — the ambient store's cost: what was built

* **Where.** Built on the probe branch `p10-a4-ambient` (off `2fb7f54`;
  not merged): `d6836f8`, `9c8dc82`, `d3fb379`, `07ac660`, `be498b1`,
  `aca96c4` (the runtime seam), `4e31af1`, `389b4be`. The probe is copied to
  `main` as `probes/p10-ambient/`, with the seam as
  `probes/p10-ambient/seam.patch` (the runtime on `main` is unchanged). A
  standalone workspace over the path crates (its `README.md` lists every
  command):
  * `i18n/` — a 112-message trippy-shaped corpus (63 plain, 24 with
    arguments, 10 plurals, 15 with markup) in `en` and `fr`, built by
    `mf2-build` with `Emit::Native`, laid out as the 1.x book's native page;
  * `ambient/` — variants (b) and (c): the store, the lookup, `Display`,
    `to_string` / `to_cow`, the zero-copy Ratatui `Line`, the theme;
  * `bench/` — each variant as a frame and as single-message cases, a
    counting allocator, `ambient-bench check | allocs | breakdown | time |
    time-mt | pools`, and the thread tests;
  * `cli-a/`, `cli-b/` — one three-message CLI on (a) and on (b).
* **The variants.** Each formats in `fr`, not the source locale.
  * **(a)** 1.x: an app-owned `NativeI18n` passed by reference; `format` →
    `String`; `mf2_ratatui::line` with `MarkupStyles`.
  * **(b)** the design: `install(&'static Corpus)` fills a `OnceLock` store.
    The catalogs read the executable's bytes in place and the `Catalog`
    values are leaked once, so each is `&'static`. The active locale is one
    `AtomicUsize`. A thread's override is one thread-local `Cell`, set by
    `with_locale` and restored by a guard. The settings (bidi, time zone,
    theme) sit behind an `RwLock` with a generation counter; each thread
    keeps a copy and re-reads the lock only when the generation moved.
    A simple message is borrowed (`to_cow` → `&'static str`); others are
    formatted into a reused scratch and copied out once; `Display` streams
    into the `fmt::Formatter`. Markup goes through a zero-copy sink, styled
    by a theme keyed by `markup_key`.
  * **(b-copy)** (b)'s store with 1.x's outputs (a `String` per text, 1.x's
    sink): the store's own cost, all else as (a).
  * **(c)** (b), reading the settings lock on every format.
  * **(d)** the port's shape: `thread_local! { RefCell<Option<NativeI18n>> }`
    and a `t!`-like wrapper; a `String` per message; 1.x's `line`.
* **Correct first** (`ambient-bench check`, 2026-09-28): all six variants
  render the 112 messages with (a)'s text and styles, compared character by
  character with each character's style (the zero-copy sink splits spans
  differently). In (b)'s frame, 100 spans borrow catalog text and 43 own
  theirs (34 formatted messages, 9 placeholders).

### Allocations

Exact; `ambient-bench allocs` run twice, byte-identical tables
(2026-09-28). One iteration after a warm-up. "Frame" is the 112 messages
into Ratatui `Line`s: plain ones `Line::from(Span)`, markup ones styled.
Allocations / bytes requested:

| case | (a) 1.x | (b) design | (b-copy) | (c) | (d) port |
|---|---:|---:|---:|---:|---:|
| simple → `String` | 1 / 13 | 1 / 13 | | 1 / 13 | 1 / 13 |
| simple → `&'static str` | — | **0** | | 0 | — |
| one `:integer` argument → `String` | 1 / 8 | 1 / 7 | | 1 / 7 | 1 / 8 |
| one string argument → `String` | 2 / 33 | 1 / 22 | | | 2 / 33 |
| one string argument → `println!` | 2 / 33 | **0** | | | 2 / 33 |
| markup → `Line` (`status.connected`) | 10 / 557 | **3 / 203** | 10 / 557 | 3 / 203 | 10 / 557 |
| **frame** | **413 / 27,631** | **227 / 21,483** | 386 / 27,043 | 227 / 21,483 | 413 / 27,631 |

(b)'s three ways of proving text `'static` (below) allocate alike: R1, R2
and S are each 3 / 203 and 227 / 21,483. `ToString` through `Display`
(`Show(&m).to_string()`) is 2 / 33, against the inherent `to_string`'s
1 / 22.

Where a frame's allocations go (`ambient-bench breakdown`). In (b):
* a plain simple message costs only its `Line`'s `Vec` (63 messages: 2 →
  1);
* a markup message costs its `Vec` and one `String` per placeholder (the 7
  one-key hints: 6 → 1; `status.connected`: 10 → 3).

What is left is not the store's:
* a `&str` argument from a variable is copied into an `Arc<str>`, one per
  argument (a literal is not: the macro emits `ArgValue::str_static`) —
  C1's measurement;
* each of the 10 `.match` messages allocates **4 times inside the runtime**
  (its `Scratch` working lists: declarations, inputs, selectors), in 1.x
  and in the design alike (7 → 6 a message);
* the `Line`'s own `Vec`, which any `Line` needs.

### Time

Taken **under load** (the other probes building: load average 9–16 on the
8-thread i7-1165G7), so only the ratios are the result. The variants run
round-robin in one process, each round a batch of iterations per variant
(≈ 2 ms of the first), the starting variant rotating; each row is the
median of 101 rounds' means, with their 10th–90th percentiles.
`ambient-bench time 101` (seam build), 2026-09-28, load 8.95 → 9.50:

| case | variant | median ns | p10–p90 | × (a) |
|---|---|---:|---:|---:|
| lookup: catalog + settings | (a) / (b) / (c) / (d) | 7.7 / 14.1 / 41.2 / 10.2 | | 1 / 1.82 / 5.32 / 1.32 |
| simple → `String` | (a) | 187 | 181–219 | 1.00 |
| | (b) | 144 | 136–159 | **0.77** |
| | (c) | 142 | 136–168 | 0.76 |
| | (d) | 191 | 180–209 | 1.02 |
| simple → `&'static str` | (b) / (c) | 84 / 84 | 80–89 | |
| one `:integer` argument → `String` | (a) | 1,693 | 1,584–1,860 | 1.00 |
| | (b) | 1,749 | 1,718–1,808 | 1.03 |
| | (c) | 1,754 | 1,689–1,872 | 1.04 |
| | (d) | 1,707 | 1,661–1,909 | 1.01 |
| one string argument → `String` | (a) / (b) / (d) | 861 / 862 / 887 | | 1.00 / 1.00 / 1.03 |
| | `Show(..).to_string()` | 972 | | 1.13 |
| one string argument → `println!` | (a) | 979 | 829–2,652 | 1.00 |
| | (b) | 821 | 716–2,156 | **0.84** |
| | (d) | 1,058 | 865–2,388 | 1.08 |
| markup → `Line` | (a) | 2,978 | 2,905–3,397 | 1.00 |
| | (b-copy) | 3,035 | 2,864–3,523 | 1.02 |
| | (b), R2 | 2,402 | 2,300–2,821 | **0.81** |
| | (b), R1 | 2,464 | 2,347–2,641 | 0.83 |
| | (b), S | 2,349 | 2,210–2,676 | 0.79 |
| | (c) | 2,416 | 2,301–2,693 | 0.81 |
| | (d) | 3,002 | 2,919–3,721 | 1.01 |
| **frame** (112 messages) | (a) | 136,018 | 122,203–152,422 | 1.00 |
| | (b-copy) | 136,392 | 125,486–154,422 | 1.00 |
| | (b), R2 | 120,001 | 108,225–145,001 | **0.88** |
| | (b), R1 | 118,610 | 106,902–166,914 | 0.87 |
| | (b), S | 116,591 | 104,968–143,971 | 0.86 |
| | (c) | 118,971 | 106,470–133,828 | 0.87 |
| | (d) | 140,038 | 124,360–199,932 | 1.03 |

Three earlier runs of the same binary without the seam (loads 15–22) agree:
(b)'s frame 0.85–0.87 × (a), its markup `Line` 0.79–0.82 ×, (b-copy)
0.97–1.00 ×, (d) 1.02–1.04 ×. The one-argument row is the only one where
(b) is not below (a): 1.00–1.03 × across the runs (the store's lookup plus
the scratch's copy; the same one allocation).

Frames on several threads at once (`ambient-bench time-mt THREADS 21`: each
round starts the threads on a barrier and each thread times 40 frames; the
round's figure is the median thread's). Five runs, 2026-09-28, loads
10.7–22:

| threads | (b) × (a) | (c) × (a) | (d) × (a) |
|---|---:|---:|---:|
| 8 (three runs) | 0.78 / 0.83 / 0.84 | 0.79 / 0.90 / 0.95 | 1.01 / 1.12 / 1.18 |
| 4 (two runs) | 0.81 / 0.93 | 0.78 / 0.99 | 0.93 / 1.19 |

Under this load the threaded runs cannot separate (b) from (c). The
single-thread lookup can: (c)'s read lock costs ≈ 27–31 ns a format more
than (b)'s copy (41–46 against 14–15 ns; 1.x's handle 7.4–7.7 ns); per
frame that is ≈ 3 µs of ≈ 120 µs.

### Stripped size

`cargo build --profile stripped -p cli-a -p cli-b` (release: opt-level 3,
fat LTO; symbols stripped). The same CLI both ways: the system's language
or the first argument; one simple message, one with a string argument, one
plural.

| | bytes |
|---|---:|
| (a) `NativeI18n` | 1,145,640 |
| (b) the store, printed through `Display` | **1,140,392 (−5,248)** |

The first (b) was **+3,784 B**. `nm -S --size-sort` over both showed a
`Display` impl per description type (`Show<Tr>` 3,200 B, `Show<TrArgs>`
1,976 B), each inlining the whole path; and, once they shared one function
taking `&dyn Msg`, the **parts path** (≈ 3.8 KB) — kept alive by the
vtable, which lists `Message::parts`, in a CLI that never calls it. Each
form now forwards to one non-generic function taking a trait object that
lists only what that form calls (`write` for text, `parts` for Ratatui),
and R2's pools are built by the first Ratatui conversion, not by `install`.

### Zero copy

* **Constant text.** A simple message is `&'static str` straight from the
  `&'static Catalog`: `to_cow` allocates nothing (above).
* **Pattern text parts**, recovered as `'static` in safe code (`ptr as
  usize`, `str::get`, `ptr::eq`; no `unsafe`); a part that cannot be proven
  is copied, so correctness never depends on the proof. Three methods,
  measured side by side (the tables above):
  * **R1** — a pointer-range check against the catalog's `&'static` bytes,
    then `from_utf8` on the re-slice: O(length) a part;
  * **R2** — the same check against the catalog's string pool, validated
    once as `&'static str` (2,731 B, both locales, in 1,389 ns, best of
    101: `ambient-bench pools`); `str::get` re-slices in O(1). **No change
    to the runtime**;
  * **S** — a hidden seam, `PartSink::part_catalog_text(&Catalog, StrRef)`
    in `mf2-runtime` (commit `aca96c4`), whose default is today's
    behaviour; the native sink resolves the reference against its own
    `&'static Catalog` after a `ptr::eq`. No range check, no pool.
* **S against B1 and B12.** `cargo xtask size` in this worktree on
  `2fb7f54` (`--out target/p10-a4/size-base`), then with the seam
  (`--keep`: the same generated sources at the same paths; the runtime
  rebuilt): **byte-identical**, every application at both scales (B1 fixed
  26,416 B gz, B5 8.3 B gz a site, the whole app 41,872 B gz, both runs);
  the gate's applications never format to parts. `bench/b12/check.sh`
  (its `runtime` harness formats to parts through a `PartSink`): runtime
  38,336 → 38,324 B raw (−12), 19,223 → **19,224 B gz (+1)**; **B12 clean**
  in both runs.
* Recorded for A1's reference (not this probe's A/B): this worktree's
  baseline, B1 26,416 / B5 8.3 / whole app 41,872 B gz, differs from A1's
  main-tree 26,676 / 8.2 / 41,889 at the same commit. The cause, checked
  by the coordinator: `Cargo.lock` is not committed, and the worktree
  resolved its dependencies afresh — Leptos `0.9.0-beta2` (the main tree's
  lock has `0.9.0-beta`), newer `js-sys` / `wasm-bindgen` and others. **A
  size A/B is valid only within one tree and one lock**; the ±64 B gates
  compare against a base measured under the same lock (A1's figure is the
  main tree's, until its lock is updated).

### Threads

`cargo test --release -p ambient-bench`; the probe's tests run in parallel
with each other on purpose, since the store is process-wide.
* 16 threads, half pinned to `en` and half to `fr` by `with_locale`, each
  formatting 2,000 rounds: a simple message (`to_cow` and `Display`), an
  argument message and a markup `Line`. All correct, while another test
  changes the app-wide locale, bidi and time zone.
* `set_locale`, `set_bidi` and `set_time_zone` are seen by another thread's
  next format (a worker answering over a channel).
* `with_locale` nests, and is restored when its body unwinds; an unknown
  locale is an error.
* Formatting before `install()` panics with "call install() at start-up"
  (a separate test binary).

All pass; the three-test binary passed 20 of 20 repeated runs.

### Verdict

* **The ambient store (C2)** against method §3's gate, both parts **met**:
  * time per frame ≤ 1.x: (b) is 0.86–0.88 × (a) (0.85–0.87 × in the
    earlier runs), measured interleaved in one process rather than by
    alternating binaries;
  * stripped size ≤ 1.x: −5,248 B.
  * The store's own cost, isolated by (b-copy), is within noise
    (1.00 × per frame).
  * (d), the port's `RefCell`, is 1.02–1.04 ×, and 413 allocations like
    (a).
* **The text-borrowing method chosen: S**, the hidden seam.
  * Its cost on the client is −12 B raw / +1 B gz on B12's runtime harness
    and 0 B on the whole-app gate (the gate is ±64 B gz); B12 stays clean.
  * It is the fastest of the three (within noise), needs neither a pool
    nor knowledge of the catalog's layout, and makes "catalog text arrives
    with its reference" a contract rather than an inference from pointers.
  * It is the pattern `Sink::push_catalog_text` already set.
  * **Fallback:** R2, which changes nothing in the runtime.
* **Done when** — D17's figures against the gates, and the text-borrowing
  method chosen: **met**. Nothing needs the owner.

### What it means for C2 and C5 (interpretation)

* **C2: keep (b)'s shape.** A `OnceLock` store of `&'static` catalogs, an
  atomic locale index, a thread-local override with a restoring guard, and
  settings copied per thread by generation. The per-thread copy saves
  ≈ 27–31 ns a format over the lock. The native-only panic naming
  `install()` works as decided.
* **The public forms stay thin.** `Display` for each description, and
  every `From<…>` into Ratatui, should be one-line forwarders to one
  non-generic function taking a trait object that lists only what that
  form calls. A `&dyn Message` would pull the parts path into every CLI
  (+≈ 3.8 KB here).
* **Keep an inherent `to_string`** beside `Display`. It is faster and
  allocates once (0.99–1.00 × against `ToString`'s 1.09–1.13 ×; 1 against
  2 allocations). `println!("{}", tr!(…))` streams with none. This is
  A5's `inherent_to_string_shadow_display` question.
* **C5 inherits the zero-copy sink.** Text parts are borrowed through S;
  a placeholder is one `String`; the open-element stack is inline and keyed
  by `markup_key`, so markup names never allocate; a line break becomes a
  borrowed `" "` span. With it, a markup line falls from 10 allocations to
  its `Line`'s `Vec` plus one per placeholder (and C1's argument copies),
  and the frame from 413 to 227. C5's
  allocation gate (≤ 1.x and ≤ the upstream re-implementation) should hold
  on this evidence for 1.x; the upstream baseline is A1's `upstream.rs`.
* **Not the store's, and left to their owners:**
  * the `Arc<str>` copy of a variable `&str` argument (C1);
  * the runtime's 4 `Scratch` allocations per `.match` message, which
    affect 1.x and 2.0 alike.
* **Not measured here:** the ambient lookup's first step when `ssr` is also
  on (the request context before the native store, D17's order). In a
  native-only build that step is compiled out; with both features unified
  in one workspace, every native format pays it. C2 should time it.

## A5 — `Display` / `Debug` against B12: what was built

* **Where.** The probe branch `p10-a5-display` (one commit, **`70e0b27`** on
  `2fb7f54`; not merged). On `main`: `probes/p10-display/` holds the
  variant as `display.patch`, the scripts (`measure.sh`, `named.sh`,
  `twiggy.sh`, `ours.sh`, `clippy-variant.sh`, the positive control's
  `control.sh` / `control-stripped.sh`) and the analysis (`analysis/*.py`).
  The saved outputs stay in that worktree's git-ignored `target/a5/`
  (`{base,variant}/`, `named/`, `analysis/`'s outputs) until the worktree
  is removed.
* **The variant** (`crates/leptos-mf2`, 11 files, +243 −55):
  * `src/display.rs`, new and compiled in every build, holds the string
    conversions, moved out of the Leptos-only `glue/view.rs`: the inherent
    `to_string()` (isolated), `to_plain_string()`, `to_display_string()`,
    `From<_> for String`, and **`Display` for `Tr`, `TrArgs`, `TrRich`,
    `TrDyn`**. With a Leptos mode they read the ambient catalog:
    `text::to_string`, and for `Display` a new `text::fmt_display` that
    writes the text through `Formatter::pad` (so `{:<12}` pads) with no
    `String` in between. With no Leptos mode both write nothing: a stand-in
    until C2's native store.
  * `to_string` carries `#[allow(clippy::inherent_to_string_shadow_display,
    reason = "the client's fmt-free form of what `Display` writes (B12, 06
    §3); both produce the same text")]`, in place of 1.x's
    `inherent_to_string` allow.
  * **`Debug`**: derived on `TrArgs`, `TrDyn`, `DateTimeValue`, `Stored`,
    `RequestI18n`, `Handler<H>`, `Flat<F>`, `SignalArg<S>`; hand-written for
    `TrRich` (id, arguments, number of handlers), `ArgValue` (`Custom(..)`,
    `Source(..)`), `Text` (the text, quoted), `ArgList` (a list),
    `NestingHandler(..)`, `FlatHandler(..)`. `Tr` had it already.
  * `tests/render.rs`: `{}`, `ToString::to_string` and the inherent
    `to_string()` give the same isolated text; `{:<16}` pads; the `Debug`
    shapes (`TrArgs { id: MsgId(0), args: [Str("Ada")] }`,
    `TrRich { …, handlers: 1 }`, `Custom(..)`).
* **How it was measured.** `bash target/a5/measure.sh base` at `2fb7f54`
  (2026-09-28, 04:53–06:45), then `… variant` at `70e0b27` (06:45–07:36).
  Each runs `cargo xtask size`, `cargo xtask b5 --view`,
  `cargo xtask b12-generated` and `bash bench/b12/check.sh` with
  `CARGO_BUILD_JOBS=2`, and keeps every wasm. `bash target/a5/named.sh
  base|variant` rebuilt three clients with their symbol names kept
  (`strip = false`, `wasm-opt -Oz --debuginfo`) for twiggy: `tr` at 1,860
  sites, `tr-view` at 1,860 sites (a view workload generated once into
  `target/a5wl`), and the fixture client. In the `tr` app `leptos-mf2` is
  the Leptos-free core (`workload-i18n/hydrate` turns on only
  `mf2/host-web`), and its sites format through `Tr::format`. `tr-view`
  has the Leptos layer, and its string positions call the inherent
  `to_string()` that the variant moved.
* **The A/B is valid** (checked):
  * HEAD moved to `70e0b27` at 06:37:42 (`git reflog --date=iso`), while
    the base's `check.sh` was running (06:36:30–06:45:08). B12's harnesses
    depend only on `mf2-catalog`, `mf2-runtime`, `mf2-fn-number`,
    `mf2-fn-datetime` and `mf2-host-web` (`bench/b12/*/Cargo.toml`), none
    of which the variant touches, and base and variant `b12.txt` and
    `size.tsv` are byte-identical. Every other base figure, and the base's
    named builds (05:46–06:05), finished before 06:37:42;
  * **one resolution.** Each generated app has its own `Cargo.lock`, which
    each run resolves again. The variant's named build of `tr`, in the
    target directory the base's used, rebuilt only `leptos-mf2`, `mf2`,
    `workload-i18n` and the app; no dependency unit was built (the
    `.fingerprint` files written after 07:00). The `idlit`, `idlit-view`
    and `dummy` wasm are byte-identical between the runs at both scales
    (`cmp` over `target/a5/{base,variant}/wasm/`);
  * this worktree's base (26,400 / 8.3 / 41,871) is not A1's main-tree
    figure (26,676 / 8.2 / 41,889) nor A4's worktree's (26,416 / 8.3 /
    41,872) at the same commit: a lock of its own, as A4's record says. The
    gates below compare within this tree.

### Figures

| Figure | Base `2fb7f54` | Variant `70e0b27` | Change | Gate | Command |
|---|---:|---:|---:|---|---|
| **B1, fixed** | 26,400 B gz | 26,317 | **−83** | ±64 | `cargo xtask size` |
| **B5**, per site | 8.318 B gz (shown 8.3) | 8.350 (8.3) | **+0.032** | ±0.2 | same |
| **whole app**, 1,860 sites | 41,871 B gz | 41,848 | **−23** | ambition 105,120 | same |
| the `dummy` bound: per site / fixed | 25.2 / 27,383 | 25.3 / 27,300 | | reported | same |
| `tr` at 1,860 sites: opt raw / opt gz | 2,419,654 / 693,681 | 2,419,575 / 693,658 | −79 / −23 | | same |
| `tr` at 3,720 sites | 4,455,995 / 1,203,373 | 4,455,916 / 1,203,410 | −79 / +37 | | same |
| `idlit`, `dummy`, both scales | | | byte-identical | | `cmp` |
| `b5 --view`: per site, against `idlit-view` | 10.398 B gz (10.4) | 10.398 (10.4) | 0 | ±0.2 | `cargo xtask b5 --view` |
| `b5 --view`: fixed | 24,956 B gz | 24,958 | +2 | | same |
| `tr-view` at 1,860 / 3,720: opt raw | 1,890,880 / 3,375,262 | 1,890,883 / 3,375,265 | +3 / +3 | | same |
| `tr-view` at 1,860 / 3,720: opt gz | 572,548 / 956,397 | 572,550 / 956,399 | +2 / +2 | | same |
| `idlit-view`, `dummy` | | | byte-identical | | `cmp` |
| **B1′** on the generated module (F − E) | +0 B | +0 B | | +0 | `cargo xtask b12-generated` |
| B13 (B − A) | 13,573 B avoided | 13,573 | 0 | 13,599 ± 10 % | same |
| the fixture client, raw: E / F | 349,158 / 349,158 | 349,158 / 349,158 | 0 | | same |
| the fixture client, raw: A / B | 365,010 / 378,583 | 365,001 / 378,574 | −9 / −9 | | same |
| **B12** | clean | clean; report byte-identical | | clean | `bash bench/b12/check.sh` |

B12's per-harness figures are identical in both runs: the reader 6,781 B gz;
B1's runtime part 18,888 (the core numbers 5,407); B2 2,022; B3 5,486;
B1′ for `fn-number` and `fn-datetime` 0; the date semantics 3,605; B4
`datetime-intl` 5,157 plus 672 of JS; B1′ for `intl` −63. Each gated
harness shows no panic import, 0 fmt symbols and 0 panic symbols; the control
harness shows both. The verdict line, both times: "B12: clean (no panic
path, no core::fmt in the reader, the runtime, the numeric and the date
functions); B13: shown".

### Why B1's fixed part moved −83 B gz

Observed:

1. **The arithmetic.** `b5::delta` (`xtask/src/b5.rs`) fits two scales:
   marginal = (Δ@3720 − Δ@1860) / 1,860 and fixed = Δ@1860 − 1,860 ×
   marginal, where Δ = `tr` − `idlit`. Because 3,720 = 2 × 1,860, that is
   **fixed = 2·Δ@1860 − Δ@3720**, and the whole app is Δ@1860. `idlit` did
   not change, so the fixed part moved 2 × (−23) − (+37) = **−83**, the
   whole app −23, and B5 (37 + 23) / 1,860 = +0.032.
2. **The delivered bytes.** The variant's `tr` wasm is **79 B smaller at
   both scales**, as a fixed change should be. The code section is −78 B
   and the function section −1 B: one function fewer (9,009 → 9,008;
   15,924 → 15,923). The data section (124,732 B), the element section and
   every other section are the same size, so no string constant came in (a
   reachable `Debug` impl would bring its names). Command:
   `python3 target/a5/analysis/sizes.py BASE VARIANT` over the kept
   `opt.wasm` files.
3. **Why gzip reads −23 at one scale and +37 at the other.** With one
   function fewer, every function after it is renumbered. At 1,860 sites
   272 bodies (1,394,833 B, 62 % of the code) differ in their bytes; at
   3,720 sites 602 (2,763,348 B, 66 %) do. In each case all but the seven
   below keep their length (`python3 target/a5/analysis/bodies.py BASE
   VARIANT`, with call targets normalised). The compressor's matches change
   with them. The same files read differently with each compressor:

   | | raw | flate2 level 9 (the gate) | `gzip -9 -n` | `brotli -q 11` |
   |---|---:|---:|---:|---:|
   | `tr` at 1,860 sites | −79 | −23 | −30 | −130 |
   | `tr` at 3,720 sites | −79 | +37 | +127 | −473 |
   | **the fixed part** (2·δ₁ − δ₂) | −79 | **−83** | −187 | +213 |
   | `tr-view` at 1,860 / 3,720 sites | +3 / +3 | +2 / +2 | +7 / 0 | −273 / −759 |

   (`gzip -9 -n -c F | wc -c` and `brotli -q 11 -c F | wc -c` over
   `target/a5/{base,variant}/wasm/…/opt.wasm`; the gate's column is
   `cargo xtask size`'s.)
4. **Which code.** At the function level the `tr` change is **seven bodies
   (717 B) replaced by six (639 B)**, the same sets at both scales, and all
   of them destructors. Method: `wasm-dis` over the measured `opt.wasm`,
   then `python3 target/a5/analysis/match.py`, which normalises every
   index, name and constant and names each changed function by its text in
   the named builds.
   * `drop_glue::<ArgValue>` 162 → 165 B; `Arc<DateTimeValue>::drop_slow`
     105 → 66 B.
   * `drop_glue::<Text>` (141 B) and `drop_glue::<Arc<str>>` (126 B) are
     functions of their own in the variant. Both named builds have them;
     the measured base does not.
   * Three base bodies (171, 98, 94 B) and one variant body (132 B) have no
     twin in the named builds.
   * One copy fewer of two shared drop shapes (the text of
     `drop_glue::<SplitLoaderFuture>`, 50 B; a generic
     `drop_glue::<Arc<…>>`, 37 B), and one copy more of a 9 B `Weak::drop`
     shape.

   In `tr-view` one function changed: `drop_glue::<ArgValue>`, 162 → 165 B.
   It is the same `Arc` release, written with `ptr + 4` held in a local and
   an `if` in place of a `br_if`. So the moved `to_string()`, which
   `tr-view`'s string positions call, compiles to the same code.
5. **Where that code runs** (`twiggy paths -d 3 -r 12` on the named base
   `tr`). `Arc<dyn ArgSource>::drop_slow` and `Arc<DateTimeValue>::drop_slow`
   are reached only through `drop_glue::<[ArgValue]>` and
   `drop_glue::<ArgValue>`, from the template's one formatting helper
   `support::s::<TrArgs>` and from
   `tr::with_args::<(), TrArgs::write::{closure}>`. That is the destructor
   of a `TrArgs` after it has been formatted: one copy per application,
   hence the same −79 B at both scales.

Interpretation (brief; not established): the variant adds non-generic items
that rustc compiles whether or not anything calls them. `From<TrArgs> for
String` and its siblings take a description by value and drop it, and the
`Debug` impls walk `ArgValue`. Fat LTO removes them as unused, but the
destructors they share were optimised while they were present, and were
inlined differently. The named builds (`strip = false`, `--debuginfo`) show
−160 B where the measured show −79, which fits an effect that depends on
layout.

### What twiggy sees

* **Nothing of ours is `Display`, `Debug` or `core::fmt`** in the variant's
  three named builds (`bash target/a5/twiggy.sh variant`;
  `bash target/a5/ours.sh target/a5/named/variant/size-wl-1860-tr`). The
  items that match B12's fmt pattern are **identical in name and size** to
  the base's, once binaryen's `.N` suffixes are dropped: `tr` 72 items /
  10,354 B, `tr-view` 72 / 10,354 B, the fixture 32 / 3,272 B. All of them
  belong to `core`, `alloc`, `std` and Leptos's stack (`hydration_context`,
  `leptos`, `wasm_bindgen`, `wasm_split_helpers`, `async_once_cell`). The
  list of every `Display` / `Debug` impl in `tr` is the same in both.
* **`twiggy diff`** on the named builds
  (`twiggy diff -n 60 target/a5/named/base/X/opt.wasm target/a5/named/variant/X/opt.wasm`):
  * **`tr`, +53 B:** the name section +213 and the code −160, all
    destructors. `Arc<dyn ArgSource>::drop_slow` −96;
    `Weak<dyn ArgSource>::drop` −89 against `Weak<dyn SharedContext>::drop`
    +89 (one merged body under another name);
    `Arc<DateTimeValue>::drop_slow` −39;
    `drop_glue::<Arc<oneshot::Inner<Option<Owner>>>>` −39;
    `Weak<oneshot::Inner<…>>::drop` +11; `drop_glue::<ArgValue>` +3.
  * **`tr-view`, +2 B:** `drop_glue::<ArgValue>` +3; a 57 B body renamed
    from `drop_glue::<TrArgs>` to `drop_glue::<ArgList>`; names −1.
  * **The fixture, +3 B:** renamed duplicates (`.165` → `.170`, present in
    both builds), and destructors of `ArgValue`, `Text`, `Arc<str>` and
    `Arc<dyn ArgSource>` moving by ±5 to ±19 B.
* **The named builds are not the measured ones.** With their names stripped
  (`wasm-opt --strip-debug --strip-producers`), named `tr` goes 2,422,903 →
  2,422,743 (−160) where the measured goes 2,419,654 → 2,419,575 (−79);
  `tr-view` is +3 in both. That is why the measured files were compared
  function by function above.
* **The check can fail (positive control).** The fixture client was built
  with one `format!("{save} {items:?}")`, where `save` is a `Tr` and
  `items` a `TrArgs` (a working-tree edit in `target/a5-dev`, reverted and
  never committed).
  * The named build (`target/a5-dev/target/a5/control.sh`, then
    `twiggy.sh`'s fmt grep) has **79 fmt items (20,175 B)** against 32
    (3,272 B). **11 are ours**: `<Tr as Display>::fmt`; `Debug` for
    `TrArgs`, `ArgList`, `ArgValue`, `Text`, `Option<Text>` and
    `Arc<DateTimeValue>`; `mf2-runtime`'s `Date` and `Time`.
  * Built as `b12-generated` builds its A (`control-stripped.sh
    plain|control`): **365,001 → 405,860 B raw; after `wasm-opt -Oz`
    328,579 → 364,332; `gzip -9` of that 87,371 → 103,975 (+16,604 B)**.

### Lints and tests

`bash target/a5/clippy-variant.sh` (in `target/a5-dev`, 2026-09-28; log:
`target/a5/clippy-variant.log`) ran two kinds of step:
* every `cargo xtask ci` clippy step that compiles `leptos-mf2`:
  `--workspace --all-targets`, the nine `wasm32-unknown-unknown` feature
  sets, and `ssr,mark-fallback-lang --all-targets`;
* two steps CI lacks: the Leptos-free core,
  `-p leptos-mf2 --no-default-features --lib`, on the host and on wasm32.

**All exit 0** with `-D warnings`, and none reports a lint in our crates.
The only warning is the future-incompatibility note for the dependency
`proc-macro-error2` 2.0.1. `cargo test -p leptos-mf2 --features ssr --test
render`: 15 passed, the new test among them. The earlier session's
`target/a5/dev-clippy-workspace.log` holds only `exit 0`. Several of the
steps were already fresh in `target/a5-dev/target`, and cargo replays a
fresh unit's warnings.

* **`Debug` coverage**
  (`cargo clippy -p leptos-mf2 --features ssr -- -W missing_debug_implementations`,
  and the same with `--target wasm32-unknown-unknown --features hydrate`;
  outputs in `target/a5/missing-debug-{ssr,hydrate}.txt`). In `leptos-mf2`
  only the glue's tachys render states still lack `Debug`: `TrState`,
  `TrAttrState`, `TrRichState`. Outside this probe, 13 public types of
  `mf2-runtime` lack it (`Arg`, `Value`, `Part`, `ExpressionPart`,
  `MarkupPart`, `MarkupOptions`, `FnContext`, `Options`, `OptionValue`,
  `Number`, `Digits`, `Measure`, `NumberOut`), and 18 of `mf2-catalog`
  (`Entry` and the views).
* **Not run:** `cargo xtask ci` as a whole. `cargo xtask api --check` would
  fail on the branch (inferred, not run): `crates/leptos-mf2/api.txt`
  lists trait impls (`impl core::fmt::Debug for leptos_mf2::Tr`, line 262),
  and the commit adds `Display` and `Debug` impls without regenerating it.

### Verdict

* **Adopted**:
  * `Display` for the four descriptions;
  * the always-on inherent `to_string` / `to_plain_string`, with the
    `inherent_to_string_shadow_display` allow and B12 as its reason;
  * `Debug` on the call-site types, hand-written for `Custom` / `Source`.

  The fallback ("`Display` only with the std modes") is not needed.
* **B1: −83 B gz, outside the ±64 B band, downward. Judged as holding.** The
  band is there to catch a cost, and this move's cause is identified:
  * the delivered code is 79 B smaller at both scales, all of it in the
    destructors of the argument values;
  * no `Display`, `Debug` or `core::fmt` code, and no new data, reached the
    client;
  * the −83 is the fixed part's extrapolation (2 × −23 − 37) of two gzip
    readings of opposite sign.

  The whole app at the reference scale (−23 B gz) and B5 (+0.032 B a site)
  are inside their tolerances, and `b5 --view` moved +2 B fixed and 0 a
  site. The −79 B is not a saving to bank: it is the same destructors
  inlined differently, and the next change to the crate can move it back.
* **B12: clean.** `check.sh` is clean and byte-identical in both runs, but
  its harnesses do not compile `leptos-mf2`, so it cannot see this change.
  The twiggy check over the workload builds does see it; it finds nothing
  of ours, and its positive control shows it would. B1′ is +0 and B13
  13,573 B in both.
* **Done when**: met. Nothing needs the owner; one trade-off is left for
  A8 below.

### What it means for C2 (interpretation)

* **Take the probe's API, not its stand-in.** Without a Leptos mode the
  probe's `to_string()` returns `""` and `Display` writes nothing. If that
  shipped it would be a new silent failure, of the kind Part E removes. C2
  backs both with the native store's lookup (A4: formatting before
  `install()` panics and names it); a build with neither mode should not
  offer them.
* **Use A4's shape for the native forms.** The probe's `Display` is generic
  per description (`ambient::fmt<D>` → `text::fmt_display<D>`). On the web
  client that costs nothing, because nothing links it. A4 measured
  per-type `Display` impls at +3,784 B in a stripped CLI, and chose one
  non-generic forwarder taking a trait object. The inherent `to_string`
  stays (A4: faster, and one allocation against `ToString`'s two).
* **What adopting gives up.** In 1.x, and under the fallback,
  `format!("{}", tr!(…))` in a browser build does not compile. With
  `Display` always on it compiles and pays for `core::fmt`: the control's
  one `{}` and one `{:?}` cost +16.6 KB gz after `wasm-opt`. B12 thus
  moves from a type error to a rule applications must follow.
  `tr.to_string()` stays fmt-free, because method resolution picks the
  inherent method, and the book (F) should tell browser code to use it.
  The work order's rule decides on bytes alone; A8 may want to put this
  trade-off to the owner.
* **`Debug` in 2.0.** A description's `Debug` shows `MsgId(0)`, the number:
  the description carries no name (B6). A8 should state whether 2.0's
  "`Debug` everywhere" covers the runtime's and the catalog's public types
  (31 lack it). `check.sh` does see `mf2-runtime`, so adding them there is
  B12-checked directly.
* **Reading a ±64 B size gate** (B1 and every later size gate). The fixed
  part is an extrapolation, 2·δ₁ − δ₂, so it doubles gzip's noise: the same
  −79 B raw read −83 B with flate2, −187 with `gzip -9` and +213 with
  brotli. Before reaching for a fallback, read a move outside ±64 B
  alongside the raw bytes at both scales (a fixed change is equal at both)
  and a function-level diff.
* **The adopting change** regenerates `crates/leptos-mf2/api.txt`, and any
  other listing that shows these impls, with `cargo xtask api`.

## A6 — a single-crate web application: what was built

**The probe.** `probes/p10-single-crate/hello/`: Getting started's `hello`
in its lazy-route form (delivery-modes.md), as **one crate** — `build.rs`,
`mf2.toml` and `locales/` in the application, no translation crate, no
workspace members. The 1.x crates by path (the tree at `2fb7f54`); Leptos
0.9.0-beta2; cargo-leptos 0.3.9. `build.rs` runs `mf2-build` (`Emit::Both`)
and swaps the generated wrappers for A3's shape (3c), standing in for what
2.0's codegen would emit. `src/lib.rs` includes the module and keeps 1.x's
hand-shaped `setup()`. `tr!` is called from `src/pages.rs` (declared before
the include; `use crate::prelude::*`), `src/app.rs` (after it; `use
crate::tr`), the crate root (no import) and the server binary `src/main.rs`
(another crate; `use hello::tr`). Only `fn-number` is on (the corpus uses
`:integer`; hello's `fn-datetime` / `datetime-icu` were left off to spare the
machine). Everything ran under load (load average 12–18, other forks
building): times are indicative only.

| Check | Result | Observed | Command |
|---|---|---|---|
| both builds | **PASS** | `cargo check --features ssr` (lib + server binary); `cargo leptos build`: the server (`--features=ssr`) and the wasm (`--lib --target=wasm32-unknown-unknown --features=hydrate`), first build 14 min 21 s cold at `CARGO_BUILD_JOBS=1` | `results/build-first.log` |
| a translation edit leaves the wasm byte-identical | **PASS** | wasm `4758729623dccb0e` at every step: nothing changed, mtime only, a translation-only edit (fr `visit-again`), a source-text edit (en `visit-again`), each reverted; `MANIFEST_HASH` `0x445b_1e6d_8cf8_c8e0` throughout. The server binary changes with each text edit (its catalogs are embedded) and returns to `59af98f49826615d` on each revert. Each edit recompiles the one crate in both builds (8–21 s a step) — in the two-crate layout it is the i18n crate *and* the application | `./scenario.sh` → `results/scenario.txt` |
| `--split` | **PASS** | `cargo leptos build --split` emits `split___visits_view_….wasm` for the lazy route. Served (`cargo leptos serve --split`) and opened in Chromium: `/?lang=fr` renders in French with `Content-Language: fr` and `Vary: cookie, accept-language`; clicking the link to `/visits` navigates client-side (no document request) and fetches the chunk; the button, a switch to English and "Apply" fetch only `/i18n/en.….mf2b` and give "You have been here 2 times." (count kept, title switched). Console: only the probe's missing favicon | `results/browser/README.txt` |
| `cargo leptos watch`: is `watch-additional-files` still needed? | **still needed** | Without it: no restart within 240 s of a locale edit, while a control edit to `src/app.rs` restarted it 27 s later. With `watch-additional-files = ["locales"]`: restarted 18 s after the edit, the edit served, then the revert served | `./watch.sh without`, `./watch.sh with` → `results/watch-*.txt` |
| rust-analyzer | **PASS** | `rust-analyzer diagnostics .` (with `default = ["ssr"]` for the run; 156 s): the seeded misspelt id before the include (`unknown message id vistis; did you mean visits?`) and the seeded wrong argument after it (`message greeting has no variable $nmae …`) reported at the call sites; no `unresolved-macro-call` / `unresolved-proc-macro`. One unrelated error: an `E0507` rust-analyzer reports in 1.x `leptos-mf2`'s `text.rs:191` (`*slot = buf` in a `LocalKey::with` closure), which rustc compiles | `results/ra-diagnostics.txt` |
| A3's in-crate `tr!` from several modules | **PASS** | before the include through the prelude, after it by `use crate::tr`, at the root with no import, and from the server binary by `use hello::tr`; in `ssr`, `hydrate` and `--split` builds | the builds above |

**Verdict.** **One crate for the web works**: both builds, the wasm
byte-identical across translation edits, `--split`, hydration and a live
switch, rust-analyzer, and `tr!` from every module. The web starter (D5) can
offer it. **Done when:** met.

### What the one-crate layout still needs (observed)

- **`watch-additional-files = ["locales"]`** — cargo-leptos watches the
  sources, not `locales/`, in one crate as in two. A one-crate starter
  writes the line itself: the manifest is the application's own, which 1.x's
  `mf2 init` could not assume.
- **The function features, twice.** 1.x's build reads the *including*
  crate's `CARGO_FEATURE_*`, so the application declares `fn-number =
  ["mf2/fn-number"]` and turns it on from both `ssr` and `hydrate` (and
  `mf2/host-std` + `mf2/ssr`, `mf2/host-web` + `mf2/hydrate`) — A2's `links`
  metadata is what removes this.
- **`mf2 check` sees none of those features.** In the directory,
  `mf2 check` resolves the crate with cargo's defaults, where neither `ssr`
  nor `hydrate` is on, so it warns `neutral-numbers` for each locale (a
  false warning: both builds have `fn-number`); `mf2 check --features
  fn-number` reports nothing. In the two-crate layout the application names
  the features on the i18n crate's dependency, which the plain resolve does
  see.
- **`setup()` is still hand-written** in `src/lib.rs` (D3's generated setup
  removes it).

### What it means for the design (A8, C6, D3, D5)

- D5: `mf2 init --ssr` / `--islands` / `--csr` can offer the one-crate
  layout, writing `watch-additional-files = ["locales"]`; the two-crate
  layout stays for a workspace where several crates share one corpus.
- C6: `mf2 check` / `compile --site` must see the features the *builds*
  use. If 2.0 puts the function features on the `mf2` dependency itself
  (unconditionally, not behind `ssr` / `hydrate`), `mf2`'s node in a plain
  resolve carries them and the gap closes; otherwise `mf2 check` needs
  cargo-leptos's `bin-features` / `lib-features`.
- A3's shape (3c) needs nothing more on the web: the prelude and `use
  crate::tr` work in `ssr`, `hydrate` and a split chunk.

## A7 — names and coherence: what was built

* **Where.** The probe branch `p10-a7-names` (worktree
  `.claude/worktrees/agent-aaa2ebae2e252a3e5`, off `2fb7f54`; not merged,
  and kept for B1). On `main`: `probes/p10-names/`, copied from the branch,
  with the branch's crate changes as `probes/p10-names/helper-crates.patch`. Two sessions: the first built and checked the branch on 0.9 and
  was stopped by the session limit; the second finished the measurements,
  the 0.8 line and this record.
  * `b258474` — the six components in per-line helper crates behind a table
    (**v1**: `install()` installs the table, the client switch inside it);
  * `7dbcfd8` — the client switch out of the table, into a `LocaleSwitcher`
    wrapper (**v2**);
  * `9c945ed` — each component installs the table, not `install()` (**v3**,
    the branch's form);
  * `f54cf6d` — `probes/p10-names/coherence.sh`;
  * `6014948` — the scripts behind this record (`probes/p10-names/`
    `demos-0-8.py`, `measure-demo.mjs`, `twiggy-norm-diff.py`, `README.md`).

  Scratch, in that worktree's git-ignored `target/`: `a7-notes.md` (first
  session), `a7-*.log`, `a7-demo-size/` (the demos' A/B: every shipped
  `pkg`/`dist`, `named/` with the twiggy tables), `a7-demo-0-8/` (the 0.8
  copies and their e2e logs).
* **The helper crates.** `crates/mf2-leptos-ui-0-9` owns `src/ui.rs`: the six
  components, written with `view!`, `#[component]` and `#[prop]` as in 1.x.
  `crates/mf2-leptos-ui-0-8/src/ui.rs` is a symlink to it. The 0.8 crate
  binds `extern crate leptos_0_8 as leptos;` — legal there, because that crate
  has no module named `leptos`. Each crate depends on its own line, on
  `mf2-catalog` (for `Dir`) and, for `hydrate` and `csr`, on
  `js-sys` / `wasm-bindgen` / `web-sys`. Its `ssr` / `hydrate` / `csr`
  features are exclusive (`compile_error!`). `forbid(unsafe_code)`; clippy's
  `unwrap_used`, `expect_used`, `indexing_slicing` and `panic` are denied.
  `publish = false` on the branch. `cargo package -p mf2-leptos-ui-0-8 --list
  --allow-dirty` lists `src/ui.rs`, so the link is packaged as a file.
* **No root rename.** Both lines are dependencies under names of their own
  (`leptos_0_9`, `tachys_0_3`, `reactive_graph_0_3`, `leptos_axum_0_9`
  beside the existing `*_0_8`), in `[workspace.dependencies]`. `leptos-mf2`
  reaches the active line through a private module:

  ```rust
  #[cfg(feature = "leptos")]
  mod line {
      #[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
      pub(crate) use ::{leptos_0_8 as leptos, mf2_leptos_ui_0_8 as ui,
          reactive_graph_0_2 as reactive_graph, tachys_0_2 as tachys};
      #[cfg(feature = "leptos-0-9")]
      pub(crate) use ::{leptos_0_9 as leptos, mf2_leptos_ui_0_9 as ui,
          reactive_graph_0_3 as reactive_graph, tachys_0_3 as tachys};
  }
  ```

  Each module imports what it names (`use crate::line::{leptos, tachys};`),
  so the bodies are unchanged. `mf2-axum` does the same
  (`line::{leptos, leptos_axum}`). At `2fb7f54` the crate roots renamed three
  crates (`leptos-mf2`) and two (`mf2-axum`); on the branch the only
  `extern crate … as leptos` is in `mf2-leptos-ui-0-8/src/lib.rs`. The
  integration tests bind the line themselves (`#[cfg(feature =
  "leptos-0-9")] extern crate leptos_0_9 as leptos;`).
* **The function table** (v3). The helper defines `pub struct Table`;
  `leptos-mf2` fills it as a `static` struct literal (its `ssr` fields are
  `cfg`'d, which a constructor's arguments could not be):

  | Field | Type | Filled with (`leptos-mf2`) | Modes |
  |---|---|---|---|
  | `locales` | `fn() -> &'static [(&'static str, Dir)]` | `state::locales` | all |
  | `source_locale` | `fn() -> &'static str` | `state::source_locale` | all |
  | `is_current` | `fn(&str) -> bool` | the active catalog's locale is `tag` (no catalog: the source locale) | all |
  | `locale_query` | `&'static str` | `links::LOCALE_QUERY` | all |
  | `islands_gate` | `&'static str` | `links::ISLANDS_GATE` | all |
  | `preload` | `fn() -> Option<(String, Option<String>)>` | the page catalog's URL and the reader's zone | `ssr` |
  | `catalog_links` | `fn() -> Vec<(&'static str, String)>` | every locale's catalog URL | `ssr` |
  | `catalog_link_rel` | `&'static str` | `links::CATALOG_LINK_REL` | `ssr` |

  * **Outside the table:** the client's switch, a `OnceLock<fn(String)>` in
    the helper (`install_switch`, `hydrate` / `csr` only).
  * **Who installs it:** each of the six, re-exported by `leptos-mf2` as a
    plain `fn X(props: XProps) -> impl IntoView` (not `#[component]`),
    calls `ui::install(&TABLE)` — a `OnceLock<&'static Table>`; later calls
    are a check — and returns `ui::X(props)`. `LocaleSwitcher` also installs
    the switch. The wrappers take the helper's own `*Props` (re-exported),
    so `view!` builds them exactly as it built 1.x's components.
    `install()` installs nothing, as in 1.x.
  * **Reading it:** the helper reads the table through `table() ->
    Option<&'static Table>`. With nothing installed, every component renders
    as with no languages, and nothing panics.
* **`mf2::leptos`**, a stand-in for 2.0's module: `#[cfg(feature =
  "leptos")] pub mod leptos { pub use leptos_mf2::{AlternateLinks,
  CatalogLinks, CatalogPreload, IslandsGate, LocaleOption, LocaleSwitcher,
  Setup, html_lang, install}; }`. `examples/demo-ssr` and `demo-islands`
  import the components from it; `demo-csr` still uses `leptos_mf2::…`.
* **The Ratatui probe** (`leptos-mf2`'s `ratatui` feature,
  `src/ratatui_probe.rs`). For `Tr`, `TrArgs`, `TrRich` and `TrDyn`:
  `From<_>` for `Span<'static>`, `Line<'static>` and `Text<'static>`;
  `Widget`; `Styled<Item = Line<'static>>`; `Display`. The bodies are
  placeholders: what is under test is that the impls coexist with the Leptos
  glue. The `probe-cow`, `probe-fromiter`, `probe-cell` and
  `probe-listitem` features add one forbidden impl each.
* **`probes/p10-names/`** (`cargo check` only; its `README.md` lists every
  file):
  * `naming` — a stand-in `mf2` with `pub mod leptos` and `pub mod axum`
    beside those crates;
  * `naming-n2` — the 0.9 line under its real name;
  * `naming-app` — an application beside both;
  * `coherence` — what a Ratatui application writes, with Leptos on too;
  * the scripts `controls.sh`, `coherence.sh`, `demos-0-8.py`,
    `measure-demo.mjs` and `twiggy-norm-diff.py`.

### Why three forms

Measured with `b5 --view`, below. **v1** put the client switch in the table,
and `install()` — which every application calls — installed it. The
`tr-view` client's fixed part grew **+4,407 B gz**: the switch reaches the
catalog fetch, the registry's walk, the cookie and the address. **v2** moved
the switch to the `LocaleSwitcher` wrapper, and the rest of the table stayed
in `install()`: **+249 B gz**. **v3** installs nothing from `install()`, so
an application that renders none of the six links neither the table nor
what it points at.

### The checks, per Leptos line

Tools: rustc 1.98.1, cargo-leptos 0.3.9, trunk 0.21.13, wasm-bindgen 0.2.128
(CLI; the library 0.2.129 in the demos' locks), wasm-opt 120, twiggy 0.8.0,
Node 23.11, Playwright 1.63.0 (Chromium and Firefox). The worktree's lock:
leptos 0.9.0-beta2 / 0.8.21, tachys 0.3.0-beta3 / 0.2.19, reactive_graph
0.3.0-beta3 / 0.2.15, leptos_axum 0.9.0-beta2 / 0.8.10, ratatui 0.30.2
(the probe's lock), ratatui-core 0.1.2, ratatui-widgets 0.3.2, axum 0.8.9.

**Which commit each ran on.** No tracked file in the worktree changed after
06:46:56, and v3 was committed at 06:47:38 (`git reflog`; file mtimes).
Every run of the first session that finished after 06:47 therefore ran on
v3's content: the 0.9 e2e checks (07:02–07:16), `l6-web` (07:13),
`l7-web` (07:29), `leptos-0-8` (06:50), clippy (06:47) and the tests
(06:48). The base measurements (`size` 05:51, `b5 --view` 05:55:13) finished
before the first edits were applied (05:55:20); until then the helper
crates were untracked files outside the build. The second session re-ran
clippy, the tests, `leptos-0-8`, the naming and the coherence checks at
`f54cf6d`; each was unchanged, and every unit was fresh.

| Check | Leptos 0.9 | Leptos 0.8 | Observed | Command (log, under the worktree's `target/`) |
|---|---|---|---|---|
| clippy `-D warnings`, CI's `leptos-mf2` set (`ssr,mark-fallback-lang` all targets; wasm32: `hydrate`, `hydrate,static-locale`, `csr`, `csr,static-locale,mark-fallback-lang`, `hydrate,fn-datetime,static-locale,mark-fallback-lang`, `csr,fn-datetime`), `mf2-axum` and `mf2` all targets | **PASS** | **PASS** (the xtask's five clippy steps) | every step `Finished`, no warning | 0.9: `bash target/a7-draft/lint.sh` (`a7-lint-head.log`); 0.8: `cargo xtask leptos-0-8` |
| clippy `-D warnings` on each helper crate itself, `ssr`, and `hydrate` / `csr` on wasm32 | **PASS** (`mf2-leptos-ui-0-9`) | **PASS** (`mf2-leptos-ui-0-8`) | six runs, `Finished`, no warning | `cargo clippy -p mf2-leptos-ui-0-{9,8} --features <mode> [--target wasm32-unknown-unknown] -- -D warnings` (`a7-lint-helpers-head.log`) |
| tests: `leptos-mf2` `render` 14 (the switcher's markup: a `GET` form, `select name="lang"` inside its label, the page's option `selected`), `time_zone` 8 (the preload link and its zone), `fallback_lang` 8, `churn` 1; `mf2-axum` 13 + 4 (`time_zone` renders `CatalogPreload`) | **PASS** | **PASS**, the same counts | `test result: ok` for each | 0.9: `bash target/a7-draft/test09.sh` (`a7-test09-head.log`); 0.8: the xtask |
| `cargo xtask leptos-0-8` — the tree's own 0.8 check, the nightly job `leptos-0-8` | — | **PASS** | both lines at once refused: "leptos-mf2: \`leptos-0-8\` is on, and so is the default \`leptos-0-9\`. For Leptos 0.8, every dependency on leptos-mf2 and mf2-axum needs \`default-features = false\` beside \`features = ["leptos-0-8"]\`."; the clippy and test steps above; conformance `layers` 4 and `l6` 3 | `CARGO_BUILD_JOBS=2 cargo xtask leptos-0-8` at `f54cf6d` (`a7-leptos-0-8-head.log`; 7.5 s, every unit fresh from `a7-leptos-0-8-v3.log`) |
| e2e `demo` | **210/210** (first session) | **210/210** | Chromium 105, Firefox 105 each: the preload link and the catalog from it, hydration changes no text, a live switch reaches text, attributes, `<title>` and `<html lang dir>`, the path-prefix switcher with and without the wasm, no message text in the client | `node run.mjs demo --base-url http://127.0.0.1:3702 --browser chromium,firefox`: 0.9 `a7-e2e-demo-09.log`; 0.8 `a7-demo-0-8/e2e-demo-08.log` |
| e2e `lazy` (`--split`) | **74/74** | **74/74** | the chunk fetched on navigation only; a direct `/lazy` load hydrates through `hydrate_lazy` and switches live | `node run.mjs lazy …`: `a7-e2e-lazy-09.log`; `a7-demo-0-8/e2e-lazy-08.log` |
| e2e `islands` | **58/58** | **58/58** | the gate the first island (`mf2_islands_gate`, the table's name), the switcher a `GET` form and not an island, no island hydrates before the catalog; the control without the gate traps | `node run.mjs islands --base-url http://127.0.0.1:3704 …`: `a7-e2e-islands-09.log`; `a7-demo-0-8/e2e-islands-08.log` |
| e2e `csr` | **98/98** | **98/98** | the 0.8 run served the 0.8 copy's `dist` (it fetched `demo-csr-7cebfc3c1dabd481_bg.wasm`, not the 0.9 build's `…44f34a8b…`) | `node run.mjs csr …`: `a7-e2e-csr-09.log`; `a7-demo-0-8/e2e-csr-08.log` |
| e2e `zone` | **40/40** (second session) | **40/40** | in `America/New_York` and `Asia/Kolkata`: a reload is served in the reader's zone and the preload link states it (the table's `preload`) | `node run.mjs zone --base-url http://127.0.0.1:3702 …`: `a7-e2e-zone-09.log`; `a7-demo-0-8/e2e-zone-08.log` |
| e2e `a11y` (28 pages of the three demos) | **720/720** | **720/720** | axe clean, the switcher's keyboard behaviour, and every negative control failing as designed | `MF2_ISLANDS_URL=http://127.0.0.1:3704 node run.mjs a11y --base-url http://127.0.0.1:3702 …`: `a7-e2e-a11y-09.log`; `a7-demo-0-8/e2e-a11y-08.log` |
| `cargo xtask l6-web` | **20/20** | not run: no 0.8 mode (L6 on 0.8 runs natively inside `leptos-0-8`) | "every engine hydrated the page and switched locale" | `a7-l6-web.log` |
| `cargo xtask l7-web` | **34/34** | not run: no 0.8 mode | L7 444/444, L7c 444/444, L7d 325/444 (+119 documented degradations), L7cd 325/444 (+119); "the ledger's L7 columns hold in chromium, firefox" | `a7-l7-web.log` |
| no Leptos procedural macro outside the six | **PASS** | **PASS** (one source) | 6 `#[component]`, 7 `view!`, 3 `#[prop]`, all in `mf2-leptos-ui-0-9/src/ui.rs`; none in `leptos-mf2`, `mf2-axum` or `mf2`. At `2fb7f54` the same 6 / 7 / 3 were in `leptos-mf2/src/components.rs` | `rg` over the four crates' `src/`, comment lines excluded; `git grep … 2fb7f54` |
| naming (`probes/p10-names`) | **PASS** | **PASS** | `cargo check --workspace` (`naming` with `leptos-0-9,axum`, `naming-n2`, `naming-app`, `coherence`) and `-p naming --no-default-features --features leptos-0-8,axum`: `Finished`; N1–N5 below | `bash probes/p10-names/controls.sh` (`a7-naming-head.log`, `a7-naming-errors-head.log`) |
| coherence, `leptos` and `ratatui` both on | **PASS** | not run (the impls do not depend on the line) | `leptos-mf2 --no-default-features --features ratatui`; `--features ssr,ratatui`; `--features hydrate,ratatui --target wasm32-unknown-unknown`; `coherence` for `ssr` and for `hydrate`: `Finished`. The four controls fail with E0119 (below) | `bash probes/p10-names/coherence.sh` (`a7-coherence-head.log`, `a7-coherence-errors-head.log`) |

**The 0.8 line, exactly.**
* **The tree's own check.** The tree builds the demos on 0.9 only. Its 0.8
  check is `cargo xtask leptos-0-8` (`.forgejo/workflows/nightly.yml`, job
  `leptos-0-8`; phase 8 A0). The one 0.8 application build in the tree is
  `cargo xtask fluent-ab`: its mf2 side renders `CatalogPreload` and
  `CatalogLinks`. It is a snapshot benchmark that builds the 1,860-site
  reference application several times, and was not run.
* **The first session's failure is not A7's.** Its `demo-ssr` 0.8 copy
  (`target/a7-draft/demo08.py`) fails identically at `2fb7f54`:
  `CARGO_BUILD_JOBS=1 cargo leptos build --split` in the copy
  (`a7-demo-0-8/demo-ssr-08-base-orig.log`) gives "package \`demo-ssr\`
  depends on \`leptos\` with feature \`lazy\` but \`leptos\` does not have
  that feature". `cargo info leptos@0.8.21` lists no `lazy` feature. The
  demo turns it on for 0.9's lazy-route hydration.
* **The browser checks on 0.8.** `python3 probes/p10-names/demos-0-8.py head`
  copies the three demos with:
  * the Leptos crates at 0.8, and `leptos-mf2` / `mf2-axum` with
    `default-features = false, features = ["leptos-0-8", …]`;
  * `demo-ssr` without `leptos/lazy`;
  * `demo-csr`'s trunk hook pointed at the worktree's manifest;
  * a copy of the harness beside them.

  Then `CARGO_BUILD_JOBS=1 cargo leptos build [--split]` or `trunk build`,
  each server on its own port, and the checks from the copied harness.
  `cargo tree` shows leptos 0.8.21, tachys 0.2.19, reactive_graph 0.2.15,
  leptos_axum 0.8.10, leptos_router 0.8.16, leptos_meta 0.8.7 and
  `mf2-leptos-ui-0-8`, with no 0.9 crate, in each server and client build.
  All six checks passed, as in the table.

### Sizes

**B1, the size gate** (`tr` against `idlit` and `dummy`, at 1,860 and 3,720
sites). The workloads were generated once at base, and every form was
measured in the same tree with the same per-app locks:
* base: `CARGO_BUILD_JOBS=2 cargo xtask size --out target/size-base`;
* each form after it: the same, with `--keep`.

| Form | `tr` opt gz, 1,860 / 3,720 | B1 fixed | B5 a site | whole app, 1,860 sites | Log |
|---|---:|---:|---:|---:|---|
| base `2fb7f54` | 693,671 / 1,203,351 | **26,402 B gz** | 8.3 | 41,861 | `a7-size-base.final.log` |
| v1 | identical | 26,402 | 8.3 | 41,861 | `a7-size-v1.log` |
| v2 | identical | 26,402 | 8.3 | 41,861 | `a7-size-v2.log` |
| v3 | identical | **26,402** | 8.3 | 41,861 | `a7-size-v3.log` (second session: `leptos-mf2` and both `tr` apps rebuilt at 13:02–13:05; the apps' lock `a1f1bcb3…` unchanged) |

Every stage is identical in all four runs: bindgen gz, opt raw and opt gz.
The `tr` template has no Leptos layer. In its app, `cargo tree --locked
--offline -e features -i leptos-mf2 --target wasm32-unknown-unknown
--no-default-features --features hydrate` shows `leptos-mf2` with no
feature on, and no `mf2-leptos-ui-*` crate in the graph. So v3 did not
strictly need the run; it was run to have the figure.

**`b5 --view`** (`tr-view`, where a description renders itself, against
`idlit-view`). Base: `CARGO_BUILD_JOBS=2 cargo xtask b5 --view --out
target/b5v`; each form after it: the same, with `--keep` (the logs say
"reusing"). `idlit-view` (528,251 / 892,759) and `dummy` were identical in
all four runs.

| Form | `tr-view` opt gz, 1,860 / 3,720 | opt raw Δ, both scales | fixed | Δ fixed | marginal a site | Δ marginal | Log |
|---|---:|---:|---:|---:|---:|---:|---|
| base | 572,533 / 956,400 | — | 24,923 | — | 10.408 | — | `a7-b5v-base.final.log` |
| v1 | 577,182 / 961,291 | +11,102 / +11,094 | 29,330 | **+4,407** | 10.538 | +0.130 | `a7-b5v-v1.log` |
| v2 | 572,817 / 956,719 | +898 / +890 | 25,172 | **+249** | 10.427 | +0.019 | `a7-b5v-v2.log` |
| v3 | 572,543 / 956,470 | +93 / +93 | 24,873 | **−50** | 10.440 | +0.032 | `a7-b5v-v3.log` |

Fixed and marginal are the xtask's difference of differences. The marginals
are recomputed from the logged sizes; the xtask prints them to one decimal
(10.4 / 10.5 / 10.4 / 10.4). v3's whole-app change is +10 B gz at 1,860
sites and +70 B gz at 3,720. **Against the gate (±64 B gz fixed,
±0.2 B a site), v3 meets it in the size workloads.** Neither workload
renders any of the six components.

**The demos: the table in applications that use the components.** Measured
in the one worktree, with each demo's own `Cargo.lock` kept. The steps:
1. At `f54cf6d`: `CARGO_BUILD_JOBS=1 cargo leptos build --release
   [--split] --frontend-only --cargo-offline` in `examples/demo-ssr`
   (`--split`, the README's size command) and `examples/demo-islands`, and
   `CARGO_BUILD_JOBS=1 trunk build --release` in `examples/demo-csr`.
2. `git switch --detach 2fb7f54`, and the same.
3. `git switch p10-a7-names`, the v3 locks restored, and the same again.

The base builds changed only the two helper crates' lock entries
(`a7-demo-size/locks/*.base-vs-v3.diff`). Every registry package kept its
version, and the v3 locks were restored byte for byte (sha256 `f2a99bf9…`,
`c1a6f667…`, `7b92fa28…`). The second v3 build reproduced the first:
demo-islands and demo-csr byte for byte. In demo-ssr the `.wasm` files and
`demo_ssr.js` were identical; only the split loader `__wasm_split.js`
differed, in the order in which it lists two names. Measured with `node
probes/p10-names/measure-demo.mjs <pkg|dist>`: gzip −9 and brotli q11 via
Node's zlib, as phase 7 measured demo-ssr (`a7-demo-size/*.md`).

| Demo (mode) | Components on the client | wasm, base → v3 (raw / gz / br) | Δ wasm | Δ every shipped file (gz / br) |
|---|---|---|---:|---:|
| demo-ssr (`hydrate`, `--split`) | 3 `LocaleSwitcher`, 6 `LocaleOption` (the shell's `CatalogPreload` and `CatalogLinks` render on the server only) | 758,046 / 317,580 / 252,342 → 759,440 / 318,039 / 252,795 | +1,394 / **+459** / +453 | **+454** / +464 (the lazy chunk 0 raw, −4 gz; `demo_ssr.js` 0 raw) |
| demo-csr (`csr`, trunk) | 1 `LocaleSwitcher`, 3 `LocaleOption` | 207,692 / 91,293 / 77,722 → 208,020 / 91,423 / 77,772 | +328 / **+130** / +50 | **+127** / +47 |
| demo-islands (islands) | none (the switcher is not an island) | 197,382 / 85,357 / 72,289 → 197,371 / 85,349 / 72,328 | −11 / −8 / +39 | −9 / +41 |

**Where demo-ssr's bytes are.** The same client was built by cargo with its
symbol names kept, at base and at v3:
`CARGO_PROFILE_WASM_RELEASE_STRIP=none cargo build --lib --target
wasm32-unknown-unknown --profile wasm-release --no-default-features
--features hydrate --target-dir target/a7-named/demo-ssr --offline`.
Then `twiggy top -n 1000000 -f json` on each, joined with `python3
probes/p10-names/twiggy-norm-diff.py`, which drops crate hashes and closure
numbers so that renames cancel (`a7-demo-size/named/*.norm-diff.txt`). The
net, before wasm-bindgen and wasm-opt:
* **code +1,339 B**, `.rodata` +344 B, types and imports +8 B (the name
  section, +3,797 B, does not ship);
* items new in v3:
  * `OnceLock<&Table>`: `initialize` 105 B and `Once::call` 176 B;
  * `OnceLock<fn(String)>`: the same, 105 B and 176 B;
  * `is_current`: 171 B, linked through the table (`html_lang` stays, since
    the page uses it);
  * `switch`: 110 B (its 1,137 B async body moved from `switch_on_submit`);
  * `install` 75 B, `install_switch` 75 B, `install_table` 16 B;
  * the wrappers' `FnOnce::call_once` shims: 64 B and 60 B;
  * `__component_locale_switcher` +44 B, `LocaleSwitcher` +32 B,
    `LocaleOption` +20 B;
* everything else nets to about zero: inlining moved, and identical-code
  folding kept a different name.

Part of `.rodata` is a second absolute source path: the shipped wasm
carries `…/crates/mf2-leptos-ui-0-9/src/ui.rs` beside
`…/crates/leptos-mf2/src/components.rs` (`strings`). demo-csr, the same
way: code +353 B, `.rodata` +40 B. `html_lang` −181 B (no longer linked),
`is_current` +145, `switch` +96, `install_table` +77.

### Naming: N1–N5 (rustc 1.98.1)

| # | What | Observed |
|---|---|---|
| N1 | 1.x's root rename, `extern crate leptos_0_8 as leptos;`, beside `pub mod leptos` | **E0260** "the name \`leptos\` is defined multiple times … previous definition of the module \`leptos\` here … \`leptos\` reimported here" (`naming/src/lib.rs:76`) — the reason 0.8 cannot keep the root rename under `mf2::leptos` |
| N2 | the 0.9 line under its real name `leptos`, `pub mod leptos`, and a bare `use leptos::prelude::RwSignal;` at the crate root | **E0432** "unresolved import \`leptos::prelude\` … could not find \`prelude\` in \`leptos\`". The local module wins; there is no ambiguity error. In the same crate, `#[::leptos::component]` and `::leptos::view!` at the root compile: only 0.8's `extern crate` binding clashed |
| N3 | `use axum::Router;` at the crate root beside `pub mod axum` | **E0603** "struct import \`Router\` is private": it resolved to `crate::axum`, whose private `use axum::Router` it names |
| N4 | `axum::Router` as a type and as an expression path at the root | **E0603** twice, the same |
| N5 | an application: `use naming::leptos;` (the module itself), then `use leptos::prelude::RwSignal as _;` beside its own `leptos` dependency | **E0432** "unresolved import \`leptos::prelude::RwSignal\` … no \`RwSignal\` in \`leptos::prelude\`"; "cannot find macro \`view\` in this scope" ×2; **E0405** "cannot find trait \`IntoView\`" ×2; and the application's own `use leptos::prelude::*` is reported unused. Importing the module shadows the application's `leptos` crate in every bare path of that module |

Positive, in the same probe: inside `pub mod axum`, `use axum::Router`
names the crate (a module is not in its own scope); `::axum::Router` at the
root names the crate; an application naming `naming::leptos::…` /
`naming::axum::…` by path, beside its own `leptos` and `axum`, compiles.
The comments in `naming-n2` and `naming-app` still expect E0659 for N2, N3
and N5; what rustc 1.98.1 reports is the table above.

**The rules, for 19:**
* inside `mf2`, reach the Leptos lines through internal aliases, never a
  crate-root `extern crate … as leptos`;
* inside `mf2`, name the `axum` crate as `::axum::…` wherever the `axum`
  module is in scope at the crate root;
* the book tells applications to import items from `mf2::leptos` or to write
  full paths, never `use mf2::leptos;` in a module that uses the `leptos`
  crate.

### Coherence: the rules and their errors

The positive set compiles with `leptos` and `ratatui` both on (`ssr`, and
`hydrate` on wasm32) and with `ratatui` alone. `coherence/src/lib.rs`:
* `Paragraph::new(t)` and `Block::bordered().title(t)`;
* `Row::new([t, t])`, `Cell::from(t)`, `ListItem::new(t)`,
  `List::new([t, t])` and `Tabs::new([t, t])`;
* `t.bold()` and `t.fg(Color::Yellow).italic()` through `Styled`;
* `[t, t].into_iter().collect::<Line>()`, `Span::from(t)`, `Text::from(r)`
  and `t.render(area, buf)`;
* `format!("{t} {a}")`, and `t.to_string()` choosing the inherent method;
* `view! { <p title=t>{t}</p> }` in the same scope.

The four controls (`cargo check -p coherence --features probe-*`):
* **Never `From<Tr> for Cow<str>`.** `probe-cow` gives **E0119**
  "conflicting implementations of trait \`From<tr::Tr>\` for type
  \`ratatui_core::text::Span<'_>\`". It is reported at our `From<Tr> for
  Span<'static>`: "conflicting implementation in crate \`ratatui_core\`: -
  impl<'a, T> From<T> for ratatui_core::text::Span<'a> where T: Into<Cow<'a,
  str>>;".
* **No `FromIterator<Tr> for Line`.** `probe-fromiter` gives **E0119**
  "conflicting implementations of trait \`FromIterator<tr::Tr>\` for type
  \`ratatui_core::text::Line<'_>\` … impl<'a, T> FromIterator<T> for
  ratatui_core::text::Line<'a> where T: Into<ratatui_core::text::Span<'a>>;".
  `Line` already collects descriptions through that blanket.
* **`Cell` and `ListItem` come through their blankets over `Into<Text>`.**
  * `probe-cell` gives **E0119** "conflicting implementations of trait
    \`From<tr::Tr>\` for type \`ratatui_widgets::table::Cell<'_>\` … impl<'a,
    T> From<T> for ratatui_widgets::table::Cell<'a> where T:
    Into<ratatui_core::text::Text<'a>>;".
  * `probe-listitem` gives **E0119** "… for type \`ListItem<'_>\` …
    impl<'a, T> From<T> for ListItem<'a> where T:
    Into<ratatui_core::text::Text<'a>>;".

### Verdict

**Done when** ("the module and crate names confirmed; the coherence rules
for 19"):
* **Names: confirmed.**
  * `mf2::leptos` works with no root rename, on both lines.
  * `mf2-leptos-ui-0-9` and `-0-8` build and render on their lines.
  * `pub mod axum` sits beside `axum`, with one rule at the crate root.
  * No Leptos procedural macro is outside the six (item 2).
* **The coherence rules: confirmed**, each with its error (item 4).
* **Hydration: not broken on either line.** The six browser checks of the
  demos are green on 0.9 and on 0.8 (on 0.8 through copies, since the tree
  builds the demos on 0.9 only). `l6-web` and `l7-web` are green on 0.9;
  they have no 0.8 mode.
* **The table's cost (item 1).**
  * **In the gate's own measurements, v3 meets ±64 B gz:** B1 is unchanged
    at 26,402 B gz, and `b5 --view` moves −50 B gz fixed, +0.03 B a site.
  * **In applications that render the switcher on the client, v3 costs more
    than ±64 B gz:** +130 B gz in demo-csr and +459 B gz in demo-ssr (the
    wasm; +127 and +454 over every shipped file). An islands application
    whose switcher stays on the server pays nothing (−8).
* **The fallback.** Question 13's is "back to the owner with the other two
  options". Under the coordinator's rule, it applies **if applications that
  render the switcher count as a configuration that matters**. That call is
  the coordinator's; the owner was not asked.

### What B1 should reuse from the branch, and what to change (interpretation, brief)

* **Reuse:**
  * the helpers' shape: one `ui.rs`, a link in the 0.8 crate, and
    `extern crate leptos_0_8 as leptos;` only there;
  * the renamed line dependencies, and the private `line` alias module in the
    Leptos and Axum layers;
  * wrappers that take the helper's `*Props`, so `view!` is unchanged;
  * nothing installed by `install()`: v1 and v2 show what that costs every
    application;
  * `table()` as an `Option`, so a missing table cannot panic;
  * the tests' line binding;
  * `probes/p10-names/demos-0-8.py` for B1's "e2e on both Leptos lines": the
    tree has no other way to run the browser checks on 0.8.
* **Change — the install mechanism, if the demos' figures count.** About
  1,230 B of demo-ssr's +1,339 B of code are that mechanism:
  * two `std::sync::OnceLock`s (with `Once`);
  * the install calls and the wrappers' shims;
  * `is_current`, linked because a table entry points at it;
  * the switch behind a function pointer.

  One untested option would remove all of it: static dispatch. The
  helper declares a trait whose associated functions are the table's
  entries, `leptos-mf2` implements it for a zero-sized type, and the
  components are generic over it. There would be no `OnceLock` and no
  function pointer, and nothing would be linked that a page does not call.
  Its cost is not measured.
* **Measure applications that use the components.** B1's size gate should
  cover them, since `size` and `b5 --view` render none of the six:
  demo-ssr, demo-csr and demo-islands, with `measure-demo.mjs`.
* **Leave out of B1:**
  * the `ratatui` / `probe-*` features, `ratatui_probe.rs` and the
    `ratatui-widgets` workspace dependency (C5 builds the real impls);
  * `publish = false` on the helpers;
  * the demos' import change (the work order keeps the examples on the shims
    until C8 and D6).
* **B1 decides:**
  * whether the `*Props` types, which the wrappers' signatures expose, belong
    in the per-mode `api.txt`;
  * what a checkout without symlink support (`core.symlinks=false`) gets for
    the 0.8 helper's `ui.rs`. `cargo package` follows the link.

## Part B — one crate, with every old path kept by shims (B1 after A1 and A7; B2–B4 after B1; B5 with or after B4)

| Task | Deliverable | Done when |
|---|---|---|
| **B1** The types and the Leptos layer into `mf2` | **Moves:** <br>• the four core files → `crates/mf2/src/`; <br>• the Leptos layer → `crates/mf2/src/leptos/`, reaching each line through internal aliases; <br>• the six components → `crates/mf2-leptos-ui-0-9` and `-0-8` (one source, compiled once per line), behind `mf2`'s function table. <br>**Features:** `mf2` gains the Leptos dependencies and `leptos` / `leptos-0-8`, `ssr`/`hydrate`/`csr` (implying the hosts), `static-locale`, `mark-fallback-lang`; the `compile_error!`s name `mf2`'s features. <br>**`leptos-mf2` becomes a shim:** it depends on `mf2`, forwards features, and re-exports everything. <br>**Tests** move to `crates/mf2/tests/`, which ends the dev-dependency cycle. `xtask` `ci` / `msrv` steps switch to `-p mf2`. <br>**In the same commit:** docs.rs metadata, `api.txt`, `package.txt`, 04 §2.1 and 05 §9 rewritten, master plan §4; moved item docs lose their `plans/` citations | `cargo xtask ci` and `docs`; `size` within the gate; `codegen-matrix`, `scenarios`, `leptos-0-8`, `l6-web`, `l7-web`, the e2e checks, `churn`, `msrv`; the ledger unchanged |
| **B2** `mf2::native` | The `native` feature (`sys-locale`, `jiff[tz-system]`, `mf2-catalog`'s `static-bytes` / `content-hash`, std). `mf2-native` becomes a shim; its tests move | as B1, with no web change |
| **B3** `mf2::ratatui` | The `ratatui` feature (implies `native`; `ratatui-core` 0.1). `mf2-ratatui` becomes a shim; its tests move | as B2 |
| **B4** Internal users name `mf2` | Conformance (`conformance/`, `l6-web`, `l7-web`), `bench/churn`, `bench/fluent-ab/mf2`, the `workload-gen` templates the size gate measures (`tr`, `tr-view`, `fluent-converted`), and the xtask crate lists. The examples and the book stay on the shims until C8 and D6 | `conformance-report --check`, `l6-web`, `l7-web`, `size`; the ledger unchanged |
| **B5** The API listed per mode | `cargo xtask api` writes `crates/mf2/api/{core,ssr,hydrate,csr,native,ratatui}.txt` (`axum` joins with D1) from a table in the manifest. `release.rs` runs cargo-semver-checks per mode, with baseline feature sets for 1.0.0's `leptos-mf2/…` spellings | a hydrate-only public item added without its listing fails `api --check` (the negative control) |

## Part C — native and Ratatui (API work after A8's review; C1 and C2 after B1–B3; C3 before C4; C5 after C2 and C4; C6 after A2, A3 and C4; C7 after C6; C8 after C5 and C7; C9 after C8)

| Task | Deliverable | Done when |
|---|---|---|
| **C1** Arguments | A trait `IntoArg` with `#[diagnostic::on_unimplemented]` naming the accepted types, **implemented per type** (a blanket over `Into` would bypass the message): <br>• integers up to 128 bits exact (past `i64`, an exact decimal written without `core::fmt`), and `usize` without saturation; <br>• `bool`, `Cow<'static, str>` (borrowed stays static); <br>• `Path` / `OsStr` / `SystemTime` under std; jiff's `Timestamp`, `Zoned` and civil types under `native`; <br>• signals over `T: IntoArg`; `ArgValue`; `&T` for `Copy` types. <br>The macro emits `IntoArg::into_arg(e)` spanned at the argument; `From` stays for `ArgValue::from`. The `&str` copy is measured, and inlined only if it pays and B5 holds | per-conversion tests through `compile_str`; a trybuild case whose `.stderr` shows the message pointing at the argument; `b5 --view` unchanged |
| **C2** The ambient store | `mf2::native` provides: <br>• `install(&'static Corpus)` (idempotent; a different corpus is an error) and `install_from_directory` (a partial set of files is accepted; only the source locale's is required); <br>• `set_locale`, `locale()`, `locale_source()`, and `with_locale` (restored by a guard); <br>• the time-zone and bidi settings, and `Catalogs`. <br>**The one ambient lookup** (A4's design). **`Display`**, the always-on `to_string` / `to_plain_string` / `to_cow` (borrowed for a simple message), and `Debug`, as A5 decided. **The system zone** by name, else one that follows the system's DST rules — never a frozen offset | parallel `with_locale` tests; `set_locale` seen from another thread on the next format; B10 through the ambient path; a `TZ=EST5EDT,M3.2.0,M11.1.0` test across DST; `tui-gate` against A1 |
| **C3** One matcher | **Data:** CLDR's `languageMatching` added to `cargo xtask cldr-sync`'s set, and `likelySubtags` (already vendored) used. **Scope:** one matcher for native `set_locale`, `Locale::from_str`, web negotiation, and the client-only boot. **Rules:** POSIX names; exact; the script implied by likely subtags; region fallback; another script only where CLDR's data accepts it. The client's table holds only the corpus's languages and is measured against B1 | a test table, each case with its reason: <br>• `zh-Hant-TW → zh-TW`, and `zh-HK → zh-TW` when there is no `zh-HK`; <br>• Traditional → Simplified when no Traditional; <br>• `sr-Latn ↔ sr-Cyrl`; `pa-Arab ↛ pa-Guru`; <br>• `es-MX → es`, `es → es-MX`; <br>• `fr_CA.UTF-8`, `C`, `POSIX`; <br>• every case the current matchers pass, except those question 11 changes |
| **C4** The generated module | **`enum Locale`** (the build refuses variant-name collisions): `ALL`, `SOURCE`, `tag()`, `dir()`; `FromStr` through C3 (client-path code, since the wasm reaches it), whose error lists the supported locales; `Display`; `format(&impl Message)`; `clap::ValueEnum` under an optional `clap` feature. **Also:** `install()`, `set_locale(Locale)`, `with_locale`, `current_locale()`, and `markup::*` (a name hash per markup name the corpus uses); a prelude; doc comments that fit the mode (no wasm wording in a native module); compile-time choices through A2's cfg macros; one embedded byte table shared by `CATALOGS` and `CORPUS` when `ssr` and `native` are both on | unit tests in `crates/mf2-build/src/codegen.rs`; `codegen-matrix` with the native combinations; L5 unchanged; `scenarios` |
| **C5** Ratatui | **Conversions:** `From<Tr / TrArgs / TrRich / TrDyn>` for `Span`, `Line` and `Text`. Constant text is borrowed with no allocation; pattern text parts are borrowed as A4 decided; only placeholders allocate. **Traits:** `Styled` with `Item = Line<'static>`, so `.bold()` keeps a message's own markup; `Widget` for the descriptions. **Theme:** an app-wide `Theme` (markup-name hash → `Style`) with `set_theme` and a scoped `with_theme`, and defaults for `b` / `strong`, `i` / `em`, `u`, `s` / `del`, `code` / `kbd`. **Line breaks:** `Text` splits at a line break; `Line` and `Span` join with a space; a `Span` flattens markup — all documented. The old `line` / `text` / `MarkupStyles` go with C8's page | tests on a ratatui-core `Buffer` (text and styles); allocation tests; `Stylize` compiles; A7's coherence set with `leptos` and `ratatui` on |
| **C6** The build script | **`mf2_build::run()`** reads `mf2`'s features through `links` (else `CARGO_FEATURE_*`), picks what to emit, and prints `cargo::warning=` / `cargo::error=`; it exits non-zero on errors, and prints its rerun lines. **Compression** only for a web server, and no maximum-quality brotli in debug builds (review #18). **Checks:** a clear error when `datetime-icu` is on without `mf2-build`'s `icu-blob`; the single-crate layout with A3's `tr!`; `mf2 check` and `compile --site` read `mf2`'s node in the cargo resolve. **`mf2`** gains `links = "mf2-v2"` and a `build.rs` | A2's scenarios re-run on the real crates; `package --check`; `scenarios`; the edit-loop time with and without the opt-level tip, measured |
| **C7** `mf2 init` as a starter (native) | `mf2 init --cli` / `--tui` either creates a complete, runnable application, or adds translations to the current crate (`build.rs`, `locales/`, `cargo add mf2 -F native[,ratatui]`, `cargo add --build mf2-build`). It prints the `[profile.dev.build-override] opt-level = 2` tip, or writes it into a new application. `mf2 --help`'s summary names every command | the book runs `init` in `run=` blocks, so `cargo xtask docs` compiles what it makes; `mf2-cli` tests |
| **C8** The samples, the native book and the gates | `examples/tui` on 2.0. `tui-gate` becomes a gate: allocations in CI (deterministic); time nightly, alternating with the kept 1.x binary; sizes. `docs/native-apps.md` rewritten as three compiled projects (the one-file CLI, the TUI whose `main` reaches it, the two-crate workspace), added to `xtask/src/docs.rs`'s projects | `cargo xtask docs` (full); `tui-gate`; the native UX rows all fall |
| **C9** The trippy port as acceptance (the port stays untracked) | `vendor/trippy` ported to 2.0: <br>• the `thread_local` / `t!` wrapper deleted, `tr!` called directly; <br>• the messages made real MF2 (a `.match` plural instead of the `plural_flows` word; key hints as markup instead of the slicing hack; no `format!` word order); <br>• the language from `--tui-locale` through `Locale: FromStr`; <br>• upstream's 22 locale tests restored. <br>Recorded in 19 §"Prior art: trippy": call sites and keys, lines added and removed against upstream, stripped size and build time against upstream, each upstream bug class with the compile error that now catches it, and what did not fit | `cargo build` and `cargo test -p trippy-tui` in the checkout; a smoke run in a pseudo-terminal, as far as the machine allows; the record written |

## C3 (data half) — CLDR's language-matching data: what was built

Done ahead of C3's code and API (which wait for the owner's review of A8),
so that A8 can state the matching rules decided in question 11 from the
data rather than from expectation.

* **Vendored** (commit `acdbc01` on `main`), through
  `cargo xtask cldr-sync` at the pinned commit (`48.2.1` =
  `26a79cb4…`), byte-for-byte:
  * `cldr-core/supplemental/languageMatching.json` — **55,261 B**: 378
    `languageMatch` rules (`written-new`), 4 match variables, 6 paradigm
    locales;
  * `cldr-core/supplemental/territoryContainment.json` — **11,959 B**. Needed
    because the `$americas` match variable is the macro-region `019`, whose
    members (`MX`, `AR`, …) only the containment data gives.
  * The vendored set grows from 40 files (3,633,315 B) to **42 files
    (3,700,535 B)**; `PIN`'s `layout` and `vendored` lists name both (the
    text is written by `cldr-sync`, `xtask/src/cldr_sync.rs`). A second
    `cargo xtask cldr-sync` changes nothing: the sha-256 of every file under
    `third_party/cldr-json` is the same before and after
    (`14652cb3…` over the sorted list).
* **`parentLocales.json` stays cache-only.** CLDR's matcher decides by
  likely subtags and distances, not by the inheritance chain; `es-MX`'s
  preference for `es-419` over `es` comes from the `$americas` rules
  (below), which need the containment file instead.
* Nothing else asserts the vendored set (no test counts it; `locale-data`
  reads only the files it read before). `05-tooling.md` §7's list of
  vendored supplemental files should gain the two when this record is
  integrated.
* **Found by `cargo xtask ci`, fixed in the same commit:** once
  `cldr-sync` has filled its cache (`target/xtask-cache/cldr-json`, which
  did not exist on this machine before this run), `cargo xtask package`
  failed: `mf2-locale-data: LICENSE-UNICODE is a copy of
  target/xtask-cache/cldr-json/LICENSE`. The audit's map from content to
  forbidden path kept one path per digest, and the cache's upstream
  `LICENSE` — byte for byte `third_party/cldr-json/LICENSE`, the symlink's
  expected target — overwrote it. `xtask/src/package.rs` now keeps every
  path per digest and accepts the licence link when its target is among
  them; any other copy is still refused. (By the code, the audit would have
  failed the same way on any machine with the CLDR cache since Phase 9 A4
  wrote it; it passed here because the cache was absent.)

### What the file holds (observed)

| Level | Rules | Default |
|---|---|---|
| language (`desired` / `supported` one subtag) | 310, e.g. `nb`↔`no` 1, `hr`↔`bs` 4, `da`↔`nb` 8, `ca`→`es` 20 oneway, `yue`→`zh` 10 oneway, `pa`→`en` 30 oneway | `*`↔`*` **80** (#310) |
| script (`lang-Script`) | 49 (#311–#359) | `*-*`↔`*-*` **50** (#360) |
| region (`lang-Script-Region`, match variables) | 16 (#361–#376) | `*-*-*`↔`*-*-*` **4** (#377) |

Match variables: `$americas` = `019`; `$cnsar` = `HK+MO`; `$enUS` =
`AS+CA+GU+MH+MP+PH+PR+UM+US+VI`; `$maghreb` = `MA+DZ+TN+LY+MR+EH`. Paradigm
locales: `en`, `en-GB`, `es`, `es-419`, `pt-BR`, `pt-PT`.

**The script-level rules, all of them** — which languages have a cross-script
entry at all:
* **same language, both ways:** only `sr-Latn` ↔ `sr-Cyrl` **5** (#335);
* **same language, one way, towards the usual script** (a reader who asked
  for a transliteration or a sub-script accepts the usual one): `ar`, `bn`,
  `gu`, `hi`, `kn`, `ml`, `mr`, `ta`, `te` `-Latn` → native script 20;
  `zh-Latn` → `zh-Hans` 20; `zh-Hani` → `zh-Hans` and → `zh-Hant` 20;
  `ja-{Latn,Hani,Hira,Kana,Hrkt}` → `ja-Jpan` 5, `ja-{Hira,Kana}` → `ja-Hrkt`
  5; `ko-{Hani,Hang,Jamo}` → `ko-Kore` 5, `ko-Jamo` → `ko-Hang` 5;
* **another language, one way** (they make a language-level fallback
  usable across scripts): `am-Ethi`, `bn-Beng`, `ka-Geor`, `km-Khmr`,
  `kn-Knda`, `lo-Laoo`, `ml-Mlym`, `my-Mymr`, `ne-Deva`, `or-Orya`,
  `pa-Guru`, `ps-Arab`, `sd-Arab`, `si-Sinh`, `ta-Taml`, `te-Telu`,
  `ti-Ethi`, `ur-Arab`, `yi-Hebr` → `en-Latn` 10; `az-Latn`, `hy-Armn`,
  `tk-Latn`, `uz-Latn` → `ru-Cyrl` 10; `bo-Tibt`, `za-Latn` → `zh-Hans` 10.
* **There is no `zh-Hant` ↔ `zh-Hans` rule and no `pa-Arab` ↔ `pa-Guru`
  rule**; both fall to the default 50. Nor is there one for any other
  language written in two scripts (`az`, `bs`, `ff`, `ks`, `mn`, `sd`,
  `shi`, `uz`, `vai`, `yue`).

### The evidence for question 11

Computed by a throwaway reader over the three vendored files
(`target/p10-c3/evidence.py`, not in the tree): each tag maximized by
likely subtags, then per level the first rule that matches (a `oneway` rule
only in its stated direction), the three distances summed. **The threshold
is not in the data**: the JSON has no documentation keys, and the rule text
is UTS #35 Part 1 §4.4 ("Language Matching"), which is not vendored. The
verdicts below apply **ICU's LocaleMatcher default — a match only when the
total is below the default script distance, 50** — as recalled, not read
in the tree; C3 must confirm it against the specification text (see
"Needs", below).

| Reader asks → app has | Maximized | Deciding rules | Distance | Verdict |
|---|---|---|---|---|
| `zh-Hant` → `zh-Hans` | zh-Hant-TW → zh-Hans-CN | script: #360 `*-*` 50 | 50 | **refuse** |
| `zh-Hans` → `zh-Hant` | zh-Hans-CN → zh-Hant-TW | script: #360 `*-*` 50 | 50 | **refuse** |
| `zh` → `zh-TW`, `zh-TW` → `zh` | zh-Hans-CN ↔ zh-Hant-TW | script: #360 50 | 50 | **refuse** |
| `zh-Hant-TW` → `zh-TW` | both zh-Hant-TW (likelySubtags: `zh-Hant` → zh-Hant-TW, `zh-TW` → zh-Hant-TW) | — | 0 | match (exact) |
| `zh-HK` → `zh-TW` (and back) | zh-Hant-HK ↔ zh-Hant-TW | region: #376 `zh-Hant-*` 5 (HK is `$cnsar`, TW is not) | 5 | match |
| `zh-HK` → `zh-MO` | zh-Hant-HK → zh-Hant-MO | region: #374 `zh-Hant-$cnsar` 4 | 4 | match |
| `yue` → `zh-TW` / → `zh` | yue-Hant-HK → zh-Hant-TW / zh-Hans-CN | language #309 `yue`→`zh` 10 oneway; then region 4 / script 50 | 14 / 60 | match / refuse |
| `sr-Latn` → `sr-Cyrl` (and back) | sr-Latn-RS ↔ sr-Cyrl-RS | script: #335 5 (both ways) | 5 | match |
| `sr-ME` → `sr` | sr-Latn-ME → sr-Cyrl-RS | script #335 5; region #377 4 | 9 | match |
| `pa-Arab` → `pa-Guru` (and back) | pa-Arab-PK ↔ pa-Guru-IN | script: #360 50 | 50 | **refuse** |
| `pa` → `en` / `pa-PK` → `en` | pa-Guru-IN / pa-Arab-PK → en-Latn-US | language #84 30 oneway; script #324 `pa-Guru`→`en-Latn` 10 / #360 50; region 4 | 44 / 80 | match / refuse |
| `es-MX` → `es`, `es` → `es-MX` | es-Latn-MX ↔ es-Latn-ES | region: #370 `es-*-*` 5 | 5 | match |
| `es-MX` → `es-419`; `es-AR` → `es-MX` | es-Latn-MX → es-Latn-419 … | region: #368 `es-*-$americas` 4 | 4 | match (closer than `es`) |
| `es-419` → `es`; `es-ES` → `es-419` | … | region: #370 5 | 5 | match |
| `en-GB` → `en-US` (and back) | en-Latn-GB ↔ en-Latn-US | region: #367 `en-*-*` 5 | 5 | match |
| `en-AU` → `en-GB` / → `en-US` | … | region: #365 `en-*-$!enUS`→`en-*-GB` 3 / #367 5 | 3 / 5 | match (prefers en-GB) |
| `pt-PT` → `pt-BR` (and back); `pt` → `pt-PT` | pt-Latn-PT ↔ pt-Latn-BR | region: #373 `pt-*-*` 5 | 5 | match |
| `fr-CA` → `fr` (and back) | fr-Latn-CA ↔ fr-Latn-FR | region: #377 `*-*-*` 4 | 4 | match |
| region implies script: `zh-TW`→`zh-CN`, `pa-PK`→`pa-IN`, `uz-AF`→`uz`, `az-IR`→`az`, `mn-CN`→`mn`, `sd-IN`→`sd` | the scripts differ after maximizing | script: #360 50 | 50 | refuse |
| `ca` → `es` / `es` → `ca` | … | #24 `ca`→`es` 20 oneway / #310 `*` 80 | 20 / 80 | match / refuse |

* **`419` and `$americas`.** In the containment data `019` contains `021`,
  `013`, `029`, `005`; `419` is a *grouping* (`013`, `029`, `005`), not in
  `019`'s tree. The `es-MX` → `es-419` = 4 row counts `419` as in
  `$americas` because all its members are; counting only the tree gives 5,
  a tie with `es` (then settled by the paradigm locales, both of which are
  paradigms, or by order). C3 has to fix this reading from the
  specification.
* **Demotion** (ICU's, not in the data): each later entry in the reader's
  list is demoted by the default region distance (`en-US` ↔ `en-GB` = 5),
  so a region difference on the reader's first choice ties an exact match
  on the second, and the earlier wins.

### Verdict against question 11's expectations

* **Serbian Latin ↔ Cyrillic: confirmed** (5, both ways).
* **Punjabi's two scripts: confirmed refused** (no rule; 50).
* **Spanish regions: confirmed** as the owner described: `es-MX` served when
  the app has it; otherwise `es` (5), preferring `es-419` (4) when the app
  has that; and `es` → `es-MX` (5) when that is all the app has.
* **Traditional ↔ Simplified: contradicted.** CLDR 48 has no rule between
  `zh-Hant` and `zh-Hans`; the pair takes the default script distance, 50,
  which the default threshold refuses — in both directions, and for
  region-implied scripts too (`zh-TW` ↔ `zh-CN`). Following CLDR's data, a
  Traditional reader of an application that has only Simplified gets the
  source language, and so does a Simplified reader of an application that
  has only Traditional. (Today's native matcher serves `zh` to `zh-Hant-TW`
  by truncation; the web's likewise, then any `zh`.) The owner expected
  Traditional → Simplified to be accepted.

**Needs (for the coordinator):** (1) the owner's decision on Traditional ↔
Simplified — CLDR's refusal, or a documented project rule on top of the data;
(2) the matching algorithm's normative text (UTS #35 Part 1 §4.4: the
threshold, `oneway`, paradigm locales, demotion, groupings in match
variables) is not in the tree and cannot be fetched under the network rule;
C3 either works from this record and ICU's documented behaviour, or the
owner allows vendoring that section.

### For C3's rules and the client's table (interpretation)

* The rules D21 lists map onto the data: "the script implied by likely
  subtags" is maximization; "region fallback" is the region level (4–5);
  "another script only where CLDR's data accepts it" is the script level
  below the threshold (sr 5; the transliteration and sub-script one-ways;
  the native-script → English/Russian/Chinese one-ways).
* The client's table needs, for a corpus: the rules whose supported side can
  be one of its languages (or `*`), and likely subtags for its languages and
  for the languages whose readers those rules accept. Counted over the
  vendored files: the demos' `ar en fr` — 129 of 378 rules and 109 of 7,788
  likely entries; `examples/tui`'s `de en es fr` — 108 and 91; the 11-locale
  probe panel — 169 and 160. Most of it is the one-way "reader of X accepts
  English/French/Spanish" rules (a reader of Acholi accepts English at 30):
  keeping only the rules between the corpus's own languages and the
  wildcards leaves 11 / 10 / 23 rules and 4 / 5 / 12 likely entries. Which
  cut the client makes is C3's to measure against B1; the server and native
  builds can carry the whole table.

**Commands:** `cargo xtask cldr-sync` (twice; the second changes nothing);
`python3 target/p10-c3/evidence.py` (the table above; `evidence.py <desired>
<supported>` for one pair); `cargo xtask ci` green on the commit (one
earlier run failed `mf2-catalog`'s timing test `linear.rs` under load 10–13
— a different row on each re-run — and passed on the next).

## Part D — the web (D1 after B4; D2–D4 after D1; D5 after D3 and C7; D6 last)

| Task | Deliverable | Done when |
|---|---|---|
| **D1** `mf2::axum` | `mf2-axum` becomes a shim. Negotiation and catalog serving compile without a Leptos mode (a plain Axum application: a `Locale` extractor, per-request formatting through `Locale::format`); the request glue compiles only with `ssr`. `api/axum.txt` | as B1; a plain-Axum test serving a formatted response per `Accept-Language` |
| **D2** Defaults | `Negotiator::default()` becomes `?lang=`, then the cookie, then `Accept-Language` (the order Getting started writes by hand). The switcher takes its parameter name from the installed query source | unit tests; e2e `demo.mjs` with the wasm blocked |
| **D3** Server wiring and the generated setup | **Probe first:** a tower layer `mf2::axum::negotiate(Negotiator)` that negotiates, puts the result in the request's extensions, and writes `Content-Language`, `Vary` and the cookie; the render finds it through `Parts` in the Leptos context. If it holds, the `_with_context` wiring (and the silent failure when one entry point misses it) goes. **Then:** a generated `setup()` / `install()` on each side; no `[features]` block in the translation crate (A2) | the e2e checks (`demo`, `lazy`, `csr`, `islands`, `a11y`) without the context; `scenarios` byte-identical |
| **D4** Typed languages and the switch on the web | • `LocaleOption tag=Locale::Fr`; <br>• `set_locale` / `preload_locale` callable on both sides (a spawned call on the client, nothing on the server), so application code needs no `#[cfg]` pair; <br>• a reactive `current_locale()`; <br>• an options component driven by `language.<tag>` messages; <br>• markup closures that need no type annotation (`\|c\| view! { … }`, review #10) | compiled samples; e2e; `churn` for the new conversions |
| **D5** `mf2 init` as a starter (web) | `mf2 init --ssr` / `--islands` / `--csr`: a complete, runnable application, or translations added to an existing one (A6 decides whether one crate is offered) | `cargo xtask docs` builds what each makes |
| **D6** The web book and examples | Getting started (and its 0.8 variant), switching, call sites and delivery modes on 2.0; `examples/demo-{ssr,islands,csr}` and `tools/e2e` updated | `cargo xtask docs`, every e2e check, `churn`, `islands-zero`, `size`; the web UX rows all fall |

## Part E — the silent failures (required for 2.0; any order, E4 after D1)

| Task | Deliverable | Done when |
|---|---|---|
| **E1** `dropped-markup` | A lint: a translation that leaves out a markup element of the source message (as `dropped-variable` does for variables). Run by `check`, the build and `import` | a seeded-drift case; the review's `terms` example refused |
| **E2** `@do-not-translate` is not missing | Such a message counts neither as missing nor in coverage (`check`, `stats`, JSON and XLIFF exports) | the review's case: "3 of 4 missing" becomes "2 of 3" |
| **E3** `import` checks what it writes | `mf2 import` (JSON and XLIFF) runs the checks on the result and fails on errors, writing nothing. JSON import either adds ids the language lacks or names XLIFF in its message | the review's `$nom` case refused; a negative control per format |
| **E4** Server and client warnings | Logged once on the server: a page rendered without the request's language; formatting with no catalogs installed. A browser console warning in debug builds only | the warnings shown once; B1 and B12 unchanged in release (measured) |

## E1 — `dropped-markup`: what was built

Commit `a72d057` on `main`, "Phase 10 E1: `dropped-markup` — a
translation may not lose the source's markup" (written on the branch
`p10-e-silent-failures` off `8f6569e`, and cherry-picked unchanged).

**The lint.** `Lint::DroppedMarkup` (`"dropped-markup"`): a translation
that leaves out markup its source message has. Checked in
`check::against_source` beside `dropped-placeholder`, the same way: markup
**names** compared over the **whole message** (the analysis' NFC names), so
one variant may leave the markup out while another keeps it — Polish `one`
("a message") drops the bold with the count. Reported once per message at
its start: `the source message has {#link}, which this translation leaves
out, so this language loses what it marks (a link, a style)`.

**Its level — a decision, stated here.** The work order says "as
`dropped-variable` does for variables" (the lint is `dropped-placeholder`,
a warning with floor `allow`) and "the review's `terms` example refused".
A warning cannot refuse, so `dropped-markup` is an **error by default with
floor `allow`**, the catalogue's pattern for "an error, which a corpus that
means it may turn down" (`dynamic-select`, `do-not-translate`, …). Why it
differs from `dropped-placeholder`: a plural variant routinely drops
`{$count}`; nothing routinely drops a link. A corpus that drops emphasis on
purpose (italics in a script without them) sets `dropped-markup = "warn"`.
plans/05 §3's rule "a translation MAY use a subset of the source's …
markup" was changed in the same commit.

**Run by** `mf2 check`, the build (the same `check::corpus` pass
`Build::run` and `Build::check` share, so a build script now fails on it),
and `mf2 import` from E3 (commit `894c0c4`).

| Test | What it proves | Result |
|---|---|---|
| `tests/drift.rs` `every_lint_fires_on_its_own_drift_and_nothing_else_does` — the new drift `{#kbd}Esc{/kbd}` → `Esc` in `pl` | the lint fires on its drift and no other error does | PASS |
| same table, `undeclared-markup`'s drift | changed from `{#kbd}…` → `{#b}…` (which also dropped `kbd`) to `{#kbd}{#b}Esc{/b}{/kbd}`, so each drift stays one lint's | PASS |
| `a_lint_set_to_allow_says_nothing` | floor `allow` works | PASS |
| `markup_that_one_variant_keeps_is_not_dropped` (new) | Polish `one` without `{#b}`, `few`/`many`/`*` with it: clean; `{#b}` out of every variant: one error naming `{#b}` | PASS |
| `the_reference_workload_is_clean` | no workload translation drops markup | PASS |
| `tests/commands.rs` `check_refuses_a_translation_that_drops_markup` (new) | the review's case: `terms = Accept our {#link}terms{/link}.` / `Acceptez nos conditions.` → `mf2 check` exits 1, `… (in terms, locale fr) [dropped-markup]`; with the link kept it passes | PASS |
| `tests/xliff.rs` `import_refuses_a_translation_that_drops_markup` (E3's commit) | the same case through XLIFF import: refused, nothing written; with the `<pc>` kept, it lands | PASS |

**Corpora in the tree** (`mf2 check --features
fn-number,fn-datetime,datetime-icu` on each): `tools/i18n-fixture`'s
Polish `help` had dropped the source's `{#b}` (`Nacisnij {#kbd}Esc{/kbd},
aby zamknac` against `Press {#kbd}Esc{/kbd} to {#b}close{/b}`); nothing
relied on it (rg), so it was restored — without that the fixture's build
script fails. The demos, `bench/churn`, the fixture's two variants and the
book's projects (`cargo xtask docs --no-build`) drop nothing. L5's corpora
are identical twins.

**Commands run** (on the branch, `CARGO_BUILD_JOBS=2`): `cargo test -p
mf2-build` (all pass), `cargo test -p mf2-cli` (all pass),
`cargo test -p mf2-i18n-fixture --lib`, `cargo clippy -p mf2-build -p
mf2-cli -p mf2-i18n-fixture --all-targets -- -D warnings`, `cargo fmt
--all --check`, `cargo xtask docs --no-build`. `crates/mf2-build/api.txt`
gained `pub mf2_build::Lint::DroppedMarkup` by hand, in the listing's
sorted place; on `main`, `cargo xtask api --check` confirms it ("18
listings unchanged"). What ran on `main` with the three applied is in E3's
record.

**Verdict:** done — the seeded drift and the review's `terms` example are
refused by `check`, the build and (with E3) `import`.

**For the design (interpretation).** None for A8. The book's lint
reference (F2) should give `dropped-markup` its section, with the
`dropped-placeholder` contrast.

## E2 — `@do-not-translate` is not missing: what was built

Commit `3f1eed9` on `main`, "Phase 10 E2: `@do-not-translate` messages
are neither missing nor covered" (written on the branch
`p10-e-silent-failures`, and cherry-picked unchanged).

**The rule.** A message the source marks `@do-not-translate` needs no
translation: it counts neither as missing (where a language lacks it) nor
as translated (where a language copies it). Coverage is over the messages
that need translating.

**Where the count comes from.** `check::coverage_of(corpus, locale) ->
Coverage { tag, translatable, missing }` (the source's ids less its
do-not-translate ones; those the locale lacks, in manifest order). The
`missing-translation` lint uses it, and the build's `Outcome` carries one
per locale in a new `#[doc(hidden)]` field, `coverage`, which `mf2 stats`
reads — one computation, so `check` and `stats` cannot disagree. Hidden,
like `Outcome::manifest` and `catalogs`: not in `api.txt`, not promised.

**Where it shows.**
- `mf2 check` / the build: `2 of 3 messages are missing here and fall back
  to en: apply, farewell` where it said `3 of 4 …` (text, and the same
  diagnostic in `--format json`).
- `mf2 stats`: the coverage and missing columns, and `--format json`'s
  per-locale `messages` / `missing`, count only messages that need
  translating; the header says `4 messages (1 marked @do-not-translate)`,
  and the JSON gains a top-level `do_not_translate`. The catalog's own
  `missing` / `fallbacks` (what it carries) are unchanged.
- **XLIFF export: no change needed.** A do-not-translate unit is already
  `translate="no"` with no `<target>` — XLIFF 2.1's own examples do exactly
  that (the spec §5.9.8.1, vendored), and tools leave such units out of
  their counts. A test now asserts the unit has no target.
- **JSON export: no change.** It writes a language's own messages, with
  no count: a do-not-translate message the language lacks is not in it,
  and a copy it has is exported as it stands (on import, E3's checks hold
  the copy to its source through the `do-not-translate` lint). *This is an
  interpretation of the work order's "JSON and XLIFF exports": if it meant
  that `mf2 export` of the source language should leave such messages out
  of the file a translation tool counts, that is a small follow-up — not
  done, to keep the export a faithful copy of a language.*

**Also: the mark on a section or a file now covers its entries** in every
check (the resource loader adds the property to each entry under a
`@do-not-translate` section head or resource). XLIFF export already treated
it so (§6.3: `translate="no"` on the file, group or unit); `check` read
only an entry's own properties, so a `[language]` section marked once
would still have counted as missing. Consequence: the `do-not-translate`
lint and `mf2 pseudo`'s copy-as-is now also apply to such entries.

| Test | What it proves | Result |
|---|---|---|
| `tests/drift.rs` `do_not_translate_messages_are_neither_missing_nor_covered` (new) | the review's case: en has 4 messages, 1 `@do-not-translate`; fr has 1 → "2 of 3 missing: apply, farewell", `coverage[fr] = (3 translatable, 1 translated)`; the source's coverage has nothing missing; fr copying the do-not-translate message → still 1 of 3 translated; a `@do-not-translate` `[language]` section → fr lacking it reports nothing | PASS |
| `tests/commands.rs` `do_not_translate_messages_are_not_missing` (new) | through the binary: `check` text "2 of 3 …"; `stats` header "4 messages (1 marked @do-not-translate)", rows fr `33.3%` / `2` and en `100.0%` / `0`; `stats --format json`: `messages` 4, `do_not_translate` 1, fr `messages` 1, `missing` 2 | PASS |
| `tests/xliff.rs` `the_export_has_the_mapping_of_the_plan` (extended) | the do-not-translate unit `brand` is `translate="no"` and has no `<target>` | PASS |
| existing `stats_reports_coverage_sizes_and_the_pins`, `check_names_the_first_missing_translations`, the drift table, the workload round trips | nothing else moved | PASS |

**Commands run** (on the branch, `CARGO_BUILD_JOBS=2`): `cargo test -p
mf2-build`, `cargo test -p mf2-cli` (all pass), `cargo clippy -p mf2-build
-p mf2-cli --all-targets -- -D warnings`, `cargo fmt --all --check`,
`cargo xtask docs --no-build`; every corpus in the tree re-checked with
the new binary (same results as before E2). What ran on `main` with the
three applied is in E3's record.

**Verdict:** done — the review's case reads "2 of 3" (test), in `check`
and `stats`, text and JSON.

**Found along the way (interpretation, brief).** A do-not-translate
message a language lacks is served from the source's catalog, so under
`mark-fallback-lang` it is wrapped `<span lang="en">` — wrong for a
language's own name (`Français` read with English rules). The book's
pattern (each language file copies the names) avoids that; the switcher's
own `lang` handling may too. Not changed; worth a look when D4 builds the
options component driven by `language.<tag>` messages.

## E3 — `import` checks what it writes: what was built

Commit `894c0c4` on `main`, "Phase 10 E3: `mf2 import` checks what it
would write, and writes nothing on an error it brings" (written on the
branch `p10-e-silent-failures`, and cherry-picked unchanged).

**Plan → check → write.** Both formats first make the files in memory
(`exchange::Rewrite`; `xliff::import` now returns its rewrites and
refusals — `plan_target` — instead of writing). Then `checked()` copies
`locales/` into a scratch directory, puts the rewritten files in it, and
runs `Build::check` on the corpus as it stands and on the copy, with
`mf2 check`'s configuration and features (`--features`, new on `import`,
else cargo's, the same function `check` uses) and `Emit::Module`, so
nothing is compressed. The findings the copy has and the corpus does not
(a finding keyed by level, locale, id, lint, error kind and message — not
by line, which a rewrite moves; the copy's paths mapped back) are printed
as `check` prints them. **Any error among them: nothing is written**,
`mf2 import: N error(s) that fr does not have now; nothing was written`,
exit 1. Otherwise the files are written, the new warnings printed, and the
summary is as before.

**"Brings", not "has" — a decision.** An error already in the files does
not stop an import that adds none: another language's error, or one raised
only because cargo could not name the features on a translator's machine
(`check` then checks with none, and `:currency` is an error there — before
and after alike). The review's case is an error the import brings.

**JSON and new ids — a decision.** The work order allows either adding
them or naming XLIFF. JSON import keeps plans/05 §6's rule (a new message
needs a file and a section, which flat JSON does not carry; XLIFF does,
and adds them there) — but a left-out id is now a **refusal**: `mf2
import: 1 message(s) fr does not have yet were left out: farewell; JSON
import changes the messages a language has, and XLIFF adds the others
where the source has them (`mf2 export fr --format xliff`)`, exit 1 after
writing the rest (as XLIFF's unit refusals do). An id the source lacks is
named apart. XLIFF's unit-by-unit refusals (§6.3) are unchanged; what they
leave is checked like the rest. A flat-JSON *locale* is still replaced by
the document (unchanged).

| Test | What it proves | Result |
|---|---|---|
| `tests/commands.rs` `import_refuses_what_check_would_refuse` (new) | the review's `$nom` case (JSON): exit 1, `$nom is not an input of the source message … (in greeting, locale fr) [undeclared-variable]`, "nothing was written", file byte-identical; **negative control:** a clean `Salut, {$name} !` lands; an existing `$nom` in another message does not stop a clean import | PASS |
| `tests/commands.rs` `import_names_xliff_for_messages_a_language_lacks` (new) | a new id is left out and XLIFF named, an id the source lacks named apart, exit 1, the rest written | PASS |
| `tests/xliff.rs` `import_refuses_a_translation_that_drops_markup` (new) | the review's `terms` case (XLIFF): the target without the link's `<pc>` is refused (`[dropped-markup]`, nothing written); **negative control:** with the `<pc>` kept it lands, `0 message(s) changed, 1 added` | PASS |
| `xliff.rs` unit test `without_the_data_check_an_edited_code_lands` (adapted) | the negative control for `xliff-code-edited` still holds, on the returned rewrites | PASS |
| every existing import test (round trips of the reference workload, each `xliff-*` code, the JSON round trip, the book's `import --dry-run`) | unchanged behavior where nothing is wrong | PASS |

**Cost, measured:** `mf2 import pl pl.json --dry-run` on the 1,600-message
reference workload (4 locales), one message changed, debug build: 1.18 s
wall, load average ≈ 11 (other forks building). For scale, `mf2 check
--features fn-number` on the same corpus took 20.1 s under the same load —
it compresses every catalog with brotli 11 in a debug build (the review's
#18; C6's business), which the import's checks skip.

**Commands run** (on the branch, `CARGO_BUILD_JOBS=2`): `cargo test -p
mf2-cli` (all 106 pass), `cargo test -p mf2-build`, `cargo clippy -p
mf2-build -p mf2-cli --all-targets -- -D warnings`, `cargo fmt --all
--check`, `MF2_CLI_API_WRITE=1 cargo test -p mf2-cli --bin mf2 api_txt`
(`crates/mf2-cli/api.txt` gains `--features <LIST>` under `mf2 import`),
`cargo xtask docs --no-build` (the book's `import` output is unchanged).

**On `main`, the three together** (picked onto `c0c1d43` without a
conflict, 2026-09-28; `CARGO_BUILD_JOBS=3`): `cargo xtask ci` green, first
run — no test failed, `mf2-catalog`'s timing test `linear.rs` included;
`api --check` "18 listings unchanged"; `package --check` "lists
unchanged". The full `cargo xtask docs` green: every sample compiled, the
samples' translation crates rebuilt against the new `mf2-build`. The corpora
`main` gained since `8f6569e` pass the new checks: `mf2 check --features
fn-number,fn-datetime,datetime-icu` on `examples/tui/i18n` and
`probes/p10-{ambient,single-crate,tr-in-crate}` reports nothing
(`probes/p10-links` holds only English, with nothing to compare).

**Verdict:** done — the review's `$nom` case refused; a negative control
per format; JSON names XLIFF.

**For the design (interpretation).** F3 (the translator workflow) can now
say: import checks, XLIFF adds, JSON edits.

## Part F — the 2.0 book (F1 with or after C8; the rest after D6)

| Task | Deliverable | Done when |
|---|---|---|
| **F1** MF2 for developers | A chapter on MF2 itself: <br>• the `.mf2` file (`@locale`, `---`, `[section]`); <br>• variables, functions and options; plurals and ordinals; selection on several values (gender); markup; <br>• `@do-not-translate` and translator comments; <br>• side by side with Fluent, ICU MessageFormat 1 and i18next; <br>• why messages are written by id, not extracted from code | compiled blocks; linked from Getting started |
| **F2** Reference pages | `mf2.toml` (every key); every lint, with an example and its fix; every command (extending `command-line.md`); every feature of `mf2` | a test that every lint and config key has a section (review, and [04](04-leptos-integration.md) §11's lesson) |
| **F3** The translator workflow | Export, translate, import (JSON, XLIFF 2); pseudo-locales; `stats`; checks in CI | compiled / `run=` blocks |
| **F4** Testing and troubleshooting | **Testing:** `with_locale`, `TestBackend` snapshots in several languages, pseudo-locales for layout. **Troubleshooting:** a stale manifest, empty text, pages stuck in the default language, the in-crate `tr!` rule | compiled blocks |
| **F5** First pages and upgrading | A one-crate landing page; Getting started before the crate map; `[output.html.playground] runnable = false`; mechanism moved into callouts; **"Upgrading from 1.x"** (crates, features, paths, the API changes) | `mdbook build`; `cargo xtask docs` |

## Part G — removal, release, exit (G1 after Parts B–F; G2 after G1; G3 after G2; G4 last)

| Task | Deliverable | Done when |
|---|---|---|
| **G1** 18 crates become 16 | **Code and lists:** <br>• the four shims deleted; <br>• `xtask/src/packages.rs`, `msrv`, `docs`, `api`, `package` and `release` updated for the 14 crates plus the two Leptos UI helpers; <br>• `version = "2.0.0"`, with `=2.0.0` pins. <br>**Docs:** <br>• a `## 2.0.0` changelog entry absorbing 1.1.0's items; <br>• `docs/versioning.md`: what 2.x promises (per mode, the generated items, the Leptos lines), and where the releases stand; <br>• `CLAUDE.md`'s client-path list names `mf2` | `cargo xtask ci` and `docs` |
| **G2** The release checks | `cargo xtask release` as a dry run at 2.0.0 for the 16 crates: <br>• every name ours; <br>• semver-checks against 1.0.0 treating it as a major; <br>• `docs-rs`, `msrv`, `msrv --below`, `package --check --test`. <br>**Owner questions, asked here:** the 2.0.0 stubs for `leptos-mf2` / `mf2-axum`; reserving the two unpublished names. The publish stays the owner's | every existing negative control still refuses; the dry run green |
| **G3** Cold start | A fresh agent with only the book and `mf2 init` (the crates through `[patch.crates-io]` at `cargo xtask package`'s output) builds the CLI, the TUI and the Leptos application. Each stumble is fixed, and the run repeated | a clean run, recorded |
| **G4** Exit | `plans/phase-10-results.md`; `P10` in `conformance/src/matrix.rs`, `current_phase = "P10"`; the probes deleted; the master plan's "Later" reviewed | written; the harness green at `P10` |

## The order that keeps `cargo xtask ci` green

- **A0–A9:** plans, docs, excluded workspaces and xtask code, each with its tests.
- **B1 is one atomic commit.** A type cannot live in two crates, and the shim keeps every old path
  working, in examples and the book too.
- **Internal users move one at a time behind the shims** (B2–B4).
- **New APIs land before old ones go.** An old API is removed only in the commit that rewrites the
  page using it (C8, D6).
- **A commit that touches the book also runs the full `cargo xtask docs`,** since `ci` runs it with
  `--no-build`. A commit that touches the client runs `size`, `b12`, `codegen-matrix`,
  `scenarios`, the e2e checks, `l6-web` / `l7-web`, `churn` and `leptos-0-8`.
- **The shims go last** (G1), when nothing names them.

## Risks

| Risk | Mitigation |
|---|---|
| Feature unification turns `ssr`, `native`, `ratatui` and `axum` on together in a workspace | Every combination except the exclusive modes must compile, checked by a feature matrix. The generated module handles `ssr` + `native` |
| `links` is fragile under rust-analyzer or cargo-leptos | A2 first; fallback in the gate table |
| The function table costs bytes, or helper crates break hydration | A7 first; e2e on both lines; back to the owner with question 13's other options |
| A future `ratatui-core` 0.2 | An opt-in line feature, as with Leptos |
| The `links` name makes two `mf2` majors unable to share a graph | The major is in the name (`mf2-v2`); in practice two majors of this library in one application cannot work anyway |
| `mf2` is client-path code, so its `deny` lints cover the native modules too | Scoped `allow`s with reasons: the native panic from "Decided without asking", and std-only formatting in `native` / `axum` |
| New crate names hit crates.io's rate limit | Two new names only (the Leptos UI helpers) |
| The CLDR matching data is larger than expected on the client | Only the corpus's languages go in; measured; fallback in the gate table |

## Reuse

| Module | What is reused |
|---|---|
| `crates/leptos-mf2/src/{tr,arg,dynamic,markup}.rs` | moved as they are |
| `text.rs` | `with_active_text`, `format_with`, `with_scratch`: the ambient path |
| `state.rs` | `Setup`, `install`, `context_for`, `lookup_locale`: the matcher's web half |
| `catalog.rs` | the server store, and `current()` |
| `crates/mf2-native/src/locale.rs` | `match_locale`, `MULTI_SCRIPT`: the matcher's native half, replaced by C3 |
| `native.rs` | `load`, `system_time_zone` |
| `crates/mf2-ratatui/src/lib.rs` | the parts sink (markup stack, `Style::patch`, line breaks) |
| `crates/mf2-runtime/src/format.rs` | `Formatter::simple`, and the `Sink::push_catalog_text` seam pattern |
| `crates/mf2-catalog/src/reader.rs` | `Catalog::from_static`, `content_hash` |
| `crates/mf2/src/{corpus,message}.rs` | as they are |
| `crates/mf2-build/src/{codegen,features,build,report}.rs` | `Features::from_vars`, `Emit`, `to_cargo_warnings` |
| `crates/mf2-macros/src/expand.rs` | `emit` |
| `crates/mf2-cli/src/{init,cargo}.rs` | as they are |
| `crates/mf2-axum/src/*` | as they are |
| xtask `docs`, `size`, `b5`, `api`, `docs_rs`, `msrv`, `packages`, `package`, `release`, `ci`, `codegen_matrix`, `scenarios`, `leptos_0_8`, `cldr_sync` | as they are |
| `tools/i18n-fixture/tests/ui/` | the trybuild cases |
| `bench/runtime-bench` | B10 |

## Standing

* **No agent publishes, pushes, tags or rewrites history** (CLAUDE.md). The
  2.0.0 publish is the owner's, by `cargo xtask release --publish`.
* **`vendor/` and `comparison.md` are never committed;** stage files by name.
  The trippy port is edited only in C9, and stays in `vendor/`.
* **Owner questions are asked when they come up,** in plain English,
  and their answers are recorded here before the next task starts.
* **Leptos 0.9's release** is taken as a patch within the `leptos` feature's line, as the 1.x
  policy did.

## Exit (master plan §9, P10)

- [ ] every UX row falls for the four samples, against A1's 1.x counts (C8, D6)
- [ ] one crate: applications name `mf2` (+ `mf2-build`), with 16 published crates (B, D1, G1)
- [ ] the native ambient language, `Locale`, the Ratatui conversions and theme, and a one-line build (C1–C7)
- [ ] the web defaults, generated setup, typed languages and starters (D2–D5)
- [ ] one CLDR-based matcher everywhere (C3)
- [ ] the silent failures fixed (E1–E4)
- [ ] the book: native and web on 2.0, the MF2 guide, reference, translator workflow, testing, troubleshooting, upgrading (C8, D6, F1–F5)
- [ ] the gates held: web budgets within tolerance; `tui-gate` allocations, time and size (method §3)
- [ ] the trippy port finished and recorded (C9)
- [ ] the cold start clean (G3)
- [ ] `cargo xtask release` green as a dry run at 2.0.0; the publish is the owner's (G2)
- [ ] `cargo xtask ci` green; the harness green at `current_phase = "P10"`; `plans/phase-10-results.md` (G4)
