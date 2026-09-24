// Phase 7 A11 (plans/15-phase-7-work-order.md): the WCAG 2.2 AA audit's
// automated part, over every example page. `examples/demo-ssr` (built with
// `--split`) must be running at --base-url (default port 3702) and
// `examples/demo-islands` at $MF2_ISLANDS_URL (default port 3704);
// `examples/demo-csr/dist/` (after `trunk build`) is served by the check.
//
// What it asserts, and why each one is here:
//
//   * the scan: axe-core (pinned in package.json) with the WCAG 2.0/2.1/2.2
//     A and AA rules reports no violation and nothing incomplete on
//     demo-ssr `/` and `/lazy`, demo-islands `/` and demo-csr `/`, each in
//     en, fr and ar, light and dark, and on demo-ssr after a live switch to
//     ar and a client navigation to `/lazy`. axe's best-practice rules are
//     recorded (`--json`), not asserted: they are not WCAG;
//   * what axe does not measure: a control's edge against the page (1.4.11,
//     3:1), placeholder text against its field (1.4.3, 4.5:1), button text
//     and the focus ring's colour — computed in the page, both schemes;
//   * reflow (1.4.10): no horizontal scroll at 320 CSS px, nor with 1.4.12's
//     text spacing applied, and nothing clipped by a box that hides overflow;
//   * the structure the audit fixed: one `main`, the banner, navigation and
//     contentinfo outside it; the counter a `status`; demo-ssr's two routes
//     with their own titles, a client navigation changing it, and no two
//     fields with one label;
//   * the switcher (owner question 9): named by its label, no `id` in it; an
//     arrow key on the select changes the select and nothing else — no
//     switch, no navigation (3.2.2, F37); the button switches — in place
//     under `hydrate` and `csr` with focus kept, by the form's `GET ?lang=`
//     on the islands page and on demo-ssr with the wasm blocked;
//   * negative controls, one per detector: a page with `<html lang>` removed
//     and grey text fails the scan; the old border and a pale placeholder
//     fail the contrast measure; a 400 px box fails reflow and a
//     one-line box that hides a wrapped sentence is found clipped; a select that
//     switches on `change` fails the arrow-key probe.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { chooseLocale, sleep, until } from '../lib/browser.mjs';
import { serveStatic } from '../lib/static.mjs';

const AXE = readFileSync(
  fileURLToPath(new URL('../node_modules/axe-core/axe.min.js', import.meta.url)),
  'utf8',
);
const CSR_DIST = fileURLToPath(new URL('../../../examples/demo-csr/dist/', import.meta.url));
const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];
const LOCALES = ['en', 'fr', 'ar'];
const SCHEMES = ['light', 'dark'];
const NORMALISE = (text) => text.replace(/\s+/g, ' ').trim();
// WCAG 1.4.12's figures, applied to everything.
const TEXT_SPACING =
  '* { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important }' +
  ' p { margin-bottom: 2em !important }';

export async function run(ctx) {
  const ssr = ctx.baseUrl;
  const islands = (process.env.MF2_ISLANDS_URL ?? 'http://127.0.0.1:3704').replace(/\/$/, '');
  const server = await serveStatic(CSR_DIST);
  const csr = `http://127.0.0.1:${server.address().port}`;
  ctx.data.scans = [];
  try {
    const apps = [
      { name: 'demo-ssr /', url: (l) => `${ssr}/?lang=${l}`, ready: ssrReady },
      { name: 'demo-ssr /lazy', url: (l) => `${ssr}/lazy?lang=${l}`, ready: ssrReady },
      { name: 'demo-islands /', url: (l) => `${islands}/?lang=${l}`, ready: islandsReady },
      { name: 'demo-csr /', url: () => `${csr}/`, remember: true, ready: csrReady },
    ];
    for (const scheme of SCHEMES) {
      for (const app of apps) {
        for (const locale of LOCALES) {
          await auditPage(ctx, app, locale, scheme);
        }
      }
      await auditAfterASwitch(ctx, ssr, scheme);
    }
    await ssrStructure(ctx, ssr);
    await switcherLive(ctx, 'demo-ssr', `${ssr}/`, ssrReady);
    await switcherLive(ctx, 'demo-csr', `${csr}/`, csrReady);
    await switcherIslands(ctx, islands);
    await switcherWithoutWasm(ctx, ssr);
    await controls(ctx, ssr);
  } finally {
    server.close();
  }
}

