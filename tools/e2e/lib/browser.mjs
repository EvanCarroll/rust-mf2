// Shared helpers for mf2-two browser checks: browser launch with executable
// discovery, console capture, network + Resource Timing collection.

import { chromium, firefox } from 'playwright';
import { existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const TYPES = { chromium, firefox };

// Relative executable paths inside a Playwright browser directory (Linux).
const LAYOUT = {
  chromium: [
    ['chromium_headless_shell-', 'chrome-headless-shell-linux64/chrome-headless-shell'],
    ['chromium-', 'chrome-linux64/chrome'],
  ],
  firefox: [['firefox-', 'firefox/firefox']],
};

function cacheDir() {
  return process.env.PLAYWRIGHT_BROWSERS_PATH || join(homedir(), '.cache', 'ms-playwright');
}

// Newest cached build that has an executable, e.g. chromium_headless_shell-1200.
function newestCached(name) {
  const dir = cacheDir();
  if (!existsSync(dir)) return undefined;
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
  return undefined;
}

/**
 * Launches `name` ("chromium" | "firefox"). Executable, in order:
 * MF2_E2E_CHROMIUM / MF2_E2E_FIREFOX → the build this Playwright version
 * expects (if installed) → the newest cached Playwright build of that browser.
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
