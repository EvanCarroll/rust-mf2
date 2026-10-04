// Phase 6 against
// `examples/demo-ssr`, which must already be running at --base-url.
//
// The successor to `p002`, which ran against the Phase 0 probe. What it
// asserts, and why each one is here:
//
//   * negotiation is an ordered list, and the answer is *serialized into the
//     page* — `<html lang dir>`, the preload link — so the client never
//     negotiates again (04 §11 item 3);
//   * `Content-Language` and a `Vary` that names every header a source read;
//     the cookie sink writes only an explicit choice (`?lang=`; Phase 9 B3);
//   * the catalog is immutable, precompressed, and the bare tag redirects;
//   * hydration changes no text and logs nothing (P0.10) — in a browser in
//     UTC, the zone a first visit is served in (`zone.mjs` asserts the
//     reader's-zone correction elsewhere);
//   * a locale switch updates every live node, `<html lang dir>`, and the
//     `<title>` — and switching back gives exactly the server's text;
//   * a signal-valued argument still re-formats after a switch;
//   * markup is real elements, and its position follows the *message*;
//   * an attribute's bidi marks follow its name — none in `value=` and
//     `data-*`, which programs read, and around the name in `title=` — as
//     served, as hydrated, and as the registry rewrites them on a switch
//     (04 §9);
//   * text the catalog borrowed from another locale is inside
//     `<span lang [dir]>` (`mark-fallback-lang`, WCAG 3.1.2): as served,
//     adopted by hydration as the same node, removed by a switch to a
//     locale that has its own text and put back by a switch home — around
//     a text node that keeps its identity throughout (Phase 7 A14);
//   * a switch that meets a catalog from another deploy reloads into the
//     new locale, remembered in the cookie (Phase 9 B2);
//   * on a page whose language is in its URL, the switcher goes to the
//     other language's URL, with the wasm and without it (Phase 9 B4);
//   * with the wasm blocked, the switcher's form is the switch: its
//     `<select>` is named for the server's `QueryParam`, the `?lang=` it
//     submits outranks the cookie and `Accept-Language`, and the cookie it
//     leaves outranks `Accept-Language` on the next visit (Phase 10 D2);
//   * no message text is in the client bundle (B6).

import {
  chooseLocale,
  watchConsole,
  resourceTimings,
  captureSsrSnapshot,
  sleep,
  until,
} from '../lib/browser.mjs';

/** Text that must never appear in the client bundle (B6). */
const CANARIES = [
  'people are here',
  'personnes sont ici',
  'Échap',
  'اللغة',
  'Search everything',
];

const HTML = /<html[^>]*\slang="([^"]*)"[^>]*>/i;
const DIR = /<html[^>]*\sdir="([^"]*)"[^>]*>/i;
const PRELOAD = /<link[^>]*\bdata-mf2\b[^>]*>/i;
const HREF = /href="([^"]+)"/i;

function attr(html, re) {
  const m = html.match(re);
  return m ? m[1] : undefined;
}

/** `document.body.innerText`, normalised for comparison. */
const NORMALISE = (text) => text.replace(/\s+/g, ' ').trim();

