#!/usr/bin/env node
// The resident dev loop: edit app.contract → the page shows it, no cargo
// build in the loop. Usage: node host/web/dev.mjs [--app caltrain] [--port 8765] [--loopback]
//
// Binds the LAN by default (LLP 1023 D8) so a phone on the network can boot
// the plan from this URL; --loopback (or EXACT_LOOPBACK=1) restores
// 127.0.0.1 only. The agent carrier is not here and never binds the LAN.
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
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, watch } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { dirname, relative, resolve } from 'node:path';
import { resolveApp } from '../../scripts/app.mjs';
import { listAssets, readStaticFile, webContentType } from './serve.mjs';

const argv = process.argv.slice(2);
const arg = (name, fallback) => { const i = argv.indexOf(name); return i >= 0 ? argv[i + 1] : fallback; };
const app = resolveApp(arg('--app', undefined));
const port = Number(arg('--port', 8765));
const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
const host = loopback ? '127.0.0.1' : '0.0.0.0';
const root = resolve(new URL('../..', import.meta.url).pathname);
const dist = resolve(root, 'host/web/dist');
const source = resolve(app.dir, 'app.contract');
const plan = resolve(dist, 'app.plan');
if (!existsSync(resolve(dist, 'app.wasm'))) {
  const b = spawnSync('node', [resolve(root, 'host/web/build.mjs'), app.crate('web')], { cwd: root, stdio: 'inherit' });
  if (b.status !== 0) process.exit(b.status ?? 1);
}
const budget = /\|\s*Dev restart[^|]*\|\s*([^|\n]+)/.exec(readFileSync(resolve(root, 'rules/RULES.md'), 'utf8'))?.[1].trim() ?? '?';
const builtDigest = () => {
  try { return JSON.parse(readFileSync(resolve(dist, 'exact.json'), 'utf8')).plan.sha256 ?? ''; }
  catch { return ''; }
};
let bakedDigest = builtDigest();

const clients = new Set();
let seq = 0;
const pending = new Map(); // seq -> { saved, ready }
const push = (data) => { for (const res of clients) res.write(`data: ${JSON.stringify(data)}\n\n`); };
// The current revision (LLP 1023 D3): the SSE hello carries it so a client
// that fetched the page and subscribed across an edit re-fetches instead of
// missing the edit forever.
const current = { seq: 0, digest: '' };
const hello = () => JSON.stringify({ hello: true, seq: current.seq, digest: current.digest, baked: bakedDigest });

