// Phase 7 A3 against `examples/demo-ssr`
// built with `cargo leptos build --split` and running at --base-url.
//
// P0.2's lazy-route assertions, against `mf2::leptos` rather than the
// probe's glue:
//
//   * the route's code is a chunk of its own: not fetched on the home page,
//     fetched on the way to `/lazy`;
//   * inside the chunk, descriptions render from the catalog the main
//     module installed — text, an attribute, a markup element — and the
//     chunk reads the same locale state;
//   * a switch on the lazy route reaches the chunk's nodes, and gives
//     exactly the server's page for that locale;
//   * leaving the route frees its registry slots, and going back and forth
//     does not grow the registry;
//   * a page that *is* the lazy route hydrates through `hydrate_lazy` —
//     chunk loaded, no text changed — and switches live;
//   * the chunk's own event handler and reactive text work, whether the
//     route was reached by a link or loaded directly (a Phase 9
//     fix: after client navigation they did not);
//   * none of it logs anything.

import { captureSsrSnapshot, chooseLocale, sleep, until, watchConsole, watchNetwork } from '../lib/browser.mjs';

/** A few strings from the corpus, to name what is expected. */
const T = {
  en: { heading: 'A route in its own chunk', title: 'Its code arrived when you opened it' },
  ar: { heading: 'مسار في جزء مستقل', title: 'وصل رمزه عندما فتحته' },
};

const CHUNK = /\/pkg\/split_[^/]*lazy_page[^/]*\.wasm$/;

const NORMALISE = (text) => text.replace(/\s+/g, ' ').trim();

