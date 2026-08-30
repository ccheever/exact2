#!/usr/bin/env node
// The resident dev loop: edit app.contract → the page shows it, no cargo
// build in the loop. Usage: node host/web/dev.mjs [--app caltrain] [--port 8765]
//
// One Rust process (the app's `dev` bin, exact_web::dev) watches the source
// and writes each baked plan to dist/app.plan; this script serves dist/
// (index.html with dev.js added), pushes each ready plan to the page over
// server-sent events, and prints edit → present against the budget row
// "Dev restart, request to present: 100ms p50" (rules/RULES.md).
//
// The Rust side too (2026-08-30): an edit under the crates the wasm is
// built from — kernel, plan, motion, runner, host/web, gpu, the app's data,
// web, and gpu crates, the vendored Taffy — runs the same warm build
// (host/web/build.mjs), restarts the resident compiler (its plans must
// match the new format), and pushes a reload: a new wasm is a new program,
// so the page reloads rather than restarts in place. A build that fails
// shows its errors in the page's overlay, as a contract that fails does,
// and the page keeps the last good wasm. No bundler: there is nothing to
// bundle (no app JS by rule), and the watch is Node's own.
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, readFileSync, watch } from 'node:fs';
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

const clients = new Set();
let seq = 0;
const pending = new Map(); // seq -> { saved, ready }
const push = (data) => { for (const res of clients) res.write(`data: ${JSON.stringify(data)}\n\n`); };

// The resident compiler — started, and started again after a Rust rebuild.
let dev = null;
let announced = false;
function startCompiler() {
  dev = spawn('cargo', ['run', '-q', '--release', '-p', `${app}-web`, '--bin', 'dev', '--', source, plan], { cwd: root, stdio: ['ignore', 'pipe', 'inherit'], detached: true });
  const me = dev;
  console.log(`compiler pid ${dev.pid}`);
  let buffered = '';
  let first = true;
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
        // A compiler's first plan is the source as it stands — what the
        // wasm baked and a page boots from — not an edit: nothing to push,
        // nothing to time.
        if (first) { first = false; if (!announced) { announced = true; console.log(`plan ready: ${bytes} bytes (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms) — edit ${source.replace(root + '/', '')} and watch`); } continue; }
        console.log(`edit → plan ready ${(ready - saved).toFixed(0)} ms (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms, ${bytes} bytes) · pushed to ${clients.size} page${clients.size === 1 ? '' : 's'}`);
        push({ seq, bytes });
      } else if (kind === 'error') {
        console.log(`error: ${rest.join(' ')}`);
        push({ error: rest.join(' ') });
      }
    }
  });
  dev.on('exit', (code) => { if (dev === me) { console.error(`dev compiler exited ${code}`); process.exit(code ?? 1); } });
}
const killCompiler = () => { const d = dev; dev = null; if (d) { try { process.kill(-d.pid, 'SIGKILL'); } catch {} } };
startCompiler();
const stop = () => { killCompiler(); process.exit(0); };

// The Rust watch: the crates the wasm is built from.
const watched = ['kernel', 'plan', 'motion', 'runner', 'host/web', 'gpu', 'vendor/taffy', `apps/${app}/data`, `apps/${app}/web`, `apps/${app}/gpu`].map((d) => resolve(root, d)).filter(existsSync);
const wanted = /\.(rs|toml|json|wgsl|js|html)$/;
const skipped = /(^|\/)(target|dist|\.build|node_modules)(\/|$)/;
let changed = new Set();
let timer = null;
let building = false;
let again = false;
let builds = 0;
for (const dir of watched) {
  try {
    watch(dir, { recursive: true }, (_event, name) => {
      if (!name || !wanted.test(name) || skipped.test(name) || name.endsWith('dev.js')) return;
      changed.add(`${dir.replace(root + '/', '')}/${name}`);
      clearTimeout(timer);
      timer = setTimeout(rebuild, 200);
    });
  } catch (e) { console.error(`cannot watch ${dir}: ${e.message}`); }
}
function rebuild() {
  if (building) { again = true; return; }
  building = true;
  const files = [...changed]; changed = new Set();
  const t = Date.now();
  console.log(`rust: ${files.length} file${files.length === 1 ? '' : 's'} changed (${files.slice(0, 3).join(', ')}${files.length > 3 ? ', …' : ''}) — rebuilding the wasm`);
  const b = spawn('node', [resolve(root, 'host/web/build.mjs'), `${app}-web`], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  b.stdout.on('data', (d) => { out += d; });
  b.stderr.on('data', (d) => { out += d; });
  b.on('exit', (code) => {
    building = false;
    const ms = Date.now() - t;
    if (code === 0) {
      builds += 1;
      // The compiler's plans must match the wasm's format: it is built again
      // too (cargo, warm), and its first plan reaches the reloaded page.
      killCompiler();
      startCompiler();
      console.log(`rust: rebuilt in ${(ms / 1000).toFixed(1)} s · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading`);
      push({ rebuilt: builds });
    } else {
      const errors = out.split('\n').filter((l) => /^(error|warning: unused|\s+-->)/.test(l)).join('\n') || out.trim().split('\n').slice(-12).join('\n');
      console.log(`rust: build failed in ${(ms / 1000).toFixed(1)} s\n${errors}`);
      push({ error: `the wasm did not build:\n${errors}` });
    }
    if (again) { again = false; rebuild(); }
  });
}

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.plan': 'application/octet-stream', '.png': 'image/png' };
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
server.on('error', (e) => { console.error(`cannot listen on 127.0.0.1:${port}: ${e.code ?? e.message}`); killCompiler(); process.exit(1); });
server.listen(port, '127.0.0.1', () => console.log(`http://127.0.0.1:${port}/  (dev loop on apps/${app}/app.contract and the wasm's crates; ctrl-c to stop)`));
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
