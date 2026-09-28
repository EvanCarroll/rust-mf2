# tools/e2e — browser checks for rust-mf2

The permanent Playwright harness (master plan §4). It drives a **running** app
in real browsers and asserts on what a user or a crawler would see: console
output, network timing, DOM text before and after hydration, locale switching.
It outlives Phase 0: Phase 6 points it at `examples/demo-ssr`, and conformance
layer L6 can reuse its helpers.

## Install (project-local)

```sh
cd tools/e2e
npm install                 # installs the pinned `playwright` package only
npm run browsers            # optional: download the browser builds this version expects
```

Caution: `playwright install` also garbage-collects cached builds in
`~/.cache/ms-playwright` whose referencing installation no longer exists.

Browser executable, in order of preference:

1. `MF2_E2E_CHROMIUM` / `MF2_E2E_FIREFOX` / `MF2_E2E_WEBKIT` — explicit paths
   (WebKit's is its `pw_run.sh`);
2. the build the installed Playwright version expects, if present;
3. the newest Playwright build already in `$PLAYWRIGHT_BROWSERS_PATH` or
   `~/.cache/ms-playwright`, then in the repository's `target/ms-playwright`
   (`chromium_headless_shell-*`, `chromium-*`, `firefox-*`, `webkit-*`).

### WebKit

WebKit is installed into the repository's own browsers directory, so that
`playwright install` cannot garbage-collect the Chromium and Firefox builds in
`~/.cache/ms-playwright`:

```sh
cd tools/e2e
PLAYWRIGHT_BROWSERS_PATH=../../target/ms-playwright npx playwright install webkit
```

`lib/browser.mjs` finds it there (in a git worktree under `target/`, link
the main tree's `target/ms-playwright` into the worktree's `target/`).
WebKit needs system libraries that Chromium and Firefox do not
(Playwright's `install-deps` installs them with `sudo apt-get`). Since
2026-09-22 WebKit 26.6 (build 2359) starts on the development machine and
runs headless (`cargo xtask l4-web`, Phase 4). Before, on Debian 13 without
them, Playwright 1.63.0's WebKit did not start:

```
MiniBrowser: error while loading shared libraries: libgstcodecparsers-1.0.so.0: cannot open shared object file
```

and Playwright's host check also lists `libsoup-3.0.so.0` (Debian packages
`libgstreamer-plugins-bad1.0-0` and `libsoup-3.0-0`). On such a machine
`--browser all` reports `SKIP` for WebKit and runs the other two;
`--browser webkit` fails.

So when the Playwright CDN is unreachable, cached builds are used instead. In
September 2026, Playwright 1.63.0 drove the cached Chromium 143 (build 1200)
and Firefox 155 (build 1542) builds without problems.

## Run

```sh
node run.mjs <check> [--base-url URL] [--browser chromium|firefox|all] \
                     [--throttle] [--label TEXT] [--json FILE]
```

* `--base-url` — the app under test (default `$BASE_URL` or `http://127.0.0.1:3702`).
* `--browser all` — Chromium, then Firefox, then WebKit when a WebKit build
  is found (one that cannot start is reported as `SKIP` and skipped).
* `--throttle` — Chromium only: 150 ms RTT and 1.6 Mbps down, applied through
  CDP to the first page, so the timing checks show overlap at more than
  localhost speed.
* `--json FILE` — writes every assertion plus the collected data (timings,
  console messages, DOM snapshots) to `FILE`.

The exit code is 0 when every assertion passed and 1 otherwise. Each assertion
prints one `PASS`/`FAIL` line.

## Checks

One script per check in `checks/`. Each exports `run(ctx)`, where `ctx` holds
`browser`, `baseUrl`, `assert(id, pass, details)`, `data` and `log`.

| Check | App | What it asserts |
|---|---|---|
| `csr` | `examples/demo-csr` (its own static server over `examples/demo-csr/dist/`; run `trunk build` there first) | Phase 7 A2: the locale is chosen in the browser — the reader's languages on a first visit (`fr-CA` → `fr`, `de-DE` → the source locale), the remembered choice after — and an unknown remembered tag is ignored; the index is fetched once, from `index.html`'s preload, and one catalog, the chosen one; the first frame is already in that locale, `<html lang dir>` and the markup element included; a switch is live and reaches text, attributes, `<title>`, a signal-valued argument and `inLanguage`, sets `dir` for Arabic, is remembered in `localStorage` and survives a reload; switching back equals booting there; a message the Arabic catalog borrowed from English is built inside `<span lang="en" dir="ltr">` at mount and by a switch, around the text node French built, and unwrapped by a switch away (Phase 7 A14); with no catalog or no index the boot logs one `mf2:` line and mounts nothing; a switch that meets a catalog from another deploy (this build's, its manifest hash altered) is remembered and reloads into the new locale (Phase 9 B2; `demo` asserts the same on demo-ssr, with the cookie); no message text in the client bundle. Chromium's note on the `integrity` trunk writes on its wasm preload (crbug.com/981419) is the one console message it tolerates |
| `a11y` | `examples/demo-ssr` (`--split`, running at `--base-url`; default port 3702), `examples/demo-islands` (running at `$MF2_ISLANDS_URL`; default port 3704) and `examples/demo-csr/dist/` (served by the check; `trunk build` first) | Phase 7 A11, the WCAG 2.2 AA audit's automated part: axe-core 4.13.0 (pinned; MPL-2.0, a test dependency only) with the `wcag2a`/`wcag2aa`/`wcag21a`/`wcag21aa`/`wcag22a`/`wcag22aa` rules finds no violation and nothing incomplete on demo-ssr `/` and `/lazy`, demo-islands `/` and demo-csr `/`, each in en, fr and ar, light and dark, and on demo-ssr after a live switch to ar and a client navigation (28 pages); axe's best-practice results are data. Then what axe does not measure, from computed styles: field edges ≥ 3:1 against the page, placeholders ≥ 4.5:1 against their field, button text ≥ 4.5:1, the accent ≥ 3:1; no horizontal scroll at 320 CSS px; one `main` with the banner, navigation and contentinfo outside it, the counter a `status`, no duplicate `id` and none in the switcher, the switcher named by its label; demo-ssr's routes with their own titles and one label per field. The switcher: an arrow key changes the select and nothing else, Tab reaches the button and Enter switches — in place with focus kept on demo-ssr and demo-csr, by the form's `GET ?lang=` (and the server's cookie) on demo-islands and on demo-ssr with the wasm blocked. Negative controls in the page: a missing `lang` and grey text fail the scan, the old border and a pale placeholder fail the contrast measure, a 400 px box fails reflow, a select that switches on `change` fails the arrow-key probe |
| `zone` | `examples/demo-ssr` (running at `--base-url`) and `examples/demo-csr/dist/` (served by the check; `trunk build` first) | Phase 8 A7, dates in the reader's time zone, in `America/New_York` and `Asia/Kolkata` (Playwright's `timezoneId`): a first visit is served in UTC with no zone stated; after hydration the date is the text the server renders for the reader's zone, and — against what hydration writes anyway, recorded by a `MutationObserver` from before the first byte — it is the only text written; no `mf2:` message; the `mf2_tz` cookie holds the browser's own name for the zone (Chromium says `Asia/Calcutta`); a reload is served in that zone, states it on the preload link, and writes nothing more. Negative control: a browser that reports UTC as its zone gets no correction, and the first-visit assertion fails on it. demo-csr mounts in the reader's zone (the time of day `Intl` gives for it) and writes no cookie. `demo` runs its hydration assertions in a UTC browser, so that the correction does not count as a hydration change on a machine in another zone |
| `islands` | `examples/demo-islands` (running; default port 3704) | Phase 7 A1: the page is served as islands, with the gate first, and the switcher is not one (a `GET` form); hydration changes no text and logs nothing; one catalog request, from the preload; only the reactive node registers under `static-locale`; the markup message inside an island hydrates as an element; a signal-valued argument re-formats; a switch is the form's `GET ?lang=`, answered with the cookie and the server's page in the new locale, RTL included; with the catalog delayed no island hydrates until it lands, and — the control — without the gate the island with the markup message traps; no message text in the client bundle |
| `lazy` | `examples/demo-ssr`, built with `cargo leptos build --split` (running; default port 3702) | Phase 7 A3, P0.2's lazy-route assertions against `leptos-mf2`: the route's chunk is not fetched on the home page, is fetched once on the way to `/lazy`, and carries no message text; inside it a text, an attribute and a markup element render from the catalog the main module installed, and the chunk sees the locale; a switch on the lazy route (to RTL) reaches the chunk's nodes and equals the server's page; a client-built lazy page equals the server's, and so does the home page built on the way back; leaving the route frees its registry slots, and five round trips leave the registry where it started; a direct `/lazy` load hydrates through `hydrate_lazy` with the chunk loaded, changes no text, registers the same nodes as a client-built one, switches live — `inLanguage` included — and frees its slots on leaving; no console message |
| `churn` | `bench/churn` (its own static server over `target/churn/site/`; `cargo xtask churn` builds it, then runs this check) | Phase 7 A5, P0.11's churning list against `leptos-mf2`: for each row shape — a text, a text and an attribute, a signal-valued argument, a `TextProp` prop, a `Signal<String>` prop, `to_string()` in a closure, an `Oco` prop — 2,000 live rows, then 10,000 churned rows of warm-up and 100,000 more, 50 per round under a round owner; the live heap must grow by at most 64 KiB over the 100,000, and the registry must end where it started; after the churn every live row follows a switch to `fr` and back, text and `title`, except the `Oco` rows, a value, which keep their text; no console message. Per-row heap, allocations, churn time, the trigger's notify time and a switch's time are data (`--json`) |
| `l4-intl` | `conformance/l4-web` (its own static server over `target/l4-web/`) | Conformance L4 for the `intl` build (plans/01-conformance.md §3): formats the bundle `cargo xtask l4-web` compiled natively (`target/l4-web/cases.bin`) with `conformance/l4-web` built for `wasm32-unknown-unknown` with the `intl` features, and writes `target/l4-web/<browser>.txt` (one record per case) and `<browser>.json` (the engine). Asserts that the build is the `intl` one with `Intl.NumberFormat` v3 in the engine, one record per case, no console error; `cargo xtask l4-web` builds, runs this check (`--browser chromium,firefox,webkit`) and judges the records |
| `intl` | `bench/intl-probe` (its own static server: the check serves the repository's `bench/intl-probe/web/` and `target/intl-probe/`, with COOP/COEP) | Phase 4 A0, the `intl` client option: runs the probe's items in each browser — feature detection (floor), the L4 number files, the CLDR plural samples, P0.5's 100,000 ECMA-402 cases, edge cases, the panel's locale-symbol cases, speed (Chromium also at 4× CPU throttle) — and writes `target/intl-probe/results/<browser>.json`. Asserts only the harness's self-checks (the `rust` variant 70/70 on the neutral files; `rust` in wasm = native on all 100,000 cases) and that every item ran; the measurements are data, summarized by `bench/intl-probe/scripts/report.mjs`. `MF2_INTL_ITEMS=a,b` selects items. Prerequisites: `bench/intl-probe/scripts/build.sh` and `data.sh` (see `bench/intl-probe/README.md`) |
| `datetime` | `tools/e2e/datetime` (its own static server: the check serves `tools/e2e/datetime/web/` and `target/e2e-datetime/`) | Phase 4 A6, `datetime-intl` against ICU4X: formats one-message catalogs with the date functions over the `Intl` backend (`Intl.DateTimeFormat` through `mf2-host-web`'s `INTL_HOST`) — the suite's date files (`functions/{date,time,datetime}.json` with their params: L4 of the backend in each engine) and the panel's locales × 60 date/time messages — and compares each text with ICU4X's for the same catalog, within P0.10's tolerance (U+202F and U+00A0 read as U+0020). Asserts the Intl backend ran; every suite test passes (its errors, and ICU4X's; its `exp` where it has one); no errors in the panel cases; and every panel case the mapping expresses exactly (`dateStyle`/`timeStyle`) within the tolerance or one of the check's named known divergences; the component-mapped and zone-styled cases are recorded in `target/e2e-datetime/results/<browser>.json`. Prerequisite: `tools/e2e/datetime/build.sh` |

The two checks below ran against the Phase 0 probe app
`probes/p0-02-vertical-slice`, deleted in Phase 2 (C5; it is in the first
commit's history). They are kept as the starting point for the Phase 6
checks against `examples/demo-ssr`, and do not run without that app.

| Check | App | What it asserts |
|---|---|---|
| `p002` | `probes/p0-02-vertical-slice` | P0.2: HTTP-level checks (404 through the error handler, `Content-Language`/`Vary`, immutable catalog, `/i18n/<tag>` redirect, the fallback when there is no owner); the catalog preload overlaps the wasm request, and the preload is reused (one request); text is identical before and after hydration; the lazy route in both locales; switching and switching back gives the same text as server-rendered pages; all four `SsrMode`s render streamed `Tr` text from the request locale with zero server-side context misses; zero console warnings or errors; a skewed catalog is rejected |
| `p010` | `probes/p0-02-vertical-slice` | P0.10: text and attribute mismatches between server and client (expected: no warning, server text stays until the next update); structural mismatches (records the failure mode) |

`lib/static.mjs` is the static host the client-only checks serve `dist/` with
(`serveStatic`). `lib/browser.mjs` holds the reusable parts: `chooseLocale`
(select a language in `<LocaleSwitcher>` and press its button), `launch`,
`available`, `watchConsole`,
`recordAllConsole`, `watchNetwork`, `resourceTimings`, `captureSsrSnapshot`
(body text and title at `DOMContentLoaded`, which is before hydration because
the wasm boots asynchronously) and `throttle`.

### Adding a check

Create `checks/<name>.mjs` that exports `async function run(ctx)`, then run
`node run.mjs <name>`. Keep app-specific fixtures (expected strings, probe
endpoints) inside the check, and keep generic helpers in `lib/`.

## Probe-specific hooks used by `p002` / `p010`

* `GET /__probe/ctx` returns `hits=N misses=M`: server-side render-time catalog
  lookups that found, or did not find, the per-request context.
* `GET /__probe/no-ctx` formats a message outside any reactive owner.
* `mf2_live_nodes()`, exported by `/pkg/p002.js`, returns the number of live
  entries in the node-update registry. The checks also use it to detect that
  hydration has finished.
