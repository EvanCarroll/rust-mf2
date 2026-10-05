// A browser's number formatter, through the host the generated module names,
// in real browsers: one module for each of `intl`, `builtin` and `plain`,
// each built with a server's `builtin` on beside it, as an application
// writes both sides on its one `mf2` line (Phase 14 task 14.0 for `intl`).
//
// Serves the repository itself (its own static server on 127.0.0.1; only
// tools/e2e/intl-host/web/ and target/e2e-intl-host/ are served) and formats
// tools/e2e/intl-host's messages. Each build has to format with its own
// side's formatter, and its catalog to carry what that formatter reads and
// nothing else:
//
//   * `intl`: the Rust number path is not linked, and a host with no number
//     formatter gives a number's bare digits, an Unsupported Operation error
//     and a selector that matches only `*`. So the text below is the
//     browser's `Intl`'s, the host is `IntlNumbers`, and the catalog has no
//     number entry at all. If `mf2` stops naming the `Intl` number host the
//     module does not build or fails every assertion.
//   * `builtin`: the same text from the catalog's own number data, with the
//     plain host: `Intl` is not asked.
//   * `plain`: plain digits, a plural form chosen by the catalog's rule, and
//     no number data downloaded. Its corpus has no `:currency`, which the
//     build refuses there.
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

const LOCALIZED = [
  ['grouped', '1234567.5', '1,234,567.5'],
  ['price', '1234.5', '€1,234.50'],
  ['bare', '1234567.5', '1,234,567.5 in all'],
  ['items', '1', 'one item'],
  ['items', '3', 'many items'],
];

// Each build: whether the runtime asks the host for numbers, the number
// entries its catalog carries, and each case's message, `$n` and text.
const BUILDS = [
  { build: 'intl', byHost: true, entries: [], cases: LOCALIZED },
  {
    build: 'builtin',
    byHost: false,
    entries: ['plural.cardinal', 'number.symbols', 'number.patterns', 'currency.data'],
    cases: LOCALIZED,
  },
  {
    build: 'plain',
    byHost: false,
    entries: ['plural.cardinal'],
    cases: [
      ['grouped', '1234567.5', '1234567.5'],
      ['bare', '1234567.5', '1234567.5 in all'],
      ['items', '1', 'one item'],
      ['items', '3', 'many items'],
    ],
  },
];

export async function run(ctx) {
  for (const { build } of BUILDS) {
    if (!existsSync(join(OUT, `pkg-${build}`, 'e2e_intl_host.js'))) {
      throw new Error(`target/e2e-intl-host/pkg-${build}/ missing: run tools/e2e/intl-host/build.sh`);
    }
  }
  const server = await serve();
  try {
    for (const { build, byHost, entries, cases } of BUILDS) {
      const page = await ctx.browser.newPage();
      const logs = [];
      page.on('console', (m) => (m.type() === 'error' || m.type() === 'warning') && logs.push(`${m.type()}: ${m.text()}`));
      page.on('pageerror', (e) => logs.push(`pageerror: ${e}`));
      try {
        await page.goto(`http://127.0.0.1:${server.address().port}/tools/e2e/intl-host/web/index.html?build=${build}`);
        await page.waitForFunction(() => window.mf2ih !== undefined, null, { timeout: 60000 });
        const host = await page.evaluate(() => window.mf2ih.hostType());
        ctx.assert(`${build}: number-host`, host.endsWith('::IntlNumbers') === byHost, host);
        const asked = await page.evaluate(() => window.mf2ih.byHost());
        ctx.assert(`${build}: runtime-asks-the-host`, asked === byHost, asked);
        const got = await page.evaluate(() => window.mf2ih.entries());
        ctx.assert(`${build}: catalog-entries`, JSON.stringify(got) === JSON.stringify(entries), { want: entries, got });
        for (const [id, n, want] of cases) {
          const out = await page.evaluate(([i, v]) => window.mf2ih.format(i, v, '2026-10-02'), [id, n]);
          ctx.assert(`${build}: ${id}(${n})`, out.text === want && out.errors.length === 0, { want, got: out });
        }
        // The date host it names, or wraps, still answers.
        const day = await page.evaluate(() => window.mf2ih.format('day', '0', '2026-10-02'));
        ctx.assert(`${build}: date-through-the-host`, day.errors.length === 0 && day.text.includes('2026'), day);
        ctx.assert(`${build}: page-quiet`, logs.length === 0, logs.slice(0, 5));
      } finally {
        await page.close();
      }
    }
  } finally {
    server.close();
  }
}
