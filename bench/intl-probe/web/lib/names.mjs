// The number split (plan/08 §6, task 18.4): `:currency` and `:unit` with
// their names from Intl.NumberFormat and their digits, rounding and plural
// selection in Rust (`rt-names-cu`), against the Rust path (`rust-cu`), on
// the locale-symbol panel (`intl-probe-native loc`: 11 locales × 14 values ×
// the 8 :currency and 6 :unit annotations).
//
// Speed: per locale and annotation, the ns per placeholder of each variant
// (`bench_none` over the annotation's 14 messages, in wasm), the variants
// alternated in one page run, the order reversed every round; median, min,
// max over `rounds`. Text: every case of both variants against the Rust
// registry (`loc-rust.json`), compared as `loc` compares (`compareLoc`), the
// :currency and :unit families kept; `rust-cu` against it is the harness's
// self-check (it must be identical).

import { compareLoc } from './loc.mjs';
import { catalogs, engine, records, variant } from './load.mjs';

const VARIANTS = ['rust-cu', 'rt-names-cu'];
const FAMILIES = [':currency', ':unit'];

const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };

function time(probe, cat, ids, iters) {
  const t0 = performance.now();
  for (const id of ids) probe.bench_none(cat, id, iters);
  return performance.now() - t0;
}

export async function names(env, { rounds = 7, sampleMs = 20 } = {}) {
  const loaded = {};
  for (const v of VARIANTS) loaded[v] = await catalogs(env, await variant(env, v), 'loc');
  const { idx } = loaded['rust-cu'];
  // The timed groups: one per locale and annotation.
  const groups = new Map();
  for (const c of idx.cases) {
    if (!FAMILIES.some((f) => c.fn.startsWith(f))) continue;
    const key = `${c.locale}\t${c.fn}`;
    let g = groups.get(key);
    if (!g) groups.set(key, (g = { locale: c.locale, fn: c.fn, cat: c.cat, ids: [] }));
    g.ids.push(c.id);
  }
  const rows = [];
  for (const g of groups.values()) {
    // Warm up and calibrate on the slower variant.
    let per = 0;
    for (const v of VARIANTS) {
      time(loaded[v].probe, g.cat, g.ids, 200);
      per = Math.max(per, time(loaded[v].probe, g.cat, g.ids, 200) / (200 * g.ids.length));
    }
    const iters = Math.max(50, Math.round(sampleMs / Math.max(per, 1e-6) / g.ids.length));
    const ns = Object.fromEntries(VARIANTS.map((v) => [v, []]));
    for (let r = 0; r < rounds; r++) {
      const order = r % 2 === 0 ? VARIANTS : [...VARIANTS].reverse();
      for (const v of order) ns[v].push((time(loaded[v].probe, g.cat, g.ids, iters) * 1e6) / (iters * g.ids.length));
    }
    const row = { locale: g.locale, fn: g.fn, placeholders: g.ids.length, iters, rounds };
    for (const v of VARIANTS) row[v] = { median: median(ns[v]), min: Math.min(...ns[v]), max: Math.max(...ns[v]) };
    row.ratio = row['rt-names-cu'].median / row['rust-cu'].median;
    rows.push(row);
  }
  // The text, against the Rust registry.
  const rust = await env.json('loc-rust.json');
  const text = {};
  for (const v of VARIANTS) {
    const out = [];
    for (let cat = 0; cat < idx.catalogs.length; cat++) out.push(...records(loaded[v].probe.format_all(cat, true)));
    if (out.length !== idx.cases.length) throw new Error(`names: ${v} gave ${out.length} outputs for ${idx.cases.length} cases`);
    const cmp = compareLoc(idx.cases, { engine: rust.engine, out: rust.out }, { engine: `${engine()} ${v}`, out });
    text[v] = Object.fromEntries(Object.entries(cmp.families)
      .filter(([f]) => FAMILIES.includes(f))
      .map(([f, x]) => [f, { cases: x.cases, same: x.same, tally: x.tally.toJSON(), locales: x.locales }]));
  }
  return { engine: engine(), variants: VARIANTS, rows, text };
}
