// Phase 7 A2 (plans/15-phase-7-work-order.md) against `examples/demo-csr`:
// a client-only application, built by `trunk build` into
// examples/demo-csr/dist/, which this check serves itself — as any static
// host would, with no server logic at all (--base-url is not used).
//
// What it asserts, and why each one is here:
//
//   * the locale comes from the browser: the reader's languages on a first
//     visit (`fr-CA` finds `fr`; a language the build lacks gets the source
//     locale), and the remembered choice afterwards, ahead of the languages;
//   * the boot reads the catalog index through `index.html`'s preload (one
//     request) and fetches one catalog, the chosen locale's, and nothing
//     renders before it is installed: the first frame is already in the
//     chosen locale, with `<html lang dir>` to match and the markup message's
//     element in place;
//   * a switch is live — no navigation — reaches the text, the attribute, the
//     title, the reactive argument and schema.org's `inLanguage`, sets `dir`
//     for Arabic, is remembered, and a reload comes back in it; switching
//     back gives the same page as booting in that locale;
//   * text the catalog borrowed is built inside `<span lang [dir]>`
//     (`mark-fallback-lang`): the note, untranslated in Arabic on purpose,
//     is wrapped when the page mounts in Arabic and when a switch reaches
//     Arabic — around the text node French built — and unwrapped when a
//     switch leaves it (Phase 7 A14);
//   * a switch that meets a catalog from another deploy remembers the
//     choice and reloads into it (Phase 9 B2);
//   * a failed boot (no catalog, no index) logs one `mf2:` line and mounts
//     nothing, rather than trapping or rendering an empty page;
//   * no message text is in the client bundle (B6).
//
//   (cd examples/demo-csr && trunk build)
//   node run.mjs csr --browser chromium,firefox

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chooseLocale, watchConsole, resourceTimings, sleep, until } from '../lib/browser.mjs';
import { serveStatic } from '../lib/static.mjs';

const DIST = fileURLToPath(new URL('../../../examples/demo-csr/dist/', import.meta.url));
const CANARIES = [
  'people are here',
  'personnes sont ici',
  'Échap',
  'no server',
  'sans serveur',
  'اللغة',
];

const NORMALISE = (text) => text.replace(/\s+/g, ' ').trim();

// Chromium's note on the `integrity` trunk writes on its own wasm preload
// (crbug.com/981419): SRI is not applied to that preload, and is to the
// module and the stylesheet. Trunk's markup, not this library's; turning SRI
// off to silence it would be the wrong trade.
const TRUNK_SRI_NOTE = /integrity` attribute is currently ignored for preload destinations/;

/** Console warnings and errors, less the one above. */
const worth = (messages) => messages.filter((m) => !TRUNK_SRI_NOTE.test(m.text));

const serve = () => serveStatic(DIST);

export async function run(ctx) {
  const { assert, data } = ctx;
  for (const need of ['index.html', 'i18n/index.json']) {
    if (!existsSync(join(DIST, need))) {
      throw new Error(`examples/demo-csr/dist/${need} missing: run \`trunk build\` in examples/demo-csr`);
    }
  }
  const index = JSON.parse(readFileSync(join(DIST, 'i18n/index.json'), 'utf8'));
  assert('index-names-every-locale', ['ar', 'en', 'fr'].every((t) => index[t]?.startsWith(`${t}.`)), index);

  const server = await serve();
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    await firstVisit(ctx, base, index);
    await negotiation(ctx, base);
    await switchSkew(ctx, base);
    await failedBoot(ctx, base, 'no-catalog', '**/i18n/*.mf2b');
    await failedBoot(ctx, base, 'no-index', '**/i18n/index.json');
    await canary(ctx, base);
  } finally {
    server.close();
  }
  data.index = index;
}

