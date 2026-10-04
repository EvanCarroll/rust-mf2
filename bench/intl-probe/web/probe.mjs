// The probe page's entry: `window.mf2probe.<item>(options)` for the
// Playwright check (tools/e2e/checks/intl.mjs), which serves the repository
// root so that this page, target/intl-probe/pkg/ and target/intl-probe/data/
// are reachable.

import { edge } from './lib/edge.mjs';
import { ecmaHandlers, ecmaRaw } from './lib/ecma.mjs';
import { floor } from './lib/floor.mjs';
import { l4 } from './lib/l4.mjs';
import { engine } from './lib/load.mjs';
import { loc } from './lib/loc.mjs';
import { names } from './lib/names.mjs';
import { plural } from './lib/plural.mjs';
import { speed } from './lib/speed.mjs';

const BASE = new URL('../../../target/intl-probe/', import.meta.url);

const env = {
  async json(name) { return (await fetch(new URL(`data/${name}`, BASE))).json(); },
  async text(name) { return (await fetch(new URL(`data/${name}`, BASE))).text(); },
  async bytes(name) { return new Uint8Array(await (await fetch(new URL(`data/${name}`, BASE))).arrayBuffer()); },
  async module(v) {
    const m = await import(new URL(`pkg/${v}/probe.js`, BASE).href);
    await m.default({ module_or_path: new URL(`pkg/${v}/probe_bg.wasm`, BASE) });
    return m;
  },
};

window.mf2probe = {
  engine,
  floor: async () => floor(),
  l4: async ({ variant = 'intl-cu' } = {}) => l4(env, variant),
  plural: async () => plural(env),
  ecmaRaw: async () => ecmaRaw(env),
  ecmaHandlers: async () => ecmaHandlers(env),
  edge: async () => edge(env),
  loc: async () => loc(env),
  speed: async (o) => speed(env, o),
  names: async (o) => names(env, o),
};
document.getElementById('status').textContent = `ready: ${engine()}`;
