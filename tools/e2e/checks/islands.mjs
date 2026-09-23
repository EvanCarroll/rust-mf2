// Phase 7 A1 (plans/15-phase-7-work-order.md) against `examples/demo-islands`,
// which must already be running at --base-url (default port 3704).
//
// What it asserts, and why each one is here:
//
//   * the page is served as islands: the gate is the first island in the
//     document, and the counter and the switcher are islands;
//   * hydration changes no text and logs nothing, the catalog is fetched once
//     and from the preload, and the markup message *inside* an island
//     hydrates as an element;
//   * the gate holds: with the catalog delayed, no island hydrates until it
//     has arrived — and, as the control, the same page with the gate removed
//     fails the way plans/04 §7 says it must;
//   * a signal-valued argument re-formats under `static-locale` (it did not
//     before Phase 7);
//   * a switch is a cookie and a reload, and the reloaded page is the
//     server's page in the new locale, right-to-left included;
//   * no message text is in the client bundle (B6).

import {
  watchConsole,
  resourceTimings,
  captureSsrSnapshot,
  sleep,
  until,
} from '../lib/browser.mjs';

const CANARIES = [
  'people are here',
  'personnes sont ici',
  'Échap',
  'costs the wasm nothing',
  'اللغة',
];

const NORMALISE = (text) => text.replace(/\s+/g, ' ').trim();
const GATE = '<leptos-island data-component="mf2_islands_gate"></leptos-island>';

