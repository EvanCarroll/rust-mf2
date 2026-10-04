// Phase 4 task A0 — the `intl` client-option probe in real browsers
// (bench/intl-probe/README.md).
//
// Serves the repository itself (its own static server on 127.0.0.1, with
// COOP/COEP so the page is cross-origin isolated and performance.now() is
// fine-grained; only bench/intl-probe/web/ and target/intl-probe/ are
// served), opens bench/intl-probe/web/index.html and runs the probe's items
// through `window.mf2probe`:
//
//   floor         item 5: feature detection of what the option needs
//   l4            item 6: the L4 number files with the `intl-cu` registry
//                 (and the `rust` variant, the harness's self-check)
//   plural        item 4: the 15,041 CLDR samples through Intl.PluralRules
//   ecmaRaw       item 3: P0.5's 100,000 cases, Rust vs Intl.NumberFormat
//   ecmaHandlers  item 3: the same messages, `rust` vs `intl` handlers
//   edge          item 3: :integer rounding, :offset arithmetic, exact keys, `rust` vs `intl`
//   loc           item 3: the locale-symbol cases with `intl-cu`
//   speed         item 2: `rust` vs `intl`, alternated; unthrottled, then at
//                 4× CPU throttle where the engine has it (Chromium: CDP
//                 Emulation.setCPUThrottlingRate; others: "not available")
//   names         the number split (plan/08 §6): `rust-cu` vs `rt-names-cu`
//                 on the locale-symbol panel, ns per placeholder and text
//                 (bench/intl-probe/scripts/7-names.sh runs it alone)
//
// MF2_INTL_ITEMS=a,b selects items (default: all). Results go to
// target/intl-probe/results/<browser>.json (the loc outputs to
// loc-<browser>.json); bench/intl-probe/scripts/report.mjs summarizes them.
// Prerequisites: bench/intl-probe/scripts/build.sh and data.sh.
//
//   node run.mjs intl --browser all

import { createServer } from 'node:http';
import { createReadStream, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = join(REPO, 'target', 'intl-probe');
const SERVED = ['bench/intl-probe/web/', 'target/intl-probe/'];
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.mjs': 'text/javascript', '.js': 'text/javascript',
  '.wasm': 'application/wasm', '.json': 'application/json', '.jsonl': 'text/plain; charset=utf-8',
  '.bin': 'application/octet-stream',
};
const ALL = ['floor', 'l4', 'plural', 'ecmaRaw', 'ecmaHandlers', 'edge', 'loc', 'speed'];

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '');
    const file = normalize(join(REPO, path));
    const ok = file.startsWith(REPO) && SERVED.some((p) => path.startsWith(p)) && existsSync(file) && statSync(file).isFile();
    const headers = {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Resource-Policy': 'same-origin',
      'Cache-Control': 'no-store',
    };
    if (!ok) {
      res.writeHead(404, headers).end('not found');
      return;
    }
    res.writeHead(200, { ...headers, 'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream' });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

const mhz = () => {
  try {
    return Number(/cpu MHz\s*:\s*([\d.]+)/.exec(readFileSync('/proc/cpuinfo', 'utf8'))?.[1]);
  } catch (e) {
    return null;
  }
};

