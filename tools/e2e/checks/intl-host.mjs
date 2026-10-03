// Phase 14 task 14.0 — `intl` reaches `Host::numbers` in real browsers.
//
// Serves the repository itself (its own static server on 127.0.0.1; only
// tools/e2e/intl-host/web/ and target/e2e-intl-host/ are served) and formats
// tools/e2e/intl-host's messages through the host its generated module
// names. Under `number-intl` the Rust number path is not linked, and a host with no
// number formatter gives a number's bare digits, an Unsupported Operation
// error, and a selector that matches only `*`. So each assertion below has an
// answer only the browser's `Intl` gives: grouping and a currency symbol from
// `Intl.NumberFormat`, the `one` variant (no exact key `1`) from
// `Intl.PluralRules`. If `mf2`'s `number-intl` arms of `__use_host!` are removed the
// module does not build or, named the plain host, fails every assertion.
//
// Prerequisite: tools/e2e/intl-host/build.sh.
//
//   node run.mjs intl-host --browser all

import { createServer } from 'node:http';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = join(REPO, 'target', 'e2e-intl-host');
const SERVED = ['tools/e2e/intl-host/web/', 'target/e2e-intl-host/'];
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.wasm': 'application/wasm',
};

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

// Each case: message, `$n`, and the text only `Intl` gives.
const CASES = [
  ['grouped', '1234567.5', '1,234,567.5'],
  ['price', '1234.5', '€1,234.50'],
  ['items', '1', 'one item'],
  ['items', '3', 'many items'],
];

export async function run(ctx) {
  if (!existsSync(join(OUT, 'pkg', 'e2e_intl_host.js'))) {
    throw new Error('target/e2e-intl-host/pkg/ missing: run tools/e2e/intl-host/build.sh');
  }
  const server = await serve();
  const page = await ctx.browser.newPage();
  const logs = [];
  page.on('console', (m) => (m.type() === 'error' || m.type() === 'warning') && logs.push(`${m.type()}: ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`pageerror: ${e}`));
  try {
    await page.goto(`http://127.0.0.1:${server.address().port}/tools/e2e/intl-host/web/index.html`);
    await page.waitForFunction(() => window.mf2ih !== undefined, null, { timeout: 60000 });
    const host = await page.evaluate(() => window.mf2ih.hostType());
    ctx.assert('intl-number-host', host.endsWith('::IntlNumbers'), host);
    for (const [id, n, want] of CASES) {
      const got = await page.evaluate(([i, v]) => window.mf2ih.format(i, v, '2026-10-02'), [id, n]);
      ctx.assert(`${id}(${n})`, got.text === want && got.errors.length === 0, { want, got });
    }
    // The date host it wraps still answers.
    const day = await page.evaluate(() => window.mf2ih.format('day', '0', '2026-10-02'));
    ctx.assert('date-through-wrapped-host', day.errors.length === 0 && day.text.includes('2026'), day);
    ctx.assert('page-quiet', logs.length === 0, logs.slice(0, 5));
  } finally {
    await page.close();
    server.close();
  }
}
