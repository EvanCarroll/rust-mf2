// The speed of one date placeholder in a browser, `Intl` against ICU4X
// (plan/08 §9; Phase 22 runs it). Serves the repository as datetime.mjs does,
// opens tools/e2e/datetime/web/speed.html, which loads the three builds of
// tools/e2e/datetime/speed (`intl`, `icu`, `icu-cached`) and alternates
// them over the catalogs of target/e2e-datetime/speed.json: a date, a date
// and time, a date and time with a zone name, in en, pl and ar.
//
// Asserted (the run is sound, not fast): each build formats with the
// backend it was built for; every catalog loads; every value parses; every
// build formats the first value without errors; the ICU4X builds write
// ICU4X's native text byte for byte (the same blob). `Intl`'s agreement is
// recorded, not asserted (datetime.mjs owns it). Timed unthrottled, and in
// Chromium also at 4× CPU throttle (CDP).
//
// Results: target/e2e-datetime/results/speed-<browser>.json.
// Prerequisite: tools/e2e/datetime/speed.sh's build step (it runs this).
//
//   node run.mjs datetime-speed --browser all

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { serve } from './datetime.mjs';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = join(REPO, 'target', 'e2e-datetime');
const BUILDS = ['intl', 'icu', 'icu-cached'];

const mhz = () => {
  try {
    return Number(/cpu MHz\s*:\s*([\d.]+)/.exec(readFileSync('/proc/cpuinfo', 'utf8'))?.[1]);
  } catch (e) {
    return null;
  }
};

export async function run(ctx) {
  for (const need of ['speed.json', ...BUILDS.map((b) => `speed/${b}/speed.js`)]) {
    if (!existsSync(join(OUT, need))) throw new Error(`target/e2e-datetime/${need} missing: run tools/e2e/datetime/speed.sh`);
  }
  const server = await serve();
  const page = await ctx.browser.newPage();
  const logs = [];
  page.on('console', (m) => (m.type() === 'error' || m.type() === 'warning') && logs.push(`${m.type()}: ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`pageerror: ${e}`));
  try {
    await page.goto(`http://127.0.0.1:${server.address().port}/tools/e2e/datetime/web/speed.html`);
    await page.waitForFunction(() => window.mf2speed !== undefined, null, { timeout: 60000 });
    const builds = await page.evaluate(() => window.mf2speed.builds());
    const expected = await page.evaluate(() => window.mf2speed.expectedValues());
    const by = Object.fromEntries(builds.map((b) => [b.name, b]));
    ctx.assert('backends', by.intl.backend.endsWith('::Intl')
      && by.icu.backend.includes('::Icu') && !by.icu.backend.endsWith('(cache)')
      && by['icu-cached'].backend.endsWith('(cache)'), builds.map((b) => b.backend));
    ctx.assert('catalogs-load', builds.every((b) => b.loadErrors.length === 0), builds.map((b) => [b.name, b.loadErrors]));
    ctx.assert('values-parse', builds.every((b) => b.values === expected), builds.map((b) => [b.name, b.values, expected]));
    const samples = await page.evaluate(() => window.mf2speed.samples());
    const erring = samples.filter((s) => s.nativeErrors.length > 0 || BUILDS.some((b) => s[b].errors.length > 0));
    ctx.assert('no-errors', erring.length === 0, erring.slice(0, 5));
    const unlike = samples.filter((s) => s.icu.text !== s.native || s['icu-cached'].text !== s.native);
    ctx.assert('icu-writes-native-text', unlike.length === 0, unlike.slice(0, 5));
    ctx.log('intl against native (recorded):', JSON.stringify(samples.map((s) => [s.locale, s.name, s.intl.text === s.native])));
    const runs = {};
    const m0 = mhz();
    runs.unthrottled = await page.evaluate(() => window.mf2speed.run({ rounds: 9, sampleMs: 60 }));
    runs.unthrottled.mhz = [m0, mhz()];
    if (ctx.browserName === 'chromium') {
      const cdp = await page.context().newCDPSession(page);
      await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
      const m1 = mhz();
      runs.throttle4x = await page.evaluate(() => window.mf2speed.run({ rounds: 9, sampleMs: 60 }));
      runs.throttle4x.mhz = [m1, mhz()];
      await cdp.send('Emulation.setCPUThrottlingRate', { rate: 1 });
    } else {
      runs.throttle4x = 'not available: no CPU throttling in this engine through Playwright (CDP is Chromium-only)';
    }
    ctx.assert('speed-ran', runs.unthrottled.rows.length === samples.length, `${runs.unthrottled.rows.length} rows`);
    ctx.assert('page-quiet', logs.length === 0, logs.slice(0, 5));
    mkdirSync(join(OUT, 'results'), { recursive: true });
    writeFileSync(join(OUT, 'results', `speed-${ctx.browserName}.json`), `${JSON.stringify({
      browser: ctx.browserName, version: ctx.browser.version(), builds, samples, runs,
    }, null, 2)}\n`);
    ctx.data.speed = runs;
  } finally {
    await page.close();
    server.close();
  }
}
