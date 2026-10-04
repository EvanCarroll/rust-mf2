// The server's date formatter is not the browser's (plan/08 §4.3). Driven by
// `cargo xtask l7-web`, which renders conformance L7's en-US islands page
// twice with `conformance/l7-web`'s `l7-page` — once by a server with the ISO
// stand-in (target/l7-web/dates-iso/), once with ICU4X (dates-icu/) — and
// hydrates both with a client that formats with `Intl`. Each page states the
// server's formatter on its preload link (`data-mf2-dates`).
//
// The date cases are the ones whose server text differs between the two
// renders. In each engine, in a UTC browser (so that the reader's-zone
// correction has nothing to do and only the formatter is tested):
//
//   * ISO on the server: the served page holds the server's text; after
//     hydration every date case shows another text — the browser's own — and
//     at least one of them agrees with ICU4X's text, read with P0.10's
//     tolerance (U+202F and U+00A0 as a space), so the new text is a
//     localized date; every other case keeps the server's text;
//   * ICU4X on the server: nothing is rewritten — every case keeps the
//     server's text after hydration, `Intl` in the browser or not;
//   * both: hydration completes, and the console stays silent.
//
//   node run.mjs dates-formatter --browser chromium,firefox,webkit

import { createServer } from 'node:http';
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
import { extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import { watchConsole, sleep, until } from '../lib/browser.mjs';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = normalize(process.env.L7_DIR ?? join(REPO, 'target', 'l7-web')) + sep;
const LOCALE = 'en-US';
const PAGE = `islands-${LOCALE}.html`;
const PKG = '/pkg-islands/mf2_l7_web.js';
const TYPES = {
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.html': 'text/html; charset=utf-8',
  '.json': 'application/json',
  '.mf2b': 'application/octet-stream',
};

function serve(root) {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '');
    const file = normalize(join(root, path));
    if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404).end('not found');
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

/** `id → textContent`, one entry per case on the page. */
const CASE_TEXT = () =>
  Object.fromEntries(
    [...document.querySelectorAll('.case[data-id]')].map((el) => [el.dataset.id, el.textContent]),
  );

/** tachys writes an empty text node as one space on the server. */
const norm = (text) => (text === ' ' ? '' : text);

/** P0.10's tolerance: the narrow and the no-break space read as a space. */
const loose = (text) => norm(text ?? '').replace(/[\u202f\u00a0]/g, ' ');

function manifest(configuration) {
  const path = join(OUT, configuration, `${LOCALE}.json`);
  return existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : undefined;
}

/** Serves one configuration's directory, opens its page, waits for hydration. */
async function hydrate(browser, configuration, cases) {
  const server = await serve(join(OUT, configuration) + sep);
  const base = `http://127.0.0.1:${server.address().port}`;
  const context = await browser.newContext({ timezoneId: 'UTC' });
  await context.addInitScript(`document.addEventListener('DOMContentLoaded', () => {
    window.__served = (${CASE_TEXT})();
    const link = document.querySelector('link[data-mf2]');
    window.__stated = link ? link.getAttribute('data-mf2-dates') : null;
  });`);
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);
  const out = { served: {}, hydrated: {}, stated: null, error: null, console: console_ };
  try {
    await page.goto(`${base}/${PAGE}`, { waitUntil: 'load' });
    out.served = await page.evaluate(() => window.__served ?? {});
    out.stated = await page.evaluate(() => window.__stated ?? null);
    await until(
      page,
      `import('${PKG}').then((m) => m.mf2_live_nodes() >= ${cases}).catch(() => false)`,
      120000,
    );
    // The rewrite runs after the synchronous hydration; give it a turn.
    await sleep(100);
    out.hydrated = await page.evaluate(CASE_TEXT);
  } catch (e) {
    out.error = String(e?.message ?? e).split('\n')[0];
  } finally {
    await context.close();
    server.close();
  }
  return out;
}

export async function run(ctx) {
  const { browser, assert, data } = ctx;
  const iso = manifest('dates-iso');
  const icu = manifest('dates-icu');
  if (!iso || !icu) {
    assert('pages-built', false, `no dates-iso/ or dates-icu/ under ${OUT}; run \`cargo xtask l7-web\``);
    return;
  }
  const icuText = Object.fromEntries(icu.cases.map((c) => [c.id, c.text]));
  const dates = new Set(iso.cases.filter((c) => norm(c.text) !== norm(icuText[c.id])).map((c) => c.id));
  assert('date-cases', dates.size > 0, { dates: dates.size, cases: iso.cases.length });

  // ISO on the server, `Intl` in the browser: every date is rewritten.
  const a = await hydrate(browser, 'dates-iso', iso.cases.length);
  assert('iso/hydrated', a.error === null, a.error ?? undefined);
  assert('iso/states-iso', a.stated === 'iso', { stated: a.stated });
  const kept = [];
  const missed = [];
  const changed = [];
  let localized = 0;
  for (const c of iso.cases) {
    const served = norm(a.served[c.id] ?? '');
    const now = norm(a.hydrated[c.id] ?? '');
    if (dates.has(c.id)) {
      if (now === served) kept.push([c.id, served]);
      if (loose(now) === loose(icuText[c.id])) localized += 1;
    } else if (now !== served) {
      changed.push([c.id, served, now]);
    }
    if (served !== norm(c.text)) missed.push([c.id, served, c.text]);
  }
  assert('iso/served-as-rendered', missed.length === 0, missed.slice(0, 3));
  assert('iso/every-date-rewritten', kept.length === 0, { kept: kept.length, first: kept.slice(0, 3) });
  assert('iso/rewritten-to-a-localized-date', localized > 0, { localized, dates: dates.size });
  assert('iso/nothing-else-changed', changed.length === 0, changed.slice(0, 3));
  assert('iso/console-silent', a.console.length === 0, a.console.slice(0, 3));
  data.iso = { dates: dates.size, localized };

  // ICU4X on the server, `Intl` in the browser: nothing is rewritten.
  const b = await hydrate(browser, 'dates-icu', icu.cases.length);
  assert('icu/hydrated', b.error === null, b.error ?? undefined);
  assert('icu/states-icu', b.stated === 'icu', { stated: b.stated });
  const rewritten = icu.cases
    .filter((c) => norm(b.hydrated[c.id] ?? '') !== norm(b.served[c.id] ?? ''))
    .map((c) => [c.id, b.served[c.id], b.hydrated[c.id]]);
  assert('icu/not-rewritten', rewritten.length === 0, rewritten.slice(0, 3));
  assert('icu/console-silent', b.console.length === 0, b.console.slice(0, 3));
}
