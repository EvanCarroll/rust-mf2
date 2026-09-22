// Item 3, locale symbols: the panel's cases (`intl-probe-native loc`: 11
// locales × 14 values × 24 annotations — :number with grouping and digit
// options, :integer, :percent, :currency, :unit — plus a cardinal and an
// ordinal selection whose output is the category) formatted by the
// `intl-cu` variant in the engine. The output lines up with
// `intl-probe-native loc-format`'s `loc-rust.json` (the Rust registry), so
// any two outputs compare case by case (`compareLoc`).

import { Tally, classify } from './classify.mjs';
import { catalogs, engine, records, variant } from './load.mjs';

export async function loc(env) {
  const { idx, probe } = await catalogs(env, await variant(env, 'intl-cu'), 'loc');
  const out = [];
  for (let cat = 0; cat < idx.catalogs.length; cat++) out.push(...records(probe.format_all(cat, true)));
  if (out.length !== idx.cases.length) throw new Error(`loc: ${out.length} outputs for ${idx.cases.length} cases`);
  return { engine: engine(), variant: 'intl-cu', out };
}

/** The family a case belongs to, for the report. */
export function family(fn) {
  if (fn.startsWith('select')) return fn;
  if (fn.startsWith(':number')) return fn.includes('useGrouping') ? ':number useGrouping' : ':number';
  return fn.split(' ')[0];
}

/**
 * Compares two outputs of the loc cases (`{engine, out}` objects): per
 * family, same / different, the differences classified (`classify.mjs`;
 * `errors` when the error lists differ, `plural category` for the select
 * families), per locale.
 */
export function compareLoc(cases, a, b) {
  const families = {};
  for (let i = 0; i < cases.length; i++) {
    const c = cases[i];
    const fam = family(c.fn);
    const f = (families[fam] ??= { cases: 0, same: 0, tally: new Tally(6), locales: {} });
    f.cases++;
    const [at, ae] = a.out[i];
    const [bt, be] = b.out[i];
    if (at === bt && ae === be) { f.same++; continue; }
    let cls;
    if (ae !== be) cls = `errors (${ae || 'none'} | ${be || 'none'})`;
    else if (fam.startsWith('select')) cls = 'plural category';
    else cls = classify(at, bt);
    f.tally.add(cls, { locale: c.locale, src: c.src, a: at, b: bt });
    const l = (f.locales[c.locale] ??= {});
    l[cls] = (l[cls] ?? 0) + 1;
  }
  return { a: a.engine, b: b.engine, families };
}