/** A reader whose browser prefers French, from first visit to reload. */
async function firstVisit(ctx, base, index) {
  const { browser, assert, data } = ctx;
  const context = await browser.newContext({ locale: 'fr-FR' });
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);

  // The page before the wasm has run: `index.html`, and nothing rendered.
  await page.goto(`${base}/`, { waitUntil: 'load' });
  await mounted(page);

  assert('first-visit-follows-the-reader', (await lang(page)) === 'fr', await lang(page));
  assert('first-visit-dir', (await page.getAttribute('html', 'dir')) === 'ltr');
  const tagline = NORMALISE(await page.textContent('#tagline'));
  assert('first-frame-is-french', tagline.startsWith('Cette page'), tagline);
  assert('title-is-french', (await page.title()) === 'Une page sans serveur', await page.title());
  assert('markup-is-an-element', (await page.locator('#hotkey kbd').count()) === 1);
  assert(
    'markup-follows-the-message',
    NORMALISE(await page.textContent('#hotkey')).endsWith('appuyez sur Échap'),
    await page.textContent('#hotkey'),
  );
  assert('in-language', (await inLanguage(page)) === 'fr', await inLanguage(page));

  const timings = await resourceTimings(page);
  data.timings = timings.filter((t) => t.name.includes('/i18n/') || t.name.endsWith('.wasm'));
  const indexRequests = timings.filter((t) => t.name.endsWith('/i18n/index.json'));
  assert('one-index-request', indexRequests.length === 1, indexRequests.map((t) => t.initiatorType));
  assert(
    'index-came-from-the-preload',
    indexRequests[0]?.initiatorType === 'link',
    indexRequests[0]?.initiatorType,
  );
  const catalogs = timings.filter((t) => t.name.includes('.mf2b'));
  assert(
    'one-catalog-the-chosen-one',
    catalogs.length === 1 && catalogs[0].name.endsWith(`/i18n/${index.fr}`),
    catalogs.map((t) => t.name),
  );
  const frPage = NORMALISE(await page.evaluate(() => document.body.innerText));
  const frNote = await note(page, true);
  assert('own-text-is-not-marked', frNote.span === null && frNote.text.includes('Cette phrase'), frNote);

  // A signal-valued argument.
  const three = await page.textContent('#people');
  await page.click('#add-one');
  const four = await page.textContent('#people');
  assert('a-signal-argument-reformats', three !== four && four.includes('4'), { three, four });

  // ------------------------------------------------------ a live switch ---

  await page.evaluate(() => {
    window.__notReloaded = true;
  });
  const placeholder = await page.getAttribute('#search', 'placeholder');
  await chooseLocale(page, 'ar');
  await until(page, () => document.documentElement.lang === 'ar');
  assert('switch-is-live', await page.evaluate(() => window.__notReloaded === true));
  assert('rtl-switch-sets-dir', (await page.getAttribute('html', 'dir')) === 'rtl');
  const arTagline = NORMALISE(await page.textContent('#tagline'));
  assert('switch-reaches-text', /^تُبنى/.test(arTagline), arTagline);
  const arPlaceholder = await page.getAttribute('#search', 'placeholder');
  assert('switch-reaches-attributes', arPlaceholder !== placeholder && /ابحث/.test(arPlaceholder), arPlaceholder);
  assert('switch-reaches-the-title', (await page.title()) === 'صفحة بلا خادم', await page.title());
  const arPeople = NORMALISE(await page.textContent('#people'));
  assert('switch-reaches-the-argument', arPeople !== NORMALISE(four) && /هنا/.test(arPeople), arPeople);
  assert('switch-reaches-in-language', (await inLanguage(page)) === 'ar', await inLanguage(page));
  const arNote = await note(page);
  assert(
    'switch-marks-borrowed-text',
    arNote.span?.lang === 'en' && arNote.span?.dir === 'ltr' && arNote.textInSpan,
    arNote,
  );
  assert('switch-keeps-the-text-node', arNote.textKept, arNote);
  const stored = await page.evaluate(() => localStorage.getItem('mf2_locale'));
  assert('switch-is-remembered', stored === 'ar', stored);
  assert('console-is-silent', worth(console_).length === 0, worth(console_).slice(0, 4));

  // --------------------------------------------------------- reload ---

  await page.reload({ waitUntil: 'load' });
  await mounted(page);
  assert('reload-comes-back-in-the-choice', (await lang(page)) === 'ar', await lang(page));
  assert('reloaded-dir', (await page.getAttribute('html', 'dir')) === 'rtl');
  assert(
    'reloaded-first-frame',
    /^تُبنى/.test(NORMALISE(await page.textContent('#tagline'))),
    await page.textContent('#tagline'),
  );
  const mountedNote = await note(page, true);
  assert(
    'mount-marks-borrowed-text',
    mountedNote.span?.lang === 'en' && mountedNote.span?.dir === 'ltr' && mountedNote.textInSpan,
    mountedNote,
  );

  // Back to French: the same page as booting in French.
  await chooseLocale(page, 'fr');
  await until(page, () => document.documentElement.lang === 'fr');
  await sleep(50);
  const backToFr = NORMALISE(await page.evaluate(() => document.body.innerText));
  const backNote = await note(page);
  assert('switch-unmarks-own-text', backNote.span === null && backNote.textKept, backNote);
  assert('switching-back-is-booting-there', backToFr === frPage, {
    booted: frPage.slice(0, 120),
    switched: backToFr.slice(0, 120),
  });

  // English, remembered, over a reader who prefers French.
  await chooseLocale(page, 'en');
  await until(page, () => document.documentElement.lang === 'en');
  await page.reload({ waitUntil: 'load' });
  await mounted(page);
  assert('the-choice-outranks-the-reader', (await lang(page)) === 'en', await lang(page));
  assert('console-is-still-silent', worth(console_).length === 0, worth(console_).slice(0, 4));
  await context.close();
}