// ------------------------------------------------------------- the pages ---

/** One page, one locale, one scheme: the scan, the measures, the structure. */
async function auditPage(ctx, app, locale, scheme) {
  const { assert } = ctx;
  const id = `${app.name} ${locale} ${scheme}`;
  const context = await ctx.browser.newContext({ colorScheme: scheme });
  if (app.remember) {
    await context.addInitScript((tag) => localStorage.setItem('mf2_locale', tag), locale);
  }
  const page = await context.newPage();
  await page.goto(app.url(locale), { waitUntil: 'load' });
  await app.ready(page);
  assert(`${id}: in its locale`, (await lang(page)) === locale, await lang(page));

  await scan(ctx, page, id);
  checkContrast(ctx, id, await contrast(page));
  await checkStructure(ctx, page, id);
  if (scheme === 'light') await checkReflow(ctx, page, id);
  await context.close();
}

/** demo-ssr after a live switch to Arabic, then a client navigation. */
async function auditAfterASwitch(ctx, ssr, scheme) {
  const context = await ctx.browser.newContext({ colorScheme: scheme });
  const page = await context.newPage();
  await page.goto(`${ssr}/?lang=en`, { waitUntil: 'load' });
  await ssrReady(page);
  await chooseLocale(page, 'ar');
  await until(page, () => document.documentElement.lang === 'ar');
  await sleep(100);
  await scan(ctx, page, `demo-ssr / switched to ar ${scheme}`);
  await page.click('nav a[href="/lazy"]');
  await page.waitForSelector('#lazy-heading');
  await sleep(100);
  await scan(ctx, page, `demo-ssr /lazy by client navigation, ar ${scheme}`);
  await context.close();
}

// --------------------------------------------------------------- the scan ---

async function injectAxe(page) {
  if (!(await page.evaluate(() => typeof window.axe === 'object'))) {
    await page.addScriptTag({ content: AXE });
  }
}

async function axeRun(page, tags) {
  await injectAxe(page);
  return page.evaluate(async (values) => {
    const result = await window.axe.run(document, {
      runOnly: { type: 'tag', values },
      resultTypes: ['violations', 'incomplete'],
    });
    const brief = (items) =>
      items.map((r) => ({ id: r.id, impact: r.impact, nodes: r.nodes.map((n) => n.target.join(' ')) }));
    return { violations: brief(result.violations), incomplete: brief(result.incomplete) };
  }, tags);
}

async function scan(ctx, page, id) {
  const wcag = await axeRun(page, WCAG_TAGS);
  const best = await axeRun(page, ['best-practice']);
  ctx.data.scans.push({ page: id, wcag, bestPractice: best.violations });
  ctx.assert(`${id}: axe finds no WCAG violation`, wcag.violations.length === 0, wcag.violations);
  ctx.assert(`${id}: axe leaves nothing incomplete`, wcag.incomplete.length === 0, wcag.incomplete);
}

// -------------------------------------------------------------- contrast ---

/**
 * What axe does not measure, from the computed styles: each field's edge
 * against the page, its placeholder against the field, each button's text
 * against the button, and the accent (the focus ring and links) against the
 * page. The lowest ratio of each kind.
 */
function contrast(page) {
  return page.evaluate(() => {
    const rgb = (text) => {
      const m = text.match(/rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)/);
      return m ? [m[1], m[2], m[3]].map(Number) : null;
    };
    const lum = (c) => {
      const [r, g, b] = c.map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const ratio = (a, b) => {
      const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
      return (x + 0.05) / (y + 0.05);
    };
    const style = (el, pseudo) => getComputedStyle(el, pseudo);
    const page = rgb(style(document.body).backgroundColor);
    const min = (list) => (list.length ? Math.min(...list) : null);
    const fields = [...document.querySelectorAll('input, select')];
    const accent = rgb(style(document.body).getPropertyValue('--accent').trim().replace(
      /^#(..)(..)(..)$/,
      (_, r, g, b) => `rgb(${parseInt(r, 16)}, ${parseInt(g, 16)}, ${parseInt(b, 16)})`,
    ));
    return {
      fieldEdge: min(fields.map((el) => ratio(rgb(style(el).borderTopColor), page))),
      placeholder: min(
        fields
          .filter((el) => el.placeholder)
          .map((el) => ratio(rgb(style(el, '::placeholder').color), rgb(style(el).backgroundColor))),
      ),
      button: min(
        [...document.querySelectorAll('button')].map((el) =>
          ratio(rgb(style(el).color), rgb(style(el).backgroundColor)),
        ),
      ),
      accent: accent && ratio(accent, page),
      fields: fields.length,
    };
  });
}

