// P0.2 — vertical slice (probes/p0-02-vertical-slice). Checks, in one browser:
//   * zero console warnings/errors across load, hydration, lazy-route
//     navigation, locale switch and back, streamed pages;
//   * the catalog request overlaps the wasm request (Resource Timing +
//     Playwright network events), and the preload is reused (one request);
//   * text before hydration == text after hydration;
//   * the lazy route renders translated text in both locales;
//   * streamed Suspense content (all four SsrMode variants) is rendered from
//     the request's locale, with no server-side context misses (D9).

import {
  captureSsrSnapshot,
  resourceTimings,
  sleep,
  throttle,
  watchConsole,
  watchNetwork,
} from '../lib/browser.mjs';

// Fixture text (the probe's hand-built catalogs).
const T = {
  en: {
    title: 'mf2-two vertical slice',
    home: 'Home',
    welcome: 'Welcome! Every string on this page comes from a lazily loaded catalog.',
    placeholder: 'Type to search…',
    optionB: 'Option B is showing.',
    toggle: 'Toggle message',
    greeting: 'Hello from the server function.',
    lazyHeading: 'Lazy route',
    lazyTitle: 'Loaded on demand',
    streamBody: 'This text was rendered on the server after the resource resolved.',
    streamBody2: 'A second node in the same streamed chunk.',
    prop: 'This label arrived as a component prop.',
    signal: 'This text came through Signal<String>.',
    navLabel: 'Main',
  },
  ar: {
    title: 'الشريحة العمودية لـ mf2-two',
    home: 'الرئيسية',
    welcome: 'مرحبًا! كل نص في هذه الصفحة يأتي من فهرس يُحمَّل عند الحاجة.',
    placeholder: 'اكتب للبحث…',
    greeting: 'مرحبًا من دالة الخادم.',
    lazyHeading: 'المسار الكسول',
    lazyTitle: 'حُمِّل عند الطلب',
    streamBody: 'عُرض هذا النص على الخادم بعد اكتمال تحميل المورد.',
    streamBody2: 'عقدة ثانية في الجزء المُبثّ نفسه.',
    prop: 'وصلت هذه التسمية كخاصية مكوّن.',
    signal: 'جاء هذا النص عبر Signal<String>.',
    navLabel: 'رئيسي',
  },
};

const LIVE_NODES = async () => {
  try {
    const m = await import('/pkg/p002.js');
    return m.mf2_live_nodes();
  } catch {
    return -1;
  }
};

/** Waits until the wasm is initialised and the registry count is stable. */
async function waitHydrated(page, timeout = 30000) {
  const deadline = Date.now() + timeout;
  let last = -1;
  let stable = 0;
  while (Date.now() < deadline) {
    const n = await page.evaluate(LIVE_NODES);
    if (n > 0 && n === last) {
      stable += 1;
      if (stable >= 3) return n;
    } else {
      stable = 0;
    }
    last = n;
    await sleep(100);
  }
  throw new Error(`not hydrated within ${timeout} ms (live nodes ${last})`);
}

/** Normalised textContent of header + main, minus per-session output nodes. */
const PAGE_TEXT = () => {
  const parts = [];
  for (const sel of ['header', 'main']) {
    const el = document.querySelector(sel);
    if (!el) continue;
    const clone = el.cloneNode(true);
    for (const id of ['server-msg', 'from-handler']) clone.querySelector(`#${id}`)?.replaceChildren();
    parts.push(clone.textContent);
  }
  return parts.join('\n').replace(/\s+/g, ' ').trim();
};

const text = (page, sel) => page.locator(sel).first().textContent();
const attr = (page, sel, name) => page.locator(sel).first().getAttribute(name);
const htmlLangDir = (page) =>
  page.evaluate(() => [document.documentElement.lang, document.documentElement.dir, document.title]);

async function serverCounters(ctx) {
  const res = await fetch(`${ctx.baseUrl}/__probe/ctx`);
  const m = /hits=(\d+) misses=(\d+)/.exec(await res.text());
  return m ? { hits: Number(m[1]), misses: Number(m[2]) } : undefined;
}

