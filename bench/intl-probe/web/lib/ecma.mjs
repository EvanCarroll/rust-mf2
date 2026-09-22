// Item 3, neutral output: P0.5's 100,000 cases
// (`cargo run --release -p runtime-bench -- numbers ecma 100000`, one JSON
// line per case: value `v`, MF2 options `o`, the Rust display `out`, its
// error count `errs`).
//
// `ecmaRaw`: the Rust output against the engine's Intl.NumberFormat('en',
// {useGrouping: false, ...o}).format(v) — bench/runtime-bench/ecma-diff.cjs
// per engine, with the differences classified.
//
// `ecmaHandlers`: the same messages compiled natively (`intl-probe-native
// ecma`) and formatted in the engine by the `rust` variant and by the
// `intl` variant (the option's handlers: Rust semantics, Intl digits) —
// display and error names compared case by case; `rust` in wasm is also
// checked against the native display.

import { Tally, classify } from './classify.mjs';
import { catalogs, records, variant } from './load.mjs';

async function cases(env) {
  const text = await env.text('ecma.jsonl');
  return text.trim().split('\n').map((l) => JSON.parse(l));
}

export async function ecmaRaw(env) {
  const all = await cases(env);
  let same = 0, rejected = 0, rejectedBadOption = 0;
  const diff = new Tally(10);
  const rejectedOnlyIntl = [];
  for (const c of all) {
    let intl;
    try {
      intl = new Intl.NumberFormat('en', { useGrouping: false, ...c.o }).format(c.v);
    } catch (e) {
      rejected++;
      if (c.errs > 0) rejectedBadOption++;
      else if (rejectedOnlyIntl.length < 10) rejectedOnlyIntl.push({ ...c, error: String(e) });
      continue;
    }
    if (intl === c.out && c.errs === 0) { same++; continue; }
    const cls = c.errs > 0 ? 'rust-error-intl-formats' : classify(c.out, intl);
    diff.add(cls, { v: c.v, o: c.o, rust: c.out, errs: c.errs, intl });
  }
  const different = Object.values(diff.counts).reduce((a, b) => a + b, 0);
  return { cases: all.length, same, different, differences: diff, rejected, rejectedBadOption, rejectedOnlyIntl };
}

export async function ecmaHandlers(env) {
  const all = await cases(env);
  const rust = await catalogs(env, await variant(env, 'rust'), 'ecma');
  const intl = await catalogs(env, await variant(env, 'intl'), 'ecma');
  let n = 0, same = 0, rustNative = 0;
  const diff = new Tally(10);
  const nativeDiff = [];
  const t0 = Date.now();
  for (let cat = 0; cat < rust.idx.catalogs.length; cat++) {
    const r = records(rust.probe.format_all(cat, true));
    const x = records(intl.probe.format_all(cat, true));
    for (let i = 0; i < r.length; i++, n++) {
      const c = all[n];
      const [rt, re] = r[i];
      const [xt, xe] = x[i] ?? ['', 'missing'];
      if (rt === c.out && (re === '' ? 0 : re.split(',').length) === c.errs) rustNative++;
      else if (nativeDiff.length < 10) nativeDiff.push({ case: n, native: c.out, errs: c.errs, wasm: rt, wasmErrors: re });
      if (rt === xt && re === xe) { same++; continue; }
      const cls = re !== xe ? `errors differ (${re || 'none'} → ${xe || 'none'})` : classify(rt, xt);
      diff.add(cls, { v: c.v, o: c.o, rust: rt, rustErrors: re, intl: xt, intlErrors: xe });
    }
  }
  return {
    cases: n, same, different: n - same, differences: diff,
    rustWasmEqualsNative: rustNative, rustWasmDiffersFromNative: nativeDiff,
    seconds: (Date.now() - t0) / 1000,
  };
}
