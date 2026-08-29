#!/usr/bin/env node
// Serve host/web/dist for a browser. Usage: node host/web/serve.mjs [port=8765]
import { createServer } from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, extname } from 'node:path';

const dist = resolve(new URL('./dist', import.meta.url).pathname);
if (!existsSync(resolve(dist, 'app.wasm'))) { console.error('run node host/web/build.mjs first'); process.exit(2); }
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.png': 'image/png' };
const port = Number(process.argv[2] ?? 8765);
createServer((req, res) => {
  const path = resolve(dist, '.' + (req.url === '/' ? '/index.html' : req.url.split('?')[0]));
  if (!path.startsWith(dist) || !existsSync(path)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' });
  res.end(readFileSync(path));
}).listen(port, '127.0.0.1', () => console.log(`http://127.0.0.1:${port}/  (serving host/web/dist; ctrl-c to stop)`));