/** What a first visit starts in, by the reader's languages alone. */
async function negotiation(ctx, base) {
  const { browser, assert } = ctx;
  for (const [locale, expected] of [
    ['fr-CA', 'fr'],
    ['ar-EG', 'ar'],
    ['de-DE', 'en'],
  ]) {
    const context = await browser.newContext({ locale });
    const page = await context.newPage();
    await page.goto(`${base}/`, { waitUntil: 'load' });
    await mounted(page);
    assert(`reader-${locale}-gets-${expected}`, (await lang(page)) === expected, await lang(page));
    await context.close();
  }
  // A remembered locale the build does not have is ignored.
  const context = await browser.newContext({ locale: 'fr-FR' });
  await context.addInitScript(() => localStorage.setItem('mf2_locale', 'xx'));
  const page = await context.newPage();
  await page.goto(`${base}/`, { waitUntil: 'load' });
  await mounted(page);
  assert('unknown-remembered-locale-is-ignored', (await lang(page)) === 'fr', await lang(page));
  await context.close();
}

/** Where a catalog's header holds its `manifest_hash` (`mf2-catalog`'s
 * `format::header::MANIFEST_HASH`). */
const MANIFEST_HASH_AT = 8;

/**
 * A switch that meets a catalog from another deploy (Phase 9 B2): the
 * choice goes to `localStorage` and the page reloads into it. The other
 * deploy's catalog is this build's with its manifest hash altered, served
 * once (see `demo.mjs`'s `switchSkew`).
 */