export async function run(ctx) {
  const { baseUrl, browser, assert, data } = ctx;

  // ------------------------------------------------------------- HTTP ---

  // A context per request: they share a cookie jar otherwise, and the first
  // response sets `mf2_locale`, which would then decide every later one — the
  // cookie source is listed before `Accept-Language` on purpose.
  const get = async (path, headers = {}) => {
    const api = await browser.newContext();
    const response = await api.request.get(`${baseUrl}${path}`, {
      headers,
      maxRedirects: 0,
      failOnStatusCode: false,
    });
    const body = Buffer.from(await response.body());
    const out = {
      status: response.status(),
      headers: response.headers(),
      text: () => body.toString('utf8'),
      body: () => body,
    };
    await api.close();
    return out;
  };

  const english = await get('/', { 'accept-language': 'en' });
  const englishHtml = english.text();
  assert('ssr-200', english.status === 200, english.status);
  assert('ssr-lang-en', attr(englishHtml, HTML) === 'en', attr(englishHtml, HTML));
  assert('ssr-dir-ltr', attr(englishHtml, DIR) === 'ltr', attr(englishHtml, DIR));
  assert(
    'content-language-en',
    english.headers['content-language'] === 'en',
    english.headers['content-language'],
  );
  const vary = (english.headers.vary || '').toLowerCase();
  assert('vary-names-every-source', vary.includes('cookie') && vary.includes('accept-language'), vary);
  // Phase 9 B3: a locale guessed from `Accept-Language` is not remembered.
  assert('sink-leaves-a-guess-unwritten', !english.headers['set-cookie'], english.headers['set-cookie']);

  // Accept-Language alone.
  const french = await get('/', { 'accept-language': 'fr-CA,fr;q=0.9,en;q=0.2' });
  const frenchHtml = french.text();
  assert('accept-language-fr', attr(frenchHtml, HTML) === 'fr', attr(frenchHtml, HTML));

  // The cookie is listed before Accept-Language, so it wins.
  const cookieWins = await get('/', { cookie: 'mf2_locale=fr', 'accept-language': 'en' });
  assert('cookie-beats-accept-language', attr(cookieWins.text(), HTML) === 'fr');

  // The query parameter is listed first of all, and `ar` is RTL.
  const arabic = await get('/?lang=ar', { cookie: 'mf2_locale=fr', 'accept-language': 'en' });
  const arabicHtml = arabic.text();
  assert('query-beats-everything', attr(arabicHtml, HTML) === 'ar', attr(arabicHtml, HTML));
  assert('rtl-locale-sets-dir', attr(arabicHtml, DIR) === 'rtl', attr(arabicHtml, DIR));
  // An explicit choice (`?lang=`) is what the sink writes.
  assert(
    'sink-writes-an-explicit-choice',
    (arabic.headers['set-cookie'] || '').includes('mf2_locale=ar'),
    arabic.headers['set-cookie'],
  );
  // A cookie that was read is not written back: its expiry does not slide.
  assert('sink-leaves-a-read-cookie-alone', !cookieWins.headers['set-cookie'], cookieWins.headers['set-cookie']);

  // `mark-fallback-lang`: the note is untranslated in Arabic on purpose, so
  // the Arabic page borrows it from English and says so; English and French
  // have their own text and no span.
  const note = (html) => html.match(/<p[^>]*\bid="untranslated"[^>]*>([\s\S]*?)<\/p>/)?.[1] || '';
  const arNote = note(arabicHtml);
  assert(
    'served-borrowed-text-is-marked',
    /<span lang="en" dir="ltr">This sentence has not been translated/.test(arNote),
    arNote,
  );
  assert('served-own-text-is-not-marked', !/<span/.test(note(englishHtml)), note(englishHtml));
  assert('served-own-french-is-not-marked', !/<span/.test(note(frenchHtml)), note(frenchHtml));

  // The preload link is the boot data.
  const preload = englishHtml.match(PRELOAD)?.[0];
  const preloadHref = preload && attr(preload, HREF);
  assert('preload-link-present', Boolean(preloadHref), preload);
  assert(
    'preload-url-is-content-hashed',
    /^\/i18n\/en\.[0-9a-f]+\.mf2b$/.test(preloadHref || ''),
    preloadHref,
  );
  data.preloadHref = preloadHref;

  // The in-page map carries *every* locale, this page's included: after a
  // switch, the preload link no longer names the locale the user may want
  // back (conformance L6 in the browser found this by failing to come home).
  const mapped = [...englishHtml.matchAll(/data-mf2-locale="([^"]+)"/g)].map((m) => m[1]).sort();
  assert('catalog-map-is-every-locale', JSON.stringify(mapped) === '["ar","en","fr"]', mapped);

  // The catalog itself.
  const catalog = await get(preloadHref, { 'accept-encoding': 'identity' });
  assert('catalog-200', catalog.status === 200, catalog.status);
  assert(
    'catalog-immutable',
    catalog.headers['cache-control'] === 'public, max-age=31536000, immutable',
    catalog.headers['cache-control'],
  );
  const raw = catalog.body().length;
  const compressed = await get(preloadHref, { 'accept-encoding': 'br' });
  assert(
    'catalog-precompressed',
    compressed.headers['content-encoding'] === 'br',
    compressed.headers['content-encoding'],
  );
  data.catalogBytes = { raw, brotli: Number(compressed.headers['content-length'] || 0) };

  // The bare tag: one redirect, at switch time only.
  const redirect = await get('/i18n/fr');
  assert('tag-redirects', redirect.status === 307, redirect.status);
  assert(
    'redirect-target-is-immutable-url',
    /^\/i18n\/fr\.[0-9a-f]+\.mf2b$/.test(redirect.headers.location || ''),
    redirect.headers.location,
  );
  assert(
    'redirect-is-not-cached',
    redirect.headers['cache-control'] === 'no-cache',
    redirect.headers['cache-control'],
  );
  const missing = await get('/i18n/klingon');
  assert('unknown-catalog-404', missing.status === 404, missing.status);

  // A page that does not exist still goes through the app, in the right
  // locale: the file/error handler gets the context too (04 §5).
  const notFound = await get('/no-such-page', { 'accept-language': 'fr' });
  assert('error-handler-404', notFound.status === 404, notFound.status);
  assert(
    'error-handler-has-the-locale',
    attr(notFound.text(), HTML) === 'fr',
    'the fallback handler was given the context',
  );

  // ---------------------------------------------------------- browser ---

  // In UTC, the zone a first visit is served in: a reader elsewhere sees the
  // date corrected after hydration, by design, which `zone.mjs` asserts.
  // Here hydration itself must change nothing, whatever the machine's zone.
  const context = await browser.newContext({ timezoneId: 'UTC' });
  await captureSsrSnapshot(context);
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);

  await page.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  await hydrated(page);

  const ssr = await page.evaluate(() => window.__ssrSnapshot);
  const after = { text: await page.evaluate(() => document.body.innerText), title: await page.title() };
  assert('hydration-changes-no-text', NORMALISE(ssr.text) === NORMALISE(after.text), {
    before: NORMALISE(ssr.text).slice(0, 120),
    after: NORMALISE(after.text).slice(0, 120),
  });
  assert('title-survives-hydration', ssr.title === after.title, { ssr: ssr.title, after: after.title });
  assert('console-is-silent', console_.length === 0, console_.slice(0, 4));

  // Exactly one catalog request, and it is the preload the page already made.
  const timings = await resourceTimings(page);
  const catalogRequests = timings.filter((t) => t.name.includes('/i18n/'));
  assert('one-catalog-request', catalogRequests.length === 1, catalogRequests.map((t) => t.name));
  assert(
    'catalog-came-from-the-preload',
    catalogRequests[0]?.initiatorType === 'link',
    catalogRequests[0]?.initiatorType,
  );
  data.timings = timings.filter((t) => t.name.includes('/i18n/') || t.name.endsWith('.wasm'));

  // The registry has a slot per rendered description.
  const live = await liveNodes(page);
  assert('registry-holds-the-page', live > 0, live);
  data.liveNodes = live;

  // Markup is real elements.
  assert('markup-is-an-element', (await page.locator('#hotkey kbd').count()) === 1);
  const enHotkey = NORMALISE(await page.textContent('#hotkey'));

  // Bidi by attribute name, first in the server's HTML, then as hydrated.
  const served = englishHtml.match(/<input[^>]*\bid="message"[^>]*>/)?.[0] || '';
  checkAttributeBidi(assert, 'served', {
    value: attr(served, /\svalue="([^"]*)"/),
    data: attr(served, /\sdata-greeting="([^"]*)"/),
    title: attr(served, /\stitle="([^"]*)"/),
  });
  const enMessage = await messageAttributes(page);
  checkAttributeBidi(assert, 'hydrated', enMessage);

  // ------------------------------------------------------------ switch ---

  const enText = NORMALISE(await page.evaluate(() => document.body.innerText));
  await chooseLocale(page, 'fr');
  await page.waitForFunction(
    () => document.documentElement.lang === 'fr',
    undefined,
    { timeout: 5000 },
  );
  const frText = NORMALISE(await page.evaluate(() => document.body.innerText));
  assert('switch-changes-the-text', frText !== enText);
  assert('switch-updates-html-lang', (await page.getAttribute('html', 'lang')) === 'fr');
  assert('switch-updates-html-dir', (await page.getAttribute('html', 'dir')) === 'ltr');
  assert(
    'switch-updates-the-title',
    (await page.title()) !== ssr.title,
    { before: ssr.title, after: await page.title() },
  );
  const frHotkey = NORMALISE(await page.textContent('#hotkey'));
  assert('markup-follows-the-message', frHotkey !== enHotkey, { en: enHotkey, fr: frHotkey });
  assert('markup-survives-a-switch', (await page.locator('#hotkey kbd').count()) === 1);
  // The registry's rewrite, not the server, wrote these.
  const frMessage = await messageAttributes(page);
  assert('attributes-follow-the-switch', frMessage.value !== enMessage.value, {
    en: enMessage.value,
    fr: frMessage.value,
  });
  checkAttributeBidi(assert, 'switched', frMessage);

  // What the switch produced must equal what the server produces for the
  // same locale: one renderer, two entry points.
  const frServer = await serverText(context, `${baseUrl}/?lang=fr`);
  assert('switched-text-equals-server-text', frText === frServer, {
    client: frText.slice(0, 120),
    server: frServer.slice(0, 120),
  });

  // Right-to-left, and back.
  await chooseLocale(page, 'ar');
  await page.waitForFunction(() => document.documentElement.lang === 'ar', undefined, { timeout: 5000 });
  assert('rtl-switch-sets-dir', (await page.getAttribute('html', 'dir')) === 'rtl');
  // Right-to-left is where the marks matter: the Latin name inside an
  // Arabic sentence.
  checkAttributeBidi(assert, 'switched-rtl', await messageAttributes(page));
  const arText = NORMALISE(await page.evaluate(() => document.body.innerText));
  const arServer = await serverText(context, `${baseUrl}/?lang=ar`);
  assert('rtl-text-equals-server-text', arText === arServer, {
    client: arText.slice(0, 80),
    server: arServer.slice(0, 80),
  });

  await chooseLocale(page, 'en');
  await page.waitForFunction(() => document.documentElement.lang === 'en', undefined, { timeout: 5000 });
  const backText = NORMALISE(await page.evaluate(() => document.body.innerText));
  assert('switching-back-restores-the-text', backText === enText, {
    before: enText.slice(0, 120),
    after: backText.slice(0, 120),
  });
  assert('switching-leaks-no-slots', (await liveNodes(page)) === live, {
    before: live,
    after: await liveNodes(page),
  });
  // A signal-valued argument re-formats when the signal changes — and it is
  // the *same* node the locale switch has been rewriting, so this runs last:
  // changing the count would make the comparisons above compare a page the
  // server never rendered.
  const beforeClick = await page.textContent('#people');
  await page.click('#add-one');
  const afterClick = await page.textContent('#people');
  assert('a-signal-argument-reformats', beforeClick !== afterClick, { beforeClick, afterClick });

  // …and it still follows the locale afterwards.
  await chooseLocale(page, 'fr');
  await page.waitForFunction(() => document.documentElement.lang === 'fr', undefined, { timeout: 5000 });
  const frCount = await page.textContent('#people');
  assert('a-signal-argument-follows-the-locale', frCount !== afterClick, { afterClick, frCount });
  await chooseLocale(page, 'en');
  await page.waitForFunction(() => document.documentElement.lang === 'en', undefined, { timeout: 5000 });

  assert('console-is-still-silent', console_.length === 0, console_.slice(0, 4));

  // ------------------------------------------- a live switch is kept ---

  // The page was loaded as `/?lang=en`. A live switch must leave the next
  // visit in the chosen locale: it writes the cookie the server negotiates
  // from, and takes the `?lang=` out of the address, which outranks it.
  await chooseLocale(page, 'fr');
  await page.waitForFunction(() => document.documentElement.lang === 'fr', undefined, { timeout: 5000 });
  const kept = await context.cookies(baseUrl);
  const cookie = kept.find((c) => c.name === 'mf2_locale');
  assert('live-switch-writes-the-cookie', cookie?.value === 'fr', kept);
  assert('live-switch-drops-the-query', !new URL(page.url()).searchParams.has('lang'), page.url());
  await page.reload({ waitUntil: 'load' });
  await hydrated(page);
  assert(
    'live-switch-survives-a-reload',
    (await page.getAttribute('html', 'lang')) === 'fr',
    await page.getAttribute('html', 'lang'),
  );
  await chooseLocale(page, 'en');
  await page.waitForFunction(() => document.documentElement.lang === 'en', undefined, { timeout: 5000 });
  await page.reload({ waitUntil: 'load' });
  await hydrated(page);
  assert(
    'switching-back-survives-a-reload',
    (await page.getAttribute('html', 'lang')) === 'en',
    await page.getAttribute('html', 'lang'),
  );
  assert('console-is-silent-after-reloads', console_.length === 0, console_.slice(0, 4));

  // --------------------------------------------- borrowed text ---

  await borrowedText(browser, baseUrl, assert, data);

  // ------------------------------------------ a switch meets a new deploy ---

  await switchSkew(browser, baseUrl, assert, data);
  await pathPrefix(browser, baseUrl, assert, data, get);
  await formSwitch(browser, baseUrl, assert, data);

  // ----------------------------------------------------------- canary ---

  const bundle = await context.request.get(`${baseUrl}/pkg/demo_ssr.js`);
  const wasm = await context.request.get(`${baseUrl}/pkg/demo_ssr.wasm`);
  assert('client-bundle-served', bundle.status() === 200 && wasm.status() === 200, {
    js: bundle.status(),
    wasm: wasm.status(),
  });
  const js = await bundle.text();
  const wasmBytes = Buffer.from(await wasm.body());
  assert(
    'the-wasm-is-really-the-wasm',
    wasmBytes.subarray(0, 4).toString('latin1') === '\0asm',
    wasmBytes.subarray(0, 8).toString('hex'),
  );
  const found = CANARIES.filter((c) => js.includes(c) || wasmBytes.includes(Buffer.from(c)));
  assert('no-message-text-in-the-client', found.length === 0, found);

  await context.close();

  // ------------------------------------------------ a failed boot ---

  // The catalog cannot be fetched. The page does not hydrate — with no
  // catalog every message would format to nothing, and the markup message's
  // structure would not exist (P0.10) — so the reader keeps the server's
  // page, and the console says why, once.
  const failing = await browser.newContext();
  await failing.route('**/i18n/**', (route) => route.abort());
  const failingPage = await failing.newPage();
  const failingConsole = [];
  watchConsole(failingPage, failingConsole);
  await failingPage.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  const servedText = NORMALISE(await failingPage.evaluate(() => document.body.innerText));
  await until(failingPage, () => document.querySelector('#people') !== null);
  await sleep(1500);
  const failed = {
    live: await failingPage.evaluate(async () => {
      try {
        return (await import('/pkg/demo_ssr.js')).mf2_live_nodes();
      } catch {
        return -1;
      }
    }),
    text: NORMALISE(await failingPage.evaluate(() => document.body.innerText)),
    mf2: failingConsole.filter((m) => m.text.startsWith('mf2:')).map((m) => m.text),
    other: failingConsole
      .filter((m) => !m.text.startsWith('mf2:') && !/i18n|Failed to load resource/.test(m.text))
      .map((m) => m.text.slice(0, 160)),
  };
  data.failedBoot = failed;
  assert('failed-boot-does-not-hydrate', failed.live === 0, failed.live);
  assert('failed-boot-keeps-the-servers-text', failed.text === servedText);
  assert('failed-boot-says-why-once', failed.mf2.length === 1, failed.mf2);
  assert('failed-boot-does-not-trap', failed.other.length === 0, failed.other);
  await failing.close();
}

