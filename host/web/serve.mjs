#!/usr/bin/env node
// Serve host/web/dist for a browser — and, via the static exact.json the
// build emitted, for a native client (LLP 1023 D1). LAN by default (D8);
// --loopback (or EXACT_LOOPBACK=1) binds 127.0.0.1 only.
// Usage: node host/web/serve.mjs [port=8765] [--loopback]
import { createServer } from 'node:http';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { resolve, extname } from 'node:path';

const dist = resolve(new URL('./dist', import.meta.url).pathname);
if (!existsSync(resolve(dist, 'app.wasm'))) { console.error('run node host/web/build.mjs first'); process.exit(2); }
const types = { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.plan': 'application/vnd.exact.plan', '.png': 'image/png' };
const argv = process.argv.slice(2);
const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
const port = Number(argv.find((a) => !a.startsWith('--')) ?? 8765);
const host = loopback ? '127.0.0.1' : '0.0.0.0';
createServer((req, res) => {
  if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405); res.end(); return; }
  try {
    const file = req.url === '/' ? '/index.html' : req.url.split('?')[0];
    const path = resolve(dist, '.' + file);
    if (!path.startsWith(dist + '/') || !existsSync(path) || !statSync(path).isFile()) { res.writeHead(404); res.end(); return; }
    const type = file === '/exact.json' ? 'application/vnd.exact.envelope+json' : types[extname(path)] ?? 'application/octet-stream';
    res.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' });
    res.end(readFileSync(path));
  } catch { try { res.writeHead(500); res.end(); } catch { /* mid-write */ } }
}).listen(port, host, () => {
  const urls = [`http://127.0.0.1:${port}/`];
  if (!loopback) {
    const priv = (a) => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a);
    urls.push(...Object.values(networkInterfaces()).flat().filter((a) => a && !a.internal && a.family === 'IPv4').map((a) => a.address).sort((a, b) => priv(b) - priv(a)).map((a) => `http://${a}:${port}/`));
  }
  console.log(urls.join('\n') + '\n  (serving host/web/dist; ctrl-c to stop)');
});
