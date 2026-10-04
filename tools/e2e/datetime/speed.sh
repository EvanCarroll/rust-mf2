#!/usr/bin/env bash
# The speed of one date placeholder in a browser, `Intl` against ICU4X
# (plan/08 §9; Phase 22 runs it, task 22.8). Builds, runs, and writes the
# figures with this command to tools/e2e/datetime/speed-results.md:
#
#   tools/e2e/datetime/speed.sh
#
#   1. target/e2e-datetime/speed.json — the catalogs (en, pl, ar × a date, a
#      date and time, one with a zone name; each with its icu.blob) and the
#      argument values, with ICU4X's native text (the cases generator);
#   2. target/e2e-datetime/speed/<build>/ — the browser module once per
#      formatter (`intl`, `icu`, `icu-cached`), each in its own cargo
#      invocation so no feature unifies: profile `wasm-release` (opt-level
#      z, fat LTO, 1 CGU, panic=abort, strip) → `wasm-bindgen --target web`
#      → `wasm-opt -Oz`, as a client ships;
#   3. `node run.mjs datetime-speed --browser all` (tools/e2e/checks/
#      datetime-speed.mjs): Chromium, Firefox, WebKit, the builds alternated
#      in one page; unthrottled, and at 4× CPU throttle in Chromium;
#   4. speed-results.md beside this script (each run's copy is kept as
#      target/e2e-datetime/results/speed-<stamp>.md: timings drift with the
#      clock and the load, so compare runs, not one).
#
# Tools: cargo (+ wasm32-unknown-unknown), wasm-bindgen 0.2.128, wasm-opt
# (binaryen), node with tools/e2e's `npm install`.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
out="$repo/target/e2e-datetime"
results="$out/results"
mkdir -p "$out" "$results"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"
for tool in cargo wasm-bindgen wasm-opt node; do
  command -v "$tool" >/dev/null 2>&1 || { echo "speed.sh: $tool not found" >&2; exit 2; }
done
FEATURES=(--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue)
BUILDS=(intl icu icu-cached)

cd "$here"
cargo run -q --release -p e2e-datetime-cases -- "$out/cases.json"
for b in "${BUILDS[@]}"; do
  cargo build -q --target wasm32-unknown-unknown --profile wasm-release \
    -p e2e-datetime-speed --features "$b"
  pkg="$out/speed/$b"
  rm -rf "$pkg"
  wasm-bindgen --target web --out-dir "$pkg" --out-name speed \
    "target/wasm32-unknown-unknown/wasm-release/e2e_datetime_speed.wasm"
  wasm-opt -Oz "${FEATURES[@]}" "$pkg/speed_bg.wasm" -o "$pkg/speed_bg.wasm"
done

# Older results must not pass for this run's.
rm -f "$results"/speed-chromium.json "$results"/speed-firefox.json "$results"/speed-webkit.json
status=0
(cd "$repo/tools/e2e" && node run.mjs datetime-speed --browser all \
  --label datetime-speed --json "$results/speed-e2e.json") || status=$?

dirty=$(git -C "$repo" status --porcelain -- crates tools/e2e | wc -l)
machine="$(nproc) CPUs, $(grep -m1 MHz /proc/cpuinfo | sed 's/.*: *//') MHz at the end, load $(cut -d' ' -f1-3 /proc/loadavg)"
tree="crates/ and tools/e2e/ of the working tree at $(git -C "$repo" rev-parse --short HEAD) ($dirty changed paths)"
tools="$(rustc --version); wasm-bindgen $(wasm-bindgen --version | cut -d' ' -f2); $(wasm-opt --version); node $(node --version)"
stamp=$(date +%Y%m%d-%H%M%S)
node - "$results" "$here/speed-results.md" "$machine" "$tree" "$tools" "$(date -u +%Y-%m-%dT%H:%MZ)" "$status" <<'EOF'
const fs = require('fs');
const path = require('path');
const [dir, dst, machine, tree, tools, when, status] = process.argv.slice(2);
const ns = (x) => (x >= 10000 ? x.toFixed(0) : x.toPrecision(3));
const cell = (m) => `${ns(m.median)} (${ns(m.min)}–${ns(m.max)})`;
const lines = [
  '# The speed of a date in a browser: Intl against ICU4X',
  '',
  'Written by `tools/e2e/datetime/speed.sh` (plan/08 §9): re-run that command to',
  'reproduce. Nanoseconds per format of one date placeholder, the median of 9',
  'samples per build and its range, the builds alternated in one page:',
  '`intl` is `Intl.DateTimeFormat`, `icu` ICU4X over the catalog\'s blob as a',
  'client ships it, `icu-cached` the same with the formatter cache on.',
  '',
  `* When: ${when}; check exit status ${status} (0: every assertion passed).`,
  `* Machine: ${machine}.`,
  `* Built from ${tree}; ${tools}.`,
  '',
];
for (const browser of ['chromium', 'firefox', 'webkit']) {
  const f = path.join(dir, `speed-${browser}.json`);
  if (!fs.existsSync(f)) {
    lines.push(`## ${browser}`, '', 'Not run (it did not start, or the check failed before timing).', '');
    continue;
  }
  const r = JSON.parse(fs.readFileSync(f, 'utf8'));
  lines.push(`## ${browser} ${r.version}`, '');
  for (const [label, run] of Object.entries(r.runs)) {
    if (typeof run === 'string') {
      lines.push(`${label}: ${run}`, '');
      continue;
    }
    lines.push(`${label} (CPU MHz before/after: ${run.mhz.join(' / ')}; crossOriginIsolated: ${run.crossOriginIsolated}):`, '');
    lines.push('| Locale | Message | iters | `intl` ns | `icu` ns | `icu-cached` ns | icu / intl | icu-cached / intl |');
    lines.push('|---|---|---:|---:|---:|---:|---:|---:|');
    for (const row of run.rows) {
      lines.push(`| ${row.locale} | \`${row.src}\` | ${row.iters} | ${cell(row.intl)} | ${cell(row.icu)} | ${cell(row['icu-cached'])} | ${row.icuOverIntl.toFixed(2)} | ${row.cachedOverIntl.toFixed(2)} |`);
    }
    lines.push('');
  }
}
fs.writeFileSync(dst, lines.join('\n'));
EOF
cp "$here/speed-results.md" "$results/speed-$stamp.md"
echo "speed.sh: $here/speed-results.md (copy: $results/speed-$stamp.md)"
exit "$status"