/**
 * `mark-fallback-lang` in the browser (Phase 7 A14). The server wrote the
 * Arabic page's note inside `<span lang="en" dir="ltr">`; hydration must
 * adopt that span — the same node, no `mf2:` mismatch — and a switch must
 * fit the span around a text node that keeps its identity.
 */
async function borrowedText(browser, baseUrl, assert, data) {
  const context = await browser.newContext();
  // Before hydration: the span and its text node as the server wrote them.
  await context.addInitScript(() => {
    document.addEventListener('DOMContentLoaded', () => {
      const span = document.querySelector('#untranslated span');
      window.__servedSpan = span;
      window.__servedText = span?.firstChild ?? null;
    });
  });
  const page = await context.newPage();
  const messages = [];
  watchConsole(page, messages);
  await page.goto(`${baseUrl}/?lang=ar`, { waitUntil: 'load' });
  await hydrated(page);

  const probe = () =>
    page.evaluate(() => {
      const p = document.querySelector('#untranslated');
      const span = p?.querySelector('span');
      const text = window.__servedText;
      return {
        span: span ? { lang: span.getAttribute('lang'), dir: span.getAttribute('dir') } : null,
        sameSpan: span != null && span === window.__servedSpan,
        textKept: text != null && text.isConnected && p.contains(text),
        textInSpan: span != null && text?.parentNode === span,
        text: p?.textContent.replace(/\s+/g, ' ').trim(),
      };
    });

  const hydratedNote = await probe();
  assert('hydration-adopts-the-span', hydratedNote.sameSpan && hydratedNote.textInSpan, hydratedNote);
  assert(
    'hydrated-span-keeps-lang-and-dir',
    hydratedNote.span?.lang === 'en' && hydratedNote.span?.dir === 'ltr',
    hydratedNote.span,
  );
  const mismatch = messages.filter((m) => m.text.startsWith('mf2:'));
  assert('hydration-reports-no-mismatch', mismatch.length === 0, mismatch.map((m) => m.text));

  await chooseLocale(page, 'fr');
  await page.waitForFunction(() => document.documentElement.lang === 'fr', undefined, { timeout: 5000 });
  const frNote = await probe();
  assert('switch-removes-the-span', frNote.span === null, frNote);
  assert('switch-keeps-the-text-node', frNote.textKept, frNote);
  assert('switch-writes-the-own-text', /^Remarque/.test(frNote.text || ''), frNote.text);

  await chooseLocale(page, 'ar');
  await page.waitForFunction(() => document.documentElement.lang === 'ar', undefined, { timeout: 5000 });
  const arNote = await probe();
  assert(
    'switch-back-restores-the-span',
    arNote.span?.lang === 'en' && arNote.span?.dir === 'ltr' && arNote.textInSpan,
    arNote,
  );
  assert('switch-back-keeps-the-text-node', arNote.textKept, arNote);
  assert('switch-back-equals-hydrated', arNote.text === hydratedNote.text, {
    hydrated: hydratedNote.text,
    back: arNote.text,
  });
  assert('borrowed-text-console-is-silent', messages.length === 0, messages.slice(0, 4));
  data.borrowedText = { hydrated: hydratedNote, fr: frNote, ar: arNote };
  await context.close();
}

