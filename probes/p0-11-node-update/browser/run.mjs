#!/usr/bin/env node
// P0.11 in a browser: strategy A (RenderEffect per node) vs B (registry), real
// mf2-probe Tr nodes in a real DOM, with and without 4x CPU throttling (CDP,
// Chromium). Uses the Playwright install of tools/e2e read-only; serves the
// harness through page.route (no server).
//
//   node probes/p0-11-node-update/browser/run.mjs [--json FILE] [--runs N]
//
// Prerequisite: probes/p0-11-node-update/scripts/build.sh (pkg-a, pkg-b).
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { launch } from '../../../tools/e2e/lib/browser.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const probe = join(here, '..');
const { values } = parseArgs({ options: {
  json: { type: 'string' }, runs: { type: 'string', default: '1' },
  live: { type: 'string', default: '2000' }, churn: { type: 'string', default: '100000' },
  rows: { type: 'string', default: '50' }, messages: { type: 'string', default: '1600' },
} });
const P = { live: +values.live, churn: +values.churn, rows: +values.rows, messages: +values.messages, switches: 21 };
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

async function bench(page, P) {
  return page.evaluate(async (P) => {
    const m = await import('./p0_11_wasm.js');
    await m.default();
    const nextTask = () => new Promise((r) => { const c = new MessageChannel(); c.port1.onmessage = () => r(); c.port2.postMessage(0); });
    const med = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
    m.init(P.messages);
    await nextTask();
    const h0 = m.heap_live(), a0 = m.heap_allocs();
    let t = performance.now();
    m.mount_live(P.live);
    const mountMs = performance.now() - t;
    await nextTask();
    const h1 = m.heap_live(), a1 = m.heap_allocs();
    const spans = document.querySelectorAll('#live span');
    // One switch = script (sync call + microtasks until the first and last
    // live node show the new locale) + the forced style/layout that the next
    // frame would do anyway (same DOM work for both strategies).
    const sw = async (n, k0) => {
      const sync = [], script = [], layout = [], frame = [];
      const last = spans[spans.length - 1];
      for (let k = 0; k < n; k++) {
        m.prepare(k0 + k + 1);
        await nextTask();
        document.body.offsetHeight;
        const want = (k0 + k + 1) % 2 === 0 ? 'en' : 'pl';
        const t0 = performance.now();
        m.switch_prepared();
        const t1 = performance.now();
        let spins = 0;
        while (!(spans[1].textContent.startsWith(want) && last.textContent.startsWith(want))) {
          if (++spins > 100000) throw new Error(`switch ${k} not applied: ${spans[1].textContent}`);
          await Promise.resolve();
        }
        const t2 = performance.now();
        document.body.offsetHeight;
        const t3 = performance.now();
        sync.push(t1 - t0); script.push(t2 - t0); layout.push(t3 - t2); frame.push(t3 - t0);
      }
      return { syncMedian: med(sync), scriptMedian: med(script), scriptMax: Math.max(...script), scriptFirst: script[0],
               layoutMedian: med(layout), frameMedian: med(frame), frameMax: Math.max(...frame), frameFirst: frame[0] };
    };
    const before = await sw(P.switches, 0);
    const hc0 = m.heap_live();
    const samples = [];
    const batch = P.rows * 10;
    t = performance.now();
    for (let done = 0; done < P.churn; ) {
      m.churn(batch, P.rows, done);
      done += batch;
      await nextTask();
      if (done % (P.churn / 10) === 0) samples.push([done, m.heap_live() - hc0]);
    }
    const churnMs = performance.now() - t;
    const grown = m.heap_live() - hc0;
    const after = await sw(P.switches, P.switches);
    const afterSwitchGrowth = m.heap_live() - hc0;
    return {
      strategy: m.strategy(), bindings: m.bindings(),
      perNodeBytes: (h1 - h0) / P.live, perNodeAllocs: (a1 - a0) / P.live, mountMs,
      before, churnMs, grown, perChurned: grown / P.churn, samples, after, afterSwitchGrowth,
      wasmMemory: m.__wasm?.memory?.buffer?.byteLength,
    };
  }, P);
}

const out = [];
const { browser, executablePath } = await launch('chromium');
console.log(`chromium: ${executablePath}`);
for (let run = 0; run < +values.runs; run++) {
  for (const throttle of [1, 4]) {
    for (const s of ['a', 'b']) {
      const ctx = await browser.newContext();
      const page = await ctx.newPage();
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      page.on('console', (msg) => { if (msg.type() === 'error' || msg.type() === 'warning') errors.push(msg.text()); });
      await page.route('http://p011.test/**', (route) => {
        const path = new URL(route.request().url()).pathname;
        const file = path.endsWith('/index.html') ? join(here, 'index.html') : join(probe, 'target', `pkg-${s}`, path.split('/').pop());
        route.fulfill({ body: readFileSync(file), contentType: TYPES[extname(file)] ?? 'application/octet-stream' });
      });
      await page.goto(`http://p011.test/${s}/index.html`);
      if (throttle > 1) {
        const cdp = await ctx.newCDPSession(page);
        await cdp.send('Emulation.setCPUThrottlingRate', { rate: throttle });
      }
      const r = await bench(page, P);
      r.throttle = throttle; r.run = run; r.errors = errors;
      out.push(r);
      const f = (x) => x.toFixed(2);
      console.log(`${r.strategy} ${throttle}x: ${f(r.perNodeBytes)} B/node, ${f(r.perNodeAllocs)} allocs/node, mount ${f(r.mountMs)} ms; ` +
        `switch script ${f(r.before.scriptMedian)} ms (sync ${f(r.before.syncMedian)}, max ${f(r.before.scriptMax)}) + layout ${f(r.before.layoutMedian)} = ${f(r.before.frameMedian)} ms; ` +
        `churn ${P.churn} in ${f(r.churnMs)} ms, heap +${r.grown} B (${f(r.perChurned)} B/node) [${r.samples.map(([n, b]) => `${n / 1000}k:${b}`).join(' ')}]; ` +
        `after churn: first switch script ${f(r.after.scriptFirst)} ms, median ${f(r.after.scriptMedian)} + layout ${f(r.after.layoutMedian)}; heap after +${r.afterSwitchGrowth}` +
        (errors.length ? `; ${errors.length} console errors/warnings: ${errors.slice(0, 3).join(' | ')}` : ''));
      await ctx.close();
    }
  }
}
await browser.close();
if (values.json) writeFileSync(values.json, JSON.stringify({ params: P, results: out }, null, 2));
