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
   | Helper crates and the function table (B1) | A7 | the above; e2e green on both Leptos lines | back to the owner with the other two options from question 13 |
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

## Part A — the plan, baselines and probes (A0 first, then A1; A2–A7 in any order; A8 after A2–A7)

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

- **A0–A8:** plans, docs, excluded workspaces and xtask code, each with its tests.
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