const BIDI_MARKS = /[\u2066-\u2069]/;

/** `#message`'s three translated attributes. */
async function messageAttributes(page) {
  return page.evaluate(() => {
    const el = document.querySelector('#message');
    return {
      value: el?.getAttribute('value'),
      data: el?.getAttribute('data-greeting'),
      title: el?.getAttribute('title'),
    };
  });
}

/**
 * One sentence in three attributes: plain where a program reads it
 * (`value`, `data-*`), the name isolated where a person does (`title`),
 * and otherwise the same text.
 */
function checkAttributeBidi(assert, stage, { value, data, title }) {
  const shown = { value, data, title };
  assert(`${stage}-value-attribute-is-plain`, value?.includes('Ada') && !BIDI_MARKS.test(value), shown);
  assert(`${stage}-data-attribute-is-plain`, data === value, shown);
  assert(`${stage}-title-attribute-is-isolated`, title?.includes('\u2068Ada\u2069'), shown);
  assert(
    `${stage}-attributes-differ-only-by-the-marks`,
    title?.replace(/[\u2066-\u2069]/g, '') === value,
    shown,
  );
}

/** Where a catalog's header holds its `manifest_hash` (`mf2-catalog`'s
 * `format::header::MANIFEST_HASH`): 8 bytes after the magic and version. */
