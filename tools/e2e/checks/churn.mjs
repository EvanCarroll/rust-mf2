// Phase 7 A5 (plans/15-phase-7-work-order.md): P0.11's churning list on
// mf2's Leptos layer itself — what each row shape costs the heap when rows
// are created and dropped by the hundred thousand between two locale
// switches.
//
// The harness is `bench/churn` (its lib.rs says what each variant is);
// `cargo xtask churn` builds it into target/churn/site/, which this check
// serves itself as a static host would (--base-url is not used).
//
// Per variant, on a fresh page:
//
//   * 2,000 live rows mounted: the heap and allocations per row (data);
//   * a warm-up of 10,000 churned rows (the registry's slab and the
//     allocator's free lists grow once), then 100,000 more, 50 rows per
//     round under a round owner, 500 per task: the live heap sampled every
//     10,000 — asserted **flat**: at most FLAT bytes over the 100,000;
//   * the registry back where it started;
//   * the locale trigger fired alone (the switch's share that grows with dead
//     subscribers): its time, and the heap it releases (data);
//   * a live switch to `fr` and back: every live row — text, title — follows
//     both ways, so a conversion still subscribes after the churn; except
//     the `oco` rows, a value, which keep the text they were built with (the
//     control: it is why the documentation steers to `TextProp`);
//   * a silent console.
//
//   cargo xtask churn                      # build, then this, both engines
//   node run.mjs churn --browser chromium  # after a build

import { createServer } from 'node:http';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

import { watchConsole } from '../lib/browser.mjs';

const SITE = fileURLToPath(new URL('../../../target/churn/site/', import.meta.url));
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.mf2b': 'application/octet-stream',
};

const P = { live: 2000, warmup: 10000, churn: 100000, rows: 50, batch: 500, samples: 10 };

// P0.11's registry grew +64 KiB once (slab growth) and then stayed flat; the
// warm-up takes that. What is left over 100,000 rows must be less than a
// byte a row: a leak of even one small allocation per row is 100× this.
const FLAT = 65536;

// What a live row reads in each locale, by variant: the text's start, and the
// `title`'s for the one variant that has one.
const EXPECT = {
  en: { text: 'A row', args: 'Row', title: 'More about' },
  fr: { text: 'Une ligne', args: 'Ligne', title: 'En savoir' },
};

