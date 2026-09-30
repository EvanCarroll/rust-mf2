// Phase 10 A7: the shipped client of a demo, measured as plans/phase-7-results.md
// measured demo-ssr's (gzip -9 and brotli q11 through Node's zlib).
// Usage: node measure.mjs <pkg-or-dist dir> [label]
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { gzipSync, brotliCompressSync, constants } from 'node:zlib';

const dir = process.argv[2];
const label = process.argv[3] ?? '';
const files = readdirSync(dir, { recursive: true })
  .filter((f) => /\.(wasm|js)$/.test(f) && statSync(join(dir, f)).isFile())
  .sort();
let total = { raw: 0, gz: 0, br: 0 };
console.log(`| ${label} file | raw | gz | br | sha256 (12) |`);
console.log('|---|---:|---:|---:|---|');
for (const f of files) {
  const bytes = readFileSync(join(dir, f));
  const gz = gzipSync(bytes, { level: 9 }).length;
  const br = brotliCompressSync(bytes, {
    params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: bytes.length },
  }).length;
  const sha = createHash('sha256').update(bytes).digest('hex').slice(0, 12);
  total = { raw: total.raw + bytes.length, gz: total.gz + gz, br: total.br + br };
  console.log(`| ${f} | ${bytes.length} | ${gz} | ${br} | ${sha} |`);
}
console.log(`| **total** | ${total.raw} | ${total.gz} | ${total.br} | |`);