const MANIFEST_HASH_AT = 8;

/**
 * A switch that meets a catalog from another deploy (Phase 9 B2). The
 * server has moved on; this page's wasm cannot read the new build's
 * catalogs. `set_locale` must remember the choice and reload into it — at
 * boot a mismatch already reloads — rather than refuse and leave the page
 * in the old language.
 *
 * The "other deploy" is this build's French catalog with its manifest hash
 * altered, served once: the hash is the first thing the reader checks, and
 * it is all that tells two builds' catalogs apart. After the reload the
 * route serves the real bytes, as the new deploy's server would serve its
 * own wasm and catalogs together.
 */
async function switchSkew(browser, baseUrl, assert, data) {
  const context = await browser.newContext();
  const page = await context.newPage();
  const messages = [];
  watchConsole(page, messages);
  await page.goto(`${baseUrl}/?lang=en`, { waitUntil: 'load' });
  await hydrated(page);

  let skewed = 0;
  await page.route('**/i18n/fr*', async (route) => {
    if (skewed > 0) {
      await route.continue();
      return;
    }
    skewed += 1;
    const response = await route.fetch();
    const body = Buffer.from(await response.body());
    body[MANIFEST_HASH_AT] ^= 0xff;
    // The decoded bytes, so none of the response's encoding headers.
    await route.fulfill({
      status: 200,
      contentType: response.headers()['content-type'] ?? 'application/octet-stream',
      body,
    });
  });
  let navigations = 0;
  page.on('framenavigated', (frame) => {
    if (frame === page.mainFrame()) navigations += 1;
  });
  await page.evaluate(() => {
    window.__beforeSwitch = true;
  });

  await chooseLocale(page, 'fr');
  let reloaded = true;
  try {
    await page.waitForFunction(
      () => document.documentElement.lang === 'fr' && window.__beforeSwitch === undefined,
      undefined,
      { timeout: 5000 },
    );
    await hydrated(page);
  } catch {
    reloaded = false;
  }
  const cookies = await context.cookies(baseUrl);
  const after = {
    skewed,
    navigations,
    reloaded,
    lang: await page.getAttribute('html', 'lang'),
    url: page.url(),
    cookie: cookies.find((c) => c.name === 'mf2_locale')?.value,
    mf2: messages.filter((m) => m.text.startsWith('mf2:')).map((m) => m.text),
  };
  data.switchSkew = after;
  assert('skew-switch-served-the-other-deploys-catalog', skewed === 1, skewed);
  assert('skew-switch-reloads-into-the-new-locale', reloaded && after.lang === 'fr', after);
  assert('skew-switch-navigates-once', navigations === 1, navigations);
  assert('skew-switch-remembers-the-choice', after.cookie === 'fr', after.cookie);
  assert('skew-switch-drops-the-query', !new URL(after.url).searchParams.has('lang'), after.url);
  assert(
    'skew-switch-says-why-once',
    after.mf2.length === 1 && after.mf2[0].includes('another deploy'),
    after.mf2,
  );
  await context.close();
}

