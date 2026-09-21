# P0.8 — load + lookup speed: RESULT

**Question** (plans/07, plans/06 §5, plans/02 §6):
* **Load.** How long does `Catalog::new` take for 1,600 messages, split plus
  validation? Does it meet **B9** (≤ 1 ms on a 4×-throttled CPU)?
* **Lookup.** How fast are a simple lookup and a 1-argument pattern format, and
  how many allocations do they make? **B10** asks for ≤ 100 ns with 0 allocs,
  and ≤ 500 ns with ≤ 1 alloc, natively.
* **UTF-8.** Which validation strategy should the reader use: one
  `str::from_utf8` over the pool at load, or per string on access?

Inputs: P0.7's four workload catalogs in its recommended layout (built in
memory from `probes/p0-07-catalog-encoding/corpus`), read by P0.3's runtime
(`probes/p0-03-runtime-floor/rt`). Both are read-only path dependencies. The
runtime feature `utf8-per-access` switches the strategy.

**All timings were taken on a shared 8-core machine while other agents
worked** (load average 2–5). Each set is one command, listed below, so it can
be re-run in a quiet window.

## Verdicts

| Threshold | Measured (per-access UTF-8, the recommendation) | Verdict |
|---|---|---|
| **B9** load ≤ 1 ms at 4× throttle (validate + index, 1,600 messages) | Chromium, 4× CPU throttle: `Catalog::new` takes **0.073–0.079 ms** for all four locales. A whole install (the Uint8Array copy into wasm memory + `Catalog::new`) takes **0.040–0.125 ms**, including the first locale switch | **met** |
| B9, first-ever install in a fresh page | 0.160 ms copy + **1.120 ms** `Catalog::new` at 4× (median of 9; range 0.25–2.3 ms). The **next** install (switch to pl) is 0.125 ms, so ≈ 1 ms is a one-off first-execution cost (V8 compile/tier), not load work | **not met as a cold number**; see caveat 1 |
| **B10** simple ≤ 100 ns, 0 allocs (native) | **20.7 ns** (en), 28.4 ns (ar-XB); **0 allocs** over all 1,256 simple ids | **met** |
| **B10** 1-arg pattern ≤ 500 ns, ≤ 1 alloc (native) | **93.5 ns** (en), 139 ns (ar-XB) into a reused `String`: **0 allocs**. Into an owned `String::with_capacity(128)`: 106 ns, **1.02 allocs** (1 alloc; the 2 % of outputs longer than 128 B realloc once) | **met** |

With **eager** validation instead, B10 is also met: 22.8 ns and 100 ns, same
allocations. B9 is met for en and ar-XB (0.11 and 0.26 ms) but **not for
en-XA (1.10 ms)**, and pl is close at 0.78 ms. The whole-pool `from_utf8` is
slow on non-ASCII text. That, and not the rest of the load, decides the
strategy.

## UTF-8 strategy — decision: validate per string on access

| | eager: one `from_utf8` over the pool at load | **per access**: `from_utf8` on each string read |
|---|---|---|
| native `Catalog::new` en / pl / en-XA / ar-XB | 12.0 / 143.5 / 184.4 / 100.1 µs | **5.7 / 6.7 / 6.9 / 6.1 µs** |
| Chromium 4× `Catalog::new` en / pl / en-XA / ar-XB | 0.110 / 0.783 / 1.103 / 0.263 ms | **0.079 / 0.078 / 0.075 / 0.073 ms** |
| Chromium desktop `Catalog::new` | 0.039 / 0.217 / 0.302 / 0.078 ms | **0.030 / 0.024 / 0.023 / 0.022 ms** |
| first locale switch (copy + load), 4× / desktop | 2.280 / 0.725 ms | **0.125 / 0.140 ms** |
| allocations / copies at load | 1 alloc (≈ 12 KB structure head) + memmove of the pool; ≈ 12 KB slack | **0 allocs, 0 copies** (the fetched buffer is kept as is) |
| native simple lookup en / ar-XB | 22.8 / 10.9 ns | 20.7 / 28.4 ns |
| Chromium 4× simple / 1-arg / select | 98 ns / 1.02 µs / 4.32 µs | 143 ns / 1.30 µs / 5.38 µs |
| runtime code size (P0.3, no_std Δ gz) | 7,408 B | **7,022 B** |
| corrupt byte in the pool | whole catalog rejected | that one string yields the fallback; the rest works |