export async function run(ctx) {
  const { baseUrl, browser, assert, data } = ctx;
  const consoleSink = [];

  // ------------------------------------------------ home → lazy → home ---

  const context = await browser.newContext();
  const page = await context.newPage();
  watchConsole(page, consoleSink, 'navigation');
  const net = [];
  watchNetwork(page, net);

  await page.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  await hydrated(page);
  const liveHome = await liveNodes(page);
  data.liveNodes = { home: liveHome };
  assert(
    'chunk-not-fetched-on-home',
    !net.some((n) => CHUNK.test(n.url)),
    net.filter((n) => n.url.includes('/pkg/')).map((n) => n.url.replace(baseUrl, '')),
  );

  await page.click('a[href="/lazy"]');
  await page.waitForSelector('#lazy-heading', { timeout: 15000 });
  await sleep(100);
  const chunks = net.filter((n) => CHUNK.test(n.url)).map((n) => n.url.replace(baseUrl, ''));
  data.chunks = chunks;
  assert('chunk-fetched-on-navigation', chunks.length === 1, chunks);
  assert('lazy-text', (await text(page, '#lazy-heading')) === T.en.heading);
  assert('lazy-attribute', (await page.getAttribute('#lazy-heading', 'title')) === T.en.title);
  assert('lazy-markup-is-an-element', (await page.locator('#lazy-body strong').count()) === 1);
  assert('lazy-sees-the-locale', (await text(page, '#lazy-locale')) === 'en');
  const liveLazy = await liveNodes(page);
  data.liveNodes.lazy = liveLazy;
  assert('lazy-nodes-register', liveLazy > 0 && liveLazy !== liveHome, { liveHome, liveLazy });
  const lazyEn = await bodyText(page);
  const lazyEnServer = await serverText(context, `${baseUrl}/lazy?lang=en`);
  assert('client-built-lazy-equals-server', lazyEn === lazyEnServer, {
    client: lazyEn.slice(0, 160),
    server: lazyEnServer.slice(0, 160),
  });

  // A switch on the lazy route, to right-to-left.
  await switchTo(page, 'ar');
  assert('switch-sets-lang-dir', (await page.getAttribute('html', 'dir')) === 'rtl');
  assert('switch-reaches-lazy-text', (await text(page, '#lazy-heading')) === T.ar.heading);
  assert('switch-reaches-lazy-attribute', (await page.getAttribute('#lazy-heading', 'title')) === T.ar.title);
  assert('switch-keeps-lazy-markup', (await page.locator('#lazy-body strong').count()) === 1);
  assert('lazy-follows-the-locale', (await text(page, '#lazy-locale')) === 'ar');
  const lazyAr = await bodyText(page);
  const lazyArServer = await serverText(context, `${baseUrl}/lazy?lang=ar`);
  assert('switched-lazy-equals-server', lazyAr === lazyArServer, {
    client: lazyAr.slice(0, 160),
    server: lazyArServer.slice(0, 160),
  });
  assert('switch-leaks-no-slots', (await liveNodes(page)) === liveLazy, {
    liveLazy,
    after: await liveNodes(page),
  });
  await pressWorks(page, assert, 'navigated');

  // Back home: the chunk's nodes are dropped, and their slots with them.
  await page.click('a[href="/"]');
  await page.waitForSelector('#people', { timeout: 15000 });
  await sleep(100);
  const liveBack = await liveNodes(page);
  data.liveNodes.back = liveBack;
  assert('leaving-frees-the-slots', liveBack === liveHome, { liveHome, liveLazy, liveBack });
  const homeAr = await bodyText(page);
  const homeArServer = await serverText(context, `${baseUrl}/?lang=ar`);
  assert('client-built-home-equals-server', homeAr === homeArServer, {
    client: homeAr.slice(0, 160),
    server: homeArServer.slice(0, 160),
  });

  // Back and forth: the registry stays flat, and the chunk is fetched once.
  for (let i = 0; i < 5; i += 1) {
    await page.click('a[href="/lazy"]');
    await page.waitForSelector('#lazy-heading', { timeout: 15000 });
    await page.click('a[href="/"]');
    await page.waitForSelector('#people', { timeout: 15000 });
  }
  await sleep(100);
  const liveAfterChurn = await liveNodes(page);
  data.liveNodes.afterChurn = liveAfterChurn;
  assert('churn-is-flat', liveAfterChurn === liveHome, { liveHome, liveAfterChurn });
  assert(
    'chunk-fetched-once',
    net.filter((n) => CHUNK.test(n.url)).length === 1,
    net.filter((n) => CHUNK.test(n.url)).length,
  );

  // The chunk is code, not text: none of the route's messages are in it.
  const chunkUrl = net.find((n) => CHUNK.test(n.url))?.url;
  const chunkBytes = chunkUrl ? Buffer.from(await (await context.request.get(chunkUrl)).body()) : Buffer.alloc(0);
  const canaries = [T.en.heading, T.ar.heading, 'separate wasm file', 'Rendered in'];
  const inChunk = canaries.filter((c) => chunkBytes.includes(Buffer.from(c)));
  assert('chunk-is-wasm', chunkBytes.subarray(0, 4).toString('latin1') === '\0asm', chunkBytes.length);
  assert('no-message-text-in-the-chunk', inChunk.length === 0, inChunk);
  data.chunkBytes = chunkBytes.length;

  // Switching back gives the server's English page.
  await switchTo(page, 'en');
  const homeEn = await bodyText(page);
  const homeEnServer = await serverText(context, `${baseUrl}/?lang=en`);
  assert('switched-back-home-equals-server', homeEn === homeEnServer, {
    client: homeEn.slice(0, 160),
    server: homeEnServer.slice(0, 160),
  });
  await context.close();

  // --------------------------------------------- the lazy route, direct ---

  const direct = await browser.newContext();
  await captureSsrSnapshot(direct);
  const lp = await direct.newPage();
  watchConsole(lp, consoleSink, '/lazy (direct)');
  const lnet = [];
  watchNetwork(lp, lnet);
  await lp.goto(`${baseUrl}/lazy?lang=ar`, { waitUntil: 'load' });
  await hydrated(lp);
  const snap = await lp.evaluate(() => window.__ssrSnapshot);
  const afterText = await lp.evaluate(() => document.body.innerText);
  assert('direct-chunk-loaded-for-hydration', lnet.some((n) => CHUNK.test(n.url)));
  assert('direct-hydration-changes-no-text', NORMALISE(snap.text) === NORMALISE(afterText), {
    before: NORMALISE(snap.text).slice(0, 160),
    after: NORMALISE(afterText).slice(0, 160),
  });
  assert('direct-title-survives-hydration', snap.title === (await lp.title()), {
    ssr: snap.title,
    after: await lp.title(),
  });
  assert('direct-heading', (await text(lp, '#lazy-heading')) === T.ar.heading);
  assert('direct-markup-is-an-element', (await lp.locator('#lazy-body strong').count()) === 1);
  const liveDirect = await liveNodes(lp);
  data.liveNodes.direct = liveDirect;
  assert('direct-registers-the-same-nodes', liveDirect === liveLazy, { liveDirect, liveLazy });
  const inLanguageAr = await lp.getAttribute('meta[itemprop="inLanguage"]', 'content');
  await switchTo(lp, 'en');
  const inLanguageEn = await lp.getAttribute('meta[itemprop="inLanguage"]', 'content');
  assert('schema-org-inlanguage-follows-the-switch', inLanguageAr === 'ar' && inLanguageEn === 'en', {
    inLanguageAr,
    inLanguageEn,
  });
  assert(
    'direct-switch-reaches-the-chunk',
    (await text(lp, '#lazy-heading')) === T.en.heading &&
      (await lp.getAttribute('#lazy-heading', 'title')) === T.en.title &&
      (await text(lp, '#lazy-locale')) === 'en',
  );
  const directEn = await bodyText(lp);
  assert('direct-switched-equals-server', directEn === lazyEnServer, {
    client: directEn.slice(0, 160),
    server: lazyEnServer.slice(0, 160),
  });
  await pressWorks(lp, assert, 'direct');
  // Leaving a route that was hydrated (rather than built) frees its slots.
  await lp.click('a[href="/"]');
  await lp.waitForSelector('#people', { timeout: 15000 });
  await sleep(100);
  const liveDirectBack = await liveNodes(lp);
  data.liveNodes.directBack = liveDirectBack;
  assert('direct-leaving-frees-the-slots', liveDirectBack === liveHome, { liveHome, liveDirectBack });
  await direct.close();

  data.console = consoleSink;
  assert('console-is-silent', consoleSink.length === 0, consoleSink.slice(0, 6));
}