/**
 * Phase 10 D2: the default order — `?lang=`, the cookie, `Accept-Language` —
 * seen through the switcher's form alone, with the wasm blocked.
 */
async function formSwitch(browser, baseUrl, assert, data) {
  const context = await browser.newContext({ locale: 'en-US' });
  await context.route('**/*.wasm', (route) => route.abort());
  await context.addCookies([{ name: 'mf2_locale', value: 'ar', url: baseUrl }]);
  const page = await context.newPage();
  await page.goto(`${baseUrl}/`, { waitUntil: 'load' });
  const before = {
    lang: await page.getAttribute('html', 'lang'),
    name: await page.locator('.mf2-locale-switcher select').first().getAttribute('name'),
  };
  assert('form-switch-cookie-outranks-accept-language', before.lang === 'ar', before);
  assert('form-switch-select-named-for-the-query-source', before.name === 'lang', before);
  await chooseLocale(page, 'fr');
  let landed = true;
  try {
    await page.waitForURL(/[?&]lang=fr/, { timeout: 5000 });
  } catch {
    landed = false;
  }
  const after = {
    landed,
    url: page.url(),
    lang: await page.getAttribute('html', 'lang'),
    cookie: (await context.cookies(baseUrl)).find((c) => c.name === 'mf2_locale')?.value,
  };
  assert('form-switch-query-outranks-the-cookie', landed && after.lang === 'fr', after);
  assert('form-switch-remembers-the-choice', after.cookie === 'fr', after.cookie);
  await page.goto(`${baseUrl}/`, { waitUntil: 'load' });
  after.revisit = await page.getAttribute('html', 'lang');
  assert('form-switch-cookie-decides-the-next-visit', after.revisit === 'fr', after.revisit);
  data.formSwitch = { before, after };
  await context.close();
}

