// P0.10 — hydration tolerance (probes/p0-02-vertical-slice, /p010/* pages).
//   text:        server and client render different text for three nodes
//                (static &str, reactive text, attribute). Expected: no
//                warning; server text stays until the next update.
//   struct:      server renders <b>, client expects a text node (tachys path).
//   struct-tr:   same, but the client expects a `Tr` (the probe's panic-free
//                glue). The struct pages *document* the failure mode.

import { recordAllConsole, sleep } from '../lib/browser.mjs';

const CAPTURE_IDS = ['static-diff', 'dynamic-diff', 'attr-diff', 'struct-diff', 'after-count', 'only-server'];

async function snapshotAtDcl(context) {
  await context.addInitScript((ids) => {
    document.addEventListener('DOMContentLoaded', () => {
      const out = {};
      for (const id of ids) {
        const el = document.getElementById(id);
        out[id] = el ? { text: el.textContent, title: el.getAttribute('title'), html: el.innerHTML } : null;
      }
      window.__dcl = out;
    });
  }, CAPTURE_IDS);
}

const state = (ids) => {
  const out = {};
  for (const id of ids) {
    const el = document.getElementById(id);
    out[id] = el ? { text: el.textContent, title: el.getAttribute('title'), html: el.innerHTML } : null;
  }
  return out;
};

async function liveNodes(page) {
  return page.evaluate(async () => {
    try {
      return (await import('/pkg/p002.js')).mf2_live_nodes();
    } catch (e) {
      return `error: ${e}`;
    }
  });
}

async function open(ctx, path) {
  const context = await ctx.browser.newContext();
  await snapshotAtDcl(context);
  const page = await context.newPage();
  const messages = [];
  recordAllConsole(page, messages);
  await page.goto(`${ctx.baseUrl}${path}`);
  await sleep(2500); // boot + catalog + hydration, or the failure
  return { context, page, messages };
}

/** Is the app still interactive after the mismatch? Client-side nav probe. */
async function clientNavWorks(page) {
  await page.evaluate(() => {
    window.__noReload = true;
  });
  await page.click('a[href="/"]');
  await sleep(800);
  return page.evaluate(() => [window.__noReload === true, location.pathname, Boolean(document.getElementById('home-heading'))]);
}

export async function run(ctx) {
  // ------------------------------------------------------------------ text
  {
    const { context, page, messages } = await open(ctx, '/p010/text');
    const dcl = await page.evaluate(() => window.__dcl);
    const hydrated = await page.evaluate(state, CAPTURE_IDS);
    const live = await liveNodes(page);
    await page.click('#bump');
    await sleep(300);
    const afterBump = await page.evaluate(state, CAPTURE_IDS);
    const warnings = messages.filter((m) => ['warning', 'error', 'pageerror'].includes(m.type));
    ctx.data.text = { dcl, hydrated, afterBump, live, messages };
    ctx.assert('T1 SSR rendered the server variant', dcl['static-diff']?.text === 'SERVER' && dcl['dynamic-diff']?.text === 'SERVER-0' && dcl['attr-diff']?.title === 'SERVER', dcl);
    ctx.assert('T2 hydrated (registry populated)', typeof live === 'number' && live > 0, live);
    ctx.assert('T3 no console warning/error for text or attribute differences', warnings.length === 0, warnings);
    ctx.assert(
      'T4 server text stays after hydration (static, reactive, attribute)',
      hydrated['static-diff']?.text === 'SERVER' && hydrated['dynamic-diff']?.text === 'SERVER-0' && hydrated['attr-diff']?.title === 'SERVER',
      hydrated,
    );
    ctx.assert('T5 next update replaces the reactive node with client text', afterBump['dynamic-diff']?.text === 'CLIENT-1', afterBump['dynamic-diff']);
    ctx.assert(
      'T6 non-reactive nodes keep server text forever (no update ever comes)',
      afterBump['static-diff']?.text === 'SERVER' && afterBump['attr-diff']?.title === 'SERVER',
      { static: afterBump['static-diff']?.text, attr: afterBump['attr-diff']?.title },
    );
    await context.close();
  }

  // ------------------------------------------------------- struct (tachys)
  for (const path of ['/p010/struct', '/p010/struct-tr']) {
    const { context, page, messages } = await open(ctx, path);
    const dcl = await page.evaluate(() => window.__dcl);
    const hydrated = await page.evaluate(state, CAPTURE_IDS);
    const live = await liveNodes(page);
    let bumpWorks;
    try {
      await page.click('#bump', { timeout: 2000 });
      await sleep(300);
      bumpWorks = (await page.evaluate(state, CAPTURE_IDS))['after-count']?.text === '1';
    } catch (e) {
      bumpWorks = `click failed: ${e}`;
    }
    const nav = await clientNavWorks(page).catch((e) => `nav failed: ${e}`);
    const errors = messages.filter((m) => ['warning', 'error', 'pageerror'].includes(m.type));
    ctx.data[path] = { dcl, hydrated, live, bumpWorks, clientNav: nav, messages };
    ctx.log(`${path}: console`, JSON.stringify(errors.map((m) => `${m.type}: ${m.text.slice(0, 400)}`)));
    ctx.log(`${path}: DOM after`, JSON.stringify({ structDiff: hydrated['struct-diff']?.html, onlyServer: Boolean(hydrated['only-server']) }));
    ctx.log(`${path}: live nodes`, JSON.stringify(live), 'bump works:', JSON.stringify(bumpWorks), 'client nav [noReload, path, home]:', JSON.stringify(nav));
    ctx.assert(`${path}: failure mode recorded`, true, { consoleErrors: errors.length, bumpWorks, clientNav: nav });
    await context.close();
  }
}