/**
 * Presses the chunk's button twice: its handler counts, the plain reactive
 * text follows, and so does the message whose argument is the signal.
 */
async function pressWorks(page, assert, label) {
  const before = { count: await text(page, '#lazy-count'), presses: await text(page, '#lazy-presses') };
  const seen = [];
  for (const expected of ['1', '2']) {
    await page.click('#lazy-add');
    await page
      .waitForFunction((n) => document.querySelector('#lazy-count')?.textContent.trim() === n, expected, {
        timeout: 2000,
      })
      .catch(() => {});
    seen.push({ count: await text(page, '#lazy-count'), presses: await text(page, '#lazy-presses') });
  }
  assert(`${label}-handler-counts`, before.count === '0' && seen[1].count === '2', { before, seen });
  assert(
    `${label}-message-follows-the-signal`,
    new Set([before.presses, seen[0].presses, seen[1].presses]).size === 3,
    { before, seen },
  );
}

async function switchTo(page, tag) {
  await chooseLocale(page, tag);
  await page.waitForFunction((t) => document.documentElement.lang === t, tag, { timeout: 5000 });
  // Let the derived conversions (the title, `inLanguage`) re-run.
  await sleep(100);
}

/** Waits until the wasm has booted and hydration has registered its nodes. */
async function hydrated(page) {
  await until(page, async () => {
    try {
      return (await import('/pkg/demo_ssr.js')).mf2_live_nodes() > 0;
    } catch {
      return false;
    }
  });
  await sleep(50);
}

async function liveNodes(page) {
  return page.evaluate(async () => (await import('/pkg/demo_ssr.js')).mf2_live_nodes());
}

const text = async (page, selector) => NORMALISE((await page.textContent(selector)) ?? '');

const bodyText = async (page) => NORMALISE(await page.evaluate(() => document.body.innerText));

/** The server's own text for `url`, read before that page hydrates. */
async function serverText(context, url) {
  const page = await context.newPage();
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const out = await page.evaluate(() => document.body.innerText);
  await page.close();
  return NORMALISE(out);
}
