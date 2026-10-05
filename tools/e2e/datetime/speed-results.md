# The speed of a date in a browser: Intl against ICU4X

Written by `tools/e2e/datetime/speed.sh` (plan/08 §9): re-run that command to
reproduce. Nanoseconds per format of one date placeholder, the median of 9
samples per build and its range, the builds alternated in one page:
`intl` is `Intl.DateTimeFormat`, `icu` ICU4X over the catalog's blob as a
client ships it, `icu-cached` the same with the formatter cache on.

* When: 2026-10-05T01:40Z; check exit status 0 (0: every assertion passed).
* Machine: 8 CPUs, 2734.793 MHz at the end, load 1.83 1.60 1.70. Taken under load: a `cargo leptos watch` of another project
  was running (idle, no build during the run) beside a desktop browser (task 22.8).
* `Intl`'s text equals ICU4X's in both engines except en `:datetime`, en and ar
  with a zone name (the run's log; text is recorded, not asserted).
* Built from crates/ and tools/e2e/ of the working tree at 8f4bc3d (0 changed paths); rustc 1.98.1 (48a229cea 2026-09-01); wasm-bindgen 0.2.128; wasm-opt version 120; node v23.11.0.

## chromium 143.0.7499.4

unthrottled (CPU MHz before/after: 2500 / 2963.526; crossOriginIsolated: false):

| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |
|---|---|---:|---:|---:|---:|---:|---:|
| en | `{$d :date}` | 1739 | 4.83e+3 (4.37e+3–8.22e+3) | 11271 (9.66e+3–20759) | 3.22e+3 (2.53e+3–7.65e+3) | 2.33 | 0.67 |
| en | `{$d :datetime}` | 4286 | 3.97e+3 (3.83e+3–4.74e+3) | 11549 (10663–13416) | 2.68e+3 (2.50e+3–3.01e+3) | 2.91 | 0.68 |
| en | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 399 | 21554 (20802–22306) | 146366 (142356–150877) | 20551 (19799–22055) | 6.79 | 0.95 |
| pl | `{$d :date}` | 8000 | 3.20e+3 (3.07e+3–3.21e+3) | 7.50e+3 (7.30e+3–7.55e+3) | 1.85e+3 (1.80e+3–1.86e+3) | 2.34 | 0.58 |
| pl | `{$d :datetime}` | 6667 | 3.76e+3 (3.60e+3–4.50e+3) | 9.36e+3 (9.19e+3–10754) | 2.38e+3 (2.28e+3–2.98e+3) | 2.49 | 0.63 |
| pl | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 308 | 23377 (21753–28896) | 186039 (175000–205519) | 20779 (19805–22078) | 7.96 | 0.89 |
| ar | `{$d :date}` | 7500 | 4.21e+3 (3.99e+3–5.08e+3) | 7.61e+3 (7.44e+3–8.05e+3) | 2.07e+3 (2.03e+3–2.11e+3) | 1.81 | 0.49 |
| ar | `{$d :datetime}` | 5714 | 4.46e+3 (4.29e+3–5.09e+3) | 10203 (9.85e+3–10413) | 2.61e+3 (2.52e+3–2.64e+3) | 2.29 | 0.58 |
| ar | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 390 | 21026 (19744–54615) | 153846 (148462–161026) | 18718 (17949–23846) | 7.32 | 0.89 |

throttle4x (CPU MHz before/after: 3100.008 / 2546.27; crossOriginIsolated: false):

| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |
|---|---|---:|---:|---:|---:|---:|---:|
| en | `{$d :date}` | 2069 | 12856 (12373–14355) | 30643 (30159–31513) | 7.73e+3 (7.35e+3–8.70e+3) | 2.38 | 0.60 |
| en | `{$d :datetime}` | 1500 | 14867 (14733–15800) | 41933 (40400–42800) | 10200 (9.67e+3–13600) | 2.82 | 0.69 |
| en | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 100 | 81000 (72000–139000) | 562000 (546000–602000) | 79000 (72000–84000) | 6.94 | 0.98 |
| pl | `{$d :date}` | 2143 | 12832 (12459–13532) | 29911 (29631–31125) | 7.51e+3 (7.09e+3–7.93e+3) | 2.33 | 0.59 |
| pl | `{$d :datetime}` | 1579 | 15136 (14123–23939) | 38442 (36479–39075) | 9.56e+3 (5.45e+3–10006) | 2.54 | 0.63 |
| pl | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 100 | 83000 (80000–86000) | 675000 (659000–766000) | 76000 (72000–79000) | 8.13 | 0.92 |
| ar | `{$d :date}` | 2222 | 15392 (14626–18137) | 29703 (28308–30423) | 8.06e+3 (7.61e+3–9.18e+3) | 1.93 | 0.52 |
| ar | `{$d :datetime}` | 1558 | 17330 (16496–18164) | 39409 (37548–40629) | 10205 (9.82e+3–10334) | 2.27 | 0.59 |
| ar | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 100 | 82000 (79000–170000) | 604000 (589000–613000) | 73000 (68000–80000) | 7.37 | 0.89 |

## firefox 155.0

unthrottled (CPU MHz before/after: 2200 / 2416.973; crossOriginIsolated: false):

| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |
|---|---|---:|---:|---:|---:|---:|---:|
| en | `{$d :date}` | 2400 | 6.25e+3 (5.42e+3–8.33e+3) | 16250 (12917–20000) | 3.75e+3 (2.92e+3–5.00e+3) | 2.60 | 0.60 |
| en | `{$d :datetime}` | 3000 | 6.00e+3 (5.67e+3–8.33e+3) | 14333 (13667–16333) | 3.33e+3 (3.33e+3–4.00e+3) | 2.39 | 0.56 |
| en | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 273 | 29304 (25641–29304) | 175824 (164835–194139) | 21978 (21978–25641) | 6.00 | 0.75 |
| pl | `{$d :date}` | 12000 | 4.08e+3 (4.00e+3–4.33e+3) | 7.83e+3 (7.67e+3–8.08e+3) | 1.92e+3 (1.92e+3–2.00e+3) | 1.92 | 0.47 |
| pl | `{$d :datetime}` | 6000 | 4.50e+3 (4.50e+3–4.67e+3) | 9.67e+3 (9.33e+3–9.83e+3) | 2.33e+3 (2.33e+3–2.67e+3) | 2.15 | 0.52 |
| pl | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 333 | 27027 (24024–27027) | 183183 (174174–192192) | 21021 (21021–24024) | 6.78 | 0.78 |
| ar | `{$d :date}` | 6000 | 5.17e+3 (5.00e+3–5.33e+3) | 8.00e+3 (7.67e+3–9.50e+3) | 2.17e+3 (2.00e+3–2.50e+3) | 1.55 | 0.42 |
| ar | `{$d :datetime}` | 6000 | 5.67e+3 (5.33e+3–5.83e+3) | 10500 (10167–10667) | 2.83e+3 (2.67e+3–3.00e+3) | 1.85 | 0.50 |
| ar | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 308 | 25974 (25974–29221) | 191558 (191558–198052) | 22727 (19481–22727) | 7.37 | 0.88 |

throttle4x: not available: no CPU throttling in this engine through Playwright (CDP is Chromium-only)

## webkit

Not run: WebKit is not installed on this machine (tools/e2e's `npm install`
fetches only Chromium and Firefox), so this run has two engines.
