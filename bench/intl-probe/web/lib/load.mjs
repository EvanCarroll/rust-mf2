// Loading the probe's wasm variants and the natively compiled catalogs.
//
// `env` abstracts where files come from, so the same code runs in a page
// (fetch) and in node (fs):
//   env.json(name) / env.bytes(name) / env.text(name)  — files of target/intl-probe/data/
//   env.module(variant)                                 — the initialised wasm-bindgen module of
//                                                         target/intl-probe/pkg/<variant>/

/** The variant's module, checked to be the variant it claims. */
export async function variant(env, name) {
  const m = await env.module(name);
  if (m.variant() !== name) throw new Error(`pkg/${name} is the ${m.variant()} variant`);
  return m;
}

/** A `Probe` of module `m` with every catalog of data set `set` loaded (index = position). */
export async function catalogs(env, m, set) {
  const idx = await env.json(`${set}.json`);
  const bin = await env.bytes(idx.bin);
  const probe = new m.Probe();
  idx.catalogs.forEach((c, i) => {
    const k = probe.load(bin.subarray(c.off, c.off + c.len), BigInt(`0x${c.hash}`));
    if (k !== i) throw new Error(`${set}: catalog ${i} did not load (${k})`);
  });
  return { idx, probe };
}

/** Sets a probe's positional arguments from the native side's `params` list. */
export function setArgs(probe, params) {
  probe.clear_args();
  for (const a of params ?? []) {
    switch (a.kind) {
      case 'str': probe.arg_str(a.value); break;
      case 'int': probe.arg_int(a.value); break;
      case 'float': probe.arg_float(a.value); break;
      case 'decimal': probe.arg_decimal(a.value); break;
      case 'other': probe.arg_other(); break;
      default: probe.arg_unset();
    }
  }
}

/** Splits `format_all` output into `[text, errors]` records. */
export function records(s) {
  return s === '' ? [] : s.split('\x1e').map((r) => {
    const i = r.lastIndexOf('\x1f');
    return [r.slice(0, i), r.slice(i + 1)];
  });
}

/** What the running engine is. */
export function engine() {
  if (typeof navigator !== 'undefined' && navigator.userAgent) return navigator.userAgent;
  return `node ${process.version} (ICU ${process.versions.icu}, CLDR ${process.versions.cldr})`;
}
