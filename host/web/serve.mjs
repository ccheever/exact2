#!/usr/bin/env node
// Serve the current web build for a browser — and, through exact.json, for
// a native client (LLP 1023 D1). LAN by default (D8); --loopback (or
// EXACT_LOOPBACK=1) binds 127.0.0.1 only.
// Usage: node host/web/serve.mjs [port=8765] [--loopback]
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, realpathSync, statSync, readFileSync } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const PUBLIC_FILES = new Set([
  '/app.plan', '/app.wasm', '/exact.json', '/glue.js', '/gpu-glue.js',
  '/gpu.js', '/gpu_bg.wasm', '/index.html', '/manifest.json',
  // The one dot path a static origin serves: the deep-link association
  // file bake generates (LLP 1030 D1), read by Apple's CDN over HTTPS.
  '/.well-known/apple-app-site-association',
]);
const PUBLIC_TREES = ['/assets/', '/deck/', '/shaders/'];
// An origin's update streams (LLP 1030.000 D7; `scripts/origin.mjs`):
// `.exact/blobs/<sha256>` and `.exact/<channel>/<compatibility id>/…` — the
// one dot path a client fetches. Inside it every other dot name (the
// stream's `.lock`) stays private.
const UPDATE_TREE = '/.exact/';

/** Resolve one URL path to the current build, or to the stable previous tree
 * while build.mjs has renamed the current one aside. Generated top-level
 * files are explicit; app assets live only under the replaced trees, and an
 * origin's update streams under `.exact/`. Every other dot path and every
 * symlink are private, even when their target is inside a build. Returns
 * null for anything that must not be served. */
export function staticFile(dist, pathname) {
  let route;
  try { route = decodeURIComponent(pathname === '/' ? '/index.html' : pathname); }
  catch { return null; }
  if (!route.startsWith('/') || route.includes('\\') || route.includes('\0')) return null;
  const parts = route.split('/').filter(Boolean);
  const update = route.startsWith(UPDATE_TREE);
  if (parts.some((part, i) => part.startsWith('.') && !(update && i === 0)) && !PUBLIC_FILES.has(route)) return null;
  if (!PUBLIC_FILES.has(route) && !update && !PUBLIC_TREES.some((tree) => route.startsWith(tree))) return null;
  // A complete current build is authoritative even when it lacks an optional
  // route (notably GPU files). Consult previous only while the current root
  // itself is absent; otherwise two apps' artifacts could be mixed.
  let root;
  try { root = realpathSync(dist); }
  catch {
    try { root = realpathSync(`${dist}.previous`); }
    catch { return null; }
  }
  try {
    const path = resolve(root, '.' + route);
    if (!path.startsWith(root + '/')) return null;
    const real = realpathSync(path);
    // Reject both a symlink file and a file reached through a symlink dir.
    if (real !== path || !real.startsWith(root + '/') || !statSync(real).isFile()) return null;
    return { path: real, route };
  } catch { return null; }
}

/** Resolve and read together, retrying when a build rename moved the path
 * between those operations. The opened response is wholly old or wholly
 * new; a request never observes the rename window as a synthetic 404. */
export function readStaticFile(dist, pathname) {
  for (let attempt = 0; attempt < 4; attempt++) {
    const found = staticFile(dist, pathname);
    if (!found) continue;
    try { return { ...found, body: readFileSync(found.path) }; }
    catch { /* retry against dist or dist.previous */ }
  }
  return null;
}

/** The assets by digest (LLP 1023 D4's owed slice; 1026 D11; 1030 D10): every file under assets/, deck/, and shaders/ in a build, named by its path beside the page, so a client fetches by name and verifies by digest and a dev push names what changed. Sorted by name. */
export function listAssets(dir) {
  const out = [];
  const walk = (sub) => {
    const abs = resolve(dir, sub);
    if (!existsSync(abs)) return;
    for (const entry of readdirSync(abs, { withFileTypes: true })) {
      const rel = `${sub}/${entry.name}`;
      if (entry.isDirectory()) walk(rel);
      else if (entry.isFile()) {
        const bytes = readFileSync(resolve(dir, rel));
        out.push({ name: rel, url: `./${rel}`, sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length });
      }
    }
  };
  for (const tree of ['assets', 'deck', 'shaders']) walk(tree);
  return out.sort((a, b) => (a.name < b.name ? -1 : 1));
}

export function webContentType(route) {
  if (route === '/exact.json') return 'application/vnd.exact.envelope+json';
  if (route === '/manifest.json') return 'application/manifest+json';
  if (route === '/.well-known/apple-app-site-association') return 'application/json';
  return {
    '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json',
    '.wasm': 'application/wasm', '.plan': 'application/vnd.exact.plan',
    '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
    '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.woff2': 'font/woff2', '.wgsl': 'text/wgsl',
  }[extname(route).toLowerCase()] ?? 'application/octet-stream';
}

function main() {
  const dist = resolve(new URL('./dist', import.meta.url).pathname);
  if (!staticFile(dist, '/app.wasm')) { console.error('run node host/web/build.mjs first'); return 2; }
  const argv = process.argv.slice(2);
  const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
  const port = Number(argv.find((a) => !a.startsWith('--')) ?? 8765);
  const host = loopback ? '127.0.0.1' : '0.0.0.0';
  createServer((req, res) => {
    if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405); res.end(); return; }
    const found = readStaticFile(dist, new URL(req.url, 'http://exact.invalid').pathname);
    if (!found) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
    res.end(req.method === 'HEAD' ? undefined : found.body);
  }).listen(port, host, () => {
    const urls = [`http://127.0.0.1:${port}/`];
    if (!loopback) {
      const priv = (a) => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a);
      urls.push(...Object.values(networkInterfaces()).flat().filter((a) => a && !a.internal && a.family === 'IPv4').map((a) => a.address).sort((a, b) => priv(b) - priv(a)).map((a) => `http://${a}:${port}/`));
    }
    console.log(urls.join('\n') + '\n  (serving host/web/dist; ctrl-c to stop)');
  });
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main();
}
