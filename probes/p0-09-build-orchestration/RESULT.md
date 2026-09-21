# P0.9 build orchestration: RESULT

Probe for `plans/06-size-and-perf.md` §5 (P0.9) and `plans/07-phase-0-work-order.md`.
It settles **D8** (`plans/00-master-plan.md` §8) and C3's no-go condition: "P0.9
cannot give correct incremental rebuilds under cargo-leptos". The code is
throwaway. Nothing in `plans/` was edited. The proposed changes are listed
under "D8 outcome".

## Question

Does the design of `plans/05-tooling.md` §4 work end to end? The design is an
i18n crate with `locales/` and a `build.rs` that writes a manifest and catalogs
to `OUT_DIR`, plus a generated module whose exported `tr!` `macro_rules!`
forwards to a proc-macro with the manifest's absolute path baked in. The
questions are:

* Does it work under cargo-leptos's dual build, with a second crate and with
  rust-analyzer?
* Does it rebuild correctly after every kind of locale edit?
* Is the macro's overhead within ≤ 2 s per 2,000 sites?

## Verdict

| Threshold (06 §5) | Verdict |
|---|---|
| **edit → rebuild correctness** | **met**. Every scenario rebuilt exactly what depends on the change: text edit, translation-only edit, added variable (the call sites fail with a spanned error), added message, touch without a change, and no change. This holds under plain cargo, `cargo leptos build` and `cargo leptos watch`. Both builds always saw byte-identical manifests. There is one config requirement: **`cargo leptos watch` needs `watch-additional-files = ["<i18n crate>/locales"]`**, otherwise it does not rebuild after a locale edit (below) |
| **≤ 2 s macro overhead per 2,000 sites** | **met**. `cargo check` of the 2,000-site app costs **+0.2 to +0.3 s** (median) against the same app with a trivial proc-macro or with no macro at all. The proc-macro's own time is **115 to 450 ms** per 2,000 expansions, and the manifest is read once per rustc process. A debug `cargo build` shows no difference beyond noise (±10 s under load) |

**D8 is settled as designed, with four amendments** (see D8 outcome):

* bake the manifest hash beside the path;
* add a relocation fallback in the macro;
* scaffold `watch-additional-files`;
* keep the client half of the generated module free of catalog hashes.

As far as P0.9 is concerned, the C3 no-go condition does **not** apply.

## What was built

`probes/p0-09-build-orchestration/` is its own workspace, excluded from the
root workspace. It is laid out as the target design, with stand-ins for the
product crates:

| Crate | Stands in for | What it does |
|---|---|---|
| `crates/catalog` (`p09-catalog`) | `mf2-catalog[manifest, writer]` | message model; `manifest.mf2m` (ids → `MsgId`, slot names, markup names, functions, `manifest_hash` = FNV-1a 64 as in 02 §3); a probe-sized `.mf2b` catalog encoding |
| `crates/build` (`p09-build`) | `mf2-build` | **loader for the working grammar of 05 §2**: frontmatter/`@locale`, `[section]` ids, continuation lines, escaped line breaks, comments, `@param`. Also a small MF2 scanner for placeholders, options, markup, `.input`/`.local`/`.match`, variables, markup names and functions. It derives the manifest from `en`, flattens translations onto it (D5), checks that translations use no undeclared variable or markup, writes content-hashed catalogs, and generates the module. Outputs are written only when changed |
| `crates/macros` (`p09-macros`) | `mf2-macros` | `tr_impl!`: reads the manifest from the baked path, caches it per process keyed by path and verified against the baked hash, checks the id (with did-you-mean) and that the argument set equals the variables (unknown, missing and duplicate), and emits `$crate::__mf2::tr(MsgId(n))` / `tr_args(MsgId(n), [ArgValue::from(..)])` in slot order. `tr0_impl!` is the trivial timing control |
| `crates/mf2` (`p09-mf2`) | the `mf2` facade | `MsgId`, `Tr` (4 B, `Copy`, `const fn tr`), `TrArgs`, `ArgValue`, server catalogs + formatter, tachys glue after the audit prototype |
| `crates/i18n` (`p09-i18n`) | the application's i18n crate | `locales/` (workload-gen: `en`, `pl`, `en-XA`, `ar-XB`, 1,600 messages × 18 files), `build.rs` = `p09_build::Build::new().source_locale("en").run_or_exit()`, `lib.rs` = re-export of the proc-macros + `include!(…/mf2_generated.rs)` |
| `crates/second` (`p09-second`) | a second crate of the app | depends on the i18n crate only. It calls `tr!` in a `static`, in plain code and in a view, and runs client code from the app's hydrate boot |
| `apps/tr` (`workload-app-tr`) | the app | the workload-gen app, **M = 2,000 sites, seed 1**, template `templates/tr/` (data only). It is cargo-leptos SSR + hydrate, 3 `#[lazy_route]`s, and has the probe's own `server/main.rs`: the shell carries `<meta name="mf2-manifest">`, plus `/second`, `/manifest`, `/catalogs`, `/i18n/<file>` |
| `apps/trivial`, `apps/direct` | timing controls | the same 2,000 sites through `tr0!` (trivial proc-macro), and hand-expanded (no macro; `MsgId` = workload-gen's `{{index}}`, the same numbering) |

The generated module (`OUT_DIR/mf2_generated.rs`, path mode) contains:

* `MANIFEST_HASH`;
* `SOURCE_LOCALE`;
* `pub use ::p09_mf2 as __mf2;`;
* `LOCALES` (tag, rtl);
* under `#[cfg(feature = "ssr")]` only: `CATALOGS` (file name, content hash, `include_bytes!`) and `install()`;
* and:

```rust
#[macro_export]
macro_rules! tr {
    ($($t:tt)*) => {
        $crate::__tr_impl!("/…/target/debug/build/p09-i18n-2500f184bd24f399/out/manifest.mf2m" 0x0cbd1d47de048978u64 ; $crate ; $($t)*)
    };
}
```

Sizes for 1,600 messages: the manifest is 61,361 B; the catalogs are en
60,421 B, pl 75,969 B, en-XA 113,172 B and ar-XB 71,665 B (probe encoding, not
02's); the generated module is 2,280 B. `build.rs` takes 60 to 420 ms, because
it is compiled unoptimised as a build dependency.

## Commands

Everything runs from `probes/p0-09-build-orchestration/`. It uses toolchain
1.98.1 stable (from the root `rust-toolchain.toml`), cargo-leptos 0.3.7,
wasm-bindgen 0.2.128 and rust-analyzer 1.98.0. Leptos is 0.8.20, leptos_router
0.8.15, leptos_axum 0.8.10, axum 0.8.9, syn 3.0.6 and quote 1.0.47. **All
timings were taken on a shared 8-core machine under load from five other
agents (load average 4 to 5). Each one is a single script and should be re-run
in a quiet window.**

```sh
scripts/regen.sh                      # cargo xtask gen-workload all -m 2000 -t templates/{tr,trivial,direct} --out gen; sync
scripts/scenarios.sh leptos           # every rebuild scenario under `cargo leptos build` (S0–S9 below)
scripts/scenarios.sh plain            # the same under plain cargo (the two builds cargo-leptos runs, one after the other)
scripts/watch-check.sh                # `cargo leptos watch --split`, driven non-interactively
scripts/time-expansions.sh check-ssr 7     # 2,000 expansions: tr vs trivial vs direct, interleaved, medians
scripts/time-expansions.sh check-wasm 5
scripts/time-expansions.sh build-ssr 5
scripts/time-spans.sh 4                    # A/B: expansion spans (user literal vs call site) on debug build time
P09_MANIFEST_MODE=inline scripts/time-expansions.sh check-ssr 5
scripts/ra-check.sh                   # rust-analyzer diagnostics + negative control
scripts/relocate-check.sh             # move target/ and rebuild (and P09_MANIFEST_MODE=inline …)
scripts/build-leptos.sh --split; scripts/serve.sh   # browser check (console: "mf2: manifest ok <hash>")
```

The building blocks are:

* `scripts/edit.sh {text|pl|addvar|addsite|addmsg|touch|revert}`;
* `scripts/build-plain.sh` (dirty reasons from `cargo build -v`);
* `scripts/build-leptos.sh` (`CARGO_BUILD_JOBS=2 cargo leptos build`, because
  cargo-leptos runs two cargos in parallel);
* `scripts/hashes.sh`;
* `scripts/serve-check.sh`.

## Observations

### 1. Dual build (ssr + hydrate): identical manifests

cargo-leptos builds the server with `--target-dir target` and the client with
`--target-dir target/front --target wasm32-unknown-unknown`. As a result:

* there are **two OUT_DIRs, two copies of the proc-macro, and two different
  baked paths**;
* `--split` changes the front's flags and so creates a third OUT_DIR;
* plain cargo creates two more.

At every step of every scenario, all of them held a **byte-identical
manifest** (sha256 `44c0765b2662…` at baseline) and the same `MANIFEST_HASH`
and catalog file names, for example:

```
debug/build/p09-i18n-2500f184bd24f399/out                          manifest-sha=44c0765b2662  0x0cbd1d47de048978  en.cff05039a8857605.mf2b …
front/wasm32-unknown-unknown/debug/build/p09-i18n-ebfa9263c8e6c039/out  manifest-sha=44c0765b2662  0x0cbd1d47de048978  en.cff05039a8857605.mf2b …
```

At runtime, the `--split` build loaded in Chromium via the Playwright MCP
browser logs `mf2: manifest ok bd9a2a059dcdd6c8`: the hash compiled into the
wasm equals the one the server rendered into the page. It also logs
`p09-second: 2 labels; first MsgId 1`. Both appear on `/` and on the lazy
route `/r2`, with no warnings (only a favicon 404).

### 2. Incremental rebuild correctness

Every step of both scenario runs is shown in full in
`scripts/scenarios.sh leptos|plain`. Times are wall-clock under load, debug
profile.

| # | Scenario | Expected | Observed: `cargo leptos build` | Observed: plain cargo (ssr / wasm) | Met? |
|---|---|---|---|---|---|
| S1 | no change | nothing rebuilds; i18n `build.rs` does not run | 3.4 s; no crate compiled; no build script ran (cargo-leptos reruns wasm-bindgen; the site wasm is byte-identical) | 0.4 s / 0.2 s, all fresh | ✔ |
| S2 | `touch en/common.mf2` (mtime only) | correct output; ideally no rebuild | `build.rs` reruns (cargo: *"the file `crates/i18n/locales` has changed"*); i18n + second + app recompile in **both** builds; 10.2 s. Server binary and wasm **byte-identical** to before | 11.2 s / 4.6 s, same set; wasm byte-identical | ✔ correct; rebuild cost noted below |
| S3 | edit one message's text (`en`) | new text served; manifest unchanged | both OUT_DIRs: same `MANIFEST_HASH` 0cbd1d47…, `en.cff05039…` → `en.ec863d17…`; SSR count new = 1, old = 0; `/second` shows it; 7.9 s. **The wasm is byte-identical** (the client module has no catalog hashes) | 8.0 s / 5.3 s; the same, and the wasm is byte-identical | ✔ |
| S4 | add `{$extra}` to a message used by the second crate and the app | call sites fail with a good error | 1.1 s: `error: message `actions-joined-busy` needs argument `extra` (its variables: $extra, $title); write tr!("actions-joined-busy", extra = …, title = …)` → `crates/second/src/lib.rs:19:25`. Once that site is fixed: the **same error at both app call sites** (`c002.rs:64:149`, `c013.rs:30:33`, the only two). New hash 36af0782… in both OUT_DIRs (3.8 s for the second step) | same: 0.5 s / 0.4 s, then 2.2 s | ✔ |
| S5 | revert | back to the original state | hash 0cbd1d47… again, same catalog names; 12.7 s | 8.2 s / 6.0 s; wasm byte-identical to S0; in the step-by-step run the server binary (sha `4952ffcdda3d`) was also **bit-identical** to before the edits | ✔ |
| S6 | new call site `tr!("p09.added-message", who = who)` in the second crate | unknown id | `error: unknown message id `p09.added-message`` at `extra.rs:6:17`; 1.7 s | same, 0.3 s | ✔ |
| S7 | add that message to `en` | builds; the ids after it shift and stay right | both OUT_DIRs `bd9a2a05…`; `/second` → `1188	Freshly added for Ada.`; `rooms.badges.to-add-preferences` moved from MsgId 1271 to 1272 and still renders its own text on the SSR page (count 1); the browser shows the wasm hash = server hash; 22.9 s | 18.6 s / 23.8 s (50.6 s / 22.0 s in the step-by-step run, under heavier load) | ✔ |
| S8 | edit `pl` only | pl catalog renamed, manifest unchanged, wasm unchanged | `pl.2a6d7643…` → `pl.75101da7…`; `P09_LOCALE=pl` SSR shows the edit; wasm byte-identical; 9.1 s | 8.6 s / 4.5 s; wasm byte-identical | ✔ |
| — | macro diagnostics (second crate) | real `compile_error!`s with spans | `unknown message id `activity-availabel`; did you mean `activity-available`?` · `message `actions-joined-busy` has no variable `$titel` (its variables: $title); did you mean `title`?` · `argument `title` given twice` · `tr! expects a string-literal message id` · `message `activity-available` has no variable `$extra` (its variables: none)`, each pointing at the offending token | — | ✔ |

**`cargo leptos watch --split`** was driven by `scripts/watch-check.sh`:

| Config | App-source edit | Locale edit (`edit.sh text`) |
|---|---|---|
| as generated (no extra watch paths) | rebuilt and restarted in 5 to 6 s | **no rebuild within 151 s**. cargo-leptos watches the Rust sources of the lib/bin packages and their local dependencies, not `locales/` |
| `watch-additional-files = ["crates/i18n/locales"]` in `[package.metadata.leptos]` | 5 s | **rebuilt after 10 s; `/second` serves the edited text** |

In both configurations, the manifest and catalogs came from `build.rs` as in
the table above. Without a `.gitignore`, cargo-leptos warns that its watcher
"works expensively" on changes under `target/`. A first attempt without one saw
no rebuild after an mtime-only `touch` of app source within 300 s. The cause
(no `.gitignore`, or touch-only) was not isolated. The probe has a
`.gitignore`.

### 3. The second crate sees changes

`p09-second` depends on `p09-i18n` only; the facade and the proc-macro reach it
through `$crate`. It rebuilt in every scenario, failed first in S4 and S6 (it
compiles before the app), and served the new text and the new message in S3
and S7. Its `static LABELS: &[Tr] = &[tr!(…), …]` shows that the expansion is
`const`-usable across crates.

### 4. rust-analyzer expands the macro

Method (`scripts/ra-check.sh`):

* `rust-analyzer diagnostics .` loads the workspace, runs the build scripts via
  its own `cargo check`, and expands proc-macros through the toolchain's
  `rust-analyzer-proc-macro-srv`. Then it diagnoses all 219 files, including
  the 2,000 `tr!` sites of `apps/tr`, the 2,000 `tr0!` sites and the second
  crate.
* Result: 414 s, **0 errors** (12 `inactive-code` + 2 `remove-unnecessary-else`
  weak warnings, none from a macro). There is no `unresolved-macro-call`,
  `unresolved-proc-macro` or `macro-error`.
* **Negative control**: a misspelt id and a wrong argument name were put into
  the second crate. rust-analyzer then reports exactly the macro's own errors
  at the right ranges:
  * `Error Ra("macro-error") … unknown message id `activity-availabel`; did you mean `activity-available`?`
  * `… no variable `$titel` … did you mean `title`?`
  * `… needs argument `title` …`

  This proves that rust-analyzer ran the proc-macro against the manifest at
  the baked path.
* Peak RSS of `rust-analyzer diagnostics` on this 3 × 2,000-site workspace was
  about 5 GB.

### 5. Wall-clock of 2,000 expansions

Only the app crate compiles (`touch src/lib.rs`, `CARGO_INCREMENTAL=0`). The
three variants are interleaved in every round. The table gives the median in
seconds, with the number of runs. "in-macro" is the proc-macro's own time for
all 2,000 expansions in one rustc process (`P09_MACRO_STATS`), including the
one manifest read.

| Measurement | `tr` (design) | `trivial` (proc-macro, no manifest) | `direct` (no macro) | Δ tr − best control | in-macro |
|---|---|---|---|---|---|
| `cargo check`, ssr, native (7 runs) | 5.46 | 5.12 | 5.14 | **+0.32 s** | 115 to 221 ms |
| `cargo check`, ssr, final macro (5 runs) | 5.59 | 5.38 | 5.37 | **+0.22 s** | 169 to 450 ms (median 177) |
| `cargo check`, hydrate, wasm32 (5 runs) | 4.47 | 3.82 | 3.56 | +0.65 / +0.91 s | 128 to 408 ms |
| `cargo build`, ssr, debug (5 runs) | 43.2 | 43.0 | 52.6 | within noise (runs span 34 to 64 s) | 170 to 259 ms |
| span A/B, `cargo build` debug (4 runs, paired) | user span 34.8 / 47.0 / 42.6 / 42.0 | call-site span 35.2 / 46.8 / 42.7 / 42.8 | 39.0 / 48.0 / 54.5 / 52.7 | no effect | — |
| inline fallback, `cargo check` ssr, first version (5 runs) | 9.88 | 5.25 | 5.15 | **+4.7 s** ✘ | 4,300 to 4,900 ms |
| inline fallback, raw-literal fix (5 runs) | 5.73 | 5.20 | 5.20 | +0.53 s | 330 to 694 ms |

Notes:

* A first 3-run debug-build series showed `tr` 61 s against 43 s. It did not
  reproduce in the 5-run series or in the paired span A/B, and `-C
  debuginfo=0` showed no gap either (41/34 s against 34/37 s). It was load. The
  expansion now carries the user's id-literal span anyway, so debug info and
  type errors point at the call site.
* **The ≤ 2 s threshold holds with a wide margin** in every configuration of
  the design. The largest median delta was +0.9 s (wasm check, with the first
  macro version); typical is +0.2 to +0.3 s. The proc-macro itself accounts
  for about 0.2 s.

### 6. Fragility found, and fallbacks

| Case | Observed | Fallback / fix |
|---|---|---|
| **The target directory moves** (a CI cache restored at another path, a container build mounting a different path, a renamed checkout) | Moving `target/` and rebuilding from the new location leaves the i18n crate **fresh** (its build script does not rerun), so the baked path is gone. The app then failed with **2,003 errors**: `mf2: cannot read the i18n manifest at `/…/target/debug/build/p09-i18n-d913…/out/manifest.mf2m`` | **Relocation fallback in the proc-macro** (implemented): when the baked path is missing, it looks for the same `build/<pkg>-<hash>/out/manifest.mf2m` under the profile directories that rustc (the macro's host process) received as `-L dependency=…/deps`, and verifies the baked hash. Server and wasm then rebuild from the moved directory with **0 errors** (`scripts/relocate-check.sh`) |
| Remote or distributed builds whose paths differ between expansion and build-script machines | not testable here | **Inline mode** (`P09_MANIFEST_MODE=inline`, implemented): the wrapper carries the manifest bytes as a byte-string literal, so no file is read. It passes the relocation check. Cost: the generated module grows from 2 KB to 126 KB, and in-macro time is +0.3 s per 2,000 sites. That cost comes from moving the literal across the proc-macro bridge once per expansion, so it scales with **manifest size × sites** (roughly +1.6 s per 2,000 sites at 10× messages). It must skip syn on the literal (the first version cost +4.7 s). Keep it as an opt-in, not the default |
| Long-lived rust-analyzer proc-macro server; editor saves an unchanged file; a stale manifest | — | **The manifest hash is baked next to the path.** It is the proc-macro cache key and is checked against the file (a mismatch gives "stale i18n manifest … rebuild the i18n crate"). A manifest change changes the generated wrapper's tokens, so rust-analyzer re-expands (its expansion cache keys on the input tokens) |
| Any locale change recompiles the i18n crate **and every dependent**, in both builds, including an mtime-only touch and translation-only edits. cargo has no early cut-off, and the build-script output file is rewritten every run | 8 to 23 s per `cargo leptos build` for this 2,000-site app in debug; 5 to 24 s per plain build. The **outputs are deterministic**: a touch gives a byte-identical server and wasm, and a text or translation edit gives a byte-identical wasm | Accept for P5a. Mitigations to consider in P5a/P7: dev hot reload of catalogs without cargo (P7's `mf2 watch`); splitting server-only catalog embedding into a crate only the binary depends on, so translation-only edits relink the server without recompiling the app. Not measured here |
| `cargo leptos watch` ignores `locales/` | no rebuild after a locale edit | `watch-additional-files = ["<i18n crate>/locales"]`, to be written by `mf2 init` and documented |
| plain cargo and cargo-leptos do not share artifacts (different compiler flags give different metadata hashes) | the first plain build after a cargo-leptos build recompiled everything (4 min 39 s) | not a D8 issue; document it for CI (use one or the other) |
| `$crate` passed through the proc-macro | works on stable; downstream crates need no dependency on the facade or the macro crate | — |
| sccache, `cargo package` | **not tested**. sccache is present but was deliberately not used (its cache lives outside the repository). By construction, an app crate's compile inputs include the i18n rlib, which contains the baked path and hash, so a manifest change cannot be served from cache. Across checkouts, the absolute path makes sccache miss (a hit-rate cost, not a correctness one); inline mode would remove it. For `cargo package`/publish, the build script runs in the consumer's `OUT_DIR`, so the baked path is the consumer's | verify in P5a if sccache matters |

## D8 outcome

**Settled as designed** (i18n crate + `build.rs` + generated `tr!` wrapper →
proc-macro), with these changes for C1/C2 to merge into
`plans/05-tooling.md` §4 and `plans/00-master-plan.md` D8:

1. The wrapper passes **the manifest hash beside the absolute path**
   (`__tr_impl!("<path>" 0x<hash>u64 ; $crate ; …)`). The proc-macro caches by
   path, verifies the hash, and reports a stale manifest instead of using it.
2. The proc-macro has a **relocation fallback**: the same `build/…/out` suffix
   under rustc's `-L dependency=` profile directories, hash-verified. An
   **inline-bytes mode** exists as a documented opt-in for remote-execution
   builds, with its manifest-size × sites cost stated.
3. `mf2 init` writes **`watch-additional-files = ["<i18n>/locales"]`** for
   cargo-leptos, and the docs say why.
4. The generated module's **locale table carries catalog file names and hashes
   only under `ssr`**. The client needs `MANIFEST_HASH` and tags only. This is
   what keeps the wasm byte-identical across translation edits. 05 §4 now
   lists "(tag, dir, catalog file name, hash)" for the table without a cfg.
5. Implementation notes for `mf2-macros`:
   * wrap multiple errors in a block, because several `compile_error!`s in
     expression position otherwise misparse and hide the later errors (found
     and fixed here);
   * give the expansion the id literal's span;
   * never stringify a large literal on a cache hit.

## Caveats

* The catalogs, the formatter and the MF2 scanner are probe-sized. The scanner
  accepts the workload's constructs, not full MF2, and the client renders no
  text: hydration adopts the server's text, as P0.2 established. Rendering and
  size were not P0.9's questions.
* Markup handlers are not modelled: `tr!` without handlers is accepted for
  markup messages. The manifest does carry markup names, and translations are
  checked against them.
* All timings are debug-profile, under load, single machine. The ≤ 2 s verdict
  rests on paired, interleaved medians and on the macro's self-time (about
  0.2 s), both far below the threshold. Release/LTO builds were not timed; the
  expansion cost is front-end work and does not grow with optimisation.
* rust-analyzer was checked through its CLI (`diagnostics`), not a live editor
  session. The invalidation argument for editors (a hash change changes the
  wrapper tokens) is by construction and was not observed in an LSP session.
