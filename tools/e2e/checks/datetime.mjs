// Phase 4 task A6 — the `intl` date formatter against ICU4X in real browsers
// (plans/11-phase-4-work-order.md A6; plans/03-runtime.md §5.1–§5.2).
//
// Serves the repository itself (its own static server on 127.0.0.1; only
// tools/e2e/datetime/web/ and target/e2e-datetime/ are served), opens
// tools/e2e/datetime/web/index.html, and formats every case of
// target/e2e-datetime/cases.json — the locale panel × date/time messages,
// each a one-message catalog — with the browser module: `:date`, `:time`,
// `:datetime` over the `Intl` backend, `Intl.DateTimeFormat` through
// mf2-host-web. Each text is compared with ICU4X's for the same catalog (the
// server's text, from the catalog's icu.blob, recorded natively by
// tools/e2e/datetime/build.sh), within the tolerance: P0.10's U+202F
// (narrow no-break space) ↔ U+0020, and U+00A0 with them — engines write any
// of the three where CLDR 48 has U+202F ("3:04 PM", "p. m.").
//
// Cases come in four kinds (build.sh tags each):
//
//   suite        the WG suite's date files (functions/{date,time,datetime}.json,
//                en-US, with their params): layer L4 of the `Intl` backend in
//                this engine — the errors must be the test's (and ICU4X's),
//                the text the test's `exp` where it has one
//
// and three mappings onto ECMA-402 for the panel's locales:
//
//   styles       `dateStyle` / `timeStyle` — the mapping is exact: CLDR's
//                standard formats, which the semantic skeletons resolve to
//                (a time alone with `hour12` is mapped to components: V8
//                and JavaScriptCore apply `hour12` to a 24-hour locale's
//                `timeStyle` pattern by swapping its hour field, 03:04 PM)
//   components   `year` / `month` / `day` / `weekday` / `hour` … — not
//                exact: a component asks for, e.g., an abbreviated month
//                where a locale's semantic skeleton has a numeric one
//   zoned        components with `timeZoneName` (ECMA-402 does not mix it
//                with styles)
//
// Asserted: the page runs the Intl backend; every suite case passes; every
// panel case formats without errors in both; every `styles` case is within
// the tolerance or one of the
// KNOWN divergences below (a new divergence fails; a known one that no
// longer shows is logged, so the list can shrink). `components` and
// `zoned` agreement is recorded, not asserted: cosmetic, and harmless to
// hydration (P0.10: the server's text stays until the node's next update).
//
// Results: target/e2e-datetime/results/<browser>.json.
// Prerequisite: tools/e2e/datetime/build.sh.
//
//   node run.mjs datetime --browser all

import { createServer } from 'node:http';
import { createReadStream, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = join(REPO, 'target', 'e2e-datetime');
const SERVED = ['tools/e2e/datetime/web/', 'target/e2e-datetime/'];
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.wasm': 'application/wasm', '.json': 'application/json',
};

// `styles` cases outside the tolerance, with why (September 2026: Chromium
// 143, Firefox 155, WebKit 26.6 against ICU4X 2.3 / CLDR 48.2.1).
const KNOWN = [
  {
    browsers: ['chromium', 'firefox', 'webkit'], locales: ['pl'],
    src: /:date fields=year-month-day length=short|:datetime dateLength=short/,
    why: "CLDR 48's semantic skeleton for a short date (dd.MM.y, 02.01.2006) is not its short dateFormat (d.MM.y, 2.01.2006), which dateStyle gives",
  },
  {
    browsers: ['chromium', 'webkit'], locales: ['es'], src: /:datetime dateLength=long/,
    why: 'the engine\'s CLDR joins a long date and a time with ", " where CLDR 48 has "a las"',
  },
  {
    browsers: ['chromium'], locales: ['ar'], src: /:datetime dateLength=short/,
    why: 'the engine\'s CLDR joins a short date and a time with a space where CLDR 48 has "،"',
  },
  {
    browsers: ['chromium'], locales: ['cy'], src: /./,
    why: "Chromium's headless shell ships no Welsh data: cy formats as English",
  },
];

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '');
    const file = normalize(join(REPO, path));
    const ok = file.startsWith(REPO) && SERVED.some((p) => path.startsWith(p)) && existsSync(file) && statSync(file).isFile();
    const headers = { 'Cache-Control': 'no-store' };
    if (!ok) {
      res.writeHead(404, headers).end('not found');
      return;
    }
    res.writeHead(200, { ...headers, 'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream' });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

/** The tolerance: U+202F and U+00A0 read as U+0020. */
const tolerant = (s) => s.replace(/[\u202f\u00a0]/g, ' ');

