#!/usr/bin/env node
// Summarizes target/intl-probe/ (size.md, results/<engine>.json from the
// Playwright check, results/node-<item>.json from web/node.mjs, the loc
// outputs) as Markdown — the tables of RESULTS.md.
//
//   node bench/intl-probe/scripts/report.mjs [item …]   # default: every item with results
//
// Items: size, speed, ecma, edge, loc, plural, floor, l4. Writes
// target/intl-probe/report.md and prints it.

import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { compareLoc } from '../web/lib/loc.mjs';

const OUT = join(dirname(fileURLToPath(import.meta.url)), '../../../target/intl-probe');
const R = (f) => join(OUT, 'results', f);
const read = (f) => (existsSync(f) ? JSON.parse(readFileSync(f, 'utf8')) : null);

// Engines: the browsers the check ran, then node.
const engines = [];
for (const b of ['chromium', 'firefox', 'webkit']) {
  const r = read(R(`${b}.json`));
  if (r) engines.push({ name: `${b} ${r.version}`, items: r.items, mhz: r.mhz });
}
const nodeItems = {};
let nodeName = null;
for (const item of ['floor', 'l4', 'plural', 'ecmaRaw', 'ecmaHandlers', 'edge', 'speed']) {
  const r = read(R(`node-${item}.json`));
  if (r) { nodeItems[item] = r.result; nodeName = r.engine; }
}
if (nodeName) engines.push({ name: nodeName, items: nodeItems });

const lines = [];
const out = (s = '') => lines.push(s);
const n0 = (x) => (x === undefined || x === null ? '—' : Math.round(x).toLocaleString('en-US'));
const table = (head, rows) => {
  out(`| ${head.join(' | ')} |`);
  out(`|${head.map((h, i) => (i === 0 ? '---' : '---:')).join('|')}|`);
  for (const r of rows) out(`| ${r.map((c) => String(c).replaceAll('|', '\\|')).join(' | ')} |`);
  out();
};
const counts = (c) => Object.entries(c ?? {}).map(([k, v]) => `${k} ${v}`).join('; ') || '—';

function size() {
  out('## 1. Size');
  out();
  const f = join(OUT, 'size.md');
  out(existsSync(f) ? readFileSync(f, 'utf8') : '(run scripts/build.sh)');
}