**Why per access.** Whole-pool validation costs 2.6–5.8 µs natively on the
ASCII `en` pool. On non-ASCII text it costs 60–300 µs natively and up to 1 ms
at 4× throttle, because Polish letters, accented pseudo text and RLO/PDF
controls defeat `from_utf8`'s ASCII fast path. The per-access cost is +0–45 ns
per string read at 4× throttle. The reference app formats about 1,860 sites at
hydration, which adds about 0.1 ms; eager validation costs 0.03–1.0 ms more at
every load and every locale switch. Break-even is well above one page's worth
of formatting. Per access also makes the load zero-copy and removes
`from_utf8` from the load path (−386 B gz). Safety is unchanged: no `unsafe`,
no panic, and a bad slice gives the fallback representation.

**"Trusting nothing else" (the floor).** Without `unsafe` there is no
unvalidated `&str`. The per-access load, which validates the structure but not
the pool, *is* the floor for load. The cost of the whole-pool pass alone
(`from_utf8(pool)`) is 2.6–5.8 µs native for en, 93–225 µs for pl and
134–297 µs for en-XA. In Chromium it is 0.009–0.018 ms desktop and
0.030–0.042 ms at 4× for en. (Criterion and repeated runs vary by up to 2× on
the shared machine.)

## Native (criterion 0.8.2 + a counting allocator; `out/native.txt`, `out/allocs-*.md`)

| operation (en unless noted) | eager | per-access | allocs |
|---|---|---|---|
| `Catalog::new` en / pl / en-XA / ar-XB | 12.0 / 143.5 / 184.4 / 100.1 µs | 5.7 / 6.7 / 6.9 / 6.1 µs | eager 1 (11.8–12.4 KB) · per-access 0 |
| simple, fast path (`Formatter::simple`) | 22.8 ns | 20.7 ns | 0 |
| simple via `write` into a reused `String` | 25.1 ns | 28.3 ns | 0 |
| 1-arg pattern into a reused `String` | 100.2 ns | 93.5 ns | 0 |
| 1-arg pattern into a new `String::with_capacity(128)` | 117.4 ns | 106.3 ns | 1.02 |
| 1-arg pattern into a new `String::new()` | — | — | 3.07 (growth reallocs) |
| select, plural on `$count`, reused `String` | 236 ns | 317 ns | 0 |
| ar-XB: simple / 1-arg / select | 10.9 / 75.9 / 291 ns | 28.4 / 139 / 514 ns | 0 / 0 / 0 |

Timing used `cargo bench` with the release profile (fat LTO, 1 CGU), measuring
over all ids of each class in turn. The allocation counter wraps `System` and
counts `alloc` + `realloc`.

## Browser (Chromium 143.0.7499.4 via Playwright 1.63.0; `out/browser.md`, `out/browser.json`)

Setup:

* The harness is `web/` (wasm-bindgen 0.2.128, matching the CLI). It is built
  with the 06 §3 size profile, then `wasm-opt -Oz`: 14.1–14.5 KB gz including
  std and the bindgen glue.
* Files are served by `page.route` on a `localhost` origin with COOP/COEP, so
  the page is cross-origin isolated and `performance.now()` resolves 5 µs.
* Each row is 9 fresh pages; the tables show medians. The 4× rows use CDP
  `Emulation.setCPUThrottlingRate {rate: 4}`.
* Each page, in order:
  1. makes a first call with an invalid 64-byte buffer (lazy compilation of
     `load`);
  2. makes the first real install of `en`, split into the Uint8Array → wasm copy
     and `Catalog::new` on the moved buffer;
  3. switches to `pl` (copy + load);
  4. repeats installs 40 times;
  5. times 200 × `Catalog::new` per locale on a staged copy, minus 200 clones;
  6. times 200 × `from_utf8(pool)`;
  7. runs 400k simple / 100k pattern / 50k select lookups, median of 5.
* The page reported 0 errors. A spot check formatted
  `Is ⁨Ada⁩ a configuration.` with FSI…PDI and chose plural variants correctly.

