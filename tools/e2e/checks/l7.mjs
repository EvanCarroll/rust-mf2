// Conformance layer L7 in the browser. Driven by `cargo xtask l7-web`, which
// renders each set's pages with `conformance/l7-web`'s `l7-page` binary into
// target/l7-web/<configuration>/, builds the islands and the client-only
// wasm, runs this, and judges the ledger's L7 columns from what it records.
//
// For every configuration directory of target/l7-web/ (or $L7_DIR) — `all`,
// `default` — served at the root of a static server of its own (the pages
// use root-relative URLs, as an application's do), every set
// (`<locale>.json`) and both delivery modes, in a fresh browser context:
//
//   * islands (`islands-<locale>.html`, columns L7 / L7d): the server's text
//     per case, read at DOMContentLoaded, before any wasm; hydration — every
//     case an island behind the gate — changes none of it; the registry holds
//     every case;
//   * client-only (`csr-<locale>.html`, columns L7c / L7cd): the boot picks
//     the set's locale (remembered in `localStorage`, below), sets `<html
//     lang>`, and mounts every case with the text the server renders for it
//     (the `text` of `<locale>.json`);
//
// and then, in both: a switch to the twin gives every case the text the
// server renders in the twin, and switching back gives the first text again,
// exactly; the registry leaks nothing; the console stays silent.
//
// What it records (`data.pages`) is per case: `cargo xtask l7-web` turns a
// case's failures — and its page's — into that test's cell. A case's text is
// its `.case` element's `textContent`, compared exactly, except that tachys
// writes an empty text node as one space on the server and as nothing in
// the browser, so a text that is exactly " " counts as "".
//
//   node run.mjs l7 --browser chromium,firefox

