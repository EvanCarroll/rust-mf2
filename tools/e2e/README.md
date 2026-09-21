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

1. `MF2_E2E_CHROMIUM` / `MF2_E2E_FIREFOX` — explicit paths;
2. the build the installed Playwright version expects, if present;
3. the newest Playwright build already in `$PLAYWRIGHT_BROWSERS_PATH` or
   `~/.cache/ms-playwright` (`chromium_headless_shell-*`, `chromium-*`,
   `firefox-*`).

So when the Playwright CDN is unreachable, cached builds are used instead. In
September 2026, Playwright 1.63.0 drove the cached Chromium 143 (build 1200)
and Firefox 155 (build 1542) builds without problems.

## Run

```sh
node run.mjs <check> [--base-url URL] [--browser chromium|firefox|all] \
                     [--throttle] [--label TEXT] [--json FILE]
```

* `--base-url` — the app under test (default `$BASE_URL` or `http://127.0.0.1:3702`).
* `--browser all` — Chromium, then Firefox.
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
| `p002` | `probes/p0-02-vertical-slice` | P0.2: HTTP-level checks (404 through the error handler, `Content-Language`/`Vary`, immutable catalog, `/i18n/<tag>` redirect, the fallback when there is no owner); the catalog preload overlaps the wasm request, and the preload is reused (one request); text is identical before and after hydration; the lazy route in both locales; switching and switching back gives the same text as server-rendered pages; all four `SsrMode`s render streamed `Tr` text from the request locale with zero server-side context misses; zero console warnings or errors; a skewed catalog is rejected |
| `p010` | `probes/p0-02-vertical-slice` | P0.10: text and attribute mismatches between server and client (expected: no warning, server text stays until the next update); structural mismatches (records the failure mode) |

`lib/browser.mjs` holds the reusable parts: `launch`, `watchConsole`,
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