export async function run(ctx) {
  const items = (process.env.MF2_INTL_ITEMS ?? ALL.join(',')).split(',').filter(Boolean);
  // `names` alone (bench/intl-probe/scripts/7-names.sh) needs only its two
  // variants and the panel.
  const needs = items.every((i) => i === 'names')
    ? ['pkg/rust-cu/probe.js', 'pkg/rt-names-cu/probe.js', 'data/loc.json', 'data/loc-rust.json']
    : ['pkg/intl-cu/probe.js', 'pkg/rust/probe.js', 'data/l4.json'];
  for (const need of needs) {
    if (!existsSync(join(OUT, need))) throw new Error(`target/intl-probe/${need} missing: run bench/intl-probe/scripts/build.sh and data.sh`);
  }
  const server = await serve();
  const url = `http://127.0.0.1:${server.address().port}/bench/intl-probe/web/index.html`;
  const page = await ctx.browser.newPage();
  const logs = [];
  page.on('console', (m) => (m.type() === 'error' || m.type() === 'warning') && logs.push(`${m.type()}: ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`pageerror: ${e}`));
  const result = { browser: ctx.browserName, version: ctx.browser.version(), items: {} };
  try { result.tree = readFileSync(join(OUT, 'pkg', 'tree.txt'), 'utf8').trim(); } catch (e) { /* older build */ }
  try {
    await page.goto(url);
    await page.waitForFunction(() => window.mf2probe !== undefined, null, { timeout: 30000 });
    result.userAgent = await page.evaluate(() => navigator.userAgent);
    result.crossOriginIsolated = await page.evaluate(() => window.crossOriginIsolated);
    ctx.assert('page-ready', true, result.userAgent);
    const call = (item, opts) => page.evaluate(([i, o]) => window.mf2probe[i](o), [item, opts ?? {}]);
    for (const item of items) {
      const t0 = Date.now();
      try {
        if (item === 'l4') {
          const intl = await call('l4', { variant: 'intl-cu' });
          const rust = await call('l4', { variant: 'rust' });
          result.items.l4 = { intl, rust };
          // The Rust localization layer, when the build has it (mf2-fn-number).
          if (existsSync(join(OUT, 'pkg', 'rust-loc', 'probe.js'))) result.items.l4.rustLoc = await call('l4', { variant: 'rust-loc' });
          const neutral = ['number', 'integer', 'offset'].reduce((n, f) => n + rust.byFile[f].pass, 0);
          ctx.assert('l4-rust-neutral-files', neutral === 70, `rust variant ${neutral}/70 on number, integer, offset (harness self-check)`);
          ctx.assert('l4-intl-cu-ran', true, `intl-cu ${intl.pass}/${intl.total}`);
        } else if (item === 'speed') {
          const runs = {};
          const m0 = mhz();
          runs.unthrottled = await call('speed', { rounds: 9, sampleMs: 60 });
          runs.unthrottled.mhz = [m0, mhz()];
          if (ctx.browserName === 'chromium') {
            const cdp = await page.context().newCDPSession(page);
            await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
            const m1 = mhz();
            runs.throttle4x = await call('speed', { rounds: 9, sampleMs: 60 });
            runs.throttle4x.mhz = [m1, mhz()];
            await cdp.send('Emulation.setCPUThrottlingRate', { rate: 1 });
          } else {
            runs.throttle4x = 'not available: no CPU throttling in this engine through Playwright (CDP is Chromium-only)';
          }
          result.items.speed = runs;
          ctx.assert('speed-ran', runs.unthrottled.rows.length > 0, `${runs.unthrottled.rows.length} rows`);
        } else if (item === 'loc') {
          const r = await call('loc');
          mkdirSync(join(OUT, 'results'), { recursive: true });
          writeFileSync(join(OUT, 'results', `loc-${ctx.browserName}.json`), `${JSON.stringify({ engine: `${ctx.browserName} ${result.version}`, out: r.out })}\n`);
          result.items.loc = { cases: r.out.length, file: `loc-${ctx.browserName}.json` };
          ctx.assert('loc-ran', r.out.length > 0, `${r.out.length} cases`);
        } else if (item === 'ecmaHandlers') {
          const r = await call(item);
          result.items[item] = r;
          ctx.assert('ecma-rust-wasm-equals-native', r.rustWasmEqualsNative === r.cases, `${r.rustWasmEqualsNative}/${r.cases} (harness self-check)`);
          ctx.assert('ecmaHandlers-ran', true, `intl = rust on ${r.same}/${r.cases}`);
        } else {
          result.items[item] = await call(item);
          ctx.assert(`${item}-ran`, true);
        }
      } catch (e) {
        ctx.assert(`${item}-ran`, false, String(e?.stack ?? e).slice(0, 600));
      }
      ctx.log(`${item}: ${((Date.now() - t0) / 1000).toFixed(1)} s`);
    }
  } finally {
    result.console = logs;
    mkdirSync(join(OUT, 'results'), { recursive: true });
    const file = join(OUT, 'results', `${ctx.browserName}.json`);
    let previous = {};
    try { previous = JSON.parse(readFileSync(file, 'utf8')); } catch (e) { /* first run */ }
    // Items not run this time keep their previous result.
    const merged = { ...previous, ...result, items: { ...(previous.items ?? {}), ...result.items } };
    writeFileSync(file, `${JSON.stringify(merged, null, 1)}\n`);
    ctx.data.results = file.split(sep).slice(-3).join('/');
    await page.close();
    server.close();
  }
}
