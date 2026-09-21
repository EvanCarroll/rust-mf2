// Smoke test of the wasm builds under node: numloc (w-loc-cu) vs Intl glue
// (w-intl-num) on the same inputs. Usage: node scripts/wasm-smoke.cjs
const fs = require('fs');
const loc = require('../out/node/loc-cu/w_loc_cu.js');
const intl = require('../out/node/intl-num/w_intl_num.js');
const cases = [
  ['en', 0, '', '1234567.891'], ['de', 0, 'minimumFractionDigits=2', '1234.5'], ['fr', 0, '', '-9876543.21'],
  ['es', 0, '', '1234'], ['es', 0, 'useGrouping=always', '1234'], ['pl', 0, '', '12345'], ['hi', 0, '', '123456789'],
  ['ar', 0, '', '-1234.5'], ['ar-u-nu-arab', 0, '', '-1234.5'], ['ru', 3, 'maximumFractionDigits=1', '0.12345'],
  ['he', 3, '', '-0.5'], ['ja', 0, 'signDisplay=always', '42'], ['en', 4, 'currency=EUR', '1234.5'],
  ['de', 4, 'currency=USD', '-42'], ['en', 5, 'unit=kilometer unitDisplay=long', '12.5'],
];
for (const [l, f, o, v] of cases) {
  const data = fs.readFileSync(`out/locale/${l.slice(0, 2)}.all-used.bin`);
  const a = loc.fmt(data, l, f, o, v, '1').slice(0, -1);
  const b = f <= 3 ? intl.fmt(new Uint8Array(0), l, f, o, v, '1').slice(0, -1) : '(n/a)';
  console.log(`${a === b ? 'same' : f <= 3 ? 'DIFF' : '    '} ${l} f=${f} ${o} ${v}: wasm-numloc=${JSON.stringify(a)} wasm-intl=${JSON.stringify(b)}`);
}
