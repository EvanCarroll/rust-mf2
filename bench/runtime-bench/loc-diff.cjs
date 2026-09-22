// The localized :number / :percent output (mf2-fn-number, CLDR 48.2.1) against
// ECMA-402 Intl.NumberFormat over the locale panel (Phase 4, A3): P0.5's
// corpus, each case in a panel locale with a useGrouping value.
// Usage: cargo run --release -p runtime-bench -- numbers locale 100000 | node bench/runtime-bench/loc-diff.cjs
// node is a local tool (no network); its ICU/CLDR version is printed, and
// differences are classified: identical once U+00A0 and U+202F are taken as
// the same space; identical once the bidi marks U+200E/U+200F/U+061C are
// dropped; the rest listed by locale.
const lines = require('fs').readFileSync(0, 'utf8').trim().split('\n');
const grouping = { auto: 'auto', always: 'always', min2: 'min2', never: false };
const spaces = (s) => s.replace(/[  ]/g, ' ');
const marks = (s) => s.replace(/[‎‏؜]/g, '');
let same = 0, space = 0, bidi = 0, intlThrows = 0, bothErr = 0;
const other = new Map();
const shown = new Map();
for (const l of lines) {
  const c = JSON.parse(l);
  const o = { ...c.o, useGrouping: grouping[c.o.useGrouping] };
  if (c.f === 'percent') {
    o.style = 'percent';
  }
  let intl;
  try {
    intl = new Intl.NumberFormat(c.l, o).format(c.v);
  } catch (e) {
    intlThrows++;
    if (c.errs > 0) bothErr++;
    continue;
  }
  if (c.errs !== 0) {
    other.set(`${c.l} errors`, (other.get(`${c.l} errors`) ?? 0) + 1);
    continue;
  }
  if (intl === c.out) same++;
  else if (spaces(intl) === spaces(c.out)) space++;
  else if (marks(spaces(intl)) === marks(spaces(c.out))) bidi++;
  else {
    const k = `${c.l} ${c.f}`;
    other.set(k, (other.get(k) ?? 0) + 1);
    if ((shown.get(k) ?? 0) < 3) {
      shown.set(k, (shown.get(k) ?? 0) + 1);
      console.log('DIFF', JSON.stringify(c), 'intl=', JSON.stringify(intl));
    }
  }
}
const rest = [...other.values()].reduce((a, b) => a + b, 0);
console.log(`node ${process.version} ICU ${process.versions.icu} (CLDR ${process.versions.cldr}): ` +
  `${same} identical, ${space} identical up to U+00A0/U+202F, ${bidi} up to bidi marks, ${rest} different; ` +
  `${intlThrows} option sets Intl rejects, of which the runtime reported an error for ${bothErr}`);
for (const [k, n] of [...other.entries()].sort()) console.log(`  ${k}: ${n}`);
