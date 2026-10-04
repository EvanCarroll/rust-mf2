// Phase 8 A7: dates in the reader's time zone.
//
// Against `examples/demo-ssr`, running at --base-url, whose home page shows
// one instant (2026-01-01T00:00Z) through `:datetime`; and
// `examples/demo-csr/dist/`, which this check serves itself (`trunk build`
// there first). Each engine, each reader zone (Playwright's `timezoneId`):
//
//   * a first visit, with no cookie, is served in UTC and states no zone;
//     after hydration the date is the reader's — the same text the server
//     renders for that zone — and it is the only text the correction
//     changed (a `MutationObserver` from before the first byte, against
//     what hydration writes anyway: the control's writes); no `mf2:`
//     message; the `mf2_tz` cookie holds the browser's name for the zone
//     (Chromium says `Asia/Calcutta` for `Asia/Kolkata`);
//   * a reload is served in the reader's zone, states it on the preload
//     link, and hydration writes nothing more than it always does;
//   * the negative control: a browser that reports UTC as its zone gets no
//     correction, so the first visit's date stays in UTC — the first-visit
//     assertion, run on it, fails;
//   * a client-only page mounts in the reader's zone: its date shows the
//     time of day the browser's own `Intl.DateTimeFormat` gives for that
//     zone (7:00 PM in New York, 5:30 AM in Kolkata; 12:00 AM in UTC), and
//     it writes no cookie.
//
//   node run.mjs zone --base-url http://127.0.0.1:3702 --browser chromium,firefox

import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { watchConsole, until, sleep } from '../lib/browser.mjs';
import { serveStatic } from '../lib/static.mjs';

const ZONES = ['America/New_York', 'Asia/Kolkata'];
const INSTANT = 1_767_225_600_000;
const CSR_DIST = fileURLToPath(new URL('../../../examples/demo-csr/dist/', import.meta.url));

/** Text as a reader sees it: no bidi controls, spaces collapsed. */
const NORMALISE = (text) =>
  text
    .replace(/[⁦-⁩‎‏]/g, '')
    .replace(/[\s  ]+/g, ' ')
    .trim();

/** `#published`'s text in served HTML. */
function servedDate(html) {
  const m = html.match(/<p id="published">([\s\S]*?)<\/p>/);
  if (!m) return undefined;
  const text = m[1]
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<[^>]+>/g, '')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&#x27;|&#39;/g, "'")
    .replace(/&quot;/g, '"');
  return NORMALISE(text);
}

/** The preload link's `data-mf2-zone`, if the page states one. */
function statedZone(html) {
  const link = html.match(/<link[^>]*\bdata-mf2\b[^>]*>/i)?.[0] ?? '';
  return link.match(/data-mf2-zone="([^"]*)"/)?.[1];
}

/**
 * Records every text and attribute change from before the first byte, by
 * the id of the nearest element that has one: what the client wrote after
 * the parser, which only hydration and the correction do.
 */
function observeWrites() {
  window.__mf2Writes = [];
  const where = (node) => {
    let el = node.nodeType === 1 ? node : node.parentElement;
    while (el && !el.id) el = el.parentElement;
    return el ? el.id : el === null ? '(none)' : '?';
  };
  new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === 'characterData') {
        window.__mf2Writes.push({ kind: 'text', at: where(r.target), value: r.target.data });
      } else if (r.type === 'attributes' && document.readyState !== 'loading') {
        window.__mf2Writes.push({ kind: `@${r.attributeName}`, at: where(r.target) });
      }
    }
  }).observe(document, { subtree: true, characterData: true, attributes: true });
}

/** Makes the browser report UTC as its zone, whatever it is in. */
function reportUtc() {
  const resolved = Intl.DateTimeFormat.prototype.resolvedOptions;
  Intl.DateTimeFormat.prototype.resolvedOptions = function () {
    return { ...resolved.call(this), timeZone: 'UTC' };
  };
}

async function hydrated(page) {
  await until(page, async () => {
    try {
      return (await import('/pkg/demo_ssr.js')).mf2_live_nodes() > 0;
    } catch {
      return false;
    }
  });
  // Anything hydration scheduled, and the correction after it, has run.
  await sleep(300);
}

async function zoneCookie(context) {
  return (await context.cookies()).find((c) => c.name === 'mf2_tz')?.value;
}

/** Writes as comparable strings. */
const keys = (writes) => writes.map((w) => `${w.kind} ${w.at}`);

/** `writes` less one of each write in `baseline`: what the correction added. */
function beyond(writes, baseline) {
  const left = keys(baseline);
  return keys(writes).filter((k) => {
    const at = left.indexOf(k);
    if (at === -1) return true;
    left.splice(at, 1);
    return false;
  });
}

/** A first visit to `/` in `zone`, and what it left behind. */
async function firstVisit(browser, baseUrl, zone, { control = false } = {}) {
  const context = await browser.newContext({ timezoneId: zone, locale: 'en-US' });
  await context.addInitScript(observeWrites);
  if (control) await context.addInitScript(reportUtc);
  const page = await context.newPage();
  const consoleMessages = [];
  watchConsole(page, consoleMessages);
  const url = `${baseUrl}/?lang=en`;
  const served = await (await context.request.get(url)).text();
  await page.goto(url, { waitUntil: 'load' });
  await hydrated(page);
  const result = {
    served: servedDate(served),
    stated: statedZone(served),
    shown: NORMALISE(await page.locator('#published').innerText()),
    writes: await page.evaluate(() => window.__mf2Writes),
    cookie: await zoneCookie(context),
    browserZone: await page.evaluate(() => new Intl.DateTimeFormat().resolvedOptions().timeZone),
    mf2: consoleMessages.filter((m) => m.text.startsWith('mf2:')).map((m) => m.text),
  };
  return { context, page, url, result };
}

