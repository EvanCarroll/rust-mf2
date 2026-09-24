// A static host for a client-only site (`examples/demo-csr/dist/`, served by
// the `csr` and `a11y` checks): files, their types, and the cache policy
// plans/04-leptos-integration.md §6 asks for — a catalog is named by its
// content and immutable; everything else, the index included, is revalidated.

import { createServer } from 'node:http';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.json': 'application/json',
  '.mf2b': 'application/octet-stream',
};

/** Serves `root` on a free port of 127.0.0.1; resolves to the server. */
export function serveStatic(root) {
  const server = createServer((req, res) => {
    let path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '');
    if (path === '') path = 'index.html';
    const file = normalize(join(root, path));
    if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404, { 'Cache-Control': 'no-cache' }).end('not found');
      return;
    }
    const cache = file.endsWith('.mf2b') ? 'public, max-age=31536000, immutable' : 'no-cache';
    res.writeHead(200, {
      'Content-Type': TYPES[extname(file)] ?? 'application/octet-stream',
      'Cache-Control': cache,
    });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}
