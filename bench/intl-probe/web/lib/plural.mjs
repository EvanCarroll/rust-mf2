// Item 4: the 15,041 CLDR 48.2.1 plural samples (cardinal and ordinal,
// `intl-probe-native plural`) through the engine's Intl.PluralRules against
// our evaluator (`mf2_runtime::plural_category`, which agrees with CLDR on
// every sample).
//
// Per sample: minimum = maximum fraction digits = the digits the sample
// writes (so `1.0` and `1` differ, as the plural operands v/w/f/t
// require). A sample with a compact exponent (`1.2c6`) is the value
// 1,200,000 with operand e = 6; it goes through `notation: 'compact'`
// (fraction digits: the mantissa's) when the engine supports that option
// on PluralRules, and is classified apart either way.
//
// A disagreement is classified, first match wins:
//   engine-lacks-locale  the engine resolved another locale (no data for it)
//   category-set-differs the engine's pluralCategories for the locale are not
//                        CLDR 48.2.1's (the engine's CLDR version drifts)
//   compact-exponent     a `c`/`e` sample (compact notation or its absence)
//   js-number-precision  the sample does not survive conversion to a JS number
//   rule-differs         the same categories, a different rule: the engine's
//                        CLDR version (or its engine) disagrees on the rule itself

import { Tally } from './classify.mjs';

const SAMPLE = /^(-?)(\d+)(?:\.(\d+))?(?:[ce](\d+))?$/;

export async function plural(env) {
  const { samples, cldr } = await env.json('plural.json');
  let compactOk = false;
  try {
    compactOk = new Intl.PluralRules('fr', { notation: 'compact' }).resolvedOptions().notation === 'compact';
  } catch (e) { /* unsupported */ }
  // CLDR's category set per kind + locale, from the samples.
  const cats = new Map();
  for (const [kind, locale, , cat] of samples) {
    const k = `${kind}|${locale}`;
    if (!cats.has(k)) cats.set(k, new Set());
    cats.get(k).add(cat);
  }
  const rules = new Map();
  const pr = (kind, locale, fd, compact) => {
    const k = `${kind}|${locale}|${fd}|${compact}`;
    let r = rules.get(k);
    if (r === undefined) {
      const o = { type: kind, minimumFractionDigits: fd, maximumFractionDigits: fd };
      if (compact) o.notation = 'compact';
      try { r = new Intl.PluralRules(locale, o); } catch (e) { r = null; }
      rules.set(k, r);
    }
    return r;
  };
  const result = { cldr, compactSupported: compactOk, kinds: {} };
  const tallies = {};
  const perLocale = {};
  for (const [kind, locale, sample, cldrCat, ours] of samples) {
    const res = (result.kinds[kind] ??= { samples: 0, agree: 0, differ: 0, oursDiffersFromCldr: 0 });
    const tally = (tallies[kind] ??= new Tally(12));
    res.samples++;
    if (ours !== cldrCat) res.oursDiffersFromCldr++;
    const m = SAMPLE.exec(sample);
    if (!m) { res.differ++; tally.add('unparsed', { locale, sample }); continue; }
    const [, sign, int, frac = '', exp] = m;
    const fd = frac.length;
    const compact = exp !== undefined;
    const value = Number(`${sign}${int}${frac ? `.${frac}` : ''}${compact ? `e${exp}` : ''}`);
    const r = pr(kind, locale, fd, compact && compactOk);
    const got = r ? r.select(value) : 'rejected';
    if (got === ours) { res.agree++; continue; }
    res.differ++;
    let cls;
    const resolved = r ? r.resolvedOptions() : null;
    const lang = locale.split('-')[0];
    const engineSet = resolved ? [...resolved.pluralCategories].sort().join(',') : '';
    const cldrSet = [...cats.get(`${kind}|${locale}`)].sort().join(',');
    if (!resolved || resolved.locale.split('-')[0] !== lang) cls = 'engine-lacks-locale';
    else if (engineSet !== cldrSet) cls = 'category-set-differs';
    else if (compact) cls = 'compact-exponent';
    else if (`${int}${frac}`.replace(/^0+/, '').length > 15) cls = 'js-number-precision';
    else cls = 'rule-differs';
    tally.add(cls, { locale, sample, ours, engine: got });
    const pl = (perLocale[`${kind} ${locale}`] ??= { differ: 0, cls, engineLocale: resolved?.locale, engineCategories: engineSet, cldrCategories: cldrSet });
    pl.differ++;
  }
  for (const [kind, t] of Object.entries(tallies)) result.kinds[kind].differences = t;
  result.perLocale = perLocale;
  return result;
}