/** What the server renders for a reader in `zone`: the expected date. */
async function serverRendersIn(context, url, zone) {
  const html = await (await context.request.get(url, { headers: { cookie: `mf2_tz=${zone}` } })).text();
  return { date: servedDate(html), stated: statedZone(html) };
}

/**
 * The first-visit assertion, as a verdict, so the control can run it too:
 * the date is the zone's, and beyond what hydration always writes
 * (`baseline`) the only write is the date's text.
 */
function corrected(first, expected, baseline) {
  const added = beyond(first.writes, baseline);
  return (
    first.served !== undefined &&
    first.shown === expected &&
    first.shown !== first.served &&
    added.length === 1 &&
    added[0] === 'text published'
  );
}

export async function run(ctx) {
  const { browser, baseUrl, assert, data } = ctx;
  data.zone = {};

  for (const zone of ZONES) {
    const record = (data.zone[zone] = {});

    // ---- the negative control first: what hydration writes anyway ----
    const control = await firstVisit(browser, baseUrl, zone, { control: true });
    record.control = control.result;
    const baseline = control.result.writes;
    await control.context.close();

    // ---- a first visit: served in UTC, corrected after hydration ----
    const first = await firstVisit(browser, baseUrl, zone);
    const f = first.result;
    const name = f.browserZone;
    const expected = await serverRendersIn(first.context, first.url, name);
    record.first = f;
    record.expected = expected;
    assert(`${zone}: first visit served in UTC, stating no zone`, f.stated === undefined && f.served !== undefined, f);
    assert(`${zone}: the server renders another text for the zone`, expected.date !== f.served && expected.stated === name, expected);
    assert(
      `${zone}: corrected after hydration to the zone's text, and only the date written`,
      corrected(f, expected.date, baseline),
      { first: f, expected: expected.date, added: beyond(f.writes, baseline) },
    );
    assert(`${zone}: no mf2 message`, f.mf2.length === 0, f.mf2);
    assert(`${zone}: the cookie names the zone`, f.cookie === name, { cookie: f.cookie, browser: name });

    // ---- a reload: served in the zone, nothing changes ----
    const html = await (await first.context.request.get(first.url)).text();
    await first.page.reload({ waitUntil: 'load' });
    await hydrated(first.page);
    const reload = {
      served: servedDate(html),
      stated: statedZone(html),
      shown: NORMALISE(await first.page.locator('#published').innerText()),
      writes: await first.page.evaluate(() => window.__mf2Writes),
    };
    record.reload = reload;
    assert(`${zone}: a reload is served in the zone and states it`, reload.served === expected.date && reload.stated === name, reload);
    assert(
      `${zone}: nothing changes after hydration on a reload`,
      reload.shown === expected.date && beyond(reload.writes, baseline).length === 0,
      { reload, added: beyond(reload.writes, baseline) },
    );
    await first.context.close();

    // ---- the control's verdict: the first-visit assertion fails on it ----
    const c = control.result;
    assert(
      `${zone}: control — with the correction not triggered, the first-visit assertion fails`,
      !corrected(c, expected.date, baseline) && c.shown === c.served && c.cookie === undefined,
      c,
    );
  }

  // ---- client-only: mounts in the reader's zone ----
  if (!existsSync(join(CSR_DIST, 'index.html'))) {
    assert('csr: examples/demo-csr/dist is built', false, 'run `trunk build` in examples/demo-csr');
    return;
  }
  const server = await serveStatic(CSR_DIST);
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    const shownIn = async (zone) => {
      const context = await browser.newContext({ timezoneId: zone, locale: 'en-US' });
      const page = await context.newPage();
      const consoleMessages = [];
      watchConsole(page, consoleMessages);
      await page.goto(`${base}/`, { waitUntil: 'load' });
      await until(page, () => document.querySelector('#published')?.textContent.length > 0);
      const shown = NORMALISE(await page.locator('#published').innerText());
      // The time of day alone: the wording around it is the backend's.
      const time = NORMALISE(
        await page.evaluate(
          (t) => new Intl.DateTimeFormat('en', { hour: 'numeric', minute: '2-digit' }).format(new Date(t)),
          INSTANT,
        ),
      );
      const cookie = await zoneCookie(context);
      await context.close();
      return { shown, time, cookie, mf2: consoleMessages.filter((m) => m.text.startsWith('mf2:')).map((m) => m.text) };
    };
    const utc = await shownIn('UTC');
    data.zone.csr = { UTC: utc };
    for (const zone of ZONES) {
      const r = await shownIn(zone);
      data.zone.csr[zone] = r;
      assert(
        `csr ${zone}: mounts in the reader's zone`,
        r.shown.includes(r.time) && !r.shown.includes(utc.time) && utc.shown.includes(utc.time),
        { r, utc },
      );
      assert(`csr ${zone}: no cookie and no mf2 message`, r.cookie === undefined && r.mf2.length === 0, r);
    }
  } finally {
    server.close();
  }
}
