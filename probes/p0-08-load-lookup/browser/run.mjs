#!/usr/bin/env node
// P0.8 browser timing: Chromium (Playwright from tools/e2e, used read-only),
// desktop and 4× CPU throttle (CDP Emulation.setCPUThrottlingRate), both UTF-8
// strategies, fresh page per repetition (so every run has a cold first load).
//
//   node browser/run.mjs [--reps N] [--json out/browser.json]
//
// Files are served through page.route (no server) on a localhost origin with
// COOP/COEP, so the page is cross-origin isolated (5 µs performance.now()).
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { launch } from '../../../tools/e2e/lib/browser.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const { values } = parseArgs({ options: { reps: { type: 'string', default: '7' }, json: { type: 'string', default: 'out/browser.json' } } });
const REPS = Number(values.reps);
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.mf2b': 'application/octet-stream' };

function fileFor(path) {
  if (path === '/' || path === '/index.html') return join(here, 'index.html');
  if (path === '/run.js') return join(here, 'run.js');
  if (path.startsWith('/pkg-')) return join(root, 'out', path.slice(1));
  if (path.startsWith('/data/')) return join(root, 'out', 'web', path.slice(6));
  return undefined;
}

const { browser, executablePath } = await launch('chromium');
const report = { browser: browser.version(), executablePath, reps: REPS, runs: [] };
for (const variant of ['eager', 'per-access']) {
  for (const rate of [1, 4]) {
    for (let rep = 0; rep < REPS; rep++) {
      const context = await browser.newContext();
      await context.route('http://localhost:47808/**', (route) => {
        const url = new URL(route.request().url());
        const f = fileFor(url.pathname);
        if (!f) return route.fulfill({ status: 404, body: 'not found' });
        route.fulfill({
          status: 200,
          body: readFileSync(f),
          headers: {
            'content-type': TYPES[extname(f)] ?? 'application/octet-stream',
            'cross-origin-opener-policy': 'same-origin',
            'cross-origin-embedder-policy': 'require-corp',
            'cross-origin-resource-policy': 'same-origin',
            'cache-control': 'no-store',
          },
        });
      });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
      if (rate > 1) {
        const cdp = await context.newCDPSession(page);
        await cdp.send('Emulation.setCPUThrottlingRate', { rate });
      }
      await page.goto(`http://localhost:47808/index.html?variant=${variant}`);
      const r = await page.waitForFunction(() => window.__p08, null, { timeout: 180000 }).then((h) => h.jsonValue());
      report.runs.push({ variant, rate, rep, errors, ...r });
      await context.close();
      process.stderr.write(`${variant} ×${rate} rep ${rep}: cold ${r.coldLoadEnMs.toFixed(3)} ms, new(en) ${r.locales.en.newMs.toFixed(4)} ms, simple ${r.simpleNs.toFixed(1)} ns\n`);
    }
  }
}
await browser.close();
writeFileSync(join(root, values.json), JSON.stringify(report, null, 1));

// Summary (medians over reps).
const med = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
const lines = [];
lines.push(`Chromium ${report.browser}, ${REPS} fresh pages per row, medians. crossOriginIsolated: ${report.runs.every((r) => r.crossOriginIsolated)}; timer resolution ${med(report.runs.map((r) => r.timerResolutionMs)) * 1000} µs.`);
lines.push('');
lines.push('| UTF-8 strategy | CPU | first call incl. lazy compile (ms) | first real install, en: copy + Catalog::new (ms) | first switch to pl (ms) | install en, warm (ms) | Catalog::new en / pl / en-XA / ar-XB (ms) | from_utf8(pool) en (ms) | simple (ns) | 1-arg pattern (ns) | select (ns) |');
lines.push('|---|---|---|---|---|---|---|---|---|---|---|');
for (const variant of ['eager', 'per-access']) {
  for (const rate of [1, 4]) {
    const rs = report.runs.filter((r) => r.variant === variant && r.rate === rate);
    const m = (f) => med(rs.map(f));
    const news = ['en', 'pl', 'en-XA', 'ar-XB'].map((l) => m((r) => r.locales[l].newMs).toFixed(4)).join(' / ');
    lines.push(`| ${variant} | ${rate === 1 ? 'desktop' : `${rate}× throttle`} | ${m((r) => r.firstCallMs).toFixed(3)} | ${m((r) => r.coldCopyEnMs).toFixed(3)} + ${m((r) => r.coldNewEnMs).toFixed(3)} | ${m((r) => r.firstSwitchPlMs).toFixed(3)} | ${m((r) => r.locales.en.installMedianMs).toFixed(3)} | ${news} | ${m((r) => r.locales.en.utf8PoolMs).toFixed(4)} | ${m((r) => r.simpleNs).toFixed(1)} | ${m((r) => r.pattern1Ns).toFixed(0)} | ${m((r) => r.selectNs).toFixed(0)} |`);
  }
}
const errs = report.runs.flatMap((r) => r.errors);
lines.push('');
lines.push(`page errors: ${errs.length}${errs.length ? ' — ' + errs.slice(0, 3).join('; ') : ''}; spot check: ${JSON.stringify(report.runs[0].spot)}`);
console.log(lines.join('\n'));
writeFileSync(join(root, values.json.replace(/\.json$/, '.md')), lines.join('\n') + '\n');