/** Server-rendered reference text for `path` in `locale` (JS disabled). */
async function ssrReference(ctx, path, locale) {
  const context = await ctx.browser.newContext({ javaScriptEnabled: false });
  await context.addCookies([{ name: 'mf2-locale', value: locale, url: ctx.baseUrl }]);
  const page = await context.newPage();
  await page.goto(`${ctx.baseUrl}${path}`);
  const out = await page.evaluate(PAGE_TEXT);
  await context.close();
  return out;
}

async function switchLocale(page, tag) {
  await page.selectOption('#locale-select', tag);
  await page.waitForFunction((t) => document.documentElement.lang === t, tag, { timeout: 10000 });
  await sleep(150); // let derived signals (title, props) re-run
}

export async function run(ctx) {
  await http(ctx);
  const consoleSink = [];

  // ---------------------------------------------------------------- A. load
  const context = await ctx.browser.newContext({ locale: 'en-US' });
  await captureSsrSnapshot(context);
  const page = await context.newPage();
  watchConsole(page, consoleSink, 'main-flow');
  const net = [];
  watchNetwork(page, net);
  if (ctx.throttle && ctx.browserName === 'chromium') {
    // "Fast 3G"-like profile, to make overlap visible beyond localhost speed.
    await throttle(page, { latencyMs: 150, downloadKbps: 1600, uploadKbps: 750 });
  }
  const t0 = Date.now();
  await page.goto(`${ctx.baseUrl}/`);
  const liveHome = await waitHydrated(page, ctx.throttle ? 120000 : 30000);
  ctx.data.hydratedAfterMs = Date.now() - t0;
  ctx.data.liveNodesHome = liveHome;

  const [lang, dir, title] = await htmlLangDir(page);
  ctx.assert('A1 <html lang dir> = en/ltr', lang === 'en' && dir === 'ltr', { lang, dir });
  ctx.assert('A2 document.title translated after hydration', title === T.en.title, title);

  const snap = await page.evaluate(() => window.__ssrSnapshot);
  const after = await page.evaluate(() => ({ text: document.body.innerText, title: document.title }));
  ctx.assert('A3 text before hydration == text after hydration', snap?.text === after.text, {
    beforeLen: snap?.text?.length,
    afterLen: after.text.length,
  });
  ctx.assert('A4 title before hydration == after', snap?.title === after.title, [snap?.title, after.title]);

  // Timings: Resource Timing (relative to navigation start) + network events.
  const rt = await resourceTimings(page);
  const cat = rt.filter((e) => e.name.endsWith('.mf2b'));
  const wasm = rt.find((e) => /\/pkg\/p002\.wasm$/.test(e.name));
  const js = rt.find((e) => /\/pkg\/p002\.js$/.test(e.name));
  ctx.data.resourceTiming = { catalog: cat, wasm, js };
  const c = cat[0];
  ctx.assert('A5 exactly one catalog request (preload reused by fetch)', cat.length === 1, cat.map((e) => e.initiatorType));
  ctx.assert(
    'A6 catalog request starts before the wasm request finishes (overlap)',
    c && wasm && c.startTime < wasm.responseEnd,
    c && wasm && { catalog: [c.startTime, c.responseEnd], wasm: [wasm.startTime, wasm.responseEnd] },
  );
  ctx.assert('A7 catalog request starts no later than the wasm request', c && wasm && c.startTime <= wasm.startTime, c && wasm && [c.startTime, wasm.startTime]);
  const netCat = net.filter((n) => n.url.endsWith('.mf2b'));
  const netWasm = net.filter((n) => /\/pkg\/p002\.wasm$/.test(n.url));
  const base = Math.min(...net.filter((n) => n.start).map((n) => n.start));
  const rel = (n) => ({ url: n.url.replace(ctx.baseUrl, ''), start: Math.round(n.start - base), end: n.end && Math.round(n.end - base) });
  ctx.data.networkEvents = { catalog: netCat.map(rel), wasm: netWasm.map(rel) };
  ctx.assert(
    'A8 network events: catalog starts before wasm ends',
    netCat.length >= 1 && netWasm.length >= 1 && netCat[0].start < (netWasm[0].end ?? Infinity),
    ctx.data.networkEvents,
  );

  // ----------------------------------------------------- B. interactions (en)
  ctx.assert('B1 attribute (placeholder)', (await attr(page, '#search', 'placeholder')) === T.en.placeholder);
  ctx.assert('B2 TextProp component prop', (await text(page, '#prop-demo')) === T.en.prop);
  ctx.assert('B3 Signal<String>', (await text(page, '#signal-demo')) === T.en.signal);
  await page.click('#toggle');
  await page.waitForFunction((t) => document.querySelector('#if-else')?.textContent === t, T.en.optionB);
  ctx.assert('B4 if/else rebuild shows option B', true);
  ctx.assert('B5 to_string() from an event handler', (await text(page, '#from-handler')) === T.en.toggle);
  await page.click('#ask-server');
  await page.waitForFunction(() => document.querySelector('#server-msg')?.textContent.length > 0, null, { timeout: 10000 });
  ctx.assert('B6 server function formats from request context (en)', (await text(page, '#server-msg')) === T.en.greeting);

  // ------------------------------------------------- C. lazy route navigation
  await page.click('a[href="/lazy"]');
  await page.waitForSelector('#lazy-heading', { timeout: 15000 });
  await sleep(100);
  const chunk = net.filter((n) => /\/pkg\/split_[^/]+\.wasm$/.test(n.url));
  ctx.assert('C1 lazy chunk fetched on navigation', chunk.length >= 1, chunk.map((n) => n.url.replace(ctx.baseUrl, '')));
  ctx.assert('C2 lazy route text (en)', (await text(page, '#lazy-heading')) === T.en.lazyHeading);
  ctx.assert('C3 lazy route attribute (en)', (await attr(page, '#lazy-heading', 'title')) === T.en.lazyTitle);
  ctx.assert('C4 lazy route sees i18n state (current_locale)', (await text(page, '#lazy-locale')) === 'en');
  ctx.data.liveNodesLazy = await page.evaluate(LIVE_NODES);

  // --------------------------------------------- D. switch to ar on the lazy route
  await switchLocale(page, 'ar');
  const [langAr, dirAr, titleAr] = await htmlLangDir(page);
  ctx.assert('D1 <html lang dir> updated to ar/rtl', langAr === 'ar' && dirAr === 'rtl', { langAr, dirAr });
  ctx.assert('D2 document.title switched (TextProp)', titleAr === T.ar.title, titleAr);
  ctx.assert('D3 lazy route text (ar)', (await text(page, '#lazy-heading')) === T.ar.lazyHeading);
  ctx.assert('D4 lazy route attribute (ar)', (await attr(page, '#lazy-heading', 'title')) === T.ar.lazyTitle);
  ctx.assert('D5 lazy route current_locale (ar)', (await text(page, '#lazy-locale')) === 'ar');
  const lazyAr = await page.evaluate(PAGE_TEXT);
  const lazyArRef = await ssrReference(ctx, '/lazy', 'ar');
  ctx.assert('D6 switched lazy page == server-rendered ar page', lazyAr === lazyArRef, lazyAr === lazyArRef ? undefined : { lazyAr, lazyArRef });
  ctx.assert('D7 nav aria-label switched', (await attr(page, 'nav', 'aria-label')) === T.ar.navLabel);
  const cookie = (await context.cookies()).find((k) => k.name === 'mf2-locale');
  ctx.assert('D8 locale cookie set', cookie?.value === 'ar', cookie?.value);

  // ---------------------------------------------------------- E. back home (ar)
  await page.click('a[href="/"]');
  await page.waitForSelector('#home-heading', { timeout: 15000 });
  await sleep(150);
  const homeAr = await page.evaluate(PAGE_TEXT);
  const homeArRef = await ssrReference(ctx, '/', 'ar');
  ctx.assert('E1 client-built home (ar) == server-rendered ar home', homeAr === homeArRef, homeAr === homeArRef ? undefined : { homeAr, homeArRef });
  ctx.assert('E2 placeholder (ar)', (await attr(page, '#search', 'placeholder')) === T.ar.placeholder);
  ctx.assert('E3 TextProp prop (ar)', (await text(page, '#prop-demo')) === T.ar.prop);
  ctx.assert('E4 Signal<String> (ar)', (await text(page, '#signal-demo')) === T.ar.signal);
  const liveBack = await page.evaluate(LIVE_NODES);
  ctx.data.liveNodesBackHome = liveBack;
  ctx.assert('E5 registry freed the lazy route nodes (live count back to home level)', liveBack === liveHome, { liveHome, lazy: ctx.data.liveNodesLazy, liveBack });
  await page.click('#ask-server');
  await page.waitForFunction(() => document.querySelector('#server-msg')?.textContent.length > 0, null, { timeout: 10000 });
  ctx.assert('E6 server function (ar, via cookie)', (await text(page, '#server-msg')) === T.ar.greeting);

  // --------------------------------------------------------- F. switch back to en
  await switchLocale(page, 'en');
  const [langEn, dirEn, titleEn] = await htmlLangDir(page);
  ctx.assert('F1 <html lang dir> back to en/ltr', langEn === 'en' && dirEn === 'ltr', { langEn, dirEn });
  ctx.assert('F2 document.title back to en', titleEn === T.en.title, titleEn);
  const homeEn = await page.evaluate(PAGE_TEXT);
  const homeEnRef = await ssrReference(ctx, '/', 'en');
  ctx.assert('F3 switched-back home == server-rendered en home', homeEn === homeEnRef, homeEn === homeEnRef ? undefined : { homeEn, homeEnRef });
  await context.close();

  // ------------------------------------------ G. streamed pages, SSR in Arabic
  const modes = [
    ['/stream/ooo', 'cookie', true],
    ['/stream/inorder', 'cookie', true],
    ['/stream/blocked', 'accept-language', false],
    ['/stream/async', 'cookie', true],
  ];
  ctx.data.streaming = {};
  for (const [path, via, hasSecond] of modes) {
    const before = await serverCounters(ctx);
    const sctx = await ctx.browser.newContext(
      via === 'cookie' ? {} : { extraHTTPHeaders: { 'Accept-Language': 'ar-EG,ar;q=0.9,en;q=0.5' } },
    );
    if (via === 'cookie') await sctx.addCookies([{ name: 'mf2-locale', value: 'ar', url: ctx.baseUrl }]);
    await captureSsrSnapshot(sctx);
    const sp = await sctx.newPage();
    watchConsole(sp, consoleSink, path);
    await sp.goto(`${ctx.baseUrl}${path}`);
    await waitHydrated(sp);
    const snapS = await sp.evaluate(() => window.__ssrSnapshot);
    const afterS = await sp.evaluate(() => ({ text: document.body.innerText, title: document.title }));
    const body = await text(sp, '#streamed-body');
    const body2 = await text(sp, '#streamed-body-2');
    const bodyB = hasSecond ? await text(sp, '#streamed-body-b') : T.ar.streamBody;
    const [l, d, ttl] = await htmlLangDir(sp);
    const after2 = await serverCounters(ctx);
    const misses = after2 && before ? after2.misses - before.misses : undefined;
    ctx.data.streaming[path] = { via, ssrTitle: snapS?.title, misses };
    ctx.assert(`G ${path}: streamed Tr text from the request locale (ar)`, body === T.ar.streamBody && body2 === T.ar.streamBody2 && bodyB === T.ar.streamBody, { body, body2, bodyB });
    ctx.assert(`G ${path}: SSR <title> in request locale`, snapS?.title === T.ar.title, snapS?.title);
    ctx.assert(`G ${path}: <html lang dir> ar/rtl`, l === 'ar' && d === 'rtl' && ttl === T.ar.title, { l, d, ttl });
    ctx.assert(`G ${path}: text before == after hydration`, snapS?.text === afterS.text);
    ctx.assert(`G ${path}: zero server context misses`, misses === 0, { before, after: after2 });
    await sctx.close();
  }

  // ------------------------------------------- H. lazy route loaded directly (ar)
  {
    const lctx = await ctx.browser.newContext();
    await lctx.addCookies([{ name: 'mf2-locale', value: 'ar', url: ctx.baseUrl }]);
    await captureSsrSnapshot(lctx);
    const lp = await lctx.newPage();
    watchConsole(lp, consoleSink, '/lazy (direct)');
    const lnet = [];
    watchNetwork(lp, lnet);
    await lp.goto(`${ctx.baseUrl}/lazy`);
    await waitHydrated(lp);
    const snapL = await lp.evaluate(() => window.__ssrSnapshot);
    const afterL = await lp.evaluate(() => document.body.innerText);
    ctx.assert('H1 direct /lazy (ar): SSR text == hydrated text', snapL?.text === afterL);
    ctx.assert('H2 direct /lazy (ar): heading', (await text(lp, '#lazy-heading')) === T.ar.lazyHeading);
    ctx.assert('H3 direct /lazy: chunk loaded for hydrate_lazy', lnet.some((n) => /\/pkg\/split_[^/]+\.wasm$/.test(n.url)));
    await switchLocale(lp, 'en');
    ctx.assert('H4 direct /lazy: switch to en updates lazy text', (await text(lp, '#lazy-heading')) === T.en.lazyHeading && (await text(lp, '#lazy-locale')) === 'en');
    await lctx.close();
  }

  ctx.data.consoleMessages = consoleSink;
  ctx.assert('Z zero console warnings/errors (load, hydrate, lazy nav, switch & back, streaming)', consoleSink.length === 0, consoleSink.slice(0, 10));

  // ------------------------------------------------ S. deploy skew is rejected
  {
    const kctx = await ctx.browser.newContext();
    const kp = await kctx.newPage();
    const errs = [];
    watchConsole(kp, errs, 'skew');
    await kp.goto(`${ctx.baseUrl}/?skew=1`);
    await sleep(2500);
    const live = await kp.evaluate(LIVE_NODES);
    ctx.data.skew = { consoleMessages: errs, liveNodes: live };
    ctx.assert('S1 skewed catalog rejected (manifest hash mismatch), app not hydrated', errs.some((e) => e.text.includes('manifest hash mismatch')) && live === 0, { errs, live });
    await kctx.close();
  }
}