function checkContrast(ctx, id, c) {
  ctx.data.scans.at(-1).contrast = c;
  ctx.assert(`${id}: field edges ≥ 3:1`, c.fields > 0 && c.fieldEdge >= 3, c);
  ctx.assert(`${id}: placeholder ≥ 4.5:1`, c.placeholder === null || c.placeholder >= 4.5, c);
  ctx.assert(`${id}: button text ≥ 4.5:1`, c.button >= 4.5, c);
  ctx.assert(`${id}: focus ring and links ≥ 3:1`, c.accent >= 3, c);
}

// ---------------------------------------------------------------- reflow ---

async function horizontalScroll(page) {
  return page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
}

/** Boxes that hide their overflow and have some: text cut off. */
function clippedBoxes(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll('body *')]
      .filter((el) => {
        const s = getComputedStyle(el);
        return (
          /hidden|clip/.test(s.overflowX + s.overflowY) &&
          (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1)
        );
      })
      .map((el) => el.tagName),
  );
}

async function checkReflow(ctx, page, id) {
  const size = page.viewportSize();
  await page.setViewportSize({ width: 320, height: 640 });
  await sleep(50);
  const over = await horizontalScroll(page);
  ctx.assert(`${id}: no horizontal scroll at 320 px`, over <= 0, over);
  // 1.4.12's spacing, at the same width: still no scroll, and no text cut
  // off by a box that hides its overflow.
  const spaced = await page.addStyleTag({ content: TEXT_SPACING });
  await sleep(50);
  const clipped = await clippedBoxes(page);
  const spacedOver = await horizontalScroll(page);
  ctx.assert(`${id}: text spacing at 320 px: no scroll, nothing clipped`, spacedOver <= 0 && clipped.length === 0, {
    spacedOver,
    clipped,
  });
  await spaced.evaluate((el) => el.remove());
  await page.setViewportSize(size);
}

// ------------------------------------------------------------- structure ---

async function checkStructure(ctx, page, id) {
  const s = await page.evaluate(() => {
    const inMain = (sel) => [...document.querySelectorAll(sel)].some((el) => el.closest('main'));
    const form = document.querySelector('.mf2-locale-switcher');
    const ids = [...document.querySelectorAll('[id]')].map((el) => el.id);
    return {
      mains: document.querySelectorAll('main').length,
      landmarksInMain: ['header', 'nav', 'footer'].filter(inMain),
      people: document.querySelector('#people')?.getAttribute('role') ?? null,
      switcherIds: form ? form.querySelectorAll('[id]').length : null,
      duplicateIds: ids.filter((v, i) => ids.indexOf(v) !== i),
    };
  });
  const { assert } = ctx;
  assert(`${id}: one main`, s.mains === 1, s);
  assert(`${id}: banner, navigation and contentinfo outside main`, s.landmarksInMain.length === 0, s);
  if (!id.includes('/lazy')) assert(`${id}: the counter is a status`, s.people === 'status', s);
  assert(`${id}: no id in the switcher`, s.switcherIds === 0, s);
  assert(`${id}: no duplicate id`, s.duplicateIds.length === 0, s);
  // The accessible name is the visible label.
  const label = NORMALISE(await page.textContent('.mf2-locale-switcher label span'));
  const named = await page.getByRole('combobox', { name: label, exact: true }).count();
  assert(`${id}: the switcher is named by its label`, named === 1, { label, named });
}