export async function run(ctx) {
  const { baseUrl, browser, assert, data } = ctx;

  // ------------------------------------------------------------- HTML ---

  const api = await browser.newContext();
  const served = await api.request.get(`${baseUrl}/?lang=fr`);
  const html = await served.text();
  await api.close();
  assert('ssr-200', served.status() === 200, served.status());
  assert('ssr-lang-fr', /<html[^>]*\slang="fr"/.test(html));
  const islands = [...html.matchAll(/<leptos-island data-component="([^"]+)"/g)].map((m) => m[1]);
  data.islands = islands;
  assert('the-gate-is-the-first-island', islands[0] === 'mf2_islands_gate', islands);
  assert(
    'the-gate-comes-first-in-body',
    /<body>\s*<leptos-island data-component="mf2_islands_gate">/.test(html),
  );
  assert('counter-is-an-island', islands.some((i) => i.startsWith('Counter_')), islands);
  assert('switcher-is-an-island', islands.some((i) => i.startsWith('Switcher_')), islands);
  assert('server-markup-is-an-element', /<p id="hotkey">[^]*?<kbd>/.test(html));

  // ---------------------------------------------------------- browser ---

  const context = await browser.newContext();
  await captureSsrSnapshot(context);
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);

  await page.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  await hydrated(page);

  const ssr = await page.evaluate(() => window.__ssrSnapshot);
  const after = await page.evaluate(() => document.body.innerText);
  assert('hydration-changes-no-text', NORMALISE(ssr.text) === NORMALISE(after), {
    before: NORMALISE(ssr.text).slice(0, 120),
    after: NORMALISE(after).slice(0, 120),
  });
  assert('console-is-silent', console_.length === 0, console_.slice(0, 4));

  const timings = await resourceTimings(page);
  const catalogRequests = timings.filter((t) => t.name.includes('/i18n/'));
  assert('one-catalog-request', catalogRequests.length === 1, catalogRequests.map((t) => t.name));
  assert(
    'catalog-came-from-the-preload',
    catalogRequests[0]?.initiatorType === 'link',
    catalogRequests[0]?.initiatorType,
  );
  data.timings = timings.filter((t) => t.name.includes('/i18n/') || t.name.endsWith('.wasm'));

  // Under `static-locale` only a node with a reactive argument registers:
  // the counter's line, and nothing else on the page.
  assert('only-the-reactive-node-registers', (await liveNodes(page)) === 1, await liveNodes(page));
  assert('island-markup-is-an-element', (await page.locator('#island-note em').count()) === 1);

  // A signal-valued argument re-formats, and a reset gets there too.
  const three = await page.textContent('#people');
  await page.click('#add-one');
  const four = await page.textContent('#people');
  assert('a-signal-argument-reformats', three !== four && four.includes('4'), { three, four });
  await page.click('#reset');
  const zero = await page.textContent('#people');
  assert('and-again', zero.includes('0'), zero);

  // ------------------------------------------------------------ switch ---

  await page.selectOption('#mf2-locale', 'fr');
  await page.waitForURL(() => true, { waitUntil: 'load' });
  await page.waitForFunction(() => document.documentElement.lang === 'fr', undefined, {
    timeout: 10000,
  });
  await hydrated(page);
  const cookies = await context.cookies(baseUrl);
  const cookie = cookies.find((c) => c.name === 'mf2_locale');
  assert('switch-writes-the-cookie', cookie?.value === 'fr', cookie);
  const frText = NORMALISE(await page.evaluate(() => document.body.innerText));
  const frServer = await serverText(context, `${baseUrl}/?lang=fr`);
  assert('switched-page-is-the-servers', frText === frServer, {
    client: frText.slice(0, 120),
    server: frServer.slice(0, 120),
  });
  assert('switched-page-hydrates', (await liveNodes(page)) === 1);
  const frNote = NORMALISE(await page.textContent('#island-note'));
  assert('island-markup-follows-the-message', /îlot\.$/.test(frNote), frNote);

  await page.selectOption('#mf2-locale', 'ar');
  await page.waitForFunction(() => document.documentElement.lang === 'ar', undefined, {
    timeout: 10000,
  });
  await hydrated(page);
  assert('rtl-switch-sets-dir', (await page.getAttribute('html', 'dir')) === 'rtl');

  await page.selectOption('#mf2-locale', 'en');
  await page.waitForFunction(() => document.documentElement.lang === 'en', undefined, {
    timeout: 10000,
  });
  await hydrated(page);
  assert('console-is-still-silent', console_.length === 0, console_.slice(0, 4));
  await context.close();

  // ---------------------------------------------------- the gate holds ---

  // The catalog is held back; the wasm is not. Until the catalog lands, no
  // island may hydrate.
  const DELAY = 3000;
  const held = await browser.newContext();
  await held.route('**/i18n/**', async (route) => {
    await sleep(DELAY);
    await route.continue();
  });
  const heldPage = await held.newPage();
  const heldConsole = [];
  watchConsole(heldPage, heldConsole);
  await heldPage.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  await wasmLoaded(heldPage);
  // Long enough for the island walk to have reached every island if nothing
  // held it; a resource entry exists only once a response has completed.
  await sleep(300);
  const whileHeld = await heldPage.evaluate(async () => ({
    live: (await import('/pkg/demo_islands.js')).mf2_live_nodes(),
    catalogArrived: performance
      .getEntriesByType('resource')
      .some((e) => e.name.includes('/i18n/')),
  }));
  data.whileHeld = whileHeld;
  assert(
    'no-island-hydrates-before-the-catalog',
    whileHeld.live === 0 && !whileHeld.catalogArrived,
    whileHeld,
  );
  await hydrated(heldPage);
  assert('then-every-island-does', (await liveNodes(heldPage)) === 1, await liveNodes(heldPage));
  assert(
    'held-island-markup-is-an-element',
    (await heldPage.locator('#island-note em').count()) === 1,
  );
  assert('held-console-is-silent', heldConsole.length === 0, heldConsole.slice(0, 4));
  await held.close();

  // The control: the same page and the same delay, without the gate. The
  // markup message's island hydrates against no catalog, so no structure —
  // which is what the gate is for. This fails *somehow*; how is recorded.
  const ungated = await browser.newContext();
  await ungated.route('**/i18n/**', async (route) => {
    await sleep(DELAY);
    await route.continue();
  });
  await ungated.route(
    (url) => url.pathname === '/',
    async (route) => {
      const response = await route.fetch();
      const body = (await response.text()).replace(GATE, '');
      await route.fulfill({ response, body });
    },
  );
  const ungatedPage = await ungated.newPage();
  const ungatedConsole = [];
  watchConsole(ungatedPage, ungatedConsole);
  await ungatedPage.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  await wasmLoaded(ungatedPage);
  await sleep(DELAY + 1000);
  const ungatedEm = await ungatedPage.locator('#island-note em').count();
  data.withoutTheGate = { console: ungatedConsole.slice(0, 4), em: ungatedEm };
  assert(
    'without-the-gate-it-fails',
    ungatedConsole.length > 0 || ungatedEm !== 1,
    data.withoutTheGate,
  );
  await ungated.close();

  // ----------------------------------------------------------- canary ---

  const probe = await browser.newContext();
  const bundle = await probe.request.get(`${baseUrl}/pkg/demo_islands.js`);
  const wasm = await probe.request.get(`${baseUrl}/pkg/demo_islands.wasm`);
  assert('client-bundle-served', bundle.status() === 200 && wasm.status() === 200);
  const js = await bundle.text();
  const wasmBytes = Buffer.from(await wasm.body());
  const found = CANARIES.filter((c) => js.includes(c) || wasmBytes.includes(Buffer.from(c)));
  assert('no-message-text-in-the-client', found.length === 0, found);
  await probe.close();
}

/** The wasm module has been instantiated (its exports can be called). */
async function wasmLoaded(page) {
  await until(page, async () => {
    try {
      const mod = await import('/pkg/demo_islands.js');
      return mod.mf2_live_nodes() >= 0;
    } catch {
      return false;
    }
  });
}

/** The counter island has hydrated: its reactive node is registered. */
async function hydrated(page) {
  await until(page, async () => {
    try {
      return (await import('/pkg/demo_islands.js')).mf2_live_nodes() > 0;
    } catch {
      return false;
    }
  });
  await sleep(50);
}

async function liveNodes(page) {
  return page.evaluate(async () => (await import('/pkg/demo_islands.js')).mf2_live_nodes());
}

/** The server's own text for `url`, read before that page hydrates. */
async function serverText(context, url) {
  const page = await context.newPage();
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const text = await page.evaluate(() => document.body.innerText);
  await page.close();
  return NORMALISE(text);
}
