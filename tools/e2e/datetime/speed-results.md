# The speed of a date in a browser: Intl against ICU4X

Written by `tools/e2e/datetime/speed.sh` (plan/08 §9): re-run that command to
reproduce. Nanoseconds per format of one date placeholder, the median of 9
samples per build and its range, the builds alternated in one page:
`intl` is `Intl.DateTimeFormat`, `icu` ICU4X over the catalog's blob as a
client ships it, `icu-cached` the same with the formatter cache on.

* When: 2026-10-05T21:13Z; check exit status 0 (0: every assertion passed).
* Machine: 8 CPUs, 2100.014 MHz at the end, load 3.87 5.04 4.89.
* Built from crates/ and tools/e2e/ of the working tree at 3d4b448 (0 changed paths); rustc 1.98.1 (48a229cea 2026-09-01); wasm-bindgen 0.2.128; wasm-opt version 120; node v23.11.0.

## chromium 143.0.7499.4

unthrottled (CPU MHz before/after: 1900.01 / 3130.204; crossOriginIsolated: false):

| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |
|---|---|---:|---:|---:|---:|---:|---:|
| en | `{$d :date}` | 1101 | 7.90e+3 (6.18e+3–15168) | 16440 (15259–39419) | 4.45e+3 (3.63e+3–11081) | 2.08 | 0.56 |
| en | `{$d :datetime}` | 3000 | 5.80e+3 (5.57e+3–7.87e+3) | 17267 (15800–20267) | 3.93e+3 (3.57e+3–4.90e+3) | 2.98 | 0.68 |
| en | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 295 | 28814 (26780–46102) | 193220 (174915–205763) | 27119 (25763–39661) | 6.71 | 0.94 |
| pl | `{$d :date}` | 6667 | 3.36e+3 (3.10e+3–3.87e+3) | 8.20e+3 (7.75e+3–9.10e+3) | 1.92e+3 (1.81e+3–2.16e+3) | 2.44 | 0.57 |
| pl | `{$d :datetime}` | 6316 | 3.42e+3 (3.36e+3–3.63e+3) | 9.21e+3 (8.99e+3–10149) | 2.22e+3 (2.20e+3–2.49e+3) | 2.69 | 0.65 |
| pl | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 348 | 24425 (21552–25287) | 187644 (183908–203448) | 20690 (19540–22701) | 7.68 | 0.85 |
| ar | `{$d :date}` | 7500 | 4.04e+3 (3.80e+3–4.28e+3) | 7.56e+3 (7.41e+3–8.15e+3) | 2.03e+3 (1.99e+3–2.12e+3) | 1.87 | 0.50 |
| ar | `{$d :datetime}` | 6000 | 4.22e+3 (4.05e+3–4.65e+3) | 9.75e+3 (9.58e+3–10533) | 2.43e+3 (2.40e+3–2.55e+3) | 2.31 | 0.58 |
| ar | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 401 | 20449 (19451–37905) | 145885 (144638–150623) | 18204 (17456–19451) | 7.13 | 0.89 |

throttle4x (CPU MHz before/after: 2914.87 / 2200; crossOriginIsolated: false):

| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |
|---|---|---:|---:|---:|---:|---:|---:|
| en | `{$d :date}` | 1846 | 11918 (11268–12405) | 30282 (29794–31744) | 7.26e+3 (6.99e+3–7.91e+3) | 2.54 | 0.61 |
| en | `{$d :datetime}` | 1538 | 13654 (13264–16645) | 40572 (39597–43433) | 9.23e+3 (8.71e+3–9.88e+3) | 2.97 | 0.68 |
| en | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 102 | 127451 (82353–292157) | 781373 (573529–1528431) | 110784 (73529–253922) | 6.13 | 0.87 |
| pl | `{$d :date}` | 1043 | 28284 (25216–43624) | 76510 (68840–80537) | 16491 (14957–22915) | 2.71 | 0.58 |
| pl | `{$d :datetime}` | 429 | 40793 (29837–63869) | 103030 (90210–135198) | 25641 (19580–33800) | 2.53 | 0.63 |
| pl | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 100 | 191000 (162000–229000) | 1462000 (1339000–1965000) | 159000 (145000–248000) | 7.65 | 0.83 |
| ar | `{$d :date}` | 667 | 29985 (26537–49175) | 57121 (53073–76762) | 15592 (14093–25787) | 1.90 | 0.52 |
| ar | `{$d :datetime}` | 851 | 30670 (28437–48061) | 73796 (65570–101410) | 18214 (17509–20799) | 2.41 | 0.59 |
| ar | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 100 | 108000 (101000–161000) | 816000 (802000–1051000) | 101000 (94000–150000) | 7.56 | 0.94 |

## firefox 155.0

unthrottled (CPU MHz before/after: 1760.023 / 2361.371; crossOriginIsolated: false):

| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |
|---|---|---:|---:|---:|---:|---:|---:|
| en | `{$d :date}` | 1714 | 7.58e+3 (6.42e+3–10502) | 18670 (16919–23337) | 4.67e+3 (3.50e+3–6.42e+3) | 2.46 | 0.62 |
| en | `{$d :datetime}` | 2400 | 10833 (7.92e+3–13333) | 23750 (21667–34583) | 5.83e+3 (5.00e+3–9.17e+3) | 2.19 | 0.54 |
| en | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 141 | 49645 (42553–70922) | 297872 (276596–432624) | 49645 (35461–70922) | 6.00 | 1.00 |
| pl | `{$d :date}` | 4000 | 7.25e+3 (6.75e+3–10000) | 14500 (12500–23000) | 3.75e+3 (3.00e+3–5.75e+3) | 2.00 | 0.52 |
| pl | `{$d :datetime}` | 4000 | 7.75e+3 (7.00e+3–8.75e+3) | 16750 (15500–20500) | 4.25e+3 (3.50e+3–4.75e+3) | 2.16 | 0.55 |
| pl | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 182 | 43956 (38462–54945) | 285714 (280220–318681) | 32967 (27473–43956) | 6.50 | 0.75 |
| ar | `{$d :date}` | 4000 | 7.00e+3 (6.75e+3–7.00e+3) | 10750 (10500–12750) | 3.25e+3 (2.75e+3–3.50e+3) | 1.54 | 0.46 |
| ar | `{$d :datetime}` | 3000 | 7.00e+3 (7.00e+3–8.33e+3) | 14333 (13333–15000) | 3.67e+3 (3.33e+3–4.33e+3) | 2.05 | 0.52 |
| ar | `{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}` | 197 | 40609 (35533–45685) | 263959 (248731–284264) | 25381 (25381–35533) | 6.50 | 0.63 |

throttle4x: not available: no CPU throttling in this engine through Playwright (CDP is Chromium-only)

## webkit

Not run (it did not start, or the check failed before timing).
