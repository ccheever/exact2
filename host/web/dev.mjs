#!/usr/bin/env node
// The resident dev loop: edit app.contract → the page shows it, no cargo
// build in the loop. Usage: node host/web/dev.mjs [--app caltrain] [--port 8765]
//
// One Rust process (the app's `dev` bin, exact_web::dev) watches the source
// and writes each baked plan to dist/app.plan; this script serves dist/
// (index.html with dev.js added), pushes each ready plan to the page over
// server-sent events, and prints edit → present against the budget row
// "Dev restart, request to present: 100ms p50" (rules/RULES.md).
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, readFileSync } from 'node:fs';
import { resolve, extname } from 'node:path';

const argv = process.argv.slice(2);
const arg = (name, fallback) => { const i = argv.indexOf(name); return i >= 0 ? argv[i + 1] : fallback; };
const app = arg('--app', 'caltrain');
const port = Number(arg('--port', 8765));
const root = resolve(new URL('../..', import.meta.url).pathname);
const dist = resolve(root, 'host/web/dist');
const source = resolve(root, `apps/${app}/app.contract`);
const plan = resolve(dist, 'app.plan');
if (!existsSync(resolve(dist, 'app.wasm'))) {
  const b = spawnSync('node', [resolve(root, 'host/web/build.mjs'), `${app}-web`], { cwd: root, stdio: 'inherit' });
  if (b.status !== 0) process.exit(b.status ?? 1);
}
const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';

// The resident compiler.
const dev = spawn('cargo', ['run', '-q', '--release', '-p', `${app}-web`, '--bin', 'dev', '--', source, plan], { cwd: root, stdio: ['ignore', 'pipe', 'inherit'], detached: true });
const stop = () => { try { process.kill(-dev.pid, 'SIGKILL'); } catch {} process.exit(0); };
console.log(`compiler pid ${dev.pid}`);
const clients = new Set();
let seq = 0;
const pending = new Map(); // seq -> { saved, ready }
const push = (data) => { for (const res of clients) res.write(`data: ${JSON.stringify(data)}\n\n`); };
let buffered = '';
dev.stdout.on('data', (chunk) => {
  buffered += chunk;
  const lines = buffered.split('\n');
  buffered = lines.pop();
  for (const line of lines) {
    const [kind, ...rest] = line.split(' ');
    if (kind === 'plan') {
      const [bytes, saved, compile, bake, ready] = rest.map(Number);
      seq += 1;
      pending.set(seq, { saved, ready });
      if (seq === 1) console.log(`plan ready: ${bytes} bytes (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms) — edit ${source.replace(root + '/', '')} and watch`);
      else console.log(`edit → plan ready ${(ready - saved).toFixed(0)} ms (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms, ${bytes} bytes) · pushed to ${clients.size} page${clients.size === 1 ? '' : 's'}`);
      push({ seq, bytes });
    } else if (kind === 'error') {
      console.log(`error: ${rest.join(' ')}`);
      push({ error: rest.join(' ') });
    }
  }
});
dev.on('exit', (code) => { console.error(`dev compiler exited ${code}`); process.exit(code ?? 1); });

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.plan': 'application/octet-stream' };
const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (url.pathname === '/__dev') {
    res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
    res.write(':\n\n');
    clients.add(res);
    console.log(`page connected (${clients.size})`);
    req.on('close', () => clients.delete(res));
    return;
  }
  if (url.pathname === '/__dev/reloaded') {
    const n = Number(url.searchParams.get('seq'));
    const p = pending.get(n);
    if (p) {
      const total = Number(url.searchParams.get('dom')) - p.saved;
      console.log(`  → page: fetch ${url.searchParams.get('fetch')} ms, restart ${url.searchParams.get('boot')} ms; edit → first frame in the DOM ${total.toFixed(0)} ms (budget ${budget})${total > 100 ? '  OVER BUDGET' : ''}`);
      console.log(`reloaded seq=${n} total_ms=${total.toFixed(0)}`);
    }
    res.writeHead(204); res.end();
    return;
  }
  if (url.pathname === '/__dev/painted') {
    const p = pending.get(Number(url.searchParams.get('seq')));
    if (p) console.log(`  → painted ${(Number(url.searchParams.get('paint')) - p.saved).toFixed(0)} ms after the save`);
    res.writeHead(204); res.end();
    return;
  }
  const file = url.pathname === '/' ? '/index.html' : url.pathname;
  if (file === '/dev.js') { res.writeHead(200, { 'content-type': 'text/javascript', 'cache-control': 'no-store' }); res.end(readFileSync(resolve(root, 'host/web/dev.js'))); return; }
  const path = resolve(dist, '.' + file);
  if (!path.startsWith(dist) || !existsSync(path)) { res.writeHead(404); res.end(); return; }
  let body = readFileSync(path);
  if (file === '/index.html') body = body.toString().replace('<script type="module" src="./glue.js"></script>', '<script type="module" src="./glue.js"></script>\n<script type="module" src="./dev.js"></script>');
  res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' });
  res.end(body);
});
server.listen(port, '127.0.0.1', () => console.log(`http://127.0.0.1:${port}/  (dev loop on apps/${app}/app.contract; ctrl-c to stop)`));
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