async function switchSkew(ctx, base) {
  const { browser, assert, data } = ctx;
  const context = await browser.newContext({ locale: 'en-US' });
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);
  await page.goto(`${base}/`, { waitUntil: 'load' });
  await mounted(page);

  let skewed = 0;
  await page.route('**/i18n/fr.*', async (route) => {
    if (skewed > 0) {
      await route.continue();
      return;
    }
    skewed += 1;
    const response = await route.fetch();
    const body = Buffer.from(await response.body());
    body[MANIFEST_HASH_AT] ^= 0xff;
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
    await mounted(page);
  } catch {
    reloaded = false;
  }
  const after = {
    skewed,
    navigations,
    reloaded,
    lang: await lang(page),
    stored: await page.evaluate(() => localStorage.getItem('mf2_locale')),
    mf2: worth(console_).filter((m) => m.text.startsWith('mf2:')).map((m) => m.text),
  };
  data.switchSkew = after;
  assert('skew-switch-served-the-other-deploys-catalog', skewed === 1, skewed);
  assert('skew-switch-reloads-into-the-new-locale', reloaded && after.lang === 'fr', after);
  assert('skew-switch-navigates-once', navigations === 1, navigations);
  assert('skew-switch-remembers-the-choice', after.stored === 'fr', after.stored);
  assert(
    'skew-switch-says-why-once',
    after.mf2.length === 1 && after.mf2[0].includes('another deploy'),
    after.mf2,
  );
  await context.close();
}

/** A boot whose fetch fails: one line, and nothing mounted. */
async function failedBoot(ctx, base, name, pattern) {
  const { browser, assert, data } = ctx;
  const context = await browser.newContext({ locale: 'en-US' });
  await context.route(pattern, (route) => route.fulfill({ status: 404, body: 'gone' }));
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);
  await page.goto(`${base}/`, { waitUntil: 'load' });
  // The wasm has started — its export answers — and the boot has had time
  // to fail.
  await until(page, () => typeof window.wasmBindings?.mf2_live_nodes === 'function');
  await sleep(500);
  const errors = console_.filter((m) => m.type === 'error' && !/Failed to load resource/.test(m.text));
  data[`failed-boot-${name}`] = console_;
  assert(
    `failed-boot-${name}-says-so-once`,
    errors.length === 1 && errors[0].text.startsWith('mf2:') && errors[0].text.includes('not started'),
    errors,
  );
  assert(`failed-boot-${name}-does-not-trap`, !console_.some((m) => m.type === 'pageerror'), console_);
  assert(`failed-boot-${name}-mounts-nothing`, (await page.locator('main').count()) === 0);
  await context.close();
}

/** B6: no message text in what the client downloads. */
async function canary(ctx) {
  const { assert } = ctx;
  const files = readdirSync(DIST).filter((f) => f.endsWith('.js') || f.endsWith('.wasm'));
  assert('client-bundle-built', files.some((f) => f.endsWith('.wasm')), files);
  const found = [];
  for (const f of files) {
    const bytes = readFileSync(join(DIST, f));
    for (const c of CANARIES) if (bytes.includes(Buffer.from(c))) found.push(`${f}: ${c}`);
  }
  assert('no-message-text-in-the-client', found.length === 0, found);
}

/** The application has mounted: its nodes are registered. */
/**
 * The note's span, if any, and whether the text node remembered by the
 * last `note(page, true)` is still the one on the page.
 */
async function note(page, remember = false) {
  return page.evaluate((remember) => {
    const p = document.querySelector('#untranslated');
    const span = p?.querySelector('span');
    // The message's text node: inside the span, else the last text child.
    const text = span ? span.firstChild : [...(p?.childNodes ?? [])].filter((n) => n.nodeType === 3).pop();
    if (remember) window.__noteText = text;
    return {
      span: span ? { lang: span.getAttribute('lang'), dir: span.getAttribute('dir') } : null,
      textInSpan: span != null && span.firstChild === text && text?.nodeType === 3,
      textKept: text != null && text === window.__noteText && text.isConnected,
      text: (p?.textContent ?? '').replace(/\s+/g, ' ').trim(),
    };
  }, remember);
}

async function mounted(page) {
  await until(page, () => (window.wasmBindings?.mf2_live_nodes() ?? 0) > 0);
  await sleep(50);
}

async function lang(page) {
  return page.evaluate(() => document.documentElement.lang);
}

async function inLanguage(page) {
  return page.getAttribute('meta[itemprop="inLanguage"]', 'content');
}
