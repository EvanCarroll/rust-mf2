// Item 2: time per numeric placeholder and per select, `rust` against
// `intl` (neutral output) and, when the build has it, `rust-loc` against
// `intl-loc` (locale symbols), alternated in the same page run (clock drift
// and tiering hit all alike): for each message of `speed.json` and each
// locale (`en`, `pl`), `rounds` samples per variant, the order rotated every
// round,
// each sample one call of `bench_int` / `bench_float` — `iters` formats in a
// loop inside wasm, `NoErrors`, a reused `String`, formatters cached — timed
// with performance.now(). `iters` is calibrated once per message so a
// sample takes about `sampleMs` in the slower variant; both variants run
// the same `iters`. Reported: the median ns per format of each variant, and
// the range.

import { catalogs, variant } from './load.mjs';

/** How each timed message is driven: its argument. */
const DRIVE = {
  plain: { kind: 'float', base: 1234.5678 },
  number: { kind: 'float', base: 1234.5678 },
  'number-frac2': { kind: 'float', base: 1234.5678 },
  'number-sig3': { kind: 'float', base: 1234.5678 },
  integer: { kind: 'int', start: 0 },
  select: { kind: 'int', start: 0 },
  'select-3': { kind: 'int', start: 0 },
  'select-fmt': { kind: 'int', start: 0 },
};

function run(p, cat, id, drive, iters) {
  const t0 = performance.now();
  if (drive.kind === 'int') p.bench_int(cat, id, iters, drive.start, 100);
  else p.bench_float(cat, id, iters, drive.base, 100);
  return performance.now() - t0;
}

const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };

export async function speed(env, { rounds = 9, sampleMs = 60, only } = {}) {
  const variants = ['rust', 'intl'];
  const loaded = {};
  for (const v of variants) loaded[v] = await catalogs(env, await variant(env, v), 'speed');
  try {
    for (const v of ['rust-loc', 'intl-loc']) loaded[v] = await catalogs(env, await variant(env, v), 'speed');
    variants.push('rust-loc', 'intl-loc');
  } catch (e) { /* a build without mf2-fn-number */ }
  const { idx } = loaded.rust;
  const rows = [];
  for (let cat = 0; cat < idx.catalogs.length; cat++) {
    const locale = idx.catalogs[cat].locale;
    for (let id = 0; id < idx.messages.length; id++) {
      const { name, src } = idx.messages[id];
      if (only && !only.includes(name)) continue;
      const drive = DRIVE[name];
      // Warm up and calibrate on the slower variant.
      let per = 0;
      for (const v of variants) {
        run(loaded[v].probe, cat, id, drive, 2000);
        per = Math.max(per, run(loaded[v].probe, cat, id, drive, 2000) / 2000);
      }
      const iters = Math.max(1000, Math.round(sampleMs / Math.max(per, 1e-6)));
      const ns = Object.fromEntries(variants.map((v) => [v, []]));
      for (let r = 0; r < rounds; r++) {
        const k = r % variants.length;
        const order = [...variants.slice(k), ...variants.slice(0, k)];
        for (const v of r % 2 === 0 ? order : order.reverse()) ns[v].push((run(loaded[v].probe, cat, id, drive, iters) * 1e6) / iters);
      }
      const row = { locale, name, src, iters, rounds };
      for (const v of variants) {
        row[v] = { median: median(ns[v]), min: Math.min(...ns[v]), max: Math.max(...ns[v]) };
      }
      row.ratio = row.intl.median / row.rust.median;
      if (row['rust-loc']) row.ratioLoc = row['intl-loc'].median / row['rust-loc'].median;
      rows.push(row);
    }
  }
  // What Intl itself costs, from JS, formatters cached: the floor under
  // any `intl` design (the rest of its time is the wasm → JS crossing, the
  // key, the strings in and out, and the boxed value).
  const js = [];
  for (const locale of ['en', 'pl']) {
    const nf = new Intl.NumberFormat('en', { useGrouping: false, numberingSystem: 'latn', minimumFractionDigits: 0, maximumFractionDigits: 0 });
    const pr = new Intl.PluralRules(locale, { minimumFractionDigits: 0, maximumFractionDigits: 0 });
    const texts = Array.from({ length: 100 }, (_, i) => String(i));
    const cases = {
      'NumberFormat.format(decimal string)': (i) => nf.format(texts[i % 100]),
      'NumberFormat.format(number)': (i) => nf.format(i % 100),
      'PluralRules.select(number)': (i) => pr.select(i % 100),
    };
    for (const [name, fn] of Object.entries(cases)) {
      let sink = 0;
      const iters = 200000;
      for (let i = 0; i < 20000; i++) sink += fn(i).length;
      const ns = [];
      for (let r = 0; r < rounds; r++) {
        const t0 = performance.now();
        for (let i = 0; i < iters; i++) sink += fn(i).length;
        ns.push(((performance.now() - t0) * 1e6) / iters);
      }
      js.push({ locale, name, median: median(ns), min: Math.min(...ns), max: Math.max(...ns), sink: sink > 0 });
    }
  }
  const crossOriginIsolated = typeof globalThis.crossOriginIsolated === 'boolean' ? globalThis.crossOriginIsolated : null;
  return { variants, rows, js, crossOriginIsolated };
}
