#!/usr/bin/env node
// rust-mf2 browser checks. One script per check under checks/; the app under
// test must already be running at --base-url.
//
//   node run.mjs <check> [--base-url URL] [--browser chromium|firefox|webkit|all]
//                        [--throttle] [--label TEXT] [--json FILE]
//
// `all` is Chromium, Firefox, and WebKit when a WebKit build is found; a
// browser `all` includes that cannot start is reported and skipped (named
// explicitly, it fails). Exit code 0 when every assertion passed, 1 otherwise.

import { parseArgs } from 'node:util';
import { writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { available, launch } from './lib/browser.mjs';

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    'base-url': { type: 'string', default: process.env.BASE_URL ?? 'http://127.0.0.1:3702' },
    browser: { type: 'string', default: 'chromium' },
    throttle: { type: 'boolean', default: false },
    label: { type: 'string', default: '' },
    json: { type: 'string' },
  },
});

const [checkName] = positionals;
if (!checkName) {
  console.error('usage: node run.mjs <check> [--base-url URL] [--browser chromium|firefox|webkit|all] [--throttle] [--json FILE]');
  process.exit(2);
}
const check = await import(`./checks/${checkName}.mjs`);
const all = values.browser === 'all';
const browsers = all
  ? ['chromium', 'firefox', ...(available('webkit') ? ['webkit'] : [])]
  : values.browser.split(',');

const report = { check: checkName, label: values.label, baseUrl: values['base-url'], runs: [], skipped: [] };
let failed = 0;

for (const name of browsers) {
  let launched;
  try {
    launched = await launch(name);
  } catch (e) {
    const error = String(e?.message ?? e).split('\n').filter((l) => l.trim()).slice(0, 8).join(' | ');
    if (all) {
      console.log(`[${name}] SKIP cannot start: ${error}`);
      report.skipped.push({ browser: name, error });
      continue;
    }
    throw e;
  }
  const { browser, executablePath } = launched;
  const run = { browser: name, version: browser.version(), executablePath, assertions: [], data: {} };
  const ctx = {
    baseUrl: values['base-url'].replace(/\/$/, ''),
    browserName: name,
    browser,
    throttle: values.throttle,
    data: run.data,
    log: (...a) => console.log(`[${name}]`, ...a),
    assert(id, pass, details = undefined) {
      run.assertions.push({ id, pass: Boolean(pass), details });
      if (!pass) failed += 1;
      console.log(`[${name}] ${pass ? 'PASS' : 'FAIL'} ${id}${details === undefined ? '' : ` — ${typeof details === 'string' ? details : JSON.stringify(details)}`}`);
    },
  };
  try {
    await check.run(ctx);
  } catch (e) {
    ctx.assert('check-completed', false, String(e?.stack ?? e));
  } finally {
    await browser.close();
  }
  report.runs.push(run);
}

const total = report.runs.reduce((n, r) => n + r.assertions.length, 0);
console.log(`\n${checkName}${values.label ? ` (${values.label})` : ''}: ${total - failed}/${total} assertions passed`);
// Reports are shareable: the home directory (cargo registry paths in panic
// messages, browser executable paths) is written as `~`.
if (values.json) writeFileSync(values.json, `${JSON.stringify(report, null, 2).replaceAll(homedir(), '~')}\n`);
process.exit(failed === 0 ? 0 : 1);
