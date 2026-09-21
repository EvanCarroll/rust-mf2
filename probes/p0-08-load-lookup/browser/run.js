// P0.8 in-page timing. Query: ?variant=eager|per-access
const q = new URLSearchParams(location.search);
const variant = q.get('variant') || 'eager';
const mod = await import(`./pkg-${variant}/p08_web.js`);
await mod.default({ module_or_path: `./pkg-${variant}/p08_web_bg.wasm` });

const fixtures = await (await fetch('data/fixtures.json')).json();
const bytesOf = async (loc) => new Uint8Array(await (await fetch(`data/${loc}.mf2b`)).arrayBuffer());
const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
const now = () => performance.now();

// Timer resolution actually observed.
let res = Infinity;
for (let i = 0, t = now(); i < 200000 && res > 0.0005; i++) { const u = now(); if (u > t) { res = Math.min(res, u - t); t = u; } }

const out = { variant, crossOriginIsolated: self.crossOriginIsolated, timerResolutionMs: res, locales: {} };
const all = {};
for (const loc of Object.keys(fixtures)) all[loc] = await bytesOf(loc);

// First call: an invalid 64-byte buffer (rejected at the magic check) — pays
// V8's lazy compilation of the glue and `Catalog::new`, not the load itself.
{
  const t0 = now();
  mod.load(new Uint8Array(64), 0, 0);
  out.firstCallMs = now() - t0;
}
// Cold: the first real install, split into its two steps — the copy of the
// fetched bytes into wasm memory (wasm-bindgen's Vec<u8> argument) and
// `Catalog::new` on first execution (baseline-tier code).
{
  const f = fixtures.en;
  let t0 = now();
  mod.stage(all.en);
  out.coldCopyEnMs = now() - t0;
  t0 = now();
  const n = mod.load_staged(f.hash_lo, f.hash_hi);
  out.coldNewEnMs = now() - t0;
  out.coldLoadEnMs = out.coldCopyEnMs + out.coldNewEnMs;
  out.coldLoadEnCount = n;
  // First locale switch: a second catalog, code still at the baseline tier.
  const g = fixtures.pl;
  const t1 = now();
  mod.load(all.pl, g.hash_lo, g.hash_hi);
  out.firstSwitchPlMs = now() - t1;
}
for (const [loc, f] of Object.entries(fixtures)) {
  const b = all[loc];
  const once = [];
  for (let i = 0; i < 40; i++) { const t0 = now(); mod.load(b, f.hash_lo, f.hash_hi); once.push(now() - t0); }
  mod.stage(b);
  const N = 200;
  mod.bench_load(5, f.hash_lo, f.hash_hi);
  let t0 = now(); const ok = mod.bench_load(N, f.hash_lo, f.hash_hi); const load = (now() - t0) / N;
  t0 = now(); mod.bench_clone(N); const clone = (now() - t0) / N;
  t0 = now(); mod.bench_utf8(N, f.pool_start); const utf8 = (now() - t0) / N;
  out.locales[loc] = {
    bytes: f.bytes, poolBytes: f.bytes - f.pool_start, ok: ok === N,
    installMedianMs: median(once), installMinMs: Math.min(...once),
    newMs: load - clone, cloneMs: clone, utf8PoolMs: utf8,
  };
}
// Lookups on en.
{
  const f = fixtures.en;
  mod.load(all.en, f.hash_lo, f.hash_hi);
  mod.set_ids(0, new Uint32Array(f.simple));
  mod.set_ids(1, new Uint32Array(f.pattern1));
  mod.set_ids(2, new Uint32Array(f.select));
  const time = (fn, n) => { fn(Math.min(n, 5000)); const r = []; for (let k = 0; k < 5; k++) { const t0 = now(); fn(n); r.push((now() - t0) / n * 1e6); } return median(r); };
  out.simpleNs = time(mod.bench_simple, 400000);
  out.pattern1Ns = time(mod.bench_pattern, 100000);
  out.selectNs = time(mod.bench_select, 50000);
  out.spot = [mod.format_one(f.pattern1[0], 'Ada', 0), mod.format_one(f.select[0], '', 1), mod.format_one(f.select[0], '', 7)];
}
window.__p08 = out;
document.getElementById('status').textContent = 'done';