import { createServer } from 'node:http';
import { createReadStream, existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import { watchConsole, sleep, until } from '../lib/browser.mjs';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const OUT = normalize(process.env.L7_DIR ?? join(REPO, 'target', 'l7-web')) + sep;
const CONFIGURATIONS = ['all', 'default'];
const TYPES = {
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.html': 'text/html; charset=utf-8',
  '.json': 'application/json',
  '.mf2b': 'application/octet-stream',
};
const MODES = {
  islands: { page: (l) => `islands-${l}.html`, pkg: '/pkg-islands/mf2_l7_web.js' },
  csr: { page: (l) => `csr-${l}.html`, pkg: '/pkg-csr/mf2_l7_web.js' },
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

const norm = (text) => (text === ' ' ? '' : text);

export async function run(ctx) {
  const { browser, assert, data } = ctx;
  data.pages = [];
  let ran = 0;
  for (const configuration of CONFIGURATIONS) {
    const dir = join(OUT, configuration) + sep;
    if (!existsSync(dir)) continue;
    const server = await serve(dir);
    const base = `http://127.0.0.1:${server.address().port}`;
    try {
      const sets = readdirSync(dir).filter((f) => f.endsWith('.json')).sort();
      for (const file of sets) {
        const set = JSON.parse(readFileSync(join(dir, file), 'utf8'));
        for (const mode of Object.keys(MODES)) {
          const page = await runPage(browser, base, mode, set);
          page.configuration = configuration;
          data.pages.push(page);
          ran += 1;
          assert(
            `${configuration}/${mode}/${set.locale}`,
            page.errors.length === 0 && Object.keys(page.failures).length === 0,
            {
              cases: page.cases,
              errors: page.errors.slice(0, 3),
              failures: Object.entries(page.failures).slice(0, 3),
            },
          );
        }
      }
    } finally {
      server.close();
    }
  }
  assert('pages-ran', ran > 0, ran > 0 ? { pages: ran } : `nothing under ${OUT}; run \`cargo xtask l7-web\``);
}

/** Drives one page; returns what went wrong with it and with each case. */
async function runPage(browser, base, mode, set) {
  const { page: pageFile, pkg } = MODES[mode];
  const result = { mode, locale: set.locale, cases: set.cases.length, errors: [], failures: {} };
  const fail = (id, what) => {
    result.failures[id] ??= what;
  };
  const native = Object.fromEntries(set.cases.map((c) => [c.id, c]));

  /** Every case's text must be `want(case)`; `stage` names the moment. */
  const compare = (got, stage, want) => {
    for (const c of set.cases) {
      if (!(c.id in got)) {
        fail(c.id, `${stage}: not on the page`);
      } else if (norm(got[c.id]) !== norm(want(c))) {
        fail(c.id, `${stage}: ${JSON.stringify(got[c.id])}, expected ${JSON.stringify(want(c))}`);
      }
    }
    const extra = Object.keys(got).filter((id) => !(id in native));
    if (extra.length > 0) result.errors.push(`${stage}: cases the set does not have: ${extra.slice(0, 3)}`);
  };

  const context = await browser.newContext();
  // The server's text, before any script runs.
  await context.addInitScript(`document.addEventListener('DOMContentLoaded', () => {
    window.__l7Served = (${CASE_TEXT})();
  });`);
  if (mode === 'csr') {
    // A client-only page starts in the reader's language where the build
    // has it — and the browser's `en-US` finds the twin, `en-GB`, by its
    // language (`lookup_locale`), ahead of the set's own locale. So the page
    // starts from a remembered choice of the set's locale, which the boot
    // ranks first (`mf2::leptos::links::LOCALE_STORAGE_KEY`).
    await context.addInitScript(
      `try { localStorage.setItem('mf2_locale', ${JSON.stringify(set.locale)}); } catch {}`,
    );
  }
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);
  const liveNodes = () => page.evaluate(async (pkg) => (await import(pkg)).mf2_live_nodes(), pkg);
  const lang = () => page.getAttribute('html', 'lang');

  try {
    await page.goto(`${base}/${pageFile(set.locale)}`, { waitUntil: 'load' });
    if (mode === 'islands') {
      compare(await page.evaluate(() => window.__l7Served ?? {}), 'served', (c) => c.text);
    }
    try {
      await until(
        page,
        `import('${pkg}').then((m) => m.mf2_live_nodes() >= ${set.cases.length}).catch(() => false)`,
        120000,
      );
    } catch (e) {
      result.errors.push(`never ${mode === 'islands' ? 'hydrated' : 'mounted'}: ${e.message}`);
      return result;
    }
    await sleep(50);
    compare(await page.evaluate(CASE_TEXT), mode === 'islands' ? 'hydrated' : 'mounted', (c) => c.text);
    if ((await lang()) !== set.locale) result.errors.push(`<html lang> is ${await lang()}, not ${set.locale}`);
    const live = await liveNodes();

    const toTwin = await setLocale(page, pkg, set.twin);
    if (toTwin !== null) result.errors.push(`switch to ${set.twin}: ${toTwin}`);
    if ((await lang()) !== set.twin) result.errors.push(`after the switch <html lang> is ${await lang()}`);
    compare(await page.evaluate(CASE_TEXT), `in ${set.twin}`, (c) => c.twin);

    const back = await setLocale(page, pkg, set.locale);
    if (back !== null) result.errors.push(`switch back to ${set.locale}: ${back}`);
    if ((await lang()) !== set.locale) result.errors.push(`back, <html lang> is ${await lang()}`);
    compare(await page.evaluate(CASE_TEXT), 'back', (c) => c.text);

    const after = await liveNodes();
    if (after !== live) result.errors.push(`the registry went from ${live} to ${after} slots over a switch`);
  } catch (e) {
    result.errors.push(`the check broke: ${String(e?.message ?? e).split('\n')[0]}`);
  } finally {
    for (const m of console_) result.errors.push(`console ${m.type}: ${m.text.slice(0, 200)}`);
    await context.close();
  }
  return result;
}

/** `null` on success, the error message otherwise. */
async function setLocale(page, pkg, tag) {
  const failure = await page.evaluate(
    async ([pkg, tag]) => {
      try {
        await (await import(pkg)).mf2_set_locale(tag);
        return null;
      } catch (e) {
        return String(e);
      }
    },
    [pkg, tag],
  );
  await sleep(50);
  return failure;
}
