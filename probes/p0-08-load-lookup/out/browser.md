Chromium 143.0.7499.4, 9 fresh pages per row, medians. crossOriginIsolated: true; timer resolution 4.999876022338867 µs.

| UTF-8 strategy | CPU | first call incl. lazy compile (ms) | first real install, en: copy + Catalog::new (ms) | first switch to pl (ms) | install en, warm (ms) | Catalog::new en / pl / en-XA / ar-XB (ms) | from_utf8(pool) en (ms) | simple (ns) | 1-arg pattern (ns) | select (ns) |
|---|---|---|---|---|---|---|---|---|---|---|
| eager | desktop | 1.015 | 0.095 + 0.395 | 0.725 | 0.065 | 0.0390 / 0.2170 / 0.3017 / 0.0778 | 0.0086 | 34.2 | 456 | 1358 |
| eager | 4× throttle | 2.940 | 0.105 + 1.505 | 2.280 | 0.045 | 0.1101 / 0.7830 / 1.1027 / 0.2625 | 0.0302 | 98.0 | 1021 | 4319 |
| per-access | desktop | 1.060 | 0.135 + 0.305 | 0.140 | 0.055 | 0.0304 / 0.0239 / 0.0233 / 0.0219 | 0.0179 | 53.0 | 430 | 1690 |
| per-access | 4× throttle | 2.760 | 0.160 + 1.120 | 0.125 | 0.040 | 0.0794 / 0.0778 / 0.0753 / 0.0725 | 0.0418 | 142.7 | 1299 | 5384 |

page errors: 0; spot check: ["Is ⁨Ada⁩ a configuration.","Screen key invitation profile participant 1 reached reconnecting that remove join right server.","Message be included all attachment microphone next day moment 7 background … selected command."]
