// Conformance layer L4 in the browser for the `intl` client option. Driven by
// `cargo xtask l4-web`, which compiles the cases
// natively into target/l4-web/cases.bin, builds conformance/l4-web for
// wasm32-unknown-unknown with the `intl` features into target/l4-web/pkg/,
// runs this check, and judges what it writes.
//
// Serves target/l4-web/ (its own static server on 127.0.0.1; the page is
// inline), loads the module, formats every case in the engine and writes
//
//   target/l4-web/<browser>.txt   one record per case (`id TAB record`)
//   target/l4-web/<browser>.json  the engine: Playwright's version string,
//                                 navigator.userAgent, whether Intl is ready
//
// Asserts only that the engine ran the build it should (the `intl` build,
// Intl.NumberFormat v3 present) and wrote a record for every case; the records
// are judged by `cargo xtask l4-web`.
//
//   node run.mjs l4-intl --browser all

import { createServer } from 'node:http';
import { createReadStream, existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = join(REPO, 'target', 'l4-web');
const TYPES = {
  '.js': 'text/javascript', '.wasm': 'application/wasm', '.bin': 'application/octet-stream',
};
const PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>mf2 L4, intl build</title></head>
<body><main><h1>mf2 L4, intl build</h1><p id="status">loading</p></main>
<script type="module">
import init, { run, intl_ready } from './pkg/mf2_l4_web.js';
await init();
const bundle = new Uint8Array(await (await fetch('./cases.bin')).arrayBuffer());
window.mf2l4 = { ready: intl_ready(), run: () => run(bundle) };
document.getElementById('status').textContent = 'ready';
</script></body></html>
`;

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '');
    const headers = { 'Cache-Control': 'no-store' };
    if (path === '' || path === 'index.html') {
      res.writeHead(200, { ...headers, 'Content-Type': 'text/html; charset=utf-8' }).end(PAGE);
      return;
    }
    const file = normalize(join(OUT, path));
    const ok = file.startsWith(OUT) && (path.startsWith('pkg/') || path === 'cases.bin')
      && existsSync(file) && statSync(file).isFile();
    if (!ok) {
      res.writeHead(404, headers).end('not found');
      return;
    }
    res.writeHead(200, { ...headers, 'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream' });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

export async function run(ctx) {
  for (const need of ['pkg/mf2_l4_web.js', 'pkg/mf2_l4_web_bg.wasm', 'cases.bin']) {
    if (!existsSync(join(OUT, need))) throw new Error(`target/l4-web/${need} missing: run \`cargo xtask l4-web\``);
  }
  // The bundle starts with its case count (u32, little endian).
  const cases = readFileSync(join(OUT, 'cases.bin')).readUInt32LE(0);
  const server = await serve();
  const page = await ctx.browser.newPage();
  const logs = [];
  page.on('console', (m) => (m.type() === 'error' || m.type() === 'warning') && logs.push(`${m.type()}: ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`pageerror: ${e}`));
  try {
    await page.goto(`http://127.0.0.1:${server.address().port}/index.html`);
    await page.waitForFunction(() => window.mf2l4 !== undefined, null, { timeout: 60000 });
    const engine = {
      browser: ctx.browserName,
      version: ctx.browser.version(),
      userAgent: await page.evaluate(() => navigator.userAgent),
      intlReady: await page.evaluate(() => window.mf2l4.ready),
    };
    ctx.assert('intl-ready', engine.intlReady, 'the intl build, and Intl.NumberFormat v3 in the engine');
    const t0 = Date.now();
    const text = await page.evaluate(() => window.mf2l4.run());
    engine.seconds = (Date.now() - t0) / 1000;
    const lines = text.split('\n').filter((l) => l !== '');
    writeFileSync(join(OUT, `${ctx.browserName}.txt`), `${lines.join('\n')}\n`);
    writeFileSync(join(OUT, `${ctx.browserName}.json`), `${JSON.stringify({ ...engine, console: logs }, null, 1)}\n`);
    ctx.assert('records', lines.length === cases, `${lines.length} records for ${cases} cases in ${engine.seconds} s`);
    ctx.assert('no-console-errors', logs.length === 0, logs.slice(0, 5).join(' | ') || undefined);
    ctx.data.engine = engine;
  } finally {
    await page.close();
    server.close();
  }
}
