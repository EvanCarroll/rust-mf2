// The number split's report (scripts/7-names.sh): reads what that script
// produced under target/intl-probe/ — size.tsv, data/names-data.tsv and the
// `names` item of results/<browser>.json — and writes one Markdown file.
//
//   node names-report.mjs --command C --tree T --machine M --started S --out FILE

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const OUT = fileURLToPath(new URL('../../../target/intl-probe/', import.meta.url));
const args = {};
for (let i = 2; i < process.argv.length; i += 2) args[process.argv[i].replace(/^--/, '')] = process.argv[i + 1];

const tsv = (file) => {
  const [head, ...lines] = readFileSync(file, 'utf8').trim().split('\n');
  const keys = head.split('\t');
  return lines.map((l) => Object.fromEntries(l.split('\t').map((v, i) => [keys[i], i === 0 ? v : Number(v)])));
};
const fmt = (n) => (Number.isFinite(n) ? Math.round(n).toLocaleString('en-US') : 'n/a');
const signed = (n) => (n > 0 ? `+${fmt(n)}` : fmt(n));
const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };

const lines = [];
const out = (s = '') => lines.push(s);

out('# The number split against the Rust path — results');
out();
out(`Command: \`${args.command}\`, started ${args.started}. Built from ${args.tree}. Machine: ${args.machine}.`);
out('Written by the command; not edited by hand. `rt-names-cu`: `:currency` and `:unit` take their names from');
out('`Intl.NumberFormat`, the digits, rounding and plural selection stay in Rust (`mf2-fn-number/intl-names`,');
out('`mf2-host-web/intl-names`). `rust-cu`: the Rust path. `rt-intl-cu`: the whole `intl` option, for reference.');
out();

// 1. The client's bytes.
out('## 1. The client: wasm + JS glue, gzip -9, over `base`');
out();
try {
  const rows = tsv(join(OUT, 'size.tsv'));
  const by = Object.fromEntries(rows.map((r) => [r.variant, r]));
  const total = (v) => by[v] && by.base ? (by[v].wasm_gz - by.base.wasm_gz) + (by[v].js_gz - by.base.js_gz) : NaN;
  out('| Variant | wasm gz | JS gz | over `base` | against `rust-cu` |');
  out('|---|---:|---:|---:|---:|');
  for (const v of ['rust-cu', 'rt-names-cu', 'rt-intl-cu']) {
    if (!by[v]) continue;
    out(`| \`${v}\` | ${fmt(by[v].wasm_gz)} | ${fmt(by[v].js_gz)} | ${signed(total(v))} | ${v === 'rust-cu' ? '' : signed(total(v) - total('rust-cu'))} |`);
  }
} catch (e) {
  out(`Not measured: ${e.message}`);
}
out();

// 2. The catalog's bytes.
out('## 2. A catalog per language: the panel\'s `:currency` and `:unit` messages, brotli (q 11, window 22)');
out();
try {
  const rows = tsv(join(OUT, 'data', 'names-data.tsv'));
  out('| Locale | `currency.data` raw B | `unit.data` raw B | catalog br, Rust path | the split | saved |');
  out('|---|---:|---:|---:|---:|---:|');
  for (const r of rows) out(`| ${r.locale} | ${fmt(r.currency_raw)} | ${fmt(r.unit_raw)} | ${fmt(r.br_with)} | ${fmt(r.br_without)} | ${signed(r.br_without - r.br_with)} |`);
} catch (e) {
  out(`Not measured: ${e.message}`);
}
out();

// 3 and 4, per engine.
const engines = [];
for (const b of ['chromium', 'firefox', 'webkit']) {
  try {
    const r = JSON.parse(readFileSync(join(OUT, 'results', `${b}.json`), 'utf8'));
    if (r.items?.names?.rows) engines.push({ name: `${r.browser} ${r.version}`, names: r.items.names });
  } catch (e) { /* not run */ }
}
out('## 3. Time per placeholder (ns, medians; the two variants alternated in one page)');
out();
if (engines.length === 0) out('Not measured: no engine ran the `names` item.');
else {
  out('| Engine | family | `rust-cu` ns, median of rows | `rt-names-cu` ns | ratio, range over rows |');
  out('|---|---|---:|---:|---|');
  for (const e of engines) {
    for (const fam of [':currency', ':unit']) {
      const rows = e.names.rows.filter((r) => r.fn.startsWith(fam));
      if (rows.length === 0) continue;
      const ratios = rows.map((r) => r.ratio);
      out(`| ${e.name} | \`${fam}\` | ${fmt(median(rows.map((r) => r['rust-cu'].median)))} | ${fmt(median(rows.map((r) => r['rt-names-cu'].median)))} | ${Math.min(...ratios).toFixed(2)}–${Math.max(...ratios).toFixed(2)}× |`);
    }
  }
  out();
  out('Per locale and annotation (median ns, `rust-cu` → `rt-names-cu`):');
  out();
  for (const e of engines) {
    out(`<details><summary>${e.name}</summary>`);
    out();
    out('| Locale | annotation | `rust-cu` | `rt-names-cu` | ratio |');
    out('|---|---|---:|---:|---:|');
    for (const r of e.names.rows) out(`| ${r.locale} | \`${r.fn}\` | ${fmt(r['rust-cu'].median)} | ${fmt(r['rt-names-cu'].median)} | ${r.ratio.toFixed(2)}× |`);
    out();
    out('</details>');
    out();
  }
}
out();
out('## 4. Text against the Rust registry (`loc-rust.json`)');
out();
if (engines.length === 0) out('Not measured: no engine ran the `names` item.');
else {
  out('| Engine | family | `rt-names-cu` same / cases | `rust-cu` same / cases (self-check) |');
  out('|---|---|---:|---:|');
  for (const e of engines) {
    for (const fam of [':currency', ':unit']) {
      const s = e.names.text['rt-names-cu']?.[fam];
      const c = e.names.text['rust-cu']?.[fam];
      if (!s) continue;
      out(`| ${e.name} | \`${fam}\` | ${s.same} / ${s.cases} | ${c ? `${c.same} / ${c.cases}` : 'n/a'} |`);
    }
  }
  out();
  for (const e of engines) {
    out(`### ${e.name}: where \`rt-names-cu\` differs`);
    out();
    for (const fam of [':currency', ':unit']) {
      const s = e.names.text['rt-names-cu']?.[fam];
      if (!s || s.same === s.cases) continue;
      out(`**\`${fam}\`** — by locale: ${Object.entries(s.locales).map(([l, c]) => `${l} ${Object.values(c).reduce((a, b) => a + b, 0)}`).join(', ')}.`);
      out();
      out('| Class | cases | examples (locale, source: Rust → split) |');
      out('|---|---:|---|');
      for (const [cls, n] of Object.entries(s.tally.counts)) {
        const ex = (s.tally.examples[cls] ?? []).slice(0, 3)
          .map((x) => `${x.locale} \`${x.src}\`: \`${JSON.stringify(x.a)}\` → \`${JSON.stringify(x.b)}\``).join('<br>');
        out(`| ${cls} | ${n} | ${ex} |`);
      }
      out();
    }
  }
}

writeFileSync(args.out, `${lines.join('\n')}\n`);