export async function run(ctx) {
  for (const need of ['cases.json', 'pkg/e2e_datetime_wasm.js']) {
    if (!existsSync(join(OUT, need))) throw new Error(`target/e2e-datetime/${need} missing: run tools/e2e/datetime/build.sh`);
  }
  const { cases } = JSON.parse(readFileSync(join(OUT, 'cases.json'), 'utf8'));
  const server = await serve();
  const page = await ctx.browser.newPage();
  const logs = [];
  page.on('console', (m) => (m.type() === 'error' || m.type() === 'warning') && logs.push(`${m.type()}: ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`pageerror: ${e}`));
  try {
    await page.goto(`http://127.0.0.1:${server.address().port}/tools/e2e/datetime/web/index.html`);
    await page.waitForFunction(() => window.mf2dt !== undefined, null, { timeout: 60000 });
    const backend = await page.evaluate(() => window.mf2dt.backend());
    ctx.assert('intl-backend', backend.endsWith('::Intl'), backend);
    const got = await page.evaluate(() => window.mf2dt.runAll());
    ctx.assert('all-cases-ran', got.length === cases.length, `${got.length}/${cases.length}`);
    const rows = cases.map((c, i) => {
      const intl = got[i];
      const kind = intl.text === c.icu ? 'identical' : tolerant(intl.text) === tolerant(c.icu) ? 'tolerated' : 'different';
      const known = kind === 'different'
        ? KNOWN.find((k) => k.browsers.includes(ctx.browserName) && k.locales.includes(c.locale) && k.src.test(c.src))
        : undefined;
      return {
        locale: c.locale, src: c.src, mapping: c.mapping, kind, known: known?.why,
        icu: c.icu, intl: intl.text, intlErrors: intl.errors, icuErrors: c.icuErrors,
      };
    });
    const summary = {};
    for (const mapping of ['suite', 'styles', 'components', 'zoned']) {
      const list = rows.filter((r) => r.mapping === mapping);
      const n = (kind) => list.filter((r) => r.kind === kind).length;
      summary[mapping] = { total: list.length, identical: n('identical'), tolerated: n('tolerated'), different: n('different') };
    }
    // L4 on the suite's date files: the test's errors (and ICU4X's), and
    // its text where it has one.
    const same = (a, b) => JSON.stringify([...a].sort()) === JSON.stringify([...b].sort());
    const suiteRows = rows.map((r, i) => [r, cases[i]]).filter(([r]) => r.mapping === 'suite');
    const failing = suiteRows.filter(([r, c]) =>
      !same(r.intlErrors, c.expErrors) || !same(r.icuErrors, c.expErrors)
      || (typeof c.exp === 'string' && (r.intl !== c.exp || r.icu !== c.exp)));
    ctx.assert('suite-date-files', suiteRows.length > 0 && failing.length === 0,
      failing.length === 0
        ? { ...summary.suite, tests: suiteRows.length }
        : failing.slice(0, 5).map(([r, c]) => ({ suite: c.suite, src: r.src, intl: r.intl, intlErrors: r.intlErrors, exp: c.exp, expErrors: c.expErrors })));
    const errors = rows.filter((r) => r.mapping !== 'suite' && (r.intlErrors.length > 0 || r.icuErrors.length > 0));
    ctx.assert('no-errors', errors.length === 0, errors.slice(0, 5));
    const styles = rows.filter((r) => r.mapping === 'styles');
    const unexplained = styles.filter((r) => r.kind === 'different' && r.known === undefined);
    ctx.assert('styles-within-tolerance', unexplained.length === 0,
      unexplained.length === 0
        ? { ...summary.styles, known: styles.filter((r) => r.known).length }
        : { ...summary.styles, unexplained: unexplained.slice(0, 8) });
    for (const k of KNOWN.filter((k) => k.browsers.includes(ctx.browserName))) {
      const shown = styles.some((r) => r.known === k.why);
      if (!shown) ctx.log(`known divergence no longer shows (the list can shrink): ${k.why}`);
    }
    ctx.log('suite text against ICU4X (recorded):', JSON.stringify(summary.suite));
    ctx.log('components (recorded):', JSON.stringify(summary.components));
    ctx.log('zoned (recorded):', JSON.stringify(summary.zoned));
    ctx.assert('page-quiet', logs.length === 0, logs.slice(0, 5));
    mkdirSync(join(OUT, 'results'), { recursive: true });
    writeFileSync(join(OUT, 'results', `${ctx.browserName}.json`), `${JSON.stringify({
      browser: ctx.browserName, version: ctx.browser.version(), summary, rows,
    }, null, 2)}\n`);
    ctx.data.summary = summary;
  } finally {
    await page.close();
    server.close();
  }
}