/** demo-ssr's routes: their own titles, and one label per field. */
async function ssrStructure(ctx, ssr) {
  const { assert } = ctx;
  const context = await ctx.browser.newContext();
  const page = await context.newPage();
  await page.goto(`${ssr}/?lang=en`, { waitUntil: 'load' });
  await ssrReady(page);
  const home = await page.title();
  const labels = await page.$$eval('label.field > span', (spans) => spans.map((s) => s.textContent.trim()));
  assert(
    'demo-ssr: no two fields share a label',
    new Set(labels).size === labels.length,
    labels,
  );
  await page.click('nav a[href="/lazy"]');
  await page.waitForSelector('#lazy-heading');
  await sleep(100);
  const lazy = await page.title();
  assert('demo-ssr: a client navigation sets the route title', lazy !== home && lazy.length > 0, { home, lazy });
  const served = await context.newPage();
  await served.goto(`${ssr}/lazy?lang=en`, { waitUntil: 'load' });
  assert('demo-ssr: /lazy is served with its own title', (await served.title()) === lazy, {
    served: await served.title(),
    navigated: lazy,
  });
  await page.click('nav a[href="/"]');
  await page.waitForSelector('#people');
  await sleep(100);
  assert('demo-ssr: leaving restores the home title', (await page.title()) === home, await page.title());
  await context.close();
}

// -------------------------------------------------------------- switcher ---

/**
 * Focuses the switcher's select and presses ArrowDown: what the keyboard does
 * to a closed select in both engines — change the value and fire `change`.
 * Returns what that did to the page.
 */
async function arrowDown(page, select = '.mf2-locale-switcher select') {
  const before = await page.evaluate((sel) => {
    window.__a11yMarker = true;
    return { lang: document.documentElement.lang, value: document.querySelector(sel).value };
  }, select);
  await page.focus(select);
  await page.keyboard.press('ArrowDown');
  await sleep(500);
  const after = await page.evaluate((sel) => {
    const el = document.querySelector(sel);
    return {
      lang: document.documentElement.lang,
      value: el?.value,
      stayed: window.__a11yMarker === true,
      focused: document.activeElement === el,
    };
  }, select);
  return { before, after };
}

const keyChangedNothing = ({ before, after }) =>
  after.stayed && after.lang === before.lang && after.value !== before.value && after.focused;

/** Tab from the select to the button, and Enter: the keyboard's switch. */
async function tabAndEnter(page) {
  await page.keyboard.press('Tab');
  const onButton = await page.evaluate(
    () => document.activeElement?.matches('.mf2-locale-switcher button[type="submit"]') ?? false,
  );
  await page.keyboard.press('Enter');
  return onButton;
}

/** `hydrate` and `csr`: the button switches in place, focus kept. */
async function switcherLive(ctx, name, url, ready) {
  const { assert } = ctx;
  const context = await ctx.browser.newContext();
  if (name === 'demo-csr') await context.addInitScript(() => localStorage.setItem('mf2_locale', 'en'));
  const page = await context.newPage();
  await page.goto(url, { waitUntil: 'load' });
  await ready(page);
  const text = NORMALISE(await page.textContent('main'));
  const key = await arrowDown(page);
  assert(`${name}: an arrow key changes the select and nothing else`, keyChangedNothing(key), key);
  assert(`${name}: … not the text`, NORMALISE(await page.textContent('main')) === text);
  const onButton = await tabAndEnter(page);
  assert(`${name}: Tab reaches the button`, onButton);
  await until(page, () => document.documentElement.lang === 'fr', 5000).catch(() => {});
  const after = await page.evaluate(() => ({
    lang: document.documentElement.lang,
    stayed: window.__a11yMarker === true,
    focus: document.activeElement?.matches('.mf2-locale-switcher button') ?? false,
  }));
  assert(`${name}: the button switches, in place, focus kept`, after.lang === 'fr' && after.stayed && after.focus, after);
  await context.close();
}

/** demo-islands: the switcher is no island; the button is the form's GET. */
async function switcherIslands(ctx, islands) {
  const { assert } = ctx;
  const context = await ctx.browser.newContext();
  const page = await context.newPage();
  await page.goto(`${islands}/?lang=en`, { waitUntil: 'load' });
  await islandsReady(page);
  const key = await arrowDown(page);
  assert('demo-islands: an arrow key changes the select and nothing else — no reload', keyChangedNothing(key), key);
  const onButton = await tabAndEnter(page);
  assert('demo-islands: Tab reaches the button', onButton);
  await page.waitForURL(/[?&]lang=fr\b/, { timeout: 10000 }).catch(() => {});
  await islandsReady(page);
  const cookie = (await context.cookies(islands)).find((c) => c.name === 'mf2_locale');
  assert('demo-islands: the button navigates to ?lang=fr, the server remembers it', (await lang(page)) === 'fr' && cookie?.value === 'fr', {
    url: page.url(),
    lang: await lang(page),
    cookie: cookie?.value,
  });
  await context.close();
}

