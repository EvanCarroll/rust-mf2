// Item 6: the L4 number files, compiled natively (one catalog per test,
// `mf2::compile_str`, as conformance/src/l4.rs does) and formatted in the
// engine with a variant's registry. Compared as l4.rs compares: the string
// with `exp` when the test has one, the errors as a multiset with
// `expErrors` (absent ⇒ none) in string and in parts output, the parts with
// `expParts` (every key `expParts` names, recursively), and the parts must
// concatenate to the string. A test the compile refuses passes when it
// refuses with exactly the expected kinds.

import { catalogs, setArgs, variant } from './load.mjs';

function subset(want, got) {
  if (Array.isArray(want)) return Array.isArray(got) && want.length === got.length && want.every((w, i) => subset(w, got[i]));
  if (want && typeof want === 'object') return got && typeof got === 'object' && Object.keys(want).every((k) => k in got && subset(want[k], got[k]));
  return want === got;
}

function concat(parts) {
  return parts.map((p) => (p.type === 'markup' ? '' : p.type === 'fallback' ? `{${p.source}}` : p.value ?? '')).join('');
}

export async function l4(env, name) {
  const m = await variant(env, name);
  const { idx, probe } = await catalogs(env, m, 'l4');
  const byFile = {};
  const failures = [];
  for (const t of idx.tests) {
    const f = (byFile[t.file] ??= { tests: 0, pass: 0 });
    f.tests++;
    const want = [...t.expErrors].sort();
    const problems = [];
    let got = null;
    if (t.rejected) {
      const k = [...new Set(t.rejected)].sort().join(',');
      if (k !== [...new Set(want)].sort().join(',')) problems.push(`compile refused with [${k}], expected [${want}]`);
    } else {
      const none = t.bidi === 'none';
      setArgs(probe, t.params);
      const text = probe.format(t.cat, 0, none);
      const errors = probe.errors();
      const partsJson = probe.parts(t.cat, 0, none);
      const partsErrors = probe.errors();
      const parts = JSON.parse(partsJson);
      got = { text, errors, parts };
      if (t.exp !== null && text !== t.exp) problems.push(`string ${JSON.stringify(text)}, expected ${JSON.stringify(t.exp)}`);
      if (errors !== want.join(',')) problems.push(`errors [${errors}], expected [${want}]`);
      if (partsErrors !== want.join(',')) problems.push(`parts errors [${partsErrors}], expected [${want}]`);
      if (t.expParts !== null && !subset(t.expParts, parts)) problems.push(`parts ${partsJson}, expected ${JSON.stringify(t.expParts)}`);
      if (concat(parts) !== text) problems.push(`parts concatenate to ${JSON.stringify(concat(parts))}, not the string`);
    }
    if (problems.length === 0) f.pass++;
    else failures.push({ test: `${t.file}.json #${t.index}`, src: t.src, problems, got });
  }
  const total = idx.tests.length;
  const pass = Object.values(byFile).reduce((n, f) => n + f.pass, 0);
  return { variant: name, total, pass, byFile, failures };
}
