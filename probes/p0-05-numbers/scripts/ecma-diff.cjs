// Compare numcore's neutral output with ECMA-402 Intl.NumberFormat('en',
// {useGrouping:false, ...}) — the spec MF2's digit options were taken from.
// Usage: cargo run -p suite --bin ecma_cases -- 20000 | node scripts/ecma-diff.cjs
const lines = require('fs').readFileSync(0, 'utf8').trim().split('\n');
let same = 0, diff = 0, intlThrows = 0, bothErr = 0, shown = 0;
for (const l of lines) {
  const c = JSON.parse(l);
  let intl;
  try {
    intl = new Intl.NumberFormat('en', { useGrouping: false, ...c.o }).format(c.v);
  } catch (e) {
    intlThrows++;
    if (c.errs > 0) bothErr++;
    continue;
  }
  if (intl === c.out && c.errs === 0) same++;
  else {
    diff++;
    if (shown++ < 15) console.log('DIFF', JSON.stringify(c), 'intl=', JSON.stringify(intl));
  }
}
console.log(`node ${process.version} ICU ${process.versions.icu}: ${same} identical, ${diff} different; ` +
  `${intlThrows} option sets Intl rejects (RangeError), of which numcore reported Bad Option for ${bothErr}`);
