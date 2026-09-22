#!/usr/bin/env node
// The probe's items in node (V8 + node's own ICU), the reference engine
// P0.5 and Phase 3 used — the same libraries the page runs.
//
//   node bench/intl-probe/web/node.mjs <item> [--variant V] [--out FILE]
//
// Items: floor, l4, plural, ecmaRaw, ecmaHandlers, edge, loc, speed. Output: the
// item's JSON result (to FILE, or stdout).

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

import { edge } from './lib/edge.mjs';
import { ecmaHandlers, ecmaRaw } from './lib/ecma.mjs';
import { floor } from './lib/floor.mjs';
import { l4 } from './lib/l4.mjs';
import { loc } from './lib/loc.mjs';
import { plural } from './lib/plural.mjs';
import { speed } from './lib/speed.mjs';

const BASE = join(dirname(fileURLToPath(import.meta.url)), '../../../target/intl-probe');
const env = {
  json: async (name) => JSON.parse(readFileSync(join(BASE, 'data', name), 'utf8')),
  text: async (name) => readFileSync(join(BASE, 'data', name), 'utf8'),
  bytes: async (name) => new Uint8Array(readFileSync(join(BASE, 'data', name))),
  async module(v) {
    const m = await import(pathToFileURL(join(BASE, 'pkg', v, 'probe.js')).href);
    m.initSync({ module: readFileSync(join(BASE, 'pkg', v, 'probe_bg.wasm')) });
    return m;
  },
};

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: { variant: { type: 'string', default: 'intl-cu' }, out: { type: 'string' } },
});
const items = {
  floor: () => floor(),
  l4: () => l4(env, values.variant),
  plural: () => plural(env),
  ecmaRaw: () => ecmaRaw(env),
  ecmaHandlers: () => ecmaHandlers(env),
  edge: () => edge(env),
  loc: () => loc(env),
  speed: () => speed(env, {}),
};
const [item] = positionals;
if (!items[item]) {
  console.error(`usage: node node.mjs <${Object.keys(items).join('|')}> [--variant V] [--out FILE]`);
  process.exit(2);
}
const result = await items[item]();
const engine = `node ${process.version} (ICU ${process.versions.icu}, CLDR ${process.versions.cldr})`;
// `loc` is an engine output for compareLoc: `{engine, out}`, as the check writes it.
const text = item === 'loc'
  ? `${JSON.stringify({ engine, out: result.out })}\n`
  : `${JSON.stringify({ engine, item, result }, null, 1)}\n`;
if (values.out) writeFileSync(values.out, text);
else process.stdout.write(text);
