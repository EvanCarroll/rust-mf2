# `leptos-fluent` A/B — snapshot

Measured once, at migration (`plans/16-phase-8-work-order.md` A5; `plans/06-size-and-perf.md` §6). Not re-run per commit; re-run only when the owner asks, with the command below at that commit.

| | |
|---|---|
| commit measured | `391ef2a99b5b38318ef7758ea50d3e4b251d7f3c` |
| date | 2026-09-25T18:12Z |
| command | `cargo xtask fluent-ab --browser chromium,firefox --runs 10` |
| machine load (1/5/15 min), before / after the browser runs | 3.20 4.71 6.25 / 3.20 4.27 5.85 |
| browsers | chromium 143.0.7499.4; firefox 155.0 |
| fluent-bundle | 0.16.0 |
| fluent-templates | 0.15.1 |
| leptos (leptos-fluent side) | 0.8.20 |
| leptos (mf2 side) | 0.8.20 |
| leptos-fluent | 0.3.1 |
| rustc | rustc 1.98.1 (48a229cea 2026-09-01) |
| tachys (both sides) | 0.2.18 |
| wasm-bindgen | wasm-bindgen 0.2.128 |
| wasm-opt | wasm-opt version 120 |

The application: the reference workload (seed 1, 1860 call sites, 1,600 messages in `en`, `pl`, `en-XA`, `ar-XB`), template `fluent-view` on `leptos-fluent`, and the same application migrated by `mf2 convert --from leptos-fluent` and finished as the migration guide says. Like with like: 8 sites show a sentence with an inline element, written on both sides as the sentence split around the element (three messages each).

## Size

The client as it ships: `wasm32-unknown-unknown`, profile `wasm-release`, `wasm-bindgen --target web`, `wasm-opt -Oz` (the flags of `cargo xtask size`). Bytes; gz is `gzip -9`, br is brotli quality 11, window 22.

| | leptos-fluent raw | gz | br | mf2 raw | gz | br |
|---|---:|---:|---:|---:|---:|---:|
| wasm | 3395362 | 975597 | 601984 | 2103806 | 613952 | 422968 |
| JS | 47782 | 8366 | 7087 | 47937 | 8465 | 7166 |
| the `en` catalog | 0 | 0 | 0 | 51394 | 20563 | 17903 |
| **a first visit in `en`** (wasm + JS + that locale's text) | 3443144 | 983963 | 609071 | 2203137 | 642980 | 448037 |

On `leptos-fluent` every locale's text is in the wasm, so every visitor downloads all four. **What each added locale costs** — on `leptos-fluent` every visitor pays it, measured as the client with all four locales less the client with `en` only (2687092 / 785839 / 552514 B raw / gz / br for `en` only), over the three: **252018 B raw, 66042 B gz, 18853 B br per locale**. On mf2 the wasm does not change; only a reader of that locale downloads its catalog:

| catalog | raw | gz | br |
|---|---:|---:|---:|
| `en` | 51394 | 20563 | 17903 |
| `pl` | 67825 | 27012 | 24021 |
| `en-XA` | 101078 | 24485 | 21445 |
| `ar-XB` | 61342 | 21237 | 18383 |

The sizes above are the whole client, every route in one module, as the size gate builds it. The application the browser times is built the way the generated applications ship, `cargo leptos build --release --split` (the lazy routes are separate chunks, fetched when first visited), with the `ab-bench` hooks; on a first visit to `/` the browser downloaded, as the test servers send them (uncompressed; the catalog is brotli, as `mf2-axum` serves it):

| engine | leptos-fluent main wasm | mf2 main wasm | mf2 `en` catalog |
|---|---:|---:|---:|
| chromium | 2024266 | 870934 | 17903 |
| firefox | 2024266 | 870934 | 17903 |


## Speed

Each run is a fresh first visit (a new browser context, so an empty cache); the two applications are run alternately, the order swapped every run, with both servers up. Medians over the runs, milliseconds, with the minimum and maximum. The timed build is the shipped one plus `ab-bench`'s hooks (`bench/fluent-ab/README.md`).

### chromium 143.0.7499.4

| measure | leptos-fluent | mf2 |
|---|---:|---:|
| first translated frame after the wasm's load (hydrated) | 158.800 (135.300–182.300) | 129.800 (101.800–175.500) |
| …the same, from the navigation's start | 174.550 (148.800–203.700) | 149.000 (126.500–210.900) |
| a simple message, µs per format | 1.184 (0.725–1.755) | 0.093 (0.057–0.224) |
| a one-argument message, µs per format | 2.034 (1.279–2.486) | 0.750 (0.474–1.644) |
| a plural select, µs per format | 3.139 (1.877–5.199) | 2.407 (1.563–4.168) |
| mount 2,000 live translated nodes | 57.450 (36.100–76.700) | 14.200 (7.400–23.700) |
| switch `en` → `pl` with 2,000 live nodes, until every node shows it | 82.400 (46.100–560.300) | 10.250 (7.300–15.700) |
| switch `pl` → `en`, the same | 80.400 (48.400–154.000) | 12.750 (7.900–14.600) |

### firefox 155.0

| measure | leptos-fluent | mf2 |
|---|---:|---:|
| first translated frame after the wasm's load (hydrated) | 114.500 (61.000–130.000) | 102.500 (49.000–123.000) |
| …the same, from the navigation's start | 140.500 (122.000–170.000) | 129.500 (106.000–159.000) |
| a simple message, µs per format | 1.005 (0.730–1.420) | 0.090 (0.070–0.120) |
| a one-argument message, µs per format | 1.615 (1.180–3.100) | 0.625 (0.540–0.820) |
| a plural select, µs per format | 2.500 (1.850–5.430) | 1.610 (1.350–2.930) |
| mount 2,000 live translated nodes | 38.500 (32.000–51.000) | 12.500 (8.000–16.000) |
| switch `en` → `pl` with 2,000 live nodes, until every node shows it | 58.500 (48.000–106.000) | 15.500 (11.000–25.000) |
| switch `pl` → `en`, the same | 61.500 (48.000–131.000) | 20.500 (14.000–41.000) |

## The same text

chromium: equal — every route (`/`, `/r1`, `/r2`, `/r3`) in `en` and `pl`, the hydrated `<main>`'s text compared whole (53622 characters), bidi isolation marks aside (an approved difference, owner question 5).

firefox: equal — every route (`/`, `/r1`, `/r2`, `/r3`) in `en` and `pl`, the hydrated `<main>`'s text compared whole (53622 characters), bidi isolation marks aside (an approved difference, owner question 5).

