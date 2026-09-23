// Conformance layer L6 in the browser (plans/01-conformance.md §3;
// plans/14-phase-6-work-order.md A6b). Driven by `cargo xtask l6-web`, which
// renders the page with `conformance/l6-web`'s `l6-page` binary, builds the
// same crate for wasm32-unknown-unknown with `hydrate`, and runs this.
//
// Serves target/l6-web/ from its own static server; the page on disk is the
// *server-rendered* one, so what the browser does to it is hydration and
// nothing else.
//
// What it asserts, and why a browser is needed for it:
//
//   * hydration changes no text, per case, and logs nothing. A text
//     difference is silent (P0.10) and a structural one traps the wasm, so
//     "the console is empty" is the assertion;
//   * the node registry holds every case;
//   * a switch to the twin locale reaches every node — the twin carries the
//     same messages, so this is about *reaching* them;
//   * switching back restores the server's text, per case, exactly.
//
// `exp` itself is not re-checked here: the server-rendered text *is* `exp`,
// which layer L6 asserts in Rust over these same cases. The question a
// browser can answer is whether the client agrees with the server.
//
//   node run.mjs l6 --browser chromium

import { createServer } from 'node:http';
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

import { watchConsole, captureSsrSnapshot, sleep } from '../lib/browser.mjs';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = join(REPO, 'target', 'l6-web');
const TYPES = {
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.html': 'text/html; charset=utf-8',
  '.json': 'application/json',
  '.mf2b': 'application/octet-stream',
};

function serve() {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(
      /^(\.\.[/\\])+/,
      '',
    );
    const file = join(OUT, path === '/' ? 'index.html' : path);
    if (!file.startsWith(OUT) || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404).end('not found');
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

/** `id → text`, one entry per rendered case. */
const CASE_TEXT = () =>
  Object.fromEntries(
    [...document.querySelectorAll('.case[data-id]')].map((el) => [
      el.dataset.id,
      el.innerText,
    ]),
  );

/** The ids whose text differs between two snapshots, with both values. */
function differences(before, after, limit = 5) {
  const out = [];
  for (const id of Object.keys(before)) {
    if (before[id] !== after[id]) out.push({ id, before: before[id], after: after[id] });
    if (out.length >= limit) break;
  }
  return out;
}

export async function run(ctx) {
  const { browser, assert, data } = ctx;

  if (!existsSync(join(OUT, 'index.html'))) {
    assert('page-rendered', false, `${OUT}/index.html is missing; run \`cargo xtask l6-web\``);
    return;
  }
  const page_ = JSON.parse(readFileSync(join(OUT, 'page.json'), 'utf8'));
  data.page = page_;

  const server = await serve();
  const base = `http://127.0.0.1:${server.address().port}`;
  const context = await browser.newContext();
  await captureSsrSnapshot(context);
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);

  try {
    await page.goto(`${base}/index.html`, { waitUntil: 'load' });

    // The server's own text, read before the wasm has booted.
    const served = await page.evaluate(CASE_TEXT);
    assert('every-case-rendered', Object.keys(served).length === page_.cases, {
      rendered: Object.keys(served).length,
      cases: page_.cases,
    });

    await hydrated(page);
    const hydratedText = await page.evaluate(CASE_TEXT);
    const changed = differences(served, hydratedText);
    assert('hydration-changes-no-text', changed.length === 0, changed);
    assert('console-is-silent-through-hydration', console_.length === 0, console_.slice(0, 3));

    const live = await liveNodes(page);
    assert('the-registry-holds-every-case', live >= page_.cases, {
      live,
      cases: page_.cases,
    });
    data.liveNodes = live;

    // The twin carries the same messages, so the switch is about reaching
    // every node rather than about what it writes.
    const switched = await setLocale(page, page_.twin);
    assert('switch-to-the-twin-succeeds', switched === null, switched);
    assert(
      'switch-updates-html-lang',
      (await page.getAttribute('html', 'lang')) !== page_.locale,
      await page.getAttribute('html', 'lang'),
    );

    // …and back: this is the comparison that catches a node the registry
    // rewrote wrongly.
    const back = await setLocale(page, page_.locale);
    assert('switch-back-succeeds', back === null, back);
    const after = await page.evaluate(CASE_TEXT);
    const drifted = differences(served, after);
    assert('switching-back-restores-the-server-text', drifted.length === 0, drifted);

    assert('the-registry-leaks-no-slots', (await liveNodes(page)) === live, {
      before: live,
      after: await liveNodes(page),
    });
    assert('console-is-silent-throughout', console_.length === 0, console_.slice(0, 3));
  } finally {
    await context.close();
    server.close();
  }
}

async function hydrated(page) {
  await page.waitForFunction(
    async () => {
      try {
        const mod = await import('/pkg/mf2_l6_web.js');
        return typeof mod.mf2_live_nodes === 'function' && mod.mf2_live_nodes() > 0;
      } catch {
        return false;
      }
    },
    undefined,
    { timeout: 60000 },
  );
  await sleep(50);
}

const liveNodes = (page) =>
  page.evaluate(async () => (await import('/pkg/mf2_l6_web.js')).mf2_live_nodes());

/** `null` on success, the error message otherwise. */
async function setLocale(page, tag) {
  const failure = await page.evaluate(async (tag) => {
    try {
      await (await import('/pkg/mf2_l6_web.js')).mf2_set_locale(tag);
      return null;
    } catch (e) {
      return String(e);
    }
  }, tag);
  await sleep(50);
  return failure;
}
