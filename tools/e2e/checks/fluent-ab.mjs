// Phase 8 A5: the `leptos-fluent` A/B in
// the browser — the reference application on `leptos-fluent` and the same
// application migrated to mf2 (bench/fluent-ab/README.md).
//
// `cargo xtask fluent-ab` builds both, starts both servers and runs this
// check with their addresses in the environment (--base-url is not used):
//
//   FLUENT_AB_FLUENT_URL, FLUENT_AB_MF2_URL   the two servers
//   FLUENT_AB_FLUENT_JS, FLUENT_AB_MF2_JS     each one's wasm-bindgen module
//   FLUENT_AB_RUNS                            first visits per side
//   FLUENT_AB_IDS                             the messages the hooks use
//
// 1. **The same text first**, so that neither side is measured with its
//    translations optimized away: every route in `en` and `pl` — the whole
//    hydrated `<main>` of each, not a sample — must be equal on both sides,
//    bidi isolation marks aside (both isolate; where is an approved
//    difference, owner question 5). A negative control shows the comparison
//    sees a difference: the `pl` page of one side against the `en` page of
//    the other.
// 2. **Then the timings**, the two sides alternately, the order swapped each
//    run, each run a fresh first visit (a new context: an empty cache):
//    * `/`: from the wasm's load, and from the navigation's start, to the
//      first frame after hydration (`performance.mark('ab-hydrated')`, set
//      by the hooks in an animation frame after the application hydrated);
//    * `/ab` (no route: the router's fallback, so nothing else on the page
//      is translated): 2,000 live translated text nodes mounted; formats
//      to a `String` — a simple message, one with one argument, a plural
//      select — 100,000 at a time (Firefox's clock ticks in whole
//      milliseconds); a switch `en` → `pl` and back, timed until every
//      one of the 2,000 nodes shows the new text (a MutationObserver
//      checks after each batch of DOM changes). mf2's switch fetches the
//      catalog: it is preloaded first, and one switch there and back warms
//      both sides before the timed one.

import { readFileSync } from 'node:fs';

import { sleep, until, watchConsole } from '../lib/browser.mjs';

const env = process.env;
const SIDES = {
  fluent: { url: env.FLUENT_AB_FLUENT_URL, js: env.FLUENT_AB_FLUENT_JS },
  mf2: { url: env.FLUENT_AB_MF2_URL, js: env.FLUENT_AB_MF2_JS },
};
const RUNS = Number(env.FLUENT_AB_RUNS ?? 10);
const ROUTES = ['/', '/r1', '/r2', '/r3'];
const NODES = 2000;
const ITERS = 100000;
const FORMAT_REPS = 5;
// U+061C, U+200E, U+200F, U+2066–U+2069: the marks an isolating formatter
// adds around a placeable.
const BIDI = /[؜‎‏⁦-⁩]/g;

async function open(browser, side, locale, path) {
  const context = await browser.newContext({ locale });
  const { hostname } = new URL(SIDES[side].url);
  // Each side's own language cookie, as its switch would have written it.
  await context.addCookies([
    { name: 'lang', value: locale, domain: hostname, path: '/' },
    { name: 'mf2_locale', value: locale, domain: hostname, path: '/' },
  ]);
  const page = await context.newPage();
  const console = [];
  watchConsole(page, console, `${side} ${locale} ${path}`);
  await page.goto(SIDES[side].url + path);
  await until(page, () => performance.getEntriesByName('ab-hydrated').length > 0);
  return { context, page, console };
}

const mainText = (page) =>
  page.evaluate(() => (document.querySelector('main')?.textContent ?? '').replace(/\s+/g, ' ').trim());

function stats(values) {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  if (v.length === 0) return null;
  const mid = Math.floor(v.length / 2);
  const median = v.length % 2 ? v[mid] : (v[mid - 1] + v[mid]) / 2;
  return { median, min: v[0], max: v[v.length - 1], n: v.length, values };
}

