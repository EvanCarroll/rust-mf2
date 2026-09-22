// `:integer`'s rounding, `:offset`'s arithmetic and exact-key selection
// (`intl-probe-native edge`), formatted by the `rust` and the `intl`
// variants: every message whose output or errors differ is listed.

import { catalogs, records, variant } from './load.mjs';

export async function edge(env) {
  const rust = await catalogs(env, await variant(env, 'rust'), 'edge');
  const intl = await catalogs(env, await variant(env, 'intl'), 'edge');
  const r = records(rust.probe.format_all(0, true));
  const x = records(intl.probe.format_all(0, true));
  const differ = [];
  r.forEach(([rt, re], i) => {
    const [xt, xe] = x[i];
    if (rt !== xt || re !== xe) differ.push({ src: rust.idx.sources[i], rust: rt, rustErrors: re, intl: xt, intlErrors: xe });
  });
  return { cases: r.length, same: r.length - differ.length, differ };
}