| UTF-8 | CPU | first call (lazy compile) | first install en: copy + new | first switch to pl | warm install en | `Catalog::new` en / pl / en-XA / ar-XB | simple | 1-arg | select |
|---|---|---|---|---|---|---|---|---|---|
| eager | desktop | 1.015 ms | 0.095 + 0.395 ms | 0.725 ms | 0.065 ms | 0.039 / 0.217 / 0.302 / 0.078 ms | 34 ns | 456 ns | 1.36 µs |
| eager | 4× | 2.940 ms | 0.105 + 1.505 ms | 2.280 ms | 0.045 ms | 0.110 / 0.783 / 1.103 / 0.263 ms | 98 ns | 1.02 µs | 4.32 µs |
| **per-access** | desktop | 1.060 ms | 0.135 + 0.305 ms | **0.140 ms** | 0.055 ms | **0.030 / 0.024 / 0.023 / 0.022 ms** | 53 ns | 430 ns | 1.69 µs |
| **per-access** | 4× | 2.760 ms | 0.160 + 1.120 ms | **0.125 ms** | 0.040 ms | **0.079 / 0.078 / 0.075 / 0.073 ms** | 143 ns | 1.30 µs | 5.38 µs |

## Commands (from `probes/p0-08-load-lookup/`)

```sh
scripts/native.sh                 # allocation counts + criterion, both strategies -> out/native.txt,
                                  #   out/allocs-{eager,per-access}.md, out/criterion-*.txt; also writes
                                  #   out/web/{<tag>.mf2b, fixtures.json} for the browser
scripts/build-web.sh              # wasm harness twice (eager / per-access) -> out/pkg-*/
node browser/run.mjs --reps 9     # Chromium desktop + 4×, both strategies -> out/browser.{md,json}
```

`browser/run.mjs` imports `launch` from `tools/e2e/lib/browser.mjs`, which it
uses read-only; nothing was installed or modified there.

## Caveats

1. **The cold first install.** The first `Catalog::new` on real data in a fresh
   page takes 0.3 ms on desktop and 1.1 ms at 4× (range 0.25–2.3 ms). This is
   so even after `load` itself was compiled by a warm-up call. The next install
   (0.125 ms including the copy) shows the load work itself is ≈ 0.08 ms. The
   remainder is one-off first-execution cost of the callees: V8 lazy
   compilation and baseline-tier code. That cost belongs to the code, not the
   catalog. In a real app it will be shared with, or hidden by, whatever runs
   first. It should be re-measured inside P0.2's app, where the preloaded
   catalog is installed during boot.
2. Browser lookups are informative only; B10 is native. At 4×, wasm simple
   lookup is 98–143 ns and a 1-arg pattern 1.0–1.3 µs. That is about 5–10× the
   native figure, which is normal for wasm without SIMD at a quarter of the CPU.
3. The shared machine: repeated criterion runs of the same code differed by up
   to 2× (for example `from_utf8(pool)/pl` at 93 µs vs 225 µs). The ranking of
   the strategies never changed.
4. The runtime is P0.3's skeleton, which formats integers with neutral digits.
   Number formatting (P0.5) will add to pattern and select times.

## Recommendations (02 §6, Phase 2/3)

1. **Validate UTF-8 per string on access.** Keep the fetched buffer as the
   catalog, with no split copy and no `Arc<str>`. Structure validation stays in
   `Catalog::new`: header, sections, INDEX monotonicity, LOCALE and FUNCS. Each
   pool access is `str::from_utf8(slice)`, and a bad slice gives the fallback.
   This is a departure from F2/F4's "one `str::from_utf8` pass, pool as
   `Arc<str>`". Update F2/F4 accordingly (C2).
2. **Define B9 as the warm load**, validate + index, which measures ≤ 0.08 ms at
   4× for every locale. Track the cold first install separately, as part of the
   app's boot budget in P0.2 / P6, where it can be attributed to code rather
   than data.
3. **The `String` path pre-sizes.** A new `String::new()` costs about 3
   allocations per 1-arg format through growth. `leptos-mf2`'s `to_string` path
   should reserve a capacity that covers typical outputs (128 B covers 98 % here)
   or a computed bound, so that it makes exactly 1 allocation. The view paths
   write into the shared scratch buffer (06 §4 technique 12) and make 0.
4. Keep `Formatter::simple` as the entry point for the 79 % case. It is 21 ns
   native with 0 allocations, and 53–143 ns in the browser.

## Departures (plans not edited)

* 02 F2/F4: the pool is not converted to `str` at load (recommendation 1).
* The browser harness was built with wasm-bindgen, as asked. It is timed from
  JS around exported loops, not from inside wasm.

## Owner questions

None.