export async function run(ctx) {
  const { browser, assert, data, log } = ctx;
  const ids = JSON.parse(readFileSync(env.FLUENT_AB_IDS, 'utf8'));
  data.ids = ids;
  const consoles = [];

  // 1. The same text.
  let characters = 0;
  let equal = true;
  const pages = {};
  for (const locale of ['en', 'pl']) {
    for (const route of ROUTES) {
      const text = {};
      for (const side of Object.keys(SIDES)) {
        const { context, page, console } = await open(browser, side, locale, route);
        text[side] = (await mainText(page)).replace(BIDI, '');
        const lang = await page.evaluate(() => document.documentElement.lang);
        assert(`${side}-${locale}${route}-in-${locale}`, lang === locale, { lang });
        consoles.push(...console);
        await context.close();
      }
      pages[`${locale}${route}`] = text;
      const same = text.fluent === text.mf2 && text.fluent.length > 0;
      if (!same) {
        const at = [...text.fluent].findIndex((c, i) => c !== text.mf2[i]);
        assert(`same-text-${locale}${route}`, false, {
          at,
          fluent: text.fluent.slice(Math.max(0, at - 60), at + 60),
          mf2: text.mf2.slice(Math.max(0, at - 60), at + 60),
        });
        equal = false;
      } else {
        assert(`same-text-${locale}${route}`, true, `${text.fluent.length} characters`);
      }
      characters += text.fluent.length;
    }
  }
  // The control: the comparison does see a different locale.
  assert('negative-control-en-vs-pl-differ', pages['en/'].fluent !== pages['pl/'].mf2);
  data.sameText = { equal, characters, routes: ROUTES, locales: ['en', 'pl'] };

  // 2. The timings.
  const samples = { fluent: {}, mf2: {} };
  const push = (side, key, value) => (samples[side][key] ??= []).push(value);
  for (let i = 0; i < RUNS; i += 1) {
    const order = i % 2 === 0 ? ['fluent', 'mf2'] : ['mf2', 'fluent'];
    for (const side of order) {
      // The first translated frame, on the home route.
      {
        const { context, page, console } = await open(browser, side, 'en', '/');
        const t = await page.evaluate((js) => {
          const mark = performance.getEntriesByName('ab-hydrated')[0].startTime;
          const wasm = performance.getEntriesByType('resource').find((e) => e.name.endsWith(js.replace(/\.js$/, '.wasm')));
          const catalog = performance.getEntriesByType('resource').find((e) => e.name.includes('/i18n/'));
          return {
            mark,
            wasmEnd: wasm?.responseEnd,
            wasmBytes: wasm?.encodedBodySize,
            catalogBytes: catalog?.encodedBodySize,
          };
        }, SIDES[side].js);
        push(side, 'hydratedFromNavigation', t.mark);
        push(side, 'hydratedFromWasm', t.mark - t.wasmEnd);
        if (i === 0) data[`${side}Transfer`] = t;
        consoles.push(...console);
        await context.close();
      }
      // The live nodes, the formats and the switch, on a page with nothing
      // else translated.
      {
        const { context, page, console } = await open(browser, side, 'en', '/ab');
        const r = await page.evaluate(
          async ({ js, nodes, iters, reps }) => {
            const m = await import(js);
            const frame = () => new Promise((res) => requestAnimationFrame(() => res()));
            let t0 = performance.now();
            m.ab_mount(nodes);
            const mount = performance.now() - t0;
            await frame();
            const spans = [...document.querySelectorAll('#ab-nodes > span')];
            const out = { mount, count: spans.length, format: {} };
            for (const kind of [0, 1, 2]) {
              m.ab_format(kind, 1000);
              const times = [];
              for (let k = 0; k < reps; k += 1) {
                t0 = performance.now();
                const bytes = m.ab_format(kind, iters);
                times.push(((performance.now() - t0) * 1000) / iters);
                if (bytes === 0) return { error: `ab_format(${kind}) formatted nothing` };
              }
              times.sort((a, b) => a - b);
              out.format[kind] = times[Math.floor(times.length / 2)];
            }
            // Resolves when every node's text differs from what it was.
            const switchTo = (tag) =>
              new Promise((resolve) => {
                const before = spans.map((s) => s.textContent);
                const done = () => spans.every((s, k) => s.textContent !== before[k]);
                const start = performance.now();
                const observer = new MutationObserver(() => {
                  if (done()) {
                    observer.disconnect();
                    clearTimeout(timer);
                    resolve(performance.now() - start);
                  }
                });
                const timer = setTimeout(() => {
                  observer.disconnect();
                  resolve(null);
                }, 10000);
                observer.observe(document.getElementById('ab-nodes'), {
                  subtree: true,
                  childList: true,
                  characterData: true,
                });
                if (!m.ab_switch(tag)) {
                  observer.disconnect();
                  clearTimeout(timer);
                  resolve(null);
                }
              });
            m.ab_preload('pl');
            m.ab_preload('en');
            await new Promise((res) => setTimeout(res, 300));
            out.warm = [await switchTo('pl'), await switchTo('en')];
            await frame();
            out.switch = await switchTo('pl');
            await frame();
            out.switchBack = await switchTo('en');
            out.text = spans[0].textContent;
            return out;
          },
          { js: SIDES[side].js, nodes: NODES, iters: ITERS, reps: FORMAT_REPS },
        );
        if (r.error) {
          assert(`${side}-hooks`, false, r.error);
        } else {
          if (i === 0) {
            assert(`${side}-mounted-${NODES}-nodes`, r.count === NODES, { count: r.count });
            assert(`${side}-switch-reaches-every-node`, r.switch !== null && r.switchBack !== null && !r.warm.includes(null), r);
          }
          push(side, 'mount', r.mount);
          push(side, 'format0', r.format[0]);
          push(side, 'format1', r.format[1]);
          push(side, 'format2', r.format[2]);
          push(side, 'switch', r.switch ?? NaN);
          push(side, 'switchBack', r.switchBack ?? NaN);
        }
        consoles.push(...console);
        await context.close();
      }
      await sleep(100);
    }
    log(`run ${i + 1}/${RUNS}`);
  }
  data.timings = {
    fluent: Object.fromEntries(Object.entries(samples.fluent).map(([k, v]) => [k, stats(v)])),
    mf2: Object.fromEntries(Object.entries(samples.mf2).map(([k, v]) => [k, stats(v)])),
  };
  data.console = consoles;
  const errors = consoles.filter((c) => c.type === 'pageerror' || /panicked|RuntimeError/.test(c.text));
  assert('no-page-error', errors.length === 0, errors.slice(0, 5));
}