// HTTP-level checks (no browser): error handler, headers, catalog serving,
// route-list / no-owner fallback. Exported so they can run on their own.
export async function http(ctx) {
  const get = (path, headers = {}) => fetch(`${ctx.baseUrl}${path}`, { headers, redirect: 'manual' });
  const startup = await (await get('/__probe/ctx')).text();
  ctx.data.countersAtHttpStart = startup;

  const nf = await get('/definitely-missing', { Cookie: 'mf2-locale=ar' });
  const nfBody = await nf.text();
  ctx.assert('N1 file_and_error_handler_with_context: 404 rendered in the negotiated locale', nf.status === 404 && nfBody.includes('lang="ar"') && nfBody.includes('الصفحة غير موجودة'), nf.status);
  const home = await get('/', { 'Accept-Language': 'ar;q=0.9, en;q=0.8' });
  ctx.assert('N2 Content-Language + Vary on SSR responses', home.headers.get('content-language') === 'ar' && /Accept-Language/.test(home.headers.get('vary') ?? ''), { cl: home.headers.get('content-language'), vary: home.headers.get('vary') });
  const html = await home.text();
  const href = /<link rel="preload" as="fetch" crossorigin="anonymous" href="([^"]+)" data-mf2="">/.exec(html)?.[1];
  ctx.assert('N3 shell emits <link rel=preload as=fetch crossorigin data-mf2> first in <head>', Boolean(href) && html.indexOf(href) < html.indexOf('/pkg/'), href);
  const cat = await get(href);
  const bytes = new Uint8Array(await cat.arrayBuffer());
  ctx.assert('N4 catalog served immutable', cat.status === 200 && cat.headers.get('cache-control') === 'public, max-age=31536000, immutable' && String.fromCharCode(...bytes.slice(0, 4)) === 'MF2B', { status: cat.status, cc: cat.headers.get('cache-control'), size: bytes.length });
  const redir = await get('/i18n/ar');
  ctx.assert('N5 /i18n/<tag> redirects to the hashed URL (set_locale discovery)', redir.status === 307 && redir.headers.get('location') === href, redir.headers.get('location'));
  const noCtx = await (await get('/__probe/no-ctx')).text();
  ctx.assert('N6 lookup with no owner/context falls back to the default locale (no panic)', noCtx.startsWith('Welcome!'), noCtx);
}

