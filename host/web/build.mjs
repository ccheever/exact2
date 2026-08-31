#!/usr/bin/env node
// Build the web app: the wasm under the `web` profile (size-tuned), `wasm-opt -Oz`
// when binaryen is on PATH, then `dist/` = index.html + glue.js + app.wasm.
// Usage: node host/web/build.mjs [crate=caltrain-web]
import { spawnSync } from 'node:child_process';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { resolveApp } from '../../scripts/app.mjs';

const app = resolveApp(process.argv[2]);
const crate = app.crate('web');
const kib = (n) => `${(n / 1024).toFixed(0)} KiB`;
const root = resolve(new URL('../..', import.meta.url).pathname);
const r = spawnSync('cargo', ['build', '-p', crate, '--lib', '--profile', 'web', '--target', 'wasm32-unknown-unknown'], { cwd: app.workspace, stdio: 'inherit' });
if (r.status !== 0) process.exit(r.status ?? 1);
const built = resolve(app.target, 'wasm32-unknown-unknown/web', crate.replace(/-/g, '_') + '.wasm');
const dist = resolve(root, 'host/web/dist');
mkdirSync(dist, { recursive: true });
const out = resolve(dist, 'app.wasm');

// Post-link optimization. The feature flags match what rustc's wasm32 target emits.
const opt = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', out, built], { stdio: 'inherit' });
let optNote;
if (opt.error?.code === 'ENOENT') { copyFileSync(built, out); optNote = 'wasm-opt not on PATH (brew install binaryen): shipped unoptimized'; }
else if (opt.status !== 0) process.exit(opt.status ?? 1);
else optNote = 'wasm-opt -Oz';

// The app's static files ride beside the page: `assets/…` images and an
// optional `deck/` iframe guest (@ref LLP 1020 M1). Replaced whole, so a
// deleted file does not linger in dist.
const assets = resolve(app.dir, 'assets');
rmSync(resolve(dist, 'assets'), { recursive: true, force: true });
if (existsSync(assets)) cpSync(assets, resolve(dist, 'assets'), { recursive: true });
const deck = resolve(app.dir, 'deck');
rmSync(resolve(dist, 'deck'), { recursive: true, force: true });
if (existsSync(deck)) cpSync(deck, resolve(dist, 'deck'), { recursive: true });
copyFileSync(resolve(root, 'host/web/index.html'), resolve(dist, 'index.html'));
copyFileSync(resolve(root, 'host/web/glue.js'), resolve(dist, 'glue.js'));

// The plan and its pointer card (LLP 1023 D1/D2): dist/ is a complete static
// deploy — a native client GETs the page URL, follows index.html's link to
// exact.json, and fetches app.plan; a browser never notices. The header
// fields are read from the plan bytes at the generated encoder's fixed
// offsets (plan/build.rs: 4-byte magic, u32 version, u64 format digest,
// u64 kernel schema, u64 compiler identity — all little-endian).
const planOut = resolve(dist, 'app.plan');
const dv = spawnSync('cargo', ['run', '-q', '--release', '-p', crate, '--bin', 'dev', '--', resolve(app.dir, 'app.contract'), planOut, '--once'], { cwd: app.workspace, stdio: 'inherit' });
if (dv.status !== 0) process.exit(dv.status ?? 1);
const planBytes = readFileSync(planOut);
const idLen = planBytes.readUInt32LE(32);
const appId = idLen ? planBytes.subarray(36, 36 + idLen).toString('utf8') : '';
writeFileSync(resolve(dist, 'exact.json'), JSON.stringify({
  exact: 1,
  app: appId ? { id: appId, name: app.name } : { name: app.name },
  plan: {
    url: './app.plan',
    sha256: createHash('sha256').update(planBytes).digest('hex'),
    bytes: planBytes.length,
    formatVersion: planBytes.readUInt32LE(4),
    kernelSchema: planBytes.readBigUInt64LE(16).toString(16).padStart(16, '0'),
  },
}) + '\n');
writeFileSync(resolve(dist, 'index.html'), readFileSync(resolve(dist, 'index.html'), 'utf8').replace(
  '<script type="module" src="./glue.js"></script>',
  '<link rel="alternate" type="application/vnd.exact.envelope+json" href="./exact.json">\n<script type="module" src="./glue.js"></script>',
));

// The app's GPU module (LLP 1009 D2): a second wasm the page fetches on
// demand, built with wasm-bindgen's glue (its exports are the module's ABI on
// the web) and wasm-opt. Only when the app has a GPU crate.
const gpuCrate = crate.replace(/-web$/, '-gpu');
let gpuNote = 'no GPU crate';
if (existsSync(resolve(app.dir, 'gpu', 'Cargo.toml'))) {
  const g = spawnSync('cargo', ['build', '-p', gpuCrate, '--lib', '--profile', 'web', '--target', 'wasm32-unknown-unknown', '--config', 'profile.web.strip=false'], { cwd: app.workspace, stdio: 'inherit' });
  if (g.status !== 0) process.exit(g.status ?? 1);
  const gpuWasm = resolve(app.target, 'wasm32-unknown-unknown/web', gpuCrate.replace(/-/g, '_') + '.wasm');
  const wb = spawnSync('wasm-bindgen', ['--target', 'web', '--no-typescript', '--out-dir', dist, '--out-name', 'gpu', gpuWasm], { stdio: 'inherit' });
  if (wb.error?.code === 'ENOENT') { gpuNote = 'wasm-bindgen not on PATH (cargo install wasm-bindgen-cli): GPU module not built'; }
  else if (wb.status !== 0) process.exit(wb.status ?? 1);
  else {
    const bg = resolve(dist, 'gpu_bg.wasm');
    const o = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', bg, bg], { stdio: 'inherit' });
    copyFileSync(resolve(root, 'host/web/gpu-glue.js'), resolve(dist, 'gpu-glue.js'));
    const gw = readFileSync(bg);
    gpuNote = `gpu_bg.wasm ${kib(gw.length)} (${kib(gzipSync(gw, { level: 9 }).length)} gzip${o.status === 0 ? ', wasm-opt' : ''}), gpu.js ${kib(readFileSync(resolve(dist, 'gpu.js')).length)}, on demand`;
  }
}
const wasm = readFileSync(out);
console.log(`host/web/dist: app.wasm ${kib(wasm.length)} (${kib(gzipSync(wasm, { level: 9 }).length)} gzip; ${optNote}), index.html, glue.js, app.plan ${kib(planBytes.length)}, exact.json; GPU: ${gpuNote}`);