// The resident compiler — started, and started again after a Rust rebuild.
let dev = null;
let announced = false;
function startCompiler() {
  dev = spawn('cargo', ['run', '-q', '--release', '-p', app.crate('web'), '--bin', 'dev', '--', source, plan], { cwd: app.workspace, stdio: ['ignore', 'pipe', 'inherit'], detached: true });
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
        current.seq = seq;
        try { current.digest = createHash('sha256').update(readFileSync(plan)).digest('hex'); } catch { current.digest = ''; }
        // A compiler's first plan is the source as it stands — what the
        // wasm baked and a page boots from — not an edit: nothing to push,
        // nothing to time. An early native subscriber still learns the
        // revision: the hello goes out again once it exists.
        if (first) { first = false; for (const res of clients) res.write(`data: ${hello()}\n\n`); if (!announced) { announced = true; console.log(`plan ready: ${bytes} bytes (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms) — edit ${source.replace(root + '/', '')} and watch`); } continue; }
        console.log(`edit → plan ready ${(ready - saved).toFixed(0)} ms (compile ${compile.toFixed(2)} ms, bake ${bake.toFixed(2)} ms, ${bytes} bytes) · pushed to ${clients.size} page${clients.size === 1 ? '' : 's'}\n  contract → restart with carry on every host; production: bundle`);
        push({ seq, bytes, digest: current.digest });
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

// The asset row (LLP 1030 D10; 1030.000 stage 1): an edit to an image, a
// font, a deck page, or a shader under the app's `assets/`, `deck/`, or
// `gpu/shaders/` is one digest — the file is mirrored into dist/ (what the
// page and a native client fetch), `{seq}` names the changed digests, and
// each client re-renders what referenced it, carrying state. A shader is
// classified by its interface digest (1030 D8): unchanged, it is an asset
// the client validates and swaps in; changed, it is a rebuild of the native
// host — and the wasm here, since the surfaces' Rust binds the new layout.
const assetTrees = [['assets', 'assets'], ['deck', 'deck'], ['gpu/shaders', 'shaders']].map(([from, to]) => [resolve(app.dir, from), to]).filter(([from]) => existsSync(from));
const shaderDigests = new Map();
const reflectBin = resolve(root, 'target/debug/exact-gpu-reflect');
function interfaceDigests(files) {
  if (!files.length) return new Map();
  if (!existsSync(reflectBin)) spawnSync('cargo', ['build', '-q', '-p', 'exact-gpu-reflect'], { cwd: root, stdio: 'ignore' });
  const r = spawnSync(reflectBin, ['digest', ...files], { encoding: 'utf8' });
  const out = new Map();
  for (const line of (r.stdout ?? '').trim().split('\n')) { const [name, digest, ...rest] = line.split(' '); if (name) out.set(name, digest === 'error' ? `error ${rest.join(' ')}` : digest); }
  return out;
}
for (const [from, to] of assetTrees) if (to === 'shaders') for (const [n, d] of interfaceDigests(readdirRecursive(from).filter((f) => f.endsWith('.wgsl')))) shaderDigests.set(n, d);
function readdirRecursive(dir) {
  const out = [];
  const walk = (d) => { for (const e of readdirSync(d, { withFileTypes: true })) { const p = resolve(d, e.name); if (e.isDirectory()) walk(p); else if (e.isFile()) out.push(p); } };
  if (existsSync(dir)) walk(dir);
  return out;
}
let assetChanges = new Map(); // dist-relative name -> source path (or null when removed)
let assetTimer = null;
for (const [from, to] of assetTrees) {
  try {
    watch(from, { recursive: true }, (_event, name) => {
      if (!name || skipped.test(name) || /(^|\/)\./.test(name)) return;
      assetChanges.set(`${to}/${name}`, resolve(from, name));
      clearTimeout(assetTimer);
      assetTimer = setTimeout(pushAssets, 100);
    });
  } catch (e) { console.error(`cannot watch ${from}: ${e.message}`); }
}
function pushAssets() {
  const edits = [...assetChanges]; assetChanges = new Map();
  const rows = [];
  const carriers = [];
  let needsRebuild = false;
  for (const [name, source] of edits) {
    const target = resolve(dist, name);
    if (!existsSync(source) || !statSync(source).isFile()) { rmSync(target, { force: true }); rows.push({ name, removed: true }); carriers.push(`asset ${name} removed`); continue; }
    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(source, target);
    const bytes = readFileSync(target);
    const row = { name, sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length };
    if (name.startsWith('shaders/') && name.endsWith('.wgsl')) {
      const stem = name.slice('shaders/'.length, -'.wgsl'.length);
      const digest = interfaceDigests([source]).get(stem) ?? 'error unreadable';
      const before = shaderDigests.get(stem);
      shaderDigests.set(stem, digest);
      if (digest.startsWith('error')) { carriers.push(`shader ${name}: does not validate — ${digest.slice(6)}`); push({ error: `${name}: ${digest.slice(6)}` }); continue; }
      row.interface = digest;
      if (before === digest) carriers.push(`asset ${name} → live on the web, macOS, iOS (the client validates it); production: bundle`);
      else { needsRebuild = true; carriers.push(`shader ${name}: interface ${before ?? '?'} → ${digest} — rebuild the native host; production: binary (the wasm rebuilds now)`); }
    } else {
      carriers.push(`asset ${name} → live on the web, macOS, iOS; production: bundle`);
    }
    rows.push(row);
  }
  if (!rows.length) return;
  seq += 1;
  current.seq = seq;
  console.log(`edit → assets ${rows.map((r) => r.name).join(', ')} · pushed to ${clients.size} page${clients.size === 1 ? '' : 's'}\n  ${carriers.join('\n  ')}`);
  push({ seq, digest: current.digest, assets: rows });
  if (needsRebuild) { for (const [name] of edits) changed.add(name); clearTimeout(timer); timer = setTimeout(rebuild, 200); }
}

// The Rust watch: the crates the wasm is built from.
const watched = [...['kernel', 'plan', 'motion', 'runner', 'host/web', 'gpu', 'vendor/taffy'].map((d) => resolve(root, d)), ...['data', 'web', 'gpu'].map((d) => resolve(app.dir, d))].filter(existsSync);
const wanted = /\.(rs|toml|json|js|html)$/;
const skipped = /(^|\/)(target|dist(?:\.previous)?|\.build|node_modules)(\/|$)/;
// dist mirrors the asset trees at startup. A build wrote dist once; an asset
// edited or restored while the server was down would otherwise be served
// stale, and the shader digests above are the sources' — what the wasm binds.
for (const [from, to] of assetTrees) {
  const have = new Set(readdirRecursive(resolve(dist, to)).map((p) => relative(resolve(dist, to), p)));
  for (const p of readdirRecursive(from)) {
    const name = relative(from, p);
    if (skipped.test(name) || /(^|\/)\./.test(name)) continue;
    have.delete(name);
    const target = resolve(dist, to, name);
    if (!existsSync(target) || !readFileSync(target).equals(readFileSync(p))) { mkdirSync(dirname(target), { recursive: true }); copyFileSync(p, target); }
  }
  for (const name of have) rmSync(resolve(dist, to, name), { force: true });
}
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
      // The conservative classifier (LLP 1030 D3): a Rust edit is a new
      // program on every host.
      console.log(`edit ${dir.replace(root + '/', '')}/${name} → rebuild the native host (production: binary); the web rebuilds now`);
      clearTimeout(timer);
      timer = setTimeout(rebuild, 200);
    });
  } catch (e) { console.error(`cannot watch ${dir}: ${e.message}`); }
}
// The exact classifier, after a rebuild (LLP 1030 D3; 1030.000 D5): the
// conservative line at edit time says "binary"; once the producers finish,
// the compatibility id per platform (`contract compat`, LLP 1030 D3a) says
// whether the cohort actually moved — a data-crate edit does, an edit to the
// web host's own Rust does not — and which inputs moved it.
const contractBin = resolve(root, 'target/debug/contract');
const platforms = ['web', 'macos', 'ios', 'linux'];
function cohorts() {
  if (!existsSync(contractBin)) spawnSync('cargo', ['build', '-q', '-p', 'contract'], { cwd: root, stdio: 'ignore' });
  const out = {};
  for (const platform of platforms) {
    const r = spawnSync(contractBin, ['compat', app.dir, '--platform', platform, '--json'], { encoding: 'utf8' });
    if (r.status === 0) { try { out[platform] = JSON.parse(r.stdout); } catch { /* an unreadable id is no id */ } }
  }
  return out;
}
let cohortsBefore = cohorts();
function classifyRebuild() {
  const after = cohorts();
  const lines = [];
  for (const platform of platforms) {
    const was = cohortsBefore[platform], now = after[platform];
    if (!was || !now) { lines.push(`${platform}: no compatibility id (contract compat failed)`); continue; }
    if (platform === 'web') { lines.push(`web: the origin — the page reloads now${was.id === now.id ? '' : ' (the cohort moved too)'}`); continue; }
    if (was.id === now.id) { lines.push(`${platform}: cohort ${now.id.slice(0, 8)} unchanged — nothing to ship natively for this edit`); continue; }
    const moved = Object.keys(now.inputs).filter((k) => JSON.stringify(was.inputs[k]) !== JSON.stringify(now.inputs[k]));
    lines.push(`${platform}: cohort ${was.id.slice(0, 8)} → ${now.id.slice(0, 8)} (${moved.join(', ') || 'inputs'}) — binary`);
  }
  cohortsBefore = after;
  return lines;
}