function speed() {
  out('## 2. Speed (ns per format; `rust` and `intl` alternated in each run)');
  out();
  // Every kept run (results/speed-<browser>-<stamp>.json), else the last.
  const files = existsSync(join(OUT, 'results'))
    ? readdirSync(join(OUT, 'results')).filter((f) => /^speed-.*\.json$/.test(f)).sort()
    : [];
  const runs = files.map((f) => read(R(f)));
  if (!runs.length) {
    for (const e of engines) if (e.items.speed) runs.push({ engine: e.name, speed: e.items.speed });
  }
  const byEngine = {};
  for (const r of runs) {
    for (const [mode, run] of Object.entries(r.speed)) {
      const k = `${r.engine}, ${mode}`;
      (byEngine[k] ??= []).push(typeof run === 'string' ? run : { ...run, load: r.load });
    }
  }
  const range = (xs) => (xs.length === 1 ? n0(xs[0]) : `${n0(Math.min(...xs))}–${n0(Math.max(...xs))}`);
  for (const [k, list] of Object.entries(byEngine)) {
    if (typeof list[0] === 'string') { out(`**${k}**: ${list[0]}`); out(); continue; }
    const mhz = list.flatMap((r) => r.mhz ?? []).map((m) => Math.round(m));
    const load = list.map((r) => r.load?.[0]).filter((x) => x !== undefined).map((x) => x.toFixed(1));
    out(`**${k}** — ${list.length} run(s); medians of 9 rounds each, the range over runs; CPU MHz sampled ${Math.min(...mhz)}–${Math.max(...mhz)}${load.length ? `; 1-min load ${load.join(', ')}` : ''}`);
    out();
    const vs = list[0].variants ?? ['rust', 'intl'];
    const rows = list[0].rows.map((row, i) => {
      const cells = vs.map((v) => range(list.map((r) => r.rows[i][v].median)));
      const ratio = list.map((r) => r.rows[i].ratio);
      const ratioLoc = list.map((r) => r.rows[i].ratioLoc).filter((x) => x !== undefined);
      const rr = (xs) => (xs.length === 1 ? xs[0].toFixed(2) : `${Math.min(...xs).toFixed(2)}–${Math.max(...xs).toFixed(2)}`);
      const diff = range(list.map((r) => r.rows[i].intl.median - r.rows[i].rust.median));
      const ratioRt = list.map((r) => r.rows[i].ratioRt).filter((x) => x !== undefined);
      const ratioRtLoc = list.map((r) => r.rows[i].ratioRtLoc).filter((x) => x !== undefined);
      const diffRt = vs.includes('rt-intl') ? [range(list.map((r) => r.rows[i]['rt-intl'].median - r.rows[i].rust.median))] : [];
      return [`${row.locale} · \`${row.name}\``, ...cells, diff, rr(ratio), ...(ratioLoc.length ? [rr(ratioLoc)] : []),
        ...diffRt, ...(ratioRt.length ? [rr(ratioRt)] : []), ...(ratioRtLoc.length ? [rr(ratioRtLoc)] : [])];
    });
    const locHead = vs.includes('rust-loc') ? ['intl-loc / rust-loc'] : [];
    const rtHead = [
      ...(vs.includes('rt-intl') ? ['rt-intl − rust ns', 'rt-intl / rust'] : []),
      ...(vs.includes('rt-intl-loc') ? ['rt-intl-loc / rust-loc'] : []),
    ];
    table(['locale · message', ...vs.map((v) => `${v} ns`), 'intl − rust ns', 'intl / rust', ...locHead, ...rtHead], rows);
    if (list[0].js?.length) {
      table(['Intl alone, from JS (formatter cached)', 'ns'], list[0].js.map((j, i) => [`${j.locale} · ${j.name}`, range(list.map((r) => r.js[i].median))]));
    }
  }
}

function ecma() {
  out('## 3a. Neutral agreement: P0.5\'s 100,000 cases, Rust output vs Intl.NumberFormat (ecma-diff options)');
  out();
  table(['engine', 'identical', 'different', 'differences by class', 'Intl rejects', 'of which Rust: Bad Option'],
    engines.filter((e) => e.items.ecmaRaw).map((e) => {
      const r = e.items.ecmaRaw;
      return [e.name, n0(r.same), n0(r.different), counts(r.differences.counts), n0(r.rejected), n0(r.rejectedBadOption)];
    }));
  out('## 3b. The same 100,000 messages: `rust` vs `intl` handlers in the engine');
  out();
  table(['engine', 'cases', 'same text and errors', 'different', 'by class', '`rust` in wasm = native'],
    engines.filter((e) => e.items.ecmaHandlers).map((e) => {
      const r = e.items.ecmaHandlers;
      return [e.name, n0(r.cases), n0(r.same), n0(r.different), counts(r.differences.counts), `${n0(r.rustWasmEqualsNative)}/${n0(r.cases)}`];
    }));
}

function edge() {
  out('## 3c. Edge cases (`:integer` rounding, `:offset` arithmetic, exact keys): `rust` vs `intl`');
  out();
  for (const e of engines.filter((x) => x.items.edge)) {
    const r = e.items.edge;
    out(`**${e.name}**: ${r.same}/${r.cases} identical.`);
    out();
    if (r.differ.length) table(['message', '`rust`', '`intl`'], r.differ.map((d) => [`\`${d.src}\``, `${d.rust} ${d.rustErrors}`, `${d.intl} ${d.intlErrors}`]));
  }
}

