# tools/e2e — browser checks for mf2-two

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
| `islands` | `examples/demo-islands` (running; default port 3704) | Phase 7 A1: the page is served as islands, with the gate first; hydration changes no text and logs nothing; one catalog request, from the preload; only the reactive node registers under `static-locale`; the markup message inside an island hydrates as an element; a signal-valued argument re-formats; a switch writes the cookie and reloads into the server's page in the new locale, RTL included; with the catalog delayed no island hydrates until it lands, and — the control — without the gate the island with the markup message traps; no message text in the client bundle |
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

`lib/browser.mjs` holds the reusable parts: `launch`, `available`, `watchConsole`,
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
