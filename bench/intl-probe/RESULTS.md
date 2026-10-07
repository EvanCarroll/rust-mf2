# intl-probe — results (Phase 4 A0, 2026-09-22)

The measurements of `plans/11-phase-4-work-order.md` A0: the `intl` client
option (`plans/03-runtime.md` §5.3) for the numeric functions, against the
Rust path. How to reproduce: [`README.md`](README.md). The owner decides
(decision 4); §7 separates this report's reading from its measurements.

**Trees measured.** `crates/` at **3fc4735** (Phase 3's exit, the frozen
runtime: "today's Rust path" when A0 was written) and at **732da4c** (the tip
on 2026-09-22 00:00: A1, A2, A3 and A5 committed — the runtime's API
additions, the number data, `mf2-fn-number`), each through
`INTL_PROBE_REV=<rev>` (a snapshot, so other agents' edits to `crates/` do not
move the figures). Items 2–6 ran on the 732da4c build; items 3a, 3b, 3c, 4,
5 and 6 were also run on the 3fc4735 build with the same results.

**Engines.** Chromium **143.0.7499.4** (Playwright build 1200, headless
shell) and Firefox **155.0** (build 1542), through Playwright 1.63.0; node
**v23.11.0** (V8, ICU 76.1, CLDR 46) for the items that do not need a
browser. **WebKit did not run**: Playwright's WebKit build 2359 installs
(into `target/ms-playwright`) but cannot start on this Debian 13 machine
without root —

```
MiniBrowser: error while loading shared libraries: libgstcodecparsers-1.0.so.0: cannot open shared object file: No such file or directory
```

(Playwright's host check also lists `libsoup-3.0.so.0`; Debian packages
`libgstreamer-plugins-bad1.0-0` and `libsoup-3.0-0`). `--browser all`
reports it as `SKIP`. Every WebKit figure below is therefore missing; the
harness runs it unchanged once those libraries exist.

**Tools.** rustc 1.98.1, wasm-bindgen 0.2.128, wasm-opt 120, twiggy 0.8.0,
GNU gzip 1.13. Machine: 8 CPUs whose clock drifts (1.3–4.1 GHz sampled
during the runs), shared with other agents' builds (1-minute load 2–7).

## 1. Size

`bench/intl-probe/scripts/build.sh` (06 §3: profile `wasm-release` — opt-level
z, fat LTO, 1 CGU, `panic = "abort"`, strip — then `wasm-bindgen --target
web`, `wasm-opt -Oz` with `bench/browser-no-fmt/check.sh`'s feature flags, `gzip -9 -n`;
the JS glue is `probe.js` plus the `inline_js` snippet, each gzipped). Every
variant has the same harness and exports; Δ against `base` (`:string` only).

At **732da4c** (`INTL_PROBE_REV=732da4c bench/intl-probe/scripts/build.sh`):