function loc() {
  out('## 3d. Locale symbols: the panel\'s cases (`intl-cu`), engine vs engine');
  out();
  const idx = read(join(OUT, 'data', 'loc.json'));
  if (!idx) return;
  const outs = [];
  for (const b of ['chromium', 'firefox', 'webkit', 'node']) {
    const r = read(R(`loc-${b}.json`));
    if (r) outs.push(r);
  }
  const rust = read(join(OUT, 'data', 'loc-rust.json'));
  const pairs = [];
  for (let i = 0; i < outs.length; i++) for (let j = i + 1; j < outs.length; j++) pairs.push([outs[i], outs[j]]);
  if (rust) for (const o of outs) pairs.push([rust, o]);
  for (const [a, b] of pairs) {
    const c = compareLoc(idx.cases, a, b);
    out(`**${c.a}** vs **${c.b}**`);
    out();
    table(['family', 'cases', 'same', 'differences by class', 'locales'],
      Object.entries(c.families).map(([f, v]) => [f, v.cases, v.same, counts(v.tally.counts), Object.keys(v.locales).join(' ') || '—']));
    const ex = Object.entries(c.families).flatMap(([f, v]) => Object.entries(v.tally.examples).map(([k, l]) => [f, k, l[0]]));
    if (ex.length && !c.a.startsWith('rust')) {
      table(['family · class', 'example', 'a', 'b'], ex.map(([f, k, x]) => [`${f} · ${k}`, `${x.locale} \`${x.src}\``, JSON.stringify(x.a), JSON.stringify(x.b)]));
    }
  }
}

function plural() {
  out('## 4. Plural: the 15,041 CLDR 48.2.1 samples through Intl.PluralRules vs our evaluator');
  out();
  const rows = [];
  for (const e of engines.filter((x) => x.items.plural)) {
    const p = e.items.plural;
    for (const [k, v] of Object.entries(p.kinds)) {
      rows.push([`${e.name} · ${k}`, n0(v.samples), n0(v.agree), n0(v.differ), counts(v.differences?.counts), p.compactSupported ? 'yes' : 'no']);
    }
  }
  table(['engine · kind', 'samples', 'agree', 'differ', 'differences by class', 'notation: compact'], rows);
  for (const e of engines.filter((x) => x.items.plural)) {
    const pl = Object.entries(e.items.plural.perLocale ?? {});
    if (!pl.length) continue;
    out(`**${e.name}**, by locale: ${pl.map(([k, v]) => `${k} ${v.differ} (${v.cls}${v.cls === 'engine-lacks-locale' ? `, → ${v.engineLocale}` : ''})`).join('; ')}.`);
    out();
  }
}

function floor() {
  out('## 5. Browser floor (feature detection)');
  out();
  const es = engines.filter((e) => e.items.floor);
  if (!es.length) return;
  const keys = Object.keys(es[0].items.floor.features);
  table(['feature', ...es.map((e) => e.name)], keys.map((k) => [k, ...es.map((e) => (e.items.floor.features[k] ? 'yes' : '**no**'))]));
  out(`Sanctioned units (\`Intl.supportedValuesOf('unit')\`): ${es.map((e) => `${e.name} ${e.items.floor.sanctionedUnits}`).join('; ')}.`);
  out();
}

function l4() {
  out('## 6. The L4 number files in each engine');
  out();
  const rows = [];
  for (const e of engines.filter((x) => x.items.l4)) {
    const variants = e.items.l4.intl ? e.items.l4 : { intl: e.items.l4 };
    for (const [v, r] of Object.entries(variants)) {
      rows.push([`${e.name} · \`${r.variant}\``, `${r.pass}/${r.total}`, ...['number', 'integer', 'offset', 'percent', 'currency'].map((f) => `${r.byFile[f].pass}/${r.byFile[f].tests}`)]);
    }
  }
  table(['engine · variant', 'all', 'number', 'integer', 'offset', 'percent', 'currency'], rows);
  for (const e of engines.filter((x) => x.items.l4)) {
    const r = e.items.l4.intl ?? e.items.l4;
    if (r.failures.length) {
      out(`**${e.name}**, \`${r.variant}\` failures:`);
      out();
      table(['test', 'source', 'problem'], r.failures.map((f) => [f.test, `\`${f.src}\``, f.problems.join('; ')]));
    }
  }
}

const ITEMS = { size, speed, ecma, edge, loc, plural, floor, l4 };
const want = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(ITEMS);
out(`Engines: ${engines.map((e) => e.name).join('; ') || 'none'}.`);
out();
for (const w of want) ITEMS[w]?.();
writeFileSync(join(OUT, 'report.md'), `${lines.join('\n')}\n`);
process.stdout.write(`${lines.join('\n')}\n`);