function rebuild() {
  if (building) { again = true; return; }
  building = true;
  const files = [...changed]; changed = new Set();
  const t = Date.now();
  console.log(`rust: ${files.length} file${files.length === 1 ? '' : 's'} changed (${files.slice(0, 3).join(', ')}${files.length > 3 ? ', …' : ''}) — rebuilding the wasm`);
  const b = spawn('node', [resolve(root, 'host/web/build.mjs'), app.crate('web')], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  b.stdout.on('data', (d) => { out += d; });
  b.stderr.on('data', (d) => { out += d; });
  b.on('exit', (code) => {
    building = false;
    const ms = Date.now() - t;
    if (code === 0) {
      builds += 1;
      bakedDigest = builtDigest();
      // The compiler's plans must match the wasm's format: it is built again
      // too (cargo, warm), and its first plan reaches the reloaded page.
      killCompiler();
      startCompiler();
      console.log(`rust: rebuilt in ${(ms / 1000).toFixed(1)} s · ${clients.size} page${clients.size === 1 ? '' : 's'} reloading\n  ${classifyRebuild().join('\n  ')}`);
      push({ rebuilt: builds });
    } else {
      const errors = out.split('\n').filter((l) => /^(error|warning: unused|\s+-->)/.test(l)).join('\n') || out.trim().split('\n').slice(-12).join('\n');
      console.log(`rust: build failed in ${(ms / 1000).toFixed(1)} s\n${errors}`);
      push({ error: `the wasm did not build:\n${errors}` });
    }
    if (again) { again = false; rebuild(); }
  });
}

const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  const devBeacon = url.pathname === '/__dev/reloaded' || url.pathname === '/__dev/painted';
  if (req.method !== 'GET' && req.method !== 'HEAD' && !(req.method === 'POST' && devBeacon)) { res.writeHead(405); res.end(); return; }
  if (url.pathname === '/__dev') {
    res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
    res.write(':\n\n');
    res.write(`data: ${hello()}\n\n`);
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
  // The live envelope (LLP 1023 D2): the static exact.json in dist/ plus the
  // dev tier — seq and the events stream. Built from the plan bytes so it can
  // never go stale against what the resident compiler last wrote; the header
  // offsets are the generated encoder's (plan/build.rs, little-endian).
  // Served at /exact.json, and — Stage 2's negotiation — for a GET of the
  // app URL itself whose Accept names the envelope type: the dev-server
  // shortcut past the link rung (D1); a browser never sends it.
  const wantsEnvelope = url.pathname === '/exact.json'
    || (url.pathname === '/' && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json'));
  if (wantsEnvelope) {
    try {
      const found = readStaticFile(dist, '/app.plan');
      if (!found) throw new Error('no current plan');
      const bytes = found.body;
      const idLen = bytes.readUInt32LE(32);
      const appId = idLen ? bytes.subarray(36, 36 + idLen).toString('utf8') : '';
      res.writeHead(200, { 'content-type': 'application/vnd.exact.envelope+json', vary: 'Accept', 'cache-control': 'no-store' });
      res.end(JSON.stringify({
        exact: 1,
        app: appId ? { id: appId, name: app.name } : { name: app.name },
        plan: { url: './app.plan', sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length, formatVersion: bytes.readUInt32LE(4), kernelSchema: bytes.readBigUInt64LE(16).toString(16).padStart(16, '0') },
        assets: listAssets(dist),
        seq: current.seq,
        events: './__dev',
      }) + '\n');
    } catch { res.writeHead(404); res.end(); }
    return;
  }
  const file = url.pathname === '/' ? '/index.html' : url.pathname;
  if (file === '/dev.js') { res.writeHead(200, { 'content-type': 'text/javascript', 'cache-control': 'no-store' }); res.end(readFileSync(resolve(root, 'host/web/dev.js'))); return; }
  try {
    const found = readStaticFile(dist, file);
    if (!found) { res.writeHead(404); res.end(); return; }
    let body = found.body;
    if (file === '/index.html') body = body.toString().replace('<script type="module" src="./glue.js"></script>', '<script type="module" src="./glue.js"></script>\n<script type="module" src="./dev.js"></script>');
    res.writeHead(200, { 'content-type': webContentType(found.route), ...(file === '/index.html' ? { vary: 'Accept' } : {}), 'cache-control': 'no-store' });
    res.end(req.method === 'HEAD' ? undefined : body);
  } catch { try { res.writeHead(404); res.end(); } catch { /* mid-write */ } }
});
server.on('error', (e) => { console.error(`cannot listen on ${host}:${port}: ${e.code ?? e.message}`); killCompiler(); process.exit(1); });
server.listen(port, host, () => {
  const urls = [`http://127.0.0.1:${port}/`];
  if (!loopback) {
    // Every usable IPv4, private-range first, none silently picked (D8):
    // a utun/VPN address printed alone is a silent failure on the phone.
    const priv = (a) => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a);
    const addrs = Object.values(networkInterfaces()).flat().filter((a) => a && !a.internal && a.family === 'IPv4').map((a) => a.address).sort((a, b) => priv(b) - priv(a));
    urls.push(...addrs.map((a) => `http://${a}:${port}/`));
    if (addrs.length === 0) console.log('no LAN interface found; serving loopback only in effect');
  }
  console.log(urls.join('\n'));
  console.log(`  (dev loop on ${source.replace(root + '/', '')} and the wasm's crates; ${loopback ? 'loopback only' : 'LAN bind — --loopback to keep it local; macOS may ask to allow node'}; ctrl-c to stop)`);
});
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
