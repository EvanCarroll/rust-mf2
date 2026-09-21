// Runs the datetime-intl wasm under node (ICU/CLDR of the node build) on the
// same sample as `cargo run -p check` prints for datetime-icu.
// Usage: node scripts/intl-sample.cjs out/node/intl/w_intl.js
const m = require(require('path').resolve(process.argv[2]));
const cases = [
  [1, ''], [1, 'length=long'], [1, 'length=short'],
  [1, 'fields=month-day-weekday length=long'], [2, ''],
  [2, 'precision=second hour12=false'], [0, ''],
  [0, 'timeZoneStyle=long'], [0, 'timeZoneStyle=short timeZone=+05:30'],
  [0, 'timeZone=America/New_York timeZoneStyle=long'],
  [2, 'timeZone=Asia/Kolkata'],
  [1, 'calendar=japanese length=long'],
  [2, 'timeZone=Mars/Olympus'],
];
const names = ['{$d :datetime', '{$d :date', '{$d :time'];
console.log(`node ${process.version}, ICU ${process.versions.icu}, CLDR ${process.versions.cldr}, tz ${process.versions.tz}\n`);
for (const [f, o] of cases) {
  const row = ['en-US', 'ar', 'ja', 'ru'].map((l) => {
    const r = m.fmt(new Uint8Array(0), l, f, o, '2006-01-02T15:04:06Z');
    const [text, errs] = r.split('\u0001');
    return errs === '0' ? text : `${text}⟨err ${errs}⟩`;
  });
  console.log(`| \`${names[f]} ${o}}\` | ${row.join(' | ')} |`);
}