/** demo-ssr with no client code at all: the form is still the switch. */
async function switcherWithoutWasm(ctx, ssr) {
  const { assert } = ctx;
  const context = await ctx.browser.newContext();
  await context.route('**/*.wasm', (route) => route.abort());
  const page = await context.newPage();
  await page.goto(`${ssr}/?lang=en`, { waitUntil: 'load' });
  await sleep(300);
  await chooseLocale(page, 'ar');
  await page.waitForURL(/[?&]lang=ar\b/, { timeout: 10000 }).catch(() => {});
  await page.waitForLoadState('load');
  assert('demo-ssr, wasm blocked: the button navigates into the choice', (await lang(page)) === 'ar' && (await page.getAttribute('html', 'dir')) === 'rtl', {
    url: page.url(),
    lang: await lang(page),
  });
  const selected = await page.$eval('.mf2-locale-switcher select', (el) => el.value);
  assert('demo-ssr, wasm blocked: the served select shows the page locale', selected === 'ar', selected);
  await context.close();
}

// ------------------------------------------------------ negative controls ---

async function controls(ctx, ssr) {
  const { assert } = ctx;
  const context = await ctx.browser.newContext();
  const page = await context.newPage();
  await page.goto(`${ssr}/?lang=en`, { waitUntil: 'load' });
  await ssrReady(page);

  // The scan runs.
  await page.evaluate(() => {
    document.documentElement.removeAttribute('lang');
    document.body.style.color = '#ddd';
  });
  const bad = await axeRun(page, WCAG_TAGS);
  const ids = bad.violations.map((v) => v.id);
  assert('control: the scan reports a missing lang and grey text', ids.includes('html-has-lang') && ids.includes('color-contrast'), ids);
  await page.evaluate(() => {
    document.documentElement.setAttribute('lang', 'en');
    document.body.style.color = '';
  });

  // The contrast measure sees what axe does not.
  await page.addStyleTag({
    content: 'input, select { border-color: #d3d7df !important } ::placeholder { color: #aaa !important }',
  });
  const pale = await contrast(page);
  assert('control: the old border and a pale placeholder fail the measure', pale.fieldEdge < 3 && pale.placeholder < 4.5, pale);

  // Reflow.
  await page.evaluate(() => {
    const wide = document.createElement('div');
    wide.style.width = '400px';
    wide.style.height = '1px';
    document.querySelector('main').append(wide);
  });
  await page.setViewportSize({ width: 320, height: 640 });
  await sleep(50);
  assert('control: a 400 px box scrolls at 320 px', (await horizontalScroll(page)) > 0);
  await page.evaluate(() => {
    const box = document.createElement('p');
    box.style.cssText = 'overflow: hidden; height: 1.2em';
    box.textContent = 'A sentence long enough to wrap onto a second line at this width.';
    document.querySelector('main').append(box);
  });
  assert('control: a box that hides its overflow is found clipped', (await clippedBoxes(page)).length > 0);

  // F37: a select that switches on `change`, as the switcher used to.
  await page.evaluate(() => {
    const select = document.createElement('select');
    select.id = 'a11y-control';
    select.innerHTML = '<option value="en">en</option><option value="fr">fr</option>';
    select.addEventListener('change', () => {
      document.documentElement.lang = select.value;
    });
    document.querySelector('main').prepend(select);
  });
  const f37 = await arrowDown(page, '#a11y-control');
  assert('control: a select that switches on change fails the arrow-key probe', !keyChangedNothing(f37), f37);
  await context.close();
}

// --------------------------------------------------------------- waiting ---

async function lang(page) {
  return page.evaluate(() => document.documentElement.lang);
}

async function ssrReady(page) {
  await until(page, async () => {
    try {
      return (await import('/pkg/demo_ssr.js')).mf2_live_nodes() > 0;
    } catch {
      return false;
    }
  });
  await sleep(50);
}

async function islandsReady(page) {
  await until(page, async () => {
    try {
      return (await import('/pkg/demo_islands.js')).mf2_live_nodes() > 0;
    } catch {
      return false;
    }
  });
  await sleep(50);
}

async function csrReady(page) {
  await until(page, () => (window.wasmBindings?.mf2_live_nodes() ?? 0) > 0);
  await sleep(50);
}