| Variant | wasm raw | wasm gz | Δ wasm raw | **Δ wasm gz** | JS raw | JS gz | Δ JS raw | **Δ JS gz** | **Δ total gz** |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base` | 71,933 | 33,976 | — | — | 16,855 | 3,718 | — | — | — |
| `rust` (core, neutral) | 82,575 | 39,358 | +10,642 | **+5,382** | 16,855 | 3,718 | +0 | **+0** | **+5,382** |
| `rust-loc` (+ `mf2-fn-number`: symbols, grouping, `:percent`) | 86,102 | 41,079 | +14,169 | **+7,103** | 16,855 | 3,718 | +0 | **+0** | **+7,103** |
| `intl` (core, neutral) | 85,760 | 40,743 | +13,827 | **+6,767** | 18,698 | 4,492 | +1,843 | **+774** | **+7,541** |
| `intl-loc` (+ locale symbols, `:percent`) | 85,803 | 40,759 | +13,870 | **+6,783** | 18,698 | 4,492 | +1,843 | **+774** | **+7,557** |
| `intl-cu` (+ `:currency`, `:unit`) | 87,268 | 41,453 | +15,335 | **+7,477** | 18,864 | 4,521 | +2,009 | **+803** | **+8,280** |
| `intl-codes` (`intl`, key design A) | 84,427 | 40,191 | +12,494 | **+6,215** | 19,693 | 4,981 | +2,838 | **+1,263** | **+7,478** |

At **3fc4735** (`INTL_PROBE_REV=3fc4735 …`, base 33,787 B gz): `rust`
+5,150 B gz (B1's numeric share, measured 5,137 in `bench/browser-no-fmt`), `intl`
+6,689 + 774 JS = **+7,463**, `intl-loc` +7,497, `intl-cu` +8,218,
`intl-codes` +6,128 + 1,263 JS = +7,391 (no `rust-loc`: `mf2-fn-number` did
not exist). The builds are deterministic: rebuilding gives the same bytes.

What the rows compare, in B gz (wasm + JS):

| Function set | Rust path | `intl` option | intl − Rust |
|---|---:|---:|---:|
| core (`:number` `:integer` `:offset`, neutral) | 5,382 | 7,541 (design A: 7,478) | **+2,159** (+2,096) |
| + locale symbols, grouping, `:percent` | 7,103 (`mf2-fn-number`, measured) | 7,557 | **+454** |
| + `:currency`, `:unit` | ≈ 10,038 *(estimate: 7,103 + P0.5's 2,935 for the Rust `:currency` + `:unit` layer; re-measure once A4 lands)* | 8,280 | **≈ −1,760** *(estimate)* |

* The localization layer costs Rust **+1,721 B gz** (`rust-loc` − `rust`;
  P0.5 predicted 1.7 KB) and the option **+16 B gz** (`intl-loc` − `intl`):
  with `Intl` the locale is free. `:currency` + `:unit` cost the option +694 B
  gz of wasm and +29 of JS; P0.5 measured +2,935 B gz for them in Rust.
* The core is where the option loses. Attribution (twiggy over the
  `wasm-syms` build of 3fc4735 before `wasm-opt`, raw bytes, Δ against
  `base`; LLVM inlines across these names, so the split is approximate and
  the ≈ figures are the author's estimates): the Rust
  numeric code is 10,717 B, of which the functions the option removes —
  `Decimal::round` 903, `write_digits` 257, `raw_precision` 245, `raw_fixed`
  125, `shift` 120, `write_parts` 142, `write_display` 98, the plural reader
  140, plus the display, operands and rule evaluation inlined into `resolve`
  (4,954) and `matches` (1,671) — come to ≈ 3–3.5 KB raw. The option keeps
  the rest (option tables, digit plan, operand rules, `:offset`, inheritance)
  and adds: the key builder 1,653 + name lookups ≈ 370, `format_parts`'s
  splitting 466, the value's text buffer ≈ 600, the boxed value and its
  downcast ≈ 480, the host glue ≈ 370, strings ≈ 440 — ≈ 5 KB raw, plus the
  JS glue. Some of that is the probe's (the boxed value; its own decimal
  text code standing in for the runtime's private `Decimal::add`); an option
  inside the runtime would save perhaps 1 KB raw of it, not enough to change
  the sign of the core's row.
* Two key designs (`fn/src/host.rs`): B (default) passes `name=value` pairs
  from the option tables the wasm already holds, A (`intl-codes`) passes
  one-character codes and keeps Intl's option names in the JavaScript. B
  saves 489 B gz of JavaScript and costs 552 B gz of wasm: the same total
  within 63 B.
* Neither design adds a `core::fmt` or panic symbol over `base` (twiggy
  names of every variant; one `copy_from_slice` length-mismatch path of the
  first cut was removed).
* **Catalog data the option drops**, per locale
  (`INTL_PROBE_REV=732da4c bench/intl-probe/scripts/data.sh` →
  `target/intl-probe/data/catalog-data.md`; `plural.cardinal` +
  `plural.ordinal` and A2's `number.symbols` + `number.patterns` for
  `:percent` and `:currency`, gzip -9 of a 10-message catalog with and
  without them):

  | | en | es | de | fr | ar | he | ja | hi | ru | pl | cy |
  |---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
  | entries, raw B | 92 | 61 | 51 | 78 | 128 | 138 | 72 | 74 | 78 | 88 | 107 |
  | Δ catalog gz B | 86 | 50 | 41 | 76 | 106 | 80 | 65 | 70 | 70 | 80 | 93 |

  Currency and unit display data (A4) do not exist yet; P0.5 measured
  145–260 B gz per locale for four currencies and 195–363 for five units ×
  three widths, which the option would also drop.

## 2. Speed

`INTL_PROBE_REV=732da4c bench/intl-probe/scripts/2-speed.sh`, run three
times (00:03, 00:06, 00:09 on 2026-09-22; every run kept in
`target/intl-probe/results/speed-<browser>-<stamp>.json`, the table is
`node bench/intl-probe/scripts/report.mjs speed`). In each run, per message
and locale, the four variants take 9 rounds each in one page, the order
rotated every round; a sample is one `bench_int` / `bench_float` call —
`iters` formats in a loop inside wasm (formatters cached, `NoErrors`, a
reused `String`), calibrated to ≈ 60 ms — timed with `performance.now()` in
a cross-origin-isolated page. Messages (`intl-probe-native speed`, catalogs
for `en` and `pl`; one slot `n`): `plain` = `{$n}` (unannotated, the
harness's floor), `number` = `{$n :number}`, `number-frac2` = `{$n :number
minimumFractionDigits=2 maximumFractionDigits=2}`, `number-sig3` = `{$n
:number maximumSignificantDigits=3}` (these four with floats 1234.5678 +
k/4), `integer` = `{$n :integer}`, `select` = `.input {$n :integer} .match
$n one {{one}} * {{other}}`, `select-3` = the same with `one few many *`,
`select-fmt` = `one {{{$n} item}} * {{{$n} items}}` (these with integers
0–99). The runs were taken on the pre-clippy build of the same code (§1's
figures are 18–23 B gz larger after a clippy clean-up; no behaviour
changed).

**The machine was loaded** (other agents' builds: 1-minute load 6.1, 5.4,
3.3; CPU clock sampled 1.4–3.2 GHz), so the absolute nanoseconds spread up
to 2.5× between runs; the ratios, from variants alternated within a run,
spread little. They are the figures to read:

| | `:number` (3 option sets) | `:integer` | select (1 and 3 keys) | select + placeholder |
|---|---:|---:|---:|---:|
| Chromium, `intl` / `rust` | 2.1–2.9× | 3.6–4.7× | 1.7–3.0× | 3.6–4.4× |
| Chromium, `intl-loc` / `rust-loc` | 1.8–2.3× | 2.8–3.9× | 1.7–2.8× | 3.1–3.8× |
| Chromium at 4×, `intl` / `rust` | 2.3–3.3× | 4.1–5.0× | 1.9–3.4× | 3.6–4.5× |
| Firefox, `intl` / `rust` | 2.8–3.3× | 5.5–6.1× | 2.4–3.8× | 5.1–5.5× |
| Firefox, `intl-loc` / `rust-loc` | 2.3–3.5× | 3.8–4.2× | 2.5–3.7× | 4.2–4.6× |

In time, `intl` − `rust` per format is **+2.0 to +10.5 µs** in Chromium
unthrottled (+10.5 to +40 µs at 4×) and **+2.2 to +8.5 µs** in Firefox; the
unannotated `plain` row is equal in all variants within the noise (±0.3 µs
unthrottled, ±1 µs at 4×), so the difference is the numeric handler. `Intl` itself, called from JavaScript
with a cached formatter, costs 0.56–1.41 µs per `format`/`select` in
Chromium (2.2–7.1 µs at 4×) and 0.35–0.75 µs in Firefox: the larger part of
the difference is the crossing (the key and the value decoded from wasm
memory, the result encoded back into a wasm allocation), the key build and
the boxed value — reducible by a tighter glue, but not below `Intl`'s own
call, which alone takes about as long as the whole Rust format of the same
message.

**Chromium 143, unthrottled** — 3 run(s); medians of 9 rounds each, the range over runs; CPU MHz sampled 1400–3100; 1-min load 6.1, 5.4, 3.3

| locale · message | rust ns | intl ns | rust-loc ns | intl-loc ns | intl − rust ns | intl / rust | intl-loc / rust-loc |
|---|---:|---:|---:|---:|---:|---:|---:|
| en · `plain` | 1,031–2,736 | 1,101–2,400 | 1,151–2,962 | 1,036–2,955 | -335–69 | 0.88–1.07 | 0.90–1.06 |
| en · `number` | 1,274–3,168 | 3,353–8,538 | 1,797–4,435 | 3,183–9,141 | 2,079–5,370 | 2.56–2.70 | 1.77–2.06 |
| en · `number-frac2` | 1,578–5,156 | 4,043–12,901 | 2,180–6,549 | 4,100–13,036 | 2,465–7,745 | 2.23–2.56 | 1.88–2.17 |
| en · `number-sig3` | 2,029–5,357 | 5,621–13,986 | 2,696–6,475 | 5,600–13,014 | 3,591–8,628 | 2.61–2.80 | 2.01–2.27 |
| en · `integer` | 1,007–2,673 | 4,512–10,199 | 1,302–3,913 | 4,275–10,767 | 3,505–7,526 | 3.81–4.48 | 2.75–3.36 |
| en · `select` | 1,733–5,100 | 5,016–11,029 | 1,777–4,151 | 4,883–10,879 | 3,093–5,929 | 2.16–3.01 | 2.62–2.76 |
| en · `select-3` | 2,162–5,734 | 4,973–12,189 | 2,328–5,803 | 5,292–11,478 | 2,810–6,456 | 2.13–2.30 | 1.98–2.52 |
| en · `select-fmt` | 2,050–5,441 | 8,603–20,277 | 2,648–5,916 | 8,608–19,571 | 6,553–14,836 | 3.73–4.41 | 3.25–3.53 |
| pl · `plain` | 1,426–4,705 | 1,361–4,548 | 1,403–4,863 | 1,364–4,680 | -157–-42 | 0.95–0.98 | 0.94–0.97 |
| pl · `number` | 1,601–4,439 | 4,678–10,967 | 2,101–5,269 | 4,304–11,250 | 3,077–6,527 | 2.47–2.92 | 2.05–2.14 |
| pl · `number-frac2` | 1,907–7,954 | 4,910–16,328 | 2,197–8,033 | 4,441–15,210 | 3,003–8,374 | 2.05–2.78 | 1.85–2.02 |
| pl · `number-sig3` | 1,557–5,819 | 4,461–16,284 | 1,817–7,005 | 4,170–14,088 | 2,904–10,465 | 2.42–2.87 | 1.93–2.29 |
| pl · `integer` | 703–2,021 | 3,279–7,234 | 892–1,838 | 2,774–7,153 | 2,576–5,213 | 3.58–4.66 | 3.11–3.89 |
| pl · `select` | 1,261–3,600 | 3,222–7,469 | 1,233–3,692 | 3,320–8,689 | 1,962–3,868 | 2.07–2.67 | 2.35–2.69 |
| pl · `select-3` | 2,700–6,981 | 5,168–11,799 | 2,724–7,120 | 5,096–11,911 | 2,468–4,818 | 1.69–1.94 | 1.67–1.90 |
| pl · `select-fmt` | 2,150–3,012 | 8,601–11,279 | 2,646–3,538 | 8,133–11,424 | 6,451–8,383 | 3.57–4.00 | 3.07–3.83 |

| Intl alone, from JS (formatter cached) | ns |
|---|---:|
| en · NumberFormat.format(decimal string) | 838–1,372 |
| en · NumberFormat.format(number) | 600–1,271 |
| en · PluralRules.select(number) | 561–995 |
| pl · NumberFormat.format(decimal string) | 810–1,407 |
| pl · NumberFormat.format(number) | 593–1,125 |
| pl · PluralRules.select(number) | 571–813 |

**Chromium 143, 4× CPU throttle** (CDP `Emulation.setCPUThrottlingRate`) — 3 run(s); medians of 9 rounds each, the range over runs; CPU MHz sampled 1578–2400; 1-min load 6.1, 5.4, 3.3

| locale · message | rust ns | intl ns | rust-loc ns | intl-loc ns | intl − rust ns | intl / rust | intl-loc / rust-loc |
|---|---:|---:|---:|---:|---:|---:|---:|
| en · `plain` | 6,625–7,267 | 6,583–7,248 | 6,528–7,084 | 6,549–7,218 | -155–291 | 0.98–1.04 | 1.00–1.02 |
| en · `number` | 7,401–9,423 | 20,974–31,043 | 9,891–12,461 | 19,415–23,588 | 13,573–21,620 | 2.78–3.29 | 1.83–1.96 |
| en · `number-frac2` | 10,009–11,643 | 24,522–30,461 | 12,916–14,627 | 25,118–28,625 | 14,514–18,818 | 2.45–2.62 | 1.87–2.04 |
| en · `number-sig3` | 8,305–16,361 | 23,044–44,722 | 10,220–18,681 | 22,876–39,829 | 14,560–28,361 | 2.70–2.77 | 2.05–2.24 |
| en · `integer` | 3,774–4,871 | 15,604–21,100 | 4,533–6,246 | 14,623–21,085 | 11,829–16,689 | 4.13–4.78 | 3.05–3.38 |
| en · `select` | 6,299–9,373 | 17,255–31,694 | 6,292–10,784 | 18,289–27,354 | 10,490–22,320 | 2.55–3.38 | 2.54–2.91 |
| en · `select-3` | 8,867–15,451 | 21,607–38,492 | 9,367–15,979 | 20,540–35,471 | 12,740–23,041 | 2.44–2.63 | 2.19–2.27 |
| en · `select-fmt` | 9,364–10,910 | 38,664–44,570 | 10,999–13,740 | 38,725–46,545 | 27,895–33,660 | 3.59–4.33 | 3.29–3.65 |
| pl · `plain` | 6,762–10,554 | 6,167–11,608 | 6,724–11,309 | 6,613–11,097 | -776–1,054 | 0.91–1.10 | 0.98–1.07 |
| pl · `number` | 7,246–16,212 | 19,139–43,741 | 9,424–18,286 | 18,945–36,412 | 11,893–27,529 | 2.64–2.87 | 1.77–2.06 |
| pl · `number-frac2` | 9,766–25,251 | 25,759–61,339 | 11,636–35,321 | 24,428–57,373 | 15,993–36,089 | 2.43–2.72 | 1.62–2.10 |
| pl · `number-sig3` | 10,494–17,090 | 28,870–44,755 | 14,145–19,831 | 27,806–57,407 | 18,377–27,665 | 2.34–2.75 | 1.97–2.92 |
| pl · `integer` | 3,846–6,065 | 16,814–26,660 | 5,042–7,996 | 16,466–24,810 | 12,968–20,595 | 4.37–4.95 | 3.10–3.28 |
| pl · `select` | 6,700–11,904 | 17,594–27,390 | 6,596–11,309 | 17,342–31,144 | 10,895–15,486 | 2.30–2.63 | 2.38–2.75 |
| pl · `select-3` | 10,810–14,296 | 21,696–26,429 | 11,032–14,637 | 21,708–26,401 | 10,887–12,132 | 1.85–2.01 | 1.70–2.04 |
| pl · `select-fmt` | 9,190–12,237 | 41,634–51,957 | 10,846–16,812 | 35,605–54,661 | 32,444–40,021 | 3.92–4.53 | 3.25–3.32 |

| Intl alone, from JS (formatter cached) | ns |
|---|---:|
| en · NumberFormat.format(decimal string) | 3,420–5,536 |
| en · NumberFormat.format(number) | 2,295–7,130 |
| en · PluralRules.select(number) | 2,218–3,884 |
| pl · NumberFormat.format(decimal string) | 3,196–4,332 |
| pl · NumberFormat.format(number) | 2,155–3,116 |
| pl · PluralRules.select(number) | 2,363–3,162 |

**Firefox 155, unthrottled** — 3 run(s); medians of 9 rounds each, the range over runs; CPU MHz sampled 1437–3168; 1-min load 6.1, 5.4, 3.3

| locale · message | rust ns | intl ns | rust-loc ns | intl-loc ns | intl − rust ns | intl / rust | intl-loc / rust-loc |
|---|---:|---:|---:|---:|---:|---:|---:|
| en · `plain` | 1,896–2,610 | 2,031–2,456 | 2,018–2,390 | 1,863–2,571 | -155–135 | 0.94–1.07 | 0.92–1.08 |
| en · `number` | 1,848–1,988 | 5,850–6,418 | 2,417–2,668 | 5,713–6,033 | 4,002–4,430 | 3.15–3.23 | 2.26–2.39 |
| en · `number-frac2` | 2,530–2,872 | 7,571–8,512 | 3,102–3,591 | 7,159–8,315 | 5,041–5,640 | 2.95–2.99 | 2.27–2.32 |
| en · `number-sig3` | 2,222–2,288 | 6,795–7,207 | 2,559–2,691 | 6,502–7,066 | 4,573–4,919 | 3.06–3.15 | 2.54–2.63 |
| en · `integer` | 789–1,073 | 4,704–6,094 | 1,015–1,465 | 4,130–6,138 | 3,916–5,022 | 5.53–5.96 | 4.07–4.19 |
| en · `select` | 1,277–1,654 | 4,844–5,814 | 1,295–1,660 | 4,795–5,704 | 3,567–4,204 | 3.51–3.79 | 3.39–3.70 |
| en · `select-3` | 1,517–2,163 | 4,527–6,530 | 1,512–2,098 | 4,564–6,357 | 3,010–4,367 | 2.86–3.02 | 2.96–3.03 |
| en · `select-fmt` | 1,267–1,851 | 7,021–9,720 | 1,484–2,224 | 6,641–9,914 | 5,754–7,868 | 5.25–5.54 | 4.46–4.62 |
| pl · `plain` | 1,324–2,474 | 1,346–2,379 | 1,366–2,297 | 1,459–2,202 | -94–22 | 0.96–1.02 | 0.96–1.07 |
| pl · `number` | 1,906–2,167 | 6,198–7,060 | 2,381–2,752 | 5,592–6,794 | 4,292–4,893 | 3.24–3.26 | 2.35–2.47 |
| pl · `number-frac2` | 2,469–3,675 | 7,360–10,241 | 3,044–4,005 | 7,025–9,355 | 4,870–6,567 | 2.79–3.02 | 2.28–2.34 |
| pl · `number-sig3` | 2,115–4,608 | 6,808–13,073 | 2,473–4,038 | 6,605–14,178 | 4,693–8,465 | 2.84–3.29 | 2.43–3.51 |
| pl · `integer` | 598–1,011 | 3,349–6,000 | 752–1,499 | 3,048–5,802 | 2,751–4,989 | 5.60–6.07 | 3.78–4.05 |
| pl · `select` | 971–1,798 | 3,266–6,355 | 994–1,884 | 3,277–6,167 | 2,295–4,557 | 3.36–3.53 | 3.27–3.37 |
| pl · `select-3` | 1,382–2,538 | 3,579–6,098 | 1,457–2,568 | 3,639–6,295 | 2,198–3,560 | 2.40–2.59 | 2.45–2.51 |
| pl · `select-fmt` | 1,298–1,928 | 6,840–9,821 | 1,566–2,351 | 6,585–10,020 | 5,542–7,893 | 5.09–5.27 | 4.20–4.44 |

| Intl alone, from JS (formatter cached) | ns |
|---|---:|
| en · NumberFormat.format(decimal string) | 406–600 |
| en · NumberFormat.format(number) | 361–533 |
| en · PluralRules.select(number) | 388–725 |
| pl · NumberFormat.format(decimal string) | 403–751 |
| pl · NumberFormat.format(number) | 349–381 |
| pl · PluralRules.select(number) | 484–524 |

**Firefox 155, 4× CPU throttle**: not available — Playwright has no CPU throttling for Firefox (CDP is Chromium-only).



## 3. Agreement

### 3a. Neutral output: P0.5's 100,000 cases, Rust vs `Intl.NumberFormat`

`bench/intl-probe/scripts/3-agreement.sh` (the Rust side is
`cargo run --release -p runtime-bench -- numbers ecma 100000`, run by
`data.sh` at the measured tree; the engine side `ecma-diff.cjs`'s comparison,
`Intl.NumberFormat('en', {useGrouping: false, …options})`, per engine):

| Engine | identical | different | `Intl` rejects the option set | of which Rust reports *Bad Option* |
|---|---:|---:|---:|---:|
| Chromium 143 | 95,675 | **0** | 4,325 | 4,325 |
| Firefox 155 | 95,675 | **0** | 4,325 | 4,325 |
| node 23.11 (ICU 76.1) | 95,675 | **0** | 4,325 | 4,325 |

The same as P0.5 and Phase 3 in node: no difference to classify (space,
symbols, digits, rounding), and every set `Intl` rejects is exactly one the
Rust path reports as *Bad Option* and still formats.

### 3b. The option's handlers on the same 100,000 messages

The messages compiled natively (`intl-probe-native ecma`, 10 catalogs of
10,000) and formatted in the engine by the `rust` and the `intl` variants,
text and error names compared: **100,000 / 100,000 identical** in Chromium,
Firefox and node — the 4,325 *Bad Option* cases included, since the handlers
hand `Intl` the resolved digit options. Harness self-check: `rust` in wasm
equals the native output on 100,000 / 100,000 in every engine.

### 3c. Edge cases: `:integer` rounding, `:offset` arithmetic, exact keys

`intl-probe-native edge`: 478 messages — 14 values × 9 rounding modes and
sign/digit options through `:integer` (whose value `Intl` rounds), `:offset`
with add/subtract 0–99 on zeros, negatives, decimals, a 40-digit value,
`1e30`, `1e-30`, and exact-key/keyword selection under digit options.
**467 / 478 identical** in Chromium, Firefox and node. The 11 differences
are one bug of the Rust path, not an engine difference:

| Message | Rust path | `intl` option |
|---|---:|---:|
| `{0 :offset subtract=1}` (… `=2`, `=5`, `=10`, `=99`) | `1` (`2` …) | `-1` (`-2` …) |
| `{-1 :offset add=0}` (`-5`, `-1.5`, `-0.25`, `-100.01`, `-98.5`) | `1` (`5` …) | `-1` (`-5` …) |

`intl-probe-native observe` shows the same natively: the runtime's
`Decimal::add` (`crates/mf2-runtime/src/number/decimal/own.rs`) sets the
result's sign to `self.neg && other.neg` whenever one side is zero, so
`0 − 5 = 5` and `−5 + 0 = 5`. Present at 3fc4735 and at 732da4c. The probe's
own addition follows IEEE 754 zero signs.

### 3d. Locale symbols: the panel

`intl-probe-native loc`: 4,004 cases — the 11 locales `en es de fr ar he
ja hi ru pl cy` × 14 values × 24 annotations (`:number` with
`useGrouping` always/min2/never, `minimumFractionDigits`,
`maximumSignificantDigits`, `signDisplay`; `:integer`; `:percent` ×2;
`:currency` ×8 — EUR, USD, JPY, `currencyDisplay` code/name/narrowSymbol,
`currencySign=accounting`, `fractionDigits=0`; `:unit` ×6) and a cardinal
and an ordinal selection per value — formatted by `intl-cu`, and natively
by the Rust registry (`intl-probe-native loc-format`, at 732da4c with
`mf2-fn-number`).

| Pair | `:number` (1,078) | `:integer` (154) | `:percent` (308) | `:currency` (1,232) | `:unit` (924) | selections (308) |
|---|---:|---:|---:|---:|---:|---:|
| Chromium vs Firefox | all | all | all | 1,190 (42 `cy`: *symbols*) | 882 (42 `cy`: *symbols*) | all |
| Chromium vs node | all | all | all | 1,190 (the same 42) | 882 (the same 42) | all |
| Firefox vs node | all | all | all | all | all | all |
| **Rust (`mf2-fn-number`) vs each engine** | **all** | **all** | **all** | — *(A4)* | — *(A4)* | **all** |

* Chromium has no Welsh currency or unit names and falls back to English:
  `{|1| :unit unit=kilometer unitDisplay=long}` is `1 kilometer` (Firefox,
  node: `1 cilometr`), `{|1| :currency currency=USD}` `$1.00` (`US$1.00`).
* The Rust localization layer (CLDR 48.2.1 from the catalog) and all three
  engines agree on every `:number`, `:integer`, `:percent` case and every
  selection of the panel — no space-character, symbol or digit difference on
  these values.
* The Rust side of `:currency` and `:unit` comes with A4. The command is
  ready: add `mf2_fn_number::{CURRENCY, UNIT}` to `FUNCTIONS` in
  `native/src/sets.rs` (feature `fn-number`) and to the `rust-loc` registry
  in `wasm/src/lib.rs`, then `INTL_PROBE_REV=<rev> scripts/data.sh` and
  `scripts/3-agreement.sh`.

## 4. Plural: the 15,041 CLDR 48.2.1 samples through `Intl.PluralRules`

`bench/intl-probe/scripts/4-plural.sh` (samples: `intl-probe-native plural`,
each with CLDR's category and our evaluator's, which agree on all 15,041;
per sample minimum = maximum fraction digits = the digits it writes; a
`c`/`e` sample through `notation: 'compact'` where the engine has it):

| Engine | cardinal agree / 12,396 | ordinal agree / 2,645 | differences |
|---|---:|---:|---|
| Chromium 143 | 12,281 | 2,620 | engine lacks the locale 128 (`cv`, `kok`, `kok-Latn`, `sgs`, `und`, ordinal `ie`); rule differs 12 (ordinal `scn`) |
| Firefox 155 | 12,111 | 2,627 | engine lacks the locale 303 (30 locales and root: `an ars bal dv guw hnj iu jbo kaj kcg lld nah nr ny osa pap sdh sgs sma smi smj sms ss ssy tig tpi ts ve vo wa`, `und`) |
| node 23.11 (ICU 76.1) | 12,227 | 2,620 | lacks the locale 128; compact exponent 54 (no `notation: 'compact'` on `PluralRules`); rule differs 12 (ordinal `scn`) |

* **No engine differs on a locale it has, except `scn` ordinals** (Chromium,
  node: `81`–`89`, `801`… are `other`, CLDR 48.2.1 says `many`; the
  category sets are equal, Firefox agrees with CLDR 48): CLDR version drift.
* **A locale the engine lacks is silently English**:
  `resolvedOptions().locale` is `en-US`, so `sgs` (5 categories) selects
  `one`/`other`. Our evaluator carries the rules of all 224 CLDR locales in
  the catalog; with the option, plural selection in those locales is wrong
  in that browser.
* JS-number precision did not show: no sample has more than 15 significant
  digits.

## 5. Browser floor (feature detection)

`bench/intl-probe/scripts/5-floor.sh` — every row is a behaviour, probed
with a resolved option and a formatted result:

| Needed by the option | Chromium 143 | Firefox 155 | node 23.11 |
|---|---|---|---|
| `NumberFormat` v3: `roundingIncrement`, `roundingMode`, `roundingPriority`, `trailingZeroDisplay`, `useGrouping: 'min2'`, `signDisplay: 'negative'`, `maximumFractionDigits` ≤ 100, `formatRange` | yes | yes | yes |
| exact decimal strings (17+ digits, past 2^53, `-0`, `1.5e3`) | yes | yes | yes |
| `formatToParts` | yes | yes | yes |
| `PluralRules` with the digit options (and v3's increment, mode, priority, trailing zeros) | yes | yes | yes |
| `PluralRules` `notation: 'compact'` (CLDR's `c`/`e` operand) | yes | yes | **no** |
| `currencyDisplay: 'narrowSymbol'`, `currencySign: 'accounting'`, `style: 'unit'`, `numberingSystem: 'latn'` | yes | yes | yes |
| sanctioned units (`Intl.supportedValuesOf('unit')`) | 45 | 45 | 45 |

WebKit: not measured (it did not start). *From general knowledge, not
measured here:* the `NumberFormat` v3 options shipped in Chrome/Edge 106
(2022), Safari 15.4 (2022) and Firefox 116 (2023); an engine without them
ignores `roundingIncrement`, `roundingMode`, `roundingPriority` and
`trailingZeroDisplay` silently (different digits, no error) and converts
decimal strings to JS numbers — a floor check at startup would be needed
to fall back, which means shipping the Rust path anyway.

## 6. The L4 number files in each engine

`bench/intl-probe/scripts/6-l4.sh`: `functions/{number,integer,offset,
percent,currency}.json` compiled natively (`mf2::compile_str`, one catalog
per test), formatted in the engine, compared as `conformance/src/l4.rs` does
(string with `exp`, errors as a multiset with `expErrors` in string and in
parts output, `expParts`, parts concatenating to the string; the files'
`defaultTestProperties`: `en-US`, `bidiIsolation: none`):

| Engine · variant | all | number | integer | offset | percent | currency |
|---|---:|---:|---:|---:|---:|---:|
| Chromium 143 · `intl-cu` | **95/95** | 41/41 | 13/13 | 16/16 | 13/13 | 12/12 |
| Firefox 155 · `intl-cu` | **95/95** | 41/41 | 13/13 | 16/16 | 13/13 | 12/12 |
| node 23.11 · `intl-cu` | **95/95** | 41/41 | 13/13 | 16/16 | 13/13 | 12/12 |
| Chromium 143, Firefox 155 · `rust` | 70/95 | 41/41 | 13/13 | 16/16 | 0/13 | 0/12 |
| Chromium 143, Firefox 155 · `rust-loc` (732da4c) | 83/95 | 41/41 | 13/13 | 16/16 | 13/13 | 0/12 |

No failure with `intl-cu` in any engine. `rust`'s and `rust-loc`'s missing
tests are *Unknown Function* (`:percent`, `:currency` not in their
registries; `mf2-fn-number`'s `:currency` is A4).

## 7. Reading (the author's, not a measurement)

*This section is the author's reading of the figures above, for the
owner's decision 4; it is not a measurement.*

**Size — the premise does not hold for the core.** 03 §5.3 estimated that
the option saves about 3 of the core numbers' 5.1 KB gz. It costs **+2.2 KB
gz more** than the Rust path instead (+40 %; about a third of it JavaScript),
because what Rust must keep under the design — option tables and
validation, the digit plan with its *Bad Option* rules, operand parsing,
`:offset`, inheritance, exact keys — is most of the numeric code, while what
`Intl` takes over (rounding, digit output, the plural evaluator) is ≈ 1.5 KB
gz, less than the glue (a key per call, strings across the boundary, the
JavaScript). A tighter integration inside the runtime would shave some of
the probe's own overhead, not flip the sign. With locale symbols and
`:percent` the two paths are within 0.5 KB gz (Rust smaller); only with
`:currency` and `:unit` does the option come out ahead, by an estimated
≈ 1.8 KB gz of code plus the currency and unit display data each catalog
would not carry (P0.5: ≈ 0.3–0.6 KB gz per locale for the codes and units
used) and the ≈ 40–110 B gz of plural and number entries measured here.
B1 (30 KB gz) is not the constraint that decides it either way.

**Costs — all measured, all real.** (1) Speed: 2–6× per numeric
placeholder, +2–10 µs unthrottled and +10–40 µs at 4× throttle on this
(loaded) machine — for a page with 100 numeric placeholders, 0.2–1 ms
unthrottled and 1–4 ms at 4× throttle per render; `Intl`'s own
call is ≥ 0.35–1.4 µs, so no glue brings it near the Rust path. (2) Locale
coverage becomes the browser's: Firefox 155 has no plural rules for 30 CLDR
locales and Chromium 143 for 5, and both silently answer with English rules;
Chromium has no Welsh currency or unit names and writes English. The Rust
path carries CLDR 48.2.1 for every locale in the catalog. (3) `:unit` would
be limited to `Intl`'s 45 sanctioned units (and their `-per-` compounds).
(4) A floor: every option needs `NumberFormat` v3, and an engine without it
mis-rounds silently, so supporting older engines means detecting that and
shipping the Rust path anyway. (5) Conformance per engine: good where
measured — 95/95 L4 tests, 100,000/100,000 cases identical to Rust in
Chromium, Firefox and node — but WebKit is unmeasured here.

**What the probe also found.** The Rust localization layer (`mf2-fn-number`)
and three engines agree on every `:number`, `:integer`, `:percent` and
selection case of the 11-locale panel; the server/client divergence the
option would add is therefore small in practice for these functions, and
concentrated where engine data is missing (Welsh names in Chromium, the
plural gaps above). And `:offset` in the Rust path gets zero signs wrong
(`{0 :offset subtract=5}` = `5`), a runtime bug independent of the option.

**On balance**, the numbers do not support adopting the option for the
numeric core or for plural selection: it is larger, slower, and weaker on
locale coverage, and the saving it promised is not there. They leave open a
narrower option — `Intl` for the *display* of `:currency` and `:unit` only,
with Rust semantics, numbers and plurals as today — where the byte saving is
(≈ 1.8 KB gz of code, the per-locale display data) and the costs are
bounded (the unit list, Chromium's Welsh names, the v3 floor, a crossing
per currency/unit placeholder). That narrower option was not built or
measured; A4 (`:currency`, `:unit` in Rust) gives the baseline to measure
it against, with `scripts/build.sh` (`rust-loc` + A4 against `intl-cu`) and
`scripts/3-agreement.sh`. Dates (`intl`, already a planned
backend, B4) were outside this probe.

## 8. The option as built (Phase 4, after owner decision 4)

Owner decision 4 adopted the option; Phase 4 built it into the runtime
(`plans/03-runtime.md` §2.7, §5.3: `mf2-runtime/intl`, `mf2-fn-number/intl`,
`mf2-host-web/intl`, `NUMBERS_HOST`). The probe measures it as variants
`rt-intl`, `rt-intl-loc`, `rt-intl-cu` — the registries of `rust`,
`rust-loc` and the new `rust-cu` (`rust-loc` + A4's `:currency` / `:unit`)
with the runtime's own `intl` — against the Rust path and A0's handlers in
the same harness. Tree: 79c4d7f (`crates/` clean); Chromium 143.0.7499.4,
Firefox 155.0 and now **WebKit 26.6** (Playwright 1.63.0; WebKit starts on the
development machine since 2026-09-22).

**Size** (`bench/intl-probe/scripts/build.sh`; B gz of wasm + JS over `base`,
deterministic — the same bytes on rebuilding):

| Function set | Rust path | A0's handlers | **as built** | built − Rust |
|---|---:|---:|---:|---:|
| core (`:number` `:integer` `:offset`, neutral) | 5,408 (`rust`) | 7,539 (`intl`) | **7,613** (`rt-intl`: 6,630 wasm + 983 JS) | **+2,205** |
| + locale symbols, grouping, `:percent` | 7,215 (`rust-loc`) | 7,552 (`intl-loc`) | **7,604** (`rt-intl-loc`) | **+389** |
| + `:currency`, `:unit` | 12,659 (`rust-cu`, measured; A0 estimated ≈ 10,038) | 8,275 (`intl-cu`) | **9,016** (`rt-intl-cu`: 8,033 + 983) | **−3,643** |

The JavaScript is 983 B gz (the inline module 779 of it, hand-minified as
A0's was, with the `Intl.NumberFormat` v3 detection A0 did separately). A0's
reading holds: the option is larger for the core, about even with locale
symbols, and smaller once `:currency` and `:unit` are used — by 3.6 KB gz
rather than A0's estimated 1.8, because the Rust `:currency` / `:unit` as
built (A4, B3) cost 5.4 KB gz where P0.5's probe measured 2.9. Each catalog
of an `intl` client also needs none of its number, currency, unit and
plural entries (§1's table; `mf2-build`'s slicing, P5a).

**Speed** (`bench/intl-probe/scripts/2-speed.sh`, three runs at 03:58,
04:01, 04:04; the variants alternated in one page; 1-minute load 1.6–2.5,
CPU 1.5–4.1 GHz sampled; ranges over the runs of per-message medians):

| Engine | `:number` ×3 | `:integer` | select (1, 3 keys) | select + placeholder | built − Rust, per format |
|---|---:|---:|---:|---:|---:|
| Chromium, `rt-intl` / `rust` | 2.24–2.65× | 3.98–4.24× | 1.65–2.43× | 3.40–3.76× | +1.1 to +5.5 µs |
| Chromium at 4× throttle | 2.24–2.65× | 3.94–4.42× | 1.63–2.45× | 3.48–3.76× | +4 to +27 µs |
| Firefox | 2.21–3.28× | 3.85–6.05× | 2.34–3.47× | 4.77–5.41× | +2.0 to +7.1 µs |
| WebKit | 2.36–2.84× | 4.90–5.06× | 1.93–2.83× | 4.11–4.41× | +1.6 to +6.1 µs |
| with symbols, `rt-intl-loc` / `rust-loc`, all engines | 1.67–2.87× | 2.72–4.23× | 1.60–3.43× | 2.80–4.48× | |

In ns per format, `rt-intl`: a numeric placeholder 2,551–5,209 (Chromium),
2,779–10,653 (Firefox), 2,975–5,906 (WebKit); a select 2,481–4,188,
2,830–10,557, 3,174–4,678; the Rust path 472–4,826 for all of them; `Intl`'s
own call from JavaScript with a cached formatter 321–779 ns (1.1–4.0 µs at
4×). **The built option is faster than A0's handlers in nearly every row of
every run** (e.g. Chromium `select` 2.35–2.43× against 2.65–2.72×): one
string crosses for the key (`locale U+0001 options`) and one for the value,
and a resolved number keeps its digit plan and its plural category instead
of a boxed value. It stays 2–6× the Rust path: `Intl`'s own call is already
about the Rust path's whole format.

**WebKit 26.6** in items 4 and 5 (`engines.sh floor plural`): every row of
the floor is `yes`, as in Chromium and Firefox. Its `Intl.PluralRules` lacks
more CLDR locales than the other two — 11,952 / 12,396 cardinal and 2,595 /
2,645 ordinal samples agree; it answers with en-US rules for 33 locales
(among them `tl`, `sh`, `scn`, `sgs`, `ars`) and has older category sets for
`cv`, `ie`, `kok`, `kok-Latn`. The runtime's own evaluator carries all 224.

**L4 in the three engines** with the option as built (`cargo xtask l4-web`,
`plans/01-conformance.md` §3): 324 / 324 runtime tests in each; no suite test
formats otherwise than the Rust path; the goldens' differences (bidi-mark
sub-parts in ar, ar-EG, he; ar-EG `currencyDisplay=never`; Chromium's Welsh
currency and unit names) are in the ledger's `[[intl]]` tables.
