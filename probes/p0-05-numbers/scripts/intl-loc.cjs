// numloc's :percent/:currency/:unit (CLDR 48.2.1) vs node Intl.NumberFormat.
// Usage: cargo run -p diff --bin loc_cases | node scripts/intl-loc.cjs
const lines = require('fs').readFileSync(0, 'utf8').trim().split('\n');
const by = {};
let shown = 0;
for (const l of lines) {
  const c = JSON.parse(l);
  const o = { ...c.o };
  if (c.f === 'percent') o.style = 'percent';
  if (c.f === 'currency') o.style = 'currency';
  if (c.f === 'unit') o.style = 'unit';
  const intl = new Intl.NumberFormat(c.loc, o).format(c.v);
  const norm = (s) => s.replace(/ /g, ' ');
  const k = `${c.f}`;
  by[k] ??= { same: 0, ws: 0, diff: 0 };
  if (intl === c.out) by[k].same++;
  else if (norm(intl) === norm(c.out)) by[k].ws++;
  else {
    by[k].diff++;
    if (shown++ < 25) console.log('DIFF', c.loc, c.f, JSON.stringify(c.o), c.v, 'ours=', JSON.stringify(c.out), 'intl=', JSON.stringify(intl));
  }
}
console.log(`node ${process.version}, ICU ${process.versions.icu}, CLDR ${process.versions.cldr}`);
for (const [k, v] of Object.entries(by)) console.log(`${k}: ${v.same} identical, ${v.ws} differ only in U+202F/U+00A0, ${v.diff} different`);