function serve() {
  const server = createServer((req, res) => {
    let path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '');
    if (path === '') path = 'index.html';
    const file = normalize(join(SITE, path));
    if (!file.startsWith(SITE) || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404).end('not found');
      return;
    }
    res.writeHead(200, {
      'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream',
      'Cache-Control': 'no-cache',
    });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

export async function run(ctx) {
  const { assert, data } = ctx;
  for (const need of ['index.html', 'i18n/index.json', 'pkg/churn_harness.js']) {
    if (!existsSync(join(SITE, need))) {
      throw new Error(`target/churn/site/${need} missing: run \`cargo xtask churn\``);
    }
  }
  const server = await serve();
  const base = `http://127.0.0.1:${server.address().port}`;
  data.params = { ...P, flat: FLAT };
  data.variants = {};
  try {
    const variants = await listVariants(ctx, base);
    for (const variant of variants) {
      data.variants[variant] = await measure(ctx, base, variant);
    }
  } finally {
    server.close();
  }
  const f = (x) => (Math.round(x * 100) / 100).toString();
  for (const [variant, r] of Object.entries(data.variants)) {
    ctx.log(
      `${variant.padEnd(9)} live ${f(r.perLiveRow)} B/row (${f(r.allocsPerLiveRow)} allocs); ` +
        `churn ${P.churn} in ${f(r.churnMs)} ms: +${r.grown} B (${f(r.grown / P.churn)} B/row) ` +
        `[${r.samples.map(([n, b]) => `${n / 1000}k:${b}`).join(' ')}]; ` +
        `notify ${f(r.notifyMs)} ms, releases ${r.released} B; switch ${f(r.switchMs)} ms`,
    );
  }
}

async function open(ctx, base) {
  const context = await ctx.browser.newContext();
  const page = await context.newPage();
  const console_ = [];
  watchConsole(page, console_);
  await page.goto(`${base}/`, { waitUntil: 'load' });
  const locale = await page.evaluate(async () => {
    const m = await import('./pkg/churn_harness.js');
    await m.default();
    window.harness = m;
    return m.boot();
  });
  return { context, page, console_, locale };
}

async function listVariants(ctx, base) {
  const { context, page } = await open(ctx, base);
  const variants = await page.evaluate(() => window.harness.variants());
  await context.close();
  return variants;
}

async function measure(ctx, base, variant) {
  const { assert } = ctx;
  const { context, page, console_, locale } = await open(ctx, base);
  assert(`${variant}:boots-in-en`, locale === 'en', locale);

  const r = await page.evaluate(
    async ({ P, variant }) => {
      const m = window.harness;
      const nextTask = () =>
        new Promise((r) => {
          const c = new MessageChannel();
          c.port1.onmessage = () => r();
          c.port2.postMessage(0);
        });
      const churn = async (n) => {
        for (let done = 0; done < n; done += P.batch) {
          m.churn(P.batch, P.rows, variant);
          await nextTask();
        }
      };
      await nextTask();
      const h0 = m.heap_live(), a0 = m.heap_allocs(), n0 = m.live_nodes();
      m.mount_live(P.live, variant);
      await nextTask();
      const h1 = m.heap_live(), a1 = m.heap_allocs(), n1 = m.live_nodes();

      await churn(P.warmup);
      const hc0 = m.heap_live();
      const samples = [];
      const step = P.churn / P.samples;
      const t0 = performance.now();
      for (let done = 0; done < P.churn; done += step) {
        await churn(step);
        samples.push([done + step, m.heap_live() - hc0]);
      }
      const churnMs = performance.now() - t0;
      const grown = m.heap_live() - hc0;
      const nodesAfter = m.live_nodes();

      const hn0 = m.heap_live();
      const t1 = performance.now();
      m.notify();
      const notifyMs = performance.now() - t1;
      await nextTask();
      await nextTask();
      const released = hn0 - m.heap_live();

      const t2 = performance.now();
      await m.switch_locale('fr');
      const switchMs = performance.now() - t2;
      await nextTask();
      return {
        perLiveRow: (h1 - h0) / P.live,
        allocsPerLiveRow: (a1 - a0) / P.live,
        nodesBefore: n0,
        nodesLive: n1,
        nodesAfter,
        churnMs,
        samples,
        grown,
        notifyMs,
        released,
        switchMs,
      };
    },
    { P, variant },
  );

  const rows = async () =>
    page.evaluate(() =>
      [...document.querySelectorAll('#live li')].map((li) => [li.textContent, li.getAttribute('title')]),
    );
  const follows = (all, tag) => {
    const want = EXPECT[variant === 'oco' ? 'en' : tag];
    const text = variant === 'args' ? want.args : want.text;
    const bad = all.filter(
      ([t, title]) => !t.startsWith(text) || (variant === 'attr' && !(title ?? '').startsWith(want.title)),
    );
    return { pass: all.length === P.live && bad.length === 0, details: { rows: all.length, bad: bad.slice(0, 3) } };
  };

  assert(`${variant}:heap-flat-under-churn`, r.grown <= FLAT, { grown: r.grown, samples: r.samples });
  assert(`${variant}:registry-back-where-it-was`, r.nodesAfter === r.nodesLive, {
    before: r.nodesBefore,
    live: r.nodesLive,
    after: r.nodesAfter,
  });
  const fr = follows(await rows(), 'fr');
  assert(`${variant}:${variant === 'oco' ? 'live-rows-keep-their-value' : 'live-rows-follow-a-switch'}`, fr.pass, fr.details);
  await page.evaluate(() => window.harness.switch_locale('en'));
  await page.evaluate(() => new Promise((r) => setTimeout(r, 0)));
  const en = follows(await rows(), 'en');
  assert(`${variant}:and-switch-back`, en.pass, en.details);
  assert(`${variant}:console-silent`, console_.length === 0, console_.slice(0, 5));
  await context.close();
  return r;
}