/**
 * Phase 9 B4: a page whose language is in its URL (`/<tag>/about`, the path
 * prefix first in the negotiator). A `?lang=` cannot change it, so its
 * switcher — given `href_of` — goes to the chosen language's URL: with the
 * wasm, by the submit handler; without it, by the form's `?lang=` and the
 * server's redirect.
 */
async function pathPrefix(browser, baseUrl, assert, data, get) {
  const SWITCHER = '#about .mf2-locale-switcher';

  // The server alone: the path wins over the query, and the redirect.
  const plain = await get('/en/about?lang=fr');
  assert('path-prefix-redirects-a-disagreeing-query', plain.status === 303 && plain.headers.location === '/fr/about', {
    status: plain.status,
    location: plain.headers.location,
  });
  const kept = await get('/en/about?x=1&lang=ar');
  assert('path-prefix-redirect-keeps-other-parameters', kept.headers.location === '/ar/about?x=1', kept.headers.location);
  const home = await get('/?lang=fr');
  assert('no-prefix-no-redirect', home.status === 200 && attr(home.text(), HTML) === 'fr', home.status);
  const fr = await get('/fr/about', { cookie: 'mf2_locale=en', 'accept-language': 'en' });
  assert('path-prefix-outranks-cookie', attr(fr.text(), HTML) === 'fr', attr(fr.text(), HTML));
  assert(
    'path-prefix-options-carry-their-urls',
    ['en', 'fr', 'ar'].every((tag) => fr.text().includes(`data-mf2-href="/${tag}/about"`)),
  );

  const run = async (label, blockWasm) => {
    const context = await browser.newContext();
    if (blockWasm) await context.route('**/*.wasm', (route) => route.abort());
    const page = await context.newPage();
    const messages = [];
    watchConsole(page, messages);
    await page.goto(`${baseUrl}/en/about`, { waitUntil: 'load' });
    if (!blockWasm) await hydrated(page);
    await chooseLocale(page, 'fr', SWITCHER);
    let landed = true;
    try {
      await page.waitForURL(`${baseUrl}/fr/about`, { timeout: 5000 });
      await page.waitForFunction(() => document.documentElement.lang === 'fr', undefined, { timeout: 5000 });
    } catch {
      landed = false;
    }
    const cookies = await context.cookies(baseUrl);
    const after = {
      landed,
      url: page.url(),
      lang: await page.getAttribute('html', 'lang'),
      cookie: cookies.find((c) => c.name === 'mf2_locale')?.value,
      errors: blockWasm ? [] : messages.filter((m) => m.type === 'error').map((m) => m.text),
    };
    data[`pathPrefix_${label}`] = after;
    assert(`path-prefix-switch-${label}-lands-on-the-url`, landed && after.lang === 'fr', after);
    assert(`path-prefix-switch-${label}-remembers-the-path`, after.cookie === 'fr', after.cookie);
    assert(`path-prefix-switch-${label}-is-silent`, after.errors.length === 0, after.errors);
    await context.close();
  };
  await run('wasm', false);
  await run('no-wasm', true);

  // The control: the same switcher with its options' URLs taken away
  // switches in place, as on a page without `href_of` — the URL stays.
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto(`${baseUrl}/en/about`, { waitUntil: 'load' });
  await hydrated(page);
  await page.evaluate(() => {
    for (const o of document.querySelectorAll('#about option')) o.removeAttribute('data-mf2-href');
  });
  await chooseLocale(page, 'fr', SWITCHER);
  await page.waitForFunction(() => document.documentElement.lang === 'fr', undefined, { timeout: 5000 }).catch(() => {});
  const control = { url: page.url(), lang: await page.getAttribute('html', 'lang') };
  data.pathPrefix_control = control;
  assert(
    'path-prefix-control-without-urls-stays-in-place',
    control.lang === 'fr' && control.url === `${baseUrl}/en/about`,
    control,
  );
  await context.close();
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
  // One frame, so that anything scheduled by hydration has run.
  await sleep(50);
}

async function liveNodes(page) {
  return page.evaluate(async () => (await import('/pkg/demo_ssr.js')).mf2_live_nodes());
}

/** The server's own text for `url`, read before that page hydrates. */
async function serverText(context, url) {
  const page = await context.newPage();
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const text = await page.evaluate(() => document.body.innerText);
  await page.close();
  return text.replace(/\s+/g, ' ').trim();
}
