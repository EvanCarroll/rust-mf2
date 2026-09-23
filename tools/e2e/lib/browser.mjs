// Shared helpers for mf2-two browser checks: browser launch with executable
// discovery, console capture, network + Resource Timing collection.

import { chromium, firefox, webkit } from 'playwright';
import { existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const TYPES = { chromium, firefox, webkit };

// The repository's own browsers directory: WebKit is installed there (README,
// "WebKit"), so that `playwright install` never garbage-collects the
// Chromium and Firefox builds of ~/.cache/ms-playwright.
const REPO_BROWSERS = fileURLToPath(new URL('../../../target/ms-playwright', import.meta.url));

// Relative executable paths inside a Playwright browser directory (Linux).
const LAYOUT = {
  chromium: [
    ['chromium_headless_shell-', 'chrome-headless-shell-linux64/chrome-headless-shell'],
    ['chromium-', 'chrome-linux64/chrome'],
  ],
  firefox: [['firefox-', 'firefox/firefox']],
  webkit: [['webkit-', 'pw_run.sh']],
};

function cacheDirs() {
  const dirs = [process.env.PLAYWRIGHT_BROWSERS_PATH || join(homedir(), '.cache', 'ms-playwright'), REPO_BROWSERS];
  return [...new Set(dirs)];
}

// Newest cached build that has an executable, e.g. chromium_headless_shell-1200,
// in the Playwright cache, then in the repository's target/ms-playwright.
function newestCached(name) {
  for (const dir of cacheDirs()) {
    if (!existsSync(dir)) continue;
    const entries = readdirSync(dir);
    for (const [prefix, rel] of LAYOUT[name] ?? []) {
      const builds = entries
        .filter((e) => e.startsWith(prefix) && /^\d+$/.test(e.slice(prefix.length)))
        .sort((a, b) => Number(b.slice(prefix.length)) - Number(a.slice(prefix.length)));
      for (const b of builds) {
        const exe = join(dir, b, rel);
        if (existsSync(exe)) return exe;
      }
    }
  }
  return undefined;
}

/**
 * Whether a build of `name` can be found (override, the expected build, or a
 * cached one) — `--browser all` runs WebKit only then.
 */
export function available(name) {
  const type = TYPES[name];
  if (!type) return false;
  const override = process.env[`MF2_E2E_${name.toUpperCase()}`];
  if (override) return existsSync(override);
  const own = type.executablePath();
  return Boolean((own && existsSync(own)) || newestCached(name));
}

/**
 * Launches `name` ("chromium" | "firefox" | "webkit"). Executable, in order:
 * MF2_E2E_CHROMIUM / MF2_E2E_FIREFOX / MF2_E2E_WEBKIT → the build this
 * Playwright version expects (if installed) → the newest cached Playwright
 * build of that browser (~/.cache/ms-playwright or $PLAYWRIGHT_BROWSERS_PATH,
 * then target/ms-playwright). WebKit's executable is its `pw_run.sh`.
 */
export async function launch(name) {
  const type = TYPES[name];
  if (!type) throw new Error(`unknown browser ${name}`);
  const override = process.env[`MF2_E2E_${name.toUpperCase()}`];
  let executablePath = override;
  if (!executablePath) {
    const own = type.executablePath();
    executablePath = own && existsSync(own) ? undefined : newestCached(name);
  }
  const browser = await type.launch({ executablePath });
  return { browser, executablePath: executablePath ?? type.executablePath() };
}

/** Records console warnings/errors and uncaught page errors into `sink`. */
export function watchConsole(page, sink, label = '') {
  page.on('console', (m) => {
    const type = m.type();
    if (type === 'warning' || type === 'error') {
      sink.push({ where: label || page.url(), type, text: m.text() });
    }
  });
  page.on('pageerror', (e) => sink.push({ where: label || page.url(), type: 'pageerror', text: String(e) }));
}

/** Every console message (any level) — for failure-mode documentation. */
export function recordAllConsole(page, sink) {
  page.on('console', (m) => sink.push({ type: m.type(), text: m.text() }));
  page.on('pageerror', (e) => sink.push({ type: 'pageerror', text: String(e) }));
}

/**
 * Network events as Playwright sees them: for each finished request, absolute
 * start (ms since epoch) and end derived from `request.timing()`.
 */
export function watchNetwork(page, sink) {
  page.on('requestfinished', (req) => {
    const t = req.timing();
    sink.push({
      url: req.url(),
      resourceType: req.resourceType(),
      start: t.startTime,
      end: t.responseEnd >= 0 ? t.startTime + t.responseEnd : undefined,
    });
  });
  page.on('requestfailed', (req) => sink.push({ url: req.url(), failed: req.failure()?.errorText }));
}

/** Resource Timing entries (ms relative to navigation start). */
export async function resourceTimings(page) {
  return page.evaluate(() =>
    performance.getEntriesByType('resource').map((e) => ({
      name: e.name,
      initiatorType: e.initiatorType,
      startTime: Math.round(e.startTime * 10) / 10,
      responseStart: Math.round(e.responseStart * 10) / 10,
      responseEnd: Math.round(e.responseEnd * 10) / 10,
      transferSize: e.transferSize,
      encodedBodySize: e.encodedBodySize,
    })),
  );
}

/** Captures body text and title at DOMContentLoaded (i.e. SSR output, before hydration). */
export async function captureSsrSnapshot(context) {
  await context.addInitScript(() => {
    document.addEventListener('DOMContentLoaded', () => {
      window.__ssrSnapshot = { text: document.body.innerText, title: document.title };
    });
  });
}

/** Applies CDP network throttling (Chromium only). */
export async function throttle(page, { latencyMs, downloadKbps, uploadKbps }) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Network.enable');
  await cdp.send('Network.emulateNetworkConditions', {
    offline: false,
    latency: latencyMs,
    downloadThroughput: (downloadKbps * 1024) / 8,
    uploadThroughput: (uploadKbps * 1024) / 8,
  });
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * Polls `probe` in `page` until it returns something truthy.
 *
 * Not `page.waitForFunction`: that does not await an async predicate — the
 * promise it gets back is truthy at once — and a probe that asks the app's
 * module anything has to `import()` it, which is async. Found in Phase 7:
 * a hydration wait built on `waitForFunction` returned before hydration.
 */
export async function until(page, probe, timeout = 20000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await page.evaluate(probe).catch(() => false)) return;
    await sleep(50);
  }
  throw new Error(`timed out after ${timeout} ms waiting for ${probe}`);
}
